use bracketlab::tournament::{Match, Project, Slot, Source, Stage};
use eframe::egui::{pos2, vec2, Pos2, Rect};
use kurbo::{CubicBez, ParamCurve, Point};
use rstar::{RTree, RTreeObject, AABB};
use std::{collections::BTreeMap, fmt::Write, path::Path};

pub const WIDTH: f32 = 354.0;
const HEIGHT: f32 = 132.0;
const PITCH_X: f32 = 436.0;
pub const MAP_PITCH: f32 = 23.0;

pub fn map_rows(game: &Match) -> Vec<(String, String)> {
    let Some(score) = &game.score else {
        return vec![];
    };
    if score.technical {
        return vec![];
    }
    if score.maps.is_empty() {
        return if game.bo > 1 {
            vec![("Детали карт не указаны".into(), String::new())]
        } else {
            vec![]
        };
    }
    score
        .maps
        .iter()
        .map(|map| {
            (
                map.name.to_uppercase(),
                format!("{} : {}", map.rounds[0], map.rounds[1]),
            )
        })
        .collect()
}

fn card_height(game: &Match) -> f32 {
    let rows = map_rows(game).len();
    if rows == 0 {
        HEIGHT
    } else {
        130.0 + MAP_PITCH * rows as f32 + 8.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slice {
    All,
    Round(usize),
    FromMatch(usize),
}

pub fn included(stage: &Stage, slice: Slice) -> Vec<bool> {
    match slice {
        Slice::All => vec![true; stage.matches.len()],
        Slice::Round(round) => stage
            .matches
            .iter()
            .map(|game| game.round == round)
            .collect(),
        Slice::FromMatch(index) => {
            let mut starts = BTreeMap::new();
            if let Some(game) = stage.matches.get(index) {
                starts.insert(game.lane.as_str(), game.round);
            }
            loop {
                let selected: Vec<_> = stage
                    .matches
                    .iter()
                    .map(|game| {
                        starts
                            .get(game.lane.as_str())
                            .is_some_and(|start| game.round >= *start)
                    })
                    .collect();
                let mut changed = false;
                for game in &stage.matches {
                    let linked = game.sources.iter().any(|source| match source {
                        Source::Winner(i) | Source::Loser(i) => selected[*i],
                        _ => false,
                    });
                    if linked
                        && starts
                            .get(game.lane.as_str())
                            .is_none_or(|start| game.round < *start)
                    {
                        starts.insert(game.lane.as_str(), game.round);
                        changed = true;
                    }
                }
                if !changed {
                    return selected;
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct Card {
    pub index: usize,
    pub rect: Rect,
}

fn envelope(rect: Rect) -> AABB<[f32; 2]> {
    AABB::from_corners([rect.min.x, rect.min.y], [rect.max.x, rect.max.y])
}

impl RTreeObject for Card {
    type Envelope = AABB<[f32; 2]>;
    fn envelope(&self) -> Self::Envelope {
        envelope(self.rect)
    }
}

pub struct Scene {
    pub cards: Vec<Card>,
    pub edges: Vec<(usize, usize, usize, bool, CubicBez)>,
    pub transfers: Vec<Transfer>,
    pub headings: Vec<(Pos2, String)>,
    pub bounds: Rect,
    index: RTree<Card>,
}

pub struct Transfer {
    pub source: usize,
    pub target: usize,
    pub slot: usize,
    pub loser: bool,
    pub points: [Pos2; 6],
}

impl Scene {
    pub fn new(stage: &Stage) -> Self {
        Self::filtered(stage, &included(stage, Slice::All))
    }

    fn filtered(stage: &Stage, selected: &[bool]) -> Self {
        let mut cards = vec![
            Card {
                index: 0,
                rect: Rect::NOTHING
            };
            stage.matches.len()
        ];
        let mut lanes: Vec<(&str, BTreeMap<usize, Vec<usize>>)> = Vec::new();
        for (id, game) in stage.matches.iter().enumerate() {
            if !selected[id] {
                continue;
            }
            let lane = game.lane.as_str();
            let index = lanes
                .iter()
                .position(|section| section.0 == lane)
                .unwrap_or_else(|| {
                    lanes.push((lane, BTreeMap::new()));
                    lanes.len() - 1
                });
            lanes[index].1.entry(game.round).or_default().push(id);
        }
        let mut headings = vec![];
        let mut lane_top = BTreeMap::new();
        let mut bounds = Rect::from_min_size(Pos2::ZERO, vec2(1.0, 1.0));
        let mut y = 0.0;
        for (lane, rounds) in lanes {
            lane_top.insert(lane, y);
            headings.push((pos2(0.0, y), lane.to_uppercase()));
            y += 58.0;
            let rows = rounds.values().map(Vec::len).max().unwrap_or(1);
            let pitch_y = rounds
                .values()
                .flatten()
                .map(|&id| card_height(&stage.matches[id]))
                .fold(HEIGHT, f32::max)
                + 30.0;
            for (column, (round, ids)) in rounds.into_iter().enumerate() {
                let x = column as f32 * PITCH_X;
                headings.push((pos2(x, y - 14.0), format!("РАУНД {:02}", round + 1)));
                for (row, id) in ids.iter().copied().enumerate() {
                    let offset = (row as f32 + 0.5) * rows as f32 / ids.len() as f32;
                    let center = pos2(x + WIDTH / 2.0, y + offset * pitch_y);
                    let rect = Rect::from_center_size(
                        center,
                        vec2(WIDTH, card_height(&stage.matches[id])),
                    );
                    cards[id] = Card { index: id, rect };
                    bounds = bounds.union(rect);
                }
            }
            y += rows as f32 * pitch_y + 68.0;
        }
        let mut edges = vec![];
        let mut transfers = vec![];
        for (target, game) in stage.matches.iter().enumerate() {
            if !selected[target] {
                continue;
            }
            for (slot, source) in game.sources.iter().enumerate() {
                let (origin, loser) = match source {
                    Source::Winner(id) => (*id, false),
                    Source::Loser(id) => (*id, true),
                    _ => continue,
                };
                if !selected[origin] {
                    continue;
                }
                let a = cards[origin].rect;
                let b = cards[target].rect;
                if stage.matches[origin].lane != game.lane {
                    let route_y = lane_top[game.lane.as_str()] - 18.0;
                    let from = pos2(a.right(), a.top() + 77.0);
                    let to = pos2(b.left(), b.top() + 59.0 + slot as f32 * 36.0);
                    let spread = (origin % 7) as f32 * 5.0;
                    let x_out = a.right() + 13.0 + spread;
                    let x_in = b.left() - 13.0 - spread;
                    let points = [
                        from,
                        pos2(x_out, from.y),
                        pos2(x_out, route_y),
                        pos2(x_in, route_y),
                        pos2(x_in, to.y),
                        to,
                    ];
                    for p in points {
                        bounds = bounds.union(Rect::from_min_max(p, p));
                    }
                    transfers.push(Transfer {
                        source: origin,
                        target,
                        slot,
                        loser,
                        points,
                    });
                    continue;
                }
                if a.right() >= b.left() {
                    continue;
                }
                let x = f64::from((a.right() + b.left()) / 2.0);
                let from = Point::new(f64::from(a.right()), f64::from(a.top() + 77.0));
                let to = Point::new(
                    f64::from(b.left()),
                    f64::from(b.top() + 59.0 + slot as f32 * 36.0),
                );
                edges.push((
                    origin,
                    target,
                    slot,
                    loser,
                    CubicBez::new(from, Point::new(x, from.y), Point::new(x, to.y), to),
                ));
            }
        }
        let index = RTree::bulk_load(
            cards
                .iter()
                .filter(|card| card.rect.is_positive())
                .cloned()
                .collect(),
        );
        Self {
            cards,
            edges,
            transfers,
            headings,
            bounds,
            index,
        }
    }

    pub fn visible(&self, rect: Rect) -> impl Iterator<Item = &Card> {
        self.index.locate_in_envelope_intersecting(&envelope(rect))
    }
}

pub fn origin(source: Source) -> String {
    match source {
        Source::Winner(i) => format!("Победитель M{:02}", i + 1),
        Source::Loser(i) => format!("Проигравший M{:02}", i + 1),
        Source::Bye => "BYE".into(),
        Source::Team(_) => "Ожидается".into(),
    }
}

pub fn curve_points(curve: &CubicBez) -> impl Iterator<Item = Pos2> + '_ {
    (0..=24).map(|i| {
        let p = curve.eval(f64::from(i) / 24.0);
        pos2(p.x as f32, p.y as f32)
    })
}

pub fn short(value: &str, limit: usize) -> String {
    let mut letters = value.chars();
    let mut prefix: String = letters.by_ref().take(limit).collect();
    if letters.next().is_some() {
        prefix.push('…');
    }
    prefix
}

#[derive(Clone, Copy)]
pub struct ExportOptions {
    pub scale: u32,
    pub margin: u32,
    pub transparent: bool,
    pub podium: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            scale: 1,
            margin: 36,
            transparent: false,
            podium: false,
        }
    }
}

pub fn export(
    project: &Project,
    selection: &[(usize, Slice)],
    options: ExportOptions,
    path: &Path,
) -> Result<(), String> {
    let margin = options.margin as f32;
    let scenes: Vec<_> = selection
        .iter()
        .map(|&(index, slice)| {
            let stage = &project.stages[index];
            (
                stage,
                slice,
                Scene::filtered(stage, &included(stage, slice)),
            )
        })
        .collect();
    let width = scenes
        .iter()
        .map(|(_, _, scene)| scene.bounds.width())
        .fold(568.0, f32::max)
        + 2.0 * margin;
    let mut height = margin + 62.0;
    let mut content = String::new();
    svg_text(
        &mut content,
        margin,
        margin + 12.0,
        27,
        "#f1f7fb",
        &fit_text(&project.name, 27.0, 550.0),
    );
    for (stage, slice, scene) in scenes {
        let states = stage.resolve();
        let mut next_height = 58.0_f32;
        let title = match slice {
            Slice::Round(round) => format!("{} / Раунд {}", stage.name, round + 1),
            Slice::FromMatch(index) => format!("{} / С матча M{:02}", stage.name, index + 1),
            Slice::All => stage.name.clone(),
        };
        svg_text(
            &mut content,
            margin,
            height,
            19,
            "#ffb02e",
            &fit_text(&title, 19.0, 550.0),
        );
        height += 30.0;
        let x_offset = scene.bounds.left();
        write!(
            content,
            r#"<g transform="translate({} {height})">"#,
            (width - scene.bounds.width()) / 2.0 - x_offset
        )
        .unwrap();
        {
            for (p, label) in &scene.headings {
                svg_text(&mut content, p.x, p.y, 13, "#9caebb", label);
            }
            for (source, target, slot, loser, c) in &scene.edges {
                let ready = states[*source].outcome.is_some()
                    && matches!(states[*target].slots[*slot], Slot::Team(_));
                let stroke = match (ready, loser) {
                    (true, false) => "#916831",
                    (true, true) => "#526a79",
                    (false, _) => "#243744",
                };
                write!(content, r#"<path d="M {} {} C {} {},{} {},{} {}" fill="none" stroke="{stroke}" stroke-width="2"/>"#,
                    c.p0.x,c.p0.y,c.p1.x,c.p1.y,c.p2.x,c.p2.y,c.p3.x,c.p3.y).unwrap();
            }
            for transfer in &scene.transfers {
                let ready = matches!(states[transfer.target].slots[transfer.slot], Slot::Team(_));
                let stroke = if !ready {
                    "#243744"
                } else if transfer.loser {
                    "#526a79"
                } else {
                    "#916831"
                };
                let points = transfer
                    .points
                    .iter()
                    .map(|p| format!("{},{}", p.x, p.y))
                    .collect::<Vec<_>>()
                    .join(" ");
                write!(content,r#"<polyline points="{points}" fill="none" stroke="{stroke}" stroke-width="1.5" stroke-linejoin="round"/>"#).unwrap();
            }
        }
        for card in &scene.cards {
            if !card.rect.is_positive() {
                continue;
            }
            let game = &stage.matches[card.index];
            let state = states[card.index];
            let x = card.rect.min.x;
            let y = card.rect.min.y;
            next_height = next_height.max(card.rect.max.y);
            let card_height = card.rect.height();
            write!(content, r##"<rect x="{x}" y="{y}" width="{WIDTH}" height="{card_height}" rx="14" fill="#11212e" stroke="#243744"/>"##).unwrap();
            svg_text(
                &mut content,
                x + 14.0,
                y + 22.0,
                12,
                "#9caebb",
                &format!(
                    "M{:02}  /  BO{}  /  {}",
                    card.index + 1,
                    game.bo,
                    crate::status(game, state)
                ),
            );
            write!(
                content,
                r##"<path d="M {} {} H {}" stroke="#243744"/>"##,
                x + 14.0,
                y + 35.0,
                x + WIDTH - 14.0
            )
            .unwrap();
            for slot in 0..2 {
                let baseline = y + 64.0 + slot as f32 * 36.0;
                let label = if state.slots[slot] == Slot::Pending {
                    origin(game.sources[slot])
                } else {
                    project.team_name(state.slots[slot]).into()
                };
                let won = state
                    .outcome
                    .is_some_and(|o| matches!(o[0], Slot::Team(_)) && o[0] == state.slots[slot]);
                let mut text_x = x + 14.0;
                if let Slot::Team(id) = state.slots[slot] {
                    if let Some(logo) = project
                        .teams
                        .iter()
                        .find(|team| team.id == id)
                        .and_then(|team| team.logo_png.as_ref())
                    {
                        write!(content, r#"<image x="{}" y="{}" width="22" height="22" href="data:image/png;base64,{logo}"/>"#, text_x, baseline - 16.0).unwrap();
                    } else {
                        write!(content, r##"<rect x="{}" y="{}" width="22" height="22" rx="5" fill="#172c3a"/>"##, text_x, baseline - 16.0).unwrap();
                        svg_text(
                            &mut content,
                            text_x + 7.0,
                            baseline,
                            11,
                            "#9caebb",
                            &short(&label, 1).trim_end_matches('…').to_uppercase(),
                        );
                    }
                    text_x += 32.0;
                }
                svg_text(
                    &mut content,
                    text_x,
                    baseline,
                    15,
                    if won { "#ffb02e" } else { "#f1f7fb" },
                    &fit_text(&label, 15.0, x + WIDTH - 46.0 - text_x),
                );
                if let Some(score) = &game.score {
                    let score = score.series[slot];
                    let color = if won { "#ffb02e" } else { "#f1f7fb" };
                    write!(content,r#"<text x="{}" y="{baseline}" text-anchor="end" font-family="Inter" font-size="20" fill="{color}">{score}</text>"#,x+WIDTH-14.0).unwrap();
                }
            }
            let rows = map_rows(game);
            if !rows.is_empty() {
                write!(
                    content,
                    r##"<path d="M {} {} H {}" stroke="#243744"/>"##,
                    x + 14.0,
                    y + 117.0,
                    x + WIDTH - 14.0
                )
                .unwrap();
                for (row, (name, score)) in rows.iter().enumerate() {
                    let baseline = y + 139.0 + row as f32 * MAP_PITCH;
                    svg_text(
                        &mut content,
                        x + 14.0,
                        baseline,
                        13,
                        "#9caebb",
                        &fit_text(name, 13.0, WIDTH - 120.0),
                    );
                    let score = score.replace('&', "&amp;");
                    write!(content,r##"<text x="{}" y="{baseline}" text-anchor="end" font-family="Inter" font-size="14" fill="#f1f7fb">{score}</text>"##,x+WIDTH-14.0).unwrap();
                }
            }
        }
        content.push_str("</g>");
        height += next_height + margin + 28.0;
    }
    if options.podium
        && selection.len() == project.stages.len()
        && selection.iter().all(|(_, slice)| *slice == Slice::All)
    {
        svg_text(
            &mut content,
            margin,
            height,
            19,
            "#ffb02e",
            "ПРИЗОВЫЕ МЕСТА",
        );
        for (i, id) in project.podium().into_iter().enumerate() {
            let y = height + 32.0 + i as f32 * 38.0;
            let color = ["#ffb02e", "#becbd9", "#d29a78"][i];
            let name = id.map_or("Место пока не определено", |id| {
                project.team_name(Slot::Team(id))
            });
            svg_text(
                &mut content,
                margin,
                y,
                15,
                color,
                &format!("{} место", i + 1),
            );
            if let Some(logo) = id
                .and_then(|id| project.teams.iter().find(|team| team.id == id))
                .and_then(|team| team.logo_png.as_ref())
            {
                write!(content,r#"<image x="{}" y="{}" width="24" height="24" href="data:image/png;base64,{logo}"/>"#,margin+92.0,y-17.0).unwrap();
            }
            svg_text(
                &mut content,
                margin + 126.0,
                y,
                16,
                "#f1f7fb",
                &fit_text(name, 16.0, 410.0),
            );
        }
        height += 132.0 + margin;
    }
    let scale = options.scale;
    let w = (width * scale as f32).ceil() as u32;
    let h = (height * scale as f32).ceil() as u32;
    if w > 16_384 || h > 16_384 || u64::from(w) * u64::from(h) > 36_000_000 {
        return Err("Изображение слишком велико. Выберите этап, раунд или меньший масштаб.".into());
    }
    let background = if options.transparent {
        String::new()
    } else {
        format!(r##"<rect width="{width}" height="{height}" fill="#050b12"/>"##)
    };
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {width} {height}">{background}{content}</svg>"#
    );
    let mut options = resvg::usvg::Options::default();
    options
        .fontdb_mut()
        .load_font_data(include_bytes!("../assets/Inter-Regular.otf").to_vec());
    options
        .fontdb_mut()
        .load_font_data(include_bytes!("../assets/Inter-SemiBold.otf").to_vec());
    let tree = resvg::usvg::Tree::from_str(&svg, &options).map_err(|e| e.to_string())?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).ok_or("Не удалось выделить память")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.save_png(path).map_err(|e| e.to_string())
}

fn fit_text(text: &str, size: f32, width: f32) -> String {
    let font = ttf_parser::Face::parse(include_bytes!("../assets/Inter-Regular.otf"), 0).unwrap();
    let scale = size / f32::from(font.units_per_em());
    let advance = |character| {
        font.glyph_index(character)
            .and_then(|glyph| font.glyph_hor_advance(glyph))
            .map_or(size, |advance| f32::from(advance) * scale)
    };
    if text.chars().map(advance).sum::<f32>() <= width {
        return text.to_owned();
    }
    let mut left = width - advance('…');
    let mut fitted: String = text
        .chars()
        .take_while(|&character| {
            left -= advance(character);
            left >= 0.0
        })
        .collect();
    fitted.push('…');
    fitted
}

fn svg_text(svg: &mut String, x: f32, y: f32, size: u32, color: &str, value: &str) {
    let value = value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    write!(svg, r#"<text x="{x}" y="{y}" font-family="Inter, Segoe UI, Arial, sans-serif" font-size="{size}" fill="{color}">{value}</text>"#).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bracketlab::tournament::{Format, Rules};
    use uuid::Uuid;

    #[test]
    fn maps_resize_cards_for_bo1_bo3_bo5_and_png_scales() {
        use bracketlab::tournament::{MapScore, Score};
        for (bo, wins, losses, tech, count) in [
            (1, 1, 0, false, 1),
            (3, 2, 0, false, 2),
            (3, 2, 1, false, 3),
            (5, 3, 0, false, 3),
            (5, 3, 2, false, 5),
            (3, 2, 0, true, 0),
            (3, 2, 0, false, 0),
            (3, 1, 0, false, 1),
        ] {
            let mut project = crate::fixtures::project("Кубок", 8);
            let mut stage = Stage::new(
                "Плей-офф".into(),
                Format::Single,
                project.teams.iter().map(|t| t.id).collect(),
                bo,
                Rules::default(),
            )
            .unwrap();
            let maps = (0..count)
                .map(|i| MapScore {
                    name: format!("Карта {i}"),
                    rounds: if i < losses { [8, 13] } else { [13, 8] },
                })
                .collect();
            stage
                .set_score(
                    0,
                    Some(Score {
                        series: [wins, losses],
                        technical: tech,
                        maps,
                        note: String::new(),
                    }),
                )
                .unwrap();
            let expected = if count == 0 && !tech { 1 } else { count };
            assert_eq!(map_rows(&stage.matches[0]).len(), usize::from(expected));
            let scene = Scene::new(&stage);
            assert_eq!(scene.cards[0].rect.height() > HEIGHT, expected > 0);
            assert!(!scene.cards[0].rect.intersects(scene.cards[1].rect));
            project.stages.push(stage);
            if bo == 5 && losses == 2 {
                let path =
                    std::env::temp_dir().join(format!("bracketlab-png-{}.png", Uuid::new_v4()));
                for (scale, slice) in [
                    (1, Slice::All),
                    (2, Slice::Round(1)),
                    (1, Slice::FromMatch(4)),
                ] {
                    export(
                        &project,
                        &[(0, slice)],
                        ExportOptions {
                            scale,
                            transparent: true,
                            podium: true,
                            ..ExportOptions::default()
                        },
                        &path,
                    )
                    .unwrap();
                    let png = image::open(&path).unwrap().into_rgba8();
                    assert_eq!(png.get_pixel(0, 0)[3], 0);
                    assert!(png.width() >= 640 * scale);
                }
                std::fs::remove_file(path).unwrap();
            }
        }
    }

    #[test]
    fn export_suffix_follows_branches_and_keeps_round_peers() {
        for format in [Format::Single, Format::Double] {
            let stage = Stage::new(
                "Плей-офф".into(),
                format,
                (1..=16).map(Uuid::from_u128).collect(),
                3,
                Rules {
                    reset: true,
                    ..Rules::default()
                },
            )
            .unwrap();
            let start = stage.matches.iter().position(|m| m.round == 1).unwrap();
            let selection = included(&stage, Slice::FromMatch(start));
            for (game, yes) in stage.matches.iter().zip(&selection) {
                if game.lane == "Верхняя сетка" {
                    assert_eq!(*yes, game.round >= 1);
                }
                if game.lane == "Гранд-финал" {
                    assert!(yes);
                }
                if game.lane == "Нижняя сетка" && game.round == 0 {
                    assert!(!yes);
                }
            }
            let scene = Scene::filtered(&stage, &selection);
            assert_eq!(
                scene.visible(scene.bounds).count(),
                selection.iter().filter(|&&yes| yes).count()
            );
            if format == Format::Double {
                assert!(!scene.transfers.is_empty());
                let lower = stage
                    .matches
                    .iter()
                    .position(|m| m.lane == "Нижняя сетка" && m.round == 2)
                    .unwrap();
                let selection = included(&stage, Slice::FromMatch(lower));
                assert!(!stage
                    .matches
                    .iter()
                    .zip(&selection)
                    .any(|(m, yes)| m.lane == "Верхняя сетка" && *yes));
            }
        }
    }

    #[test]
    fn fixtures_have_non_overlapping_cards_and_clear_connectors() {
        for (count, format) in [
            (5, Format::Single),
            (16, Format::Single),
            (64, Format::Single),
            (8, Format::Double),
            (32, Format::Double),
            (4, Format::Gsl),
            (7, Format::RoundRobin),
            (16, Format::Swiss),
        ] {
            let entrants = (1..=count).map(Uuid::from_u128).collect();
            let stage = Stage::new(
                "Этап".into(),
                format,
                entrants,
                3,
                Rules {
                    reset: true,
                    ..Rules::default()
                },
            )
            .unwrap();
            let scene = Scene::new(&stage);
            assert_eq!(scene.visible(scene.bounds).count(), stage.matches.len());
            for (index, card) in scene.cards.iter().enumerate() {
                assert!(scene.bounds.contains_rect(card.rect));
                for other in &scene.cards[index + 1..] {
                    assert!(!card.rect.intersects(other.rect));
                }
            }
            for (origin, target, _, _, edge) in &scene.edges {
                for point in curve_points(edge) {
                    for card in &scene.cards {
                        if card.index != *origin && card.index != *target {
                            assert!(
                                !card.rect.contains(point),
                                "{format:?} edge {origin}->{target}"
                            );
                        }
                    }
                }
            }
            for transfer in &scene.transfers {
                for points in transfer.points.windows(2) {
                    for step in 1..16 {
                        let sample = points[0].lerp(points[1], step as f32 / 16.0);
                        assert!(!scene
                            .cards
                            .iter()
                            .any(|card| card.rect.shrink(0.1).contains(sample)));
                    }
                }
            }
        }
    }
}
