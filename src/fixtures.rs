use bracketlab::tournament::*;
use std::path::Path;
use uuid::Uuid;

const NAMES: [&str; 16] = [
    "Северный Вектор",
    "Vostok Five",
    "Точка Сбора",
    "Amber Wolves",
    "Метеор",
    "Signal Zero",
    "Третий Контур",
    "Орион",
    "Polar Strike",
    "Линия Огня",
    "Red Comet",
    "Кварц",
    "Клуб «Белая Башня»",
    "Neon Sector",
    "Легион",
    "Atlas Team",
];

pub fn project(name: &str, count: usize) -> Project {
    let mut project = Project::new(name.into());
    project.teams = (0..count)
        .map(|index| Team {
            id: Uuid::from_u128(index as u128 + 1),
            name: if index < NAMES.len() {
                NAMES[index].into()
            } else {
                format!("Команда {:02}", index + 1)
            },
            description: String::new(),
            players: vec![],
            logo_png: None,
        })
        .collect();
    project
}

fn result(slot: usize) -> Score {
    Score {
        series: if slot == 0 { [2, 0] } else { [1, 2] },
        technical: false,
        maps: vec![],
        note: String::new(),
    }
}

pub fn preview() -> Project {
    let mut example = project("Осенний кубок", 8);
    let mut playoff = Stage::new(
        "Плей-офф".into(),
        Format::Single,
        ids(&example),
        3,
        Rules::default(),
    )
    .unwrap();
    let mut score = result(0);
    score.maps = vec![
        MapScore {
            name: "Mirage".into(),
            rounds: [13, 8],
        },
        MapScore {
            name: "Nuke".into(),
            rounds: [13, 10],
        },
    ];
    playoff.set_score(0, Some(score)).unwrap();
    let mut score = result(1);
    score.maps = vec![
        MapScore {
            name: "Inferno".into(),
            rounds: [13, 11],
        },
        MapScore {
            name: "Dust II".into(),
            rounds: [9, 13],
        },
        MapScore {
            name: "Ancient".into(),
            rounds: [10, 13],
        },
    ];
    playoff.set_score(1, Some(score)).unwrap();
    example.stages.push(playoff);
    example
}

fn ids(project: &Project) -> Vec<Uuid> {
    project.teams.iter().map(|team| team.id).collect()
}

pub fn write(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(path)?;
    let scenarios = [
        ("single-05", Format::Single, 5),
        ("single-16", Format::Single, 16),
        ("single-64", Format::Single, 64),
        ("double-08", Format::Double, 8),
        ("double-16", Format::Double, 16),
        ("gsl-04", Format::Gsl, 4),
        ("league-07", Format::RoundRobin, 7),
        ("swiss-16", Format::Swiss, 16),
    ];
    let mut projects = vec![("preview-08", preview())];
    for (slug, format, count) in scenarios {
        let mut project = project(&format!("Кубок · {} · {count}", format.label()), count);
        let rules = Rules {
            third_place: false,
            reset: true,
            second_leg: false,
        };
        let bo = if slug == "double-16" { 5 } else { 3 };
        let mut stage = Stage::new("Основная сетка".into(), format, ids(&project), bo, rules)?;
        if bo == 5 {
            let maps = ["Mirage", "Nuke", "Inferno", "Ancient", "Dust II"]
                .into_iter()
                .enumerate()
                .map(|(i, name)| MapScore {
                    name: name.into(),
                    rounds: if i % 2 == 0 { [13, 9] } else { [10, 13] },
                })
                .collect();
            stage.set_score(
                0,
                Some(Score {
                    series: [3, 2],
                    technical: false,
                    maps,
                    note: String::new(),
                }),
            )?;
        }
        project.stages.push(stage);
        projects.push((slug, project));
    }
    let mut groups = project("Кубок · Групповой этап и плей-офф", 16);
    let mut qualifiers = vec![];
    for (index, group) in ids(&groups).chunks(4).enumerate() {
        let mut stage = Stage::new(
            format!("Группа {}", index + 1),
            Format::Gsl,
            group.to_vec(),
            3,
            Rules::default(),
        )?;
        for game in 0..stage.matches.len() {
            stage.set_score(game, Some(result(game % 2)))?;
        }
        qualifiers.extend(stage.qualifiers()?.into_iter().take(2));
        groups.stages.push(stage);
    }
    let mut playoff = Stage::new(
        "Плей-офф".into(),
        Format::Single,
        qualifiers,
        3,
        Rules::default(),
    )?;
    playoff.qualification = (0..4)
        .map(|stage| Qualification { stage, places: 2 })
        .collect();
    groups.stages.push(playoff);
    projects.push(("groups-playoff", groups));
    for (slug, project) in projects {
        project.validate()?;
        std::fs::write(
            path.join(format!("{slug}.json")),
            serde_json::to_vec_pretty(&project)?,
        )?;
        let selection: Vec<_> = (0..project.stages.len())
            .map(|i| (i, crate::scene::Slice::All))
            .collect();
        crate::scene::export(
            &project,
            &selection,
            Default::default(),
            &path.join(format!("{slug}.png")),
        )?;
        if slug == "double-16" {
            crate::scene::export(
                &project,
                &[(0, crate::scene::Slice::FromMatch(8))],
                Default::default(),
                &path.join("double-16-from-upper.png"),
            )?;
        }
    }
    Ok(())
}
