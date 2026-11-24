use petgraph::{algo::is_cyclic_directed, graph::DiGraph};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Player {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub name: String,
    pub steam_id: String,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Team {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub players: Vec<Player>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo_png: Option<String>,
}

impl Team {
    pub fn remove_player(&mut self, id: Uuid) -> bool {
        let Some(index) = self.players.iter().position(|player| player.id == id) else {
            return false;
        };
        self.players.remove(index);
        true
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Single,
    Double,
    RoundRobin,
    Swiss,
    Gsl,
}

impl Format {
    pub const ALL: [Self; 5] = [
        Self::Single,
        Self::Double,
        Self::RoundRobin,
        Self::Swiss,
        Self::Gsl,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Single => "Single Elimination",
            Self::Double => "Double Elimination",
            Self::RoundRobin => "Round Robin",
            Self::Swiss => "Swiss",
            Self::Gsl => "GSL",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "id", rename_all = "snake_case")]
pub enum Source {
    Team(Uuid),
    Winner(usize),
    Loser(usize),
    Bye,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MapScore {
    pub name: String,
    pub rounds: [u8; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Score {
    pub series: [u8; 2],
    #[serde(default)]
    pub technical: bool,
    #[serde(default)]
    pub maps: Vec<MapScore>,
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Match {
    pub round: usize,
    pub lane: String,
    pub sources: [Source; 2],
    pub bo: u8,
    pub score: Option<Score>,
    pub reset_of: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Pending,
    Bye,
    Team(Uuid),
}

#[derive(Clone, Copy, Debug)]
pub struct Resolved {
    pub slots: [Slot; 2],
    pub outcome: Option<[Slot; 2]>,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stage {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub swiss_finished: bool,
    pub name: String,
    pub format: Format,
    pub entrants: Vec<Uuid>,
    pub draw_seed: Option<String>,
    pub matches: Vec<Match>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qualification: Vec<Qualification>,
    #[serde(default)]
    pub tie_break: TieBreak,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TieBreak {
    #[default]
    Seed,
    Maps,
    Rounds,
}

impl TieBreak {
    pub const ALL: [Self; 3] = [Self::Seed, Self::Maps, Self::Rounds];
    pub fn label(self) -> &'static str {
        match self {
            Self::Seed => "Посев",
            Self::Maps => "Разница карт",
            Self::Rounds => "Разница раундов",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Qualification {
    pub stage: usize,
    pub places: usize,
}

#[derive(Clone, Copy, Default)]
pub struct Rules {
    pub third_place: bool,
    pub reset: bool,
    pub second_leg: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    #[serde(default = "project_version")]
    pub version: u32,
    #[serde(default = "project_name")]
    pub name: String,
    #[serde(default)]
    pub teams: Vec<Team>,
    #[serde(default)]
    pub stages: Vec<Stage>,
}

fn project_version() -> u32 {
    1
}
fn project_name() -> String {
    "Новый турнир".into()
}

impl Project {
    pub fn remove_stage(&mut self, index: usize) -> Result<(), String> {
        if index >= self.stages.len() {
            return Err("Этап не найден".into());
        }
        if let Some(stage) = self
            .stages
            .iter()
            .find(|stage| stage.qualification.iter().any(|link| link.stage == index))
        {
            return Err(format!(
                "Этап «{}» зависит от этого этапа. Сначала удалите зависимый этап.",
                stage.name
            ));
        }
        self.stages.remove(index);
        for link in self
            .stages
            .iter_mut()
            .flat_map(|stage| &mut stage.qualification)
        {
            if link.stage > index {
                link.stage -= 1;
            }
        }
        Ok(())
    }

    pub fn podium(&self) -> [Option<Uuid>; 3] {
        let mut terminals = self.stages.iter().enumerate().filter(|(i, _)| {
            !self
                .stages
                .iter()
                .any(|stage| stage.qualification.iter().any(|link| link.stage == *i))
        });
        let Some((_, stage)) = terminals.next() else {
            return [None; 3];
        };
        // Independent groups do not determine an overall tournament champion.
        if terminals.next().is_some()
            && self
                .stages
                .last()
                .is_some_and(|stage| stage.name.contains(" · Группа "))
        {
            return [None; 3];
        }
        self.stages.last().unwrap_or(stage).podium()
    }

    pub fn new(name: String) -> Self {
        Self {
            version: 1,
            name,
            teams: vec![],
            stages: vec![],
        }
    }

    pub fn team_name(&self, slot: Slot) -> &str {
        match slot {
            Slot::Pending => "Ожидается",
            Slot::Bye => "BYE",
            Slot::Team(id) => self
                .teams
                .iter()
                .find(|t| t.id == id)
                .map_or("Команда удалена", |t| t.name.as_str()),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let ids: HashSet<_> = self.teams.iter().map(|t| t.id).collect();
        if self.version != 1 || ids.len() != self.teams.len() || ids.len() > 64 {
            return Err("Несовместимый проект или повреждённый список команд".into());
        }
        if self.teams.iter().any(|team| {
            team.players
                .iter()
                .map(|player| player.id)
                .collect::<HashSet<_>>()
                .len()
                != team.players.len()
        }) {
            return Err("Повторяющийся ID игрока в составе команды".into());
        }
        if self.teams.iter().any(|team| {
            team.logo_png.as_ref().is_some_and(|logo| {
                logo.len() > 100_000
                    || !logo
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"+/=".contains(&c))
            })
        }) {
            return Err("Логотип повреждён или слишком велик".into());
        }
        for (stage_index, stage) in self.stages.iter().enumerate() {
            let entrants: HashSet<_> = stage.entrants.iter().copied().collect();
            if !(2..=64).contains(&entrants.len())
                || entrants.len() != stage.entrants.len()
                || !entrants.is_subset(&ids)
                || stage.matches.is_empty()
                || stage.matches.len() > 4096
            {
                return Err("Некорректный состав этапа".into());
            }
            stage.validate_graph()?;
            let resolved = stage.resolve();
            for (index, game) in stage.matches.iter().enumerate() {
                if !matches!(game.bo, 1 | 3 | 5)
                    || game
                        .sources
                        .iter()
                        .any(|s| matches!(s, Source::Team(id) if !entrants.contains(id)))
                {
                    return Err("Некорректные настройки встречи".into());
                }
                if let Some(score) = &game.score {
                    validate_score(game.bo, score)?;
                    if !resolved[index].active
                        || !matches!(resolved[index].slots, [Slot::Team(a), Slot::Team(b)] if a != b)
                    {
                        return Err("Результат сохранён для недоступного матча".into());
                    }
                }
            }
            if !stage.qualification.is_empty() {
                let qualified = self.qualified(stage_index, &stage.qualification)?;
                if qualified.into_iter().collect::<HashSet<_>>() != entrants {
                    return Err("Редактирование изменило бы участников следующего этапа".into());
                }
            }
        }
        Ok(())
    }

    pub fn qualified(&self, before: usize, sources: &[Qualification]) -> Result<Vec<Uuid>, String> {
        let mut qualified = vec![];
        let mut seen_stages = HashSet::new();
        for source in sources {
            if source.stage >= before || !seen_stages.insert(source.stage) || source.places == 0 {
                return Err("Некорректная связь этапов".into());
            }
            let stage = self
                .stages
                .get(source.stage)
                .ok_or("Источник квалификации не найден")?;
            let rank = stage.qualifiers()?;
            if rank.len() < source.places {
                return Err("Недостаточно участников для квалификации".into());
            }
            for &id in &rank[..source.places] {
                if !qualified.contains(&id) {
                    qualified.push(id);
                }
            }
        }
        Ok(qualified)
    }
}

pub fn draw(entrants: &mut [Uuid], seed: u64) {
    draw_pots(entrants, seed, entrants.len(), &HashSet::new());
}

pub fn draw_pots(entrants: &mut [Uuid], seed: u64, pot_size: usize, locked: &HashSet<Uuid>) {
    let mut rng = seed;
    for pot in entrants.chunks_mut(pot_size.max(1)) {
        let mut candidates: Vec<_> = pot
            .iter()
            .copied()
            .filter(|id| !locked.contains(id))
            .collect();
        for index in (1..candidates.len()).rev() {
            let bound = index as u64 + 1;
            let threshold = bound.wrapping_neg() % bound;
            let position = loop {
                rng = rng.wrapping_add(0x9e3779b97f4a7c15);
                let mut word = rng;
                word = (word ^ (word >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                word = (word ^ (word >> 27)).wrapping_mul(0x94d049bb133111eb);
                word ^= word >> 31;
                if word >= threshold {
                    break (word % bound) as usize;
                }
            };
            candidates.swap(index, position);
        }
        let mut candidates = candidates.into_iter();
        for slot in pot {
            if !locked.contains(slot) {
                *slot = candidates.next().unwrap();
            }
        }
    }
}

pub fn group_stages(
    name: &str,
    format: Format,
    entrants: &[Uuid],
    groups: usize,
    bo: u8,
    rules: Rules,
) -> Result<Vec<Stage>, String> {
    if name.is_empty() {
        return Err("Введите название этапа".into());
    }
    if groups < 2 || groups > entrants.len() / 2 {
        return Err("Нужно минимум две команды в каждой группе".into());
    }
    let mut allocation = vec![vec![]; groups];
    for (row, pot) in entrants.chunks(groups).enumerate() {
        for (column, &id) in pot.iter().enumerate() {
            let group = if row % 2 == 0 {
                column
            } else {
                groups - 1 - column
            };
            allocation[group].push(id);
        }
    }
    allocation
        .into_iter()
        .enumerate()
        .map(|(index, group)| {
            Stage::new(
                format!("{name} · Группа {}", index + 1),
                format,
                group,
                bo,
                rules,
            )
        })
        .collect()
}

impl Stage {
    pub fn podium(&self) -> [Option<Uuid>; 3] {
        let states = self.resolve();
        let team = |slot| {
            if let Slot::Team(id) = slot {
                Some(id)
            } else {
                None
            }
        };
        if self.format == Format::Single {
            let final_result = self
                .matches
                .iter()
                .zip(&states)
                .rev()
                .find(|(game, _)| game.lane == "Верхняя сетка")
                .and_then(|(_, state)| state.outcome);
            let bronze = self
                .matches
                .iter()
                .zip(&states)
                .find(|(game, _)| game.lane == "За третье место")
                .and_then(|(_, state)| state.outcome)
                .and_then(|outcome| team(outcome[0]));
            return [
                final_result.and_then(|o| team(o[0])),
                final_result.and_then(|o| team(o[1])),
                bronze,
            ];
        }
        if self.format == Format::Swiss && !self.swiss_finished {
            return [None; 3];
        }
        let Ok(rank) = self.qualifiers() else {
            return [None; 3];
        };
        std::array::from_fn(|i| rank.get(i).copied())
    }

    pub fn swap_swiss(&mut self, first: usize, slot: usize, team: Uuid) -> Result<(), String> {
        if self.format != Format::Swiss || slot >= 2 {
            return Err("Перестановка доступна в Swiss".into());
        }
        let game = self.matches.get(first).ok_or("Матч не найден")?;
        let round = game.round;
        if game.score.is_some()
            || self.matches.last().is_none_or(|game| game.round != round)
            || !game
                .sources
                .iter()
                .all(|source| matches!(source, Source::Team(_)))
        {
            return Err("Можно поменять команды только в несыгранных парах текущего тура".into());
        }
        let (second, other_slot) = self
            .matches
            .iter()
            .enumerate()
            .find_map(|(index, game)| {
                if game.round != round
                    || game.score.is_some()
                    || index == first
                    || !game
                        .sources
                        .iter()
                        .all(|source| matches!(source, Source::Team(_)))
                {
                    return None;
                }
                game.sources
                    .iter()
                    .position(|source| *source == Source::Team(team))
                    .map(|slot| (index, slot))
            })
            .ok_or("Команда недоступна для перестановки")?;
        let previous = self.matches[first].sources[slot];
        self.matches[first].sources[slot] = Source::Team(team);
        self.matches[second].sources[other_slot] = previous;
        Ok(())
    }
    pub fn new(
        name: String,
        format: Format,
        entrants: Vec<Uuid>,
        bo: u8,
        rules: Rules,
    ) -> Result<Self, String> {
        if !(2..=64).contains(&entrants.len())
            || entrants.iter().collect::<HashSet<_>>().len() != entrants.len()
            || !matches!(bo, 1 | 3 | 5)
        {
            return Err("Нужно 2–64 уникальные команды и корректный формат серии".into());
        }
        if format == Format::Gsl && entrants.len() != 4 {
            return Err("Для GSL нужны ровно четыре команды".into());
        }
        let mut stage = Self {
            name,
            format,
            entrants,
            draw_seed: None,
            matches: vec![],
            qualification: vec![],
            swiss_finished: false,
            tie_break: TieBreak::Seed,
        };
        match format {
            Format::Single | Format::Double => stage.elimination(bo, rules),
            Format::RoundRobin => stage.round_robin(bo, rules.second_leg),
            Format::Swiss => stage.next_swiss_round(bo)?,
            Format::Gsl => stage.gsl(bo),
        }
        stage.validate_graph()?;
        Ok(stage)
    }

    fn add(&mut self, round: usize, lane: &str, sources: [Source; 2], bo: u8) -> usize {
        let id = self.matches.len();
        self.matches.push(Match {
            round,
            lane: lane.into(),
            sources,
            bo,
            score: None,
            reset_of: None,
        });
        id
    }

    fn elimination(&mut self, bo: u8, rules: Rules) {
        let size = self.entrants.len().next_power_of_two();
        let mut seeds = vec![0, 1];
        while seeds.len() < size {
            let last = seeds.len() * 2 - 1;
            seeds = seeds
                .into_iter()
                .flat_map(|seed| [seed, last - seed])
                .collect();
        }
        let mut sources: Vec<_> = seeds
            .into_iter()
            .map(|seed| {
                self.entrants
                    .get(seed)
                    .copied()
                    .map_or(Source::Bye, Source::Team)
            })
            .collect();
        let mut winners = Vec::new();
        while sources.len() > 1 {
            let round = winners.len();
            let ids: Vec<_> = sources
                .chunks_exact(2)
                .map(|pair| self.add(round, "Верхняя сетка", [pair[0], pair[1]], bo))
                .collect();
            sources = ids.iter().map(|id| Source::Winner(*id)).collect();
            winners.push(ids);
        }
        let upper = winners.last().unwrap()[0];
        if self.format == Format::Single {
            if rules.third_place && winners.len() >= 2 {
                let semifinals = &winners[winners.len() - 2];
                self.add(
                    winners.len() - 1,
                    "За третье место",
                    [Source::Loser(semifinals[0]), Source::Loser(semifinals[1])],
                    bo,
                );
            }
            return;
        }
        let mut lower: Vec<_> = winners[0].iter().map(|id| Source::Loser(*id)).collect();
        let mut round = 0;
        for dropped in winners.iter().skip(1) {
            lower = lower
                .chunks_exact(2)
                .map(|pair| Source::Winner(self.add(round, "Нижняя сетка", [pair[0], pair[1]], bo)))
                .collect();
            round += 1;
            lower = lower
                .into_iter()
                .zip(dropped.iter().rev())
                .map(|(survivor, id)| {
                    Source::Winner(self.add(
                        round,
                        "Нижняя сетка",
                        [survivor, Source::Loser(*id)],
                        bo,
                    ))
                })
                .collect();
            round += 1;
        }
        let final_round = round.max(winners.len());
        let grand = self.add(
            final_round,
            "Гранд-финал",
            [Source::Winner(upper), lower[0]],
            bo,
        );
        if rules.reset {
            let reset = self.add(
                final_round + 1,
                "Гранд-финал",
                [Source::Winner(grand), Source::Loser(grand)],
                bo,
            );
            self.matches[reset].reset_of = Some(grand);
        }
    }

    fn round_robin(&mut self, bo: u8, second_leg: bool) {
        let mut ring: Vec<_> = self.entrants.iter().copied().map(Source::Team).collect();
        if ring.len() % 2 != 0 {
            ring.push(Source::Bye);
        }
        let rounds = ring.len() - 1;
        for round in 0..rounds {
            for index in 0..ring.len() / 2 {
                let pair = [ring[index], ring[ring.len() - 1 - index]];
                if pair.contains(&Source::Bye) {
                    continue;
                }
                self.add(round, "Туры", pair, bo);
            }
            ring[1..].rotate_right(1);
        }
        if second_leg {
            for index in 0..self.matches.len() {
                let game = &self.matches[index];
                self.add(
                    game.round + rounds,
                    "Туры",
                    [game.sources[1], game.sources[0]],
                    bo,
                );
            }
        }
    }

    fn gsl(&mut self, bo: u8) {
        let ids = self.entrants.clone();
        let a = self.add(
            0,
            "Группа",
            [Source::Team(ids[0]), Source::Team(ids[3])],
            bo,
        );
        let b = self.add(
            0,
            "Группа",
            [Source::Team(ids[1]), Source::Team(ids[2])],
            bo,
        );
        let winners = self.add(1, "Группа", [Source::Winner(a), Source::Winner(b)], bo);
        let losers = self.add(1, "Группа", [Source::Loser(a), Source::Loser(b)], bo);
        self.add(
            2,
            "Группа",
            [Source::Loser(winners), Source::Winner(losers)],
            bo,
        );
    }

    pub fn standings(&self) -> Vec<(Uuid, usize, usize)> {
        let mut rows: Vec<_> = self
            .entrants
            .iter()
            .copied()
            .map(|id| (id, 0, 0, 0i32, 0i32))
            .collect();
        let indexes: HashMap<_, _> = self
            .entrants
            .iter()
            .copied()
            .enumerate()
            .map(|(index, id)| (id, index))
            .collect();
        for (game, status) in self.matches.iter().zip(self.resolve()) {
            if let Some([Slot::Team(winner), loser]) = status.outcome {
                rows[indexes[&winner]].1 += 1;
                if let Slot::Team(loser) = loser {
                    rows[indexes[&loser]].2 += 1;
                }
            }
            if let (Some(score), [Slot::Team(a), Slot::Team(b)]) = (&game.score, status.slots) {
                if !score.technical {
                    let maps = i32::from(score.series[0]) - i32::from(score.series[1]);
                    let rounds: i32 = score
                        .maps
                        .iter()
                        .map(|map| i32::from(map.rounds[0]) - i32::from(map.rounds[1]))
                        .sum();
                    rows[indexes[&a]].3 += maps;
                    rows[indexes[&b]].3 -= maps;
                    rows[indexes[&a]].4 += rounds;
                    rows[indexes[&b]].4 -= rounds;
                }
            }
        }
        rows.sort_by_key(|(_, wins, losses, maps, rounds)| {
            let tie = match self.tie_break {
                TieBreak::Seed => 0,
                TieBreak::Maps => *maps,
                TieBreak::Rounds => *rounds,
            };
            (std::cmp::Reverse(*wins), *losses, std::cmp::Reverse(tie))
        });
        rows.into_iter()
            .map(|(id, wins, losses, _, _)| (id, wins, losses))
            .collect()
    }

    pub fn max_consecutive_matches(&self) -> usize {
        let mut current = HashMap::new();
        let mut longest = 0;
        for (index, game) in self.matches.iter().enumerate() {
            for id in game.sources.iter().filter_map(|source| match source {
                Source::Team(id) => Some(id),
                _ => None,
            }) {
                let (next, count) = current.entry(id).or_insert((index, 0usize));
                *count = if *next == index { *count + 1 } else { 1 };
                *next = index + 1;
                longest = longest.max(*count);
            }
        }
        longest
    }

    pub fn qualifiers(&self) -> Result<Vec<Uuid>, String> {
        let states = self.resolve();
        if states.iter().any(|s| s.active && s.outcome.is_none()) {
            return Err("Завершите все встречи этапа".into());
        }
        let rank = match self.format {
            Format::Single | Format::Double => {
                let final_id = self
                    .matches
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(i, m)| states[*i].active && m.lane != "За третье место")
                    .ok_or("Финал не найден")?
                    .0;
                let mut rank: Vec<_> = states[final_id]
                    .outcome
                    .ok_or("Финал не завершён")?
                    .into_iter()
                    .filter_map(|slot| {
                        if let Slot::Team(id) = slot {
                            Some(id)
                        } else {
                            None
                        }
                    })
                    .collect();
                for (index, game) in self.matches.iter().enumerate().rev() {
                    let placement = if game.lane == "За третье место" {
                        0
                    } else {
                        1
                    };
                    if let Some(outcome) = states[index].outcome {
                        for slot in &outcome[placement..] {
                            if let Slot::Team(id) = slot {
                                if !rank.contains(id) {
                                    rank.push(*id);
                                }
                            }
                        }
                    }
                }
                rank
            }
            Format::Gsl => {
                let result = |index: usize| {
                    states
                        .get(index)
                        .and_then(|state| state.outcome)
                        .ok_or("Некорректная сетка GSL")
                };
                [result(2)?[0], result(4)?[0], result(4)?[1], result(3)?[1]]
                    .into_iter()
                    .filter_map(|slot| {
                        if let Slot::Team(id) = slot {
                            Some(id)
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            _ => self.standings().into_iter().map(|row| row.0).collect(),
        };
        Ok(rank)
    }

    pub fn next_swiss_round(&mut self, bo: u8) -> Result<(), String> {
        if self.swiss_finished {
            return Err("Этап Swiss завершён".into());
        }
        if self.format != Format::Swiss || !matches!(bo, 1 | 3 | 5) {
            return Err("Новый тур доступен только для Swiss".into());
        }
        let resolved = self.resolve();
        if resolved.iter().any(|s| s.outcome.is_none()) {
            return Err("Завершите предыдущий тур".into());
        }
        let round = self.matches.last().map_or(0, |m| m.round + 1);
        if round >= self.entrants.len() {
            return Err("Достигнут предел туров".into());
        }
        let mut opponents = HashSet::new();
        let mut byes = HashMap::<Uuid, usize>::new();
        for game in &resolved {
            match game.slots {
                [Slot::Team(a), Slot::Team(b)] => {
                    opponents.insert((a, b));
                    opponents.insert((b, a));
                }
                [Slot::Team(a), Slot::Bye] => *byes.entry(a).or_default() += 1,
                _ => {}
            }
        }
        let mut ranked: Vec<_> = self.standings().into_iter().map(|row| row.0).collect();
        let bye = if ranked.len() % 2 != 0 {
            let index = (0..ranked.len())
                .rev()
                .min_by_key(|i| byes.get(&ranked[*i]).copied().unwrap_or(0))
                .unwrap();
            Some(ranked.remove(index))
        } else {
            None
        };
        let mut budget = 20_000;
        let pairs = pair_swiss(&ranked, &opponents, &mut budget)
            .unwrap_or_else(|| ranked.chunks_exact(2).map(|p| [p[0], p[1]]).collect());
        for [a, b] in pairs {
            self.add(round, "Туры", [Source::Team(a), Source::Team(b)], bo);
        }
        if let Some(id) = bye {
            self.add(round, "Туры", [Source::Team(id), Source::Bye], bo);
        }
        Ok(())
    }

    pub fn validate_graph(&self) -> Result<(), String> {
        let mut graph = DiGraph::<(), ()>::new();
        let nodes: Vec<_> = self.matches.iter().map(|_| graph.add_node(())).collect();
        for (index, game) in self.matches.iter().enumerate() {
            let dependencies = game
                .sources
                .iter()
                .filter_map(|source| match source {
                    Source::Winner(id) | Source::Loser(id) => Some(*id),
                    _ => None,
                })
                .chain(game.reset_of);
            for source in dependencies {
                if source >= index {
                    return Err("Недопустимая зависимость между матчами".into());
                }
                graph.add_edge(nodes[source], nodes[index], ());
            }
        }
        if is_cyclic_directed(&graph) {
            return Err("Цикл зависимостей".into());
        }
        Ok(())
    }

    pub fn resolve(&self) -> Vec<Resolved> {
        let mut states: Vec<Resolved> = Vec::with_capacity(self.matches.len());
        for game in &self.matches {
            let active = game.reset_of.is_none_or(|id| {
                let prev = states[id];
                matches!(prev.slots[1], Slot::Team(_))
                    && prev.outcome.is_some_and(|o| o[0] == prev.slots[1])
            });
            let slots = game.sources.map(|source| match source {
                Source::Team(id) => Slot::Team(id),
                Source::Bye => Slot::Bye,
                Source::Winner(id) => states[id].outcome.map_or(Slot::Pending, |o| o[0]),
                Source::Loser(id) => states[id].outcome.map_or(Slot::Pending, |o| o[1]),
            });
            let outcome = if !active {
                None
            } else {
                match slots {
                    [Slot::Pending, _] | [_, Slot::Pending] => None,
                    [Slot::Bye, other] | [other, Slot::Bye] => Some([other, Slot::Bye]),
                    [a, b] => game
                        .score
                        .as_ref()
                        .filter(|score| score.series.contains(&(game.bo / 2 + 1)))
                        .map(|score| {
                            if score.series[0] > score.series[1] {
                                [a, b]
                            } else {
                                [b, a]
                            }
                        }),
                }
            };
            states.push(Resolved {
                slots,
                outcome,
                active,
            });
        }
        states
    }

    pub fn set_score(&mut self, index: usize, score: Option<Score>) -> Result<(), String> {
        let before = self.resolve();
        let game = self.matches.get(index).ok_or("Матч не найден")?;
        if let Some(result) = &score {
            if !before[index].active
                || !matches!(before[index].slots, [Slot::Team(a), Slot::Team(b)] if a != b)
            {
                return Err("Встреча ещё не готова к вводу счёта".into());
            }
            validate_score(game.bo, result)?;
        }
        let old = std::mem::replace(&mut self.matches[index].score, score);
        let after = self.resolve();
        if self.matches.iter().enumerate().any(|(i, m)| {
            i != index
                && m.score.is_some()
                && (before[i].slots != after[i].slots || before[i].active != after[i].active)
        }) {
            self.matches[index].score = old;
            return Err("Сначала снимите результаты зависимых матчей".into());
        }
        if self.format == Format::Swiss
            && self.matches[index].round < self.matches.last().unwrap().round
            && before[index].outcome != after[index].outcome
        {
            self.matches[index].score = old;
            return Err("Результат уже использован для жеребьёвки следующего тура".into());
        }
        Ok(())
    }
}

fn pair_swiss(
    ids: &[Uuid],
    opponents: &HashSet<(Uuid, Uuid)>,
    budget: &mut usize,
) -> Option<Vec<[Uuid; 2]>> {
    let Some((&first, rest)) = ids.split_first() else {
        return Some(vec![]);
    };
    for (index, &other) in rest.iter().enumerate() {
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        if opponents.contains(&(first, other)) {
            continue;
        }
        let mut remaining = rest.to_vec();
        remaining.remove(index);
        if let Some(mut pairs) = pair_swiss(&remaining, opponents, budget) {
            pairs.insert(0, [first, other]);
            return Some(pairs);
        }
    }
    None
}

fn validate_score(bo: u8, score: &Score) -> Result<(), String> {
    let target = bo / 2 + 1;
    let [a, b] = score.series;
    let complete = (a == target && b < target) || (b == target && a < target);
    let partial = !score.maps.is_empty() && !score.technical && a < target && b < target;
    if !complete && !partial {
        return Err(format!(
            "Для BO{bo} победителю нужно {target} побед по картам"
        ));
    }
    if score.technical && !score.maps.is_empty() {
        return Err("Технический результат не содержит сыгранных карт".into());
    }
    if !score.maps.is_empty() {
        let mut wins = [0u8; 2];
        for map in &score.maps {
            if map.name.trim().is_empty()
                || map.rounds[0] == map.rounds[1]
                || wins.contains(&target)
            {
                return Err("Проверьте название, счёт и порядок карт".into());
            }
            wins[usize::from(map.rounds[1] > map.rounds[0])] += 1;
        }
        if wins != score.series {
            return Err("Счёт серии не совпадает с картами".into());
        }
    }
    Ok(())
}
