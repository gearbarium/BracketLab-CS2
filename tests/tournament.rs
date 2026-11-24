use bracketlab::{
    storage::{decode, Storage},
    tournament::*,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

fn teams(count: usize) -> Vec<Uuid> {
    (1..=count)
        .map(|index| Uuid::from_u128(index as u128))
        .collect()
}

#[test]
fn shared_draw_and_locked_pots() {
    let mut ids = teams(8);
    draw(&mut ids, 42);
    assert_eq!(
        ids.iter().map(Uuid::as_u128).collect::<Vec<_>>(),
        [4, 2, 7, 3, 5, 1, 8, 6]
    );
    let locked = HashSet::from([ids[1], ids[5]]);
    let original = ids.clone();
    draw_pots(&mut ids, 11, 4, &locked);
    assert_eq!(ids[1], original[1]);
    assert_eq!(ids[5], original[5]);
    assert_eq!(
        ids[..4].iter().collect::<HashSet<_>>(),
        original[..4].iter().collect()
    );
}

#[test]
fn four_groups_qualify_eight_teams_to_playoff() {
    let ids = teams(16);
    let players = ids
        .iter()
        .copied()
        .map(|id| Team {
            id,
            name: format!("Команда {}", id.as_u128()),
            description: String::new(),
            players: vec![],
            logo_png: None,
        })
        .collect();
    let mut project = Project::new("Кубок".into());
    project.teams = players;
    project.stages =
        group_stages("Квалификация", Format::Gsl, &ids, 4, 3, Rules::default()).unwrap();
    for group in &mut project.stages {
        complete(group);
    }
    let qualification: Vec<_> = (0..4)
        .map(|stage| Qualification { stage, places: 2 })
        .collect();
    let qualified = project.qualified(4, &qualification).unwrap();
    assert_eq!(qualified.len(), 8);
    let mut next = Stage::new(
        "Плей-офф".into(),
        Format::Single,
        qualified,
        3,
        Rules::default(),
    )
    .unwrap();
    next.qualification = qualification;
    project.stages.push(next);
    assert!(project.validate().is_ok());
    project.stages[0].set_score(4, None).unwrap();
    assert!(project.validate().is_err());
}

fn win(slot: usize) -> Score {
    Score {
        series: if slot == 0 { [2, 0] } else { [0, 2] },
        maps: vec![],
        technical: false,
        note: String::new(),
    }
}

fn complete(stage: &mut Stage) {
    for index in 0..stage.matches.len() {
        let status = stage.resolve()[index];
        if status.active && matches!(status.slots, [Slot::Team(a), Slot::Team(b)] if a != b) {
            stage.set_score(index, Some(win(0))).unwrap();
        }
    }
}

#[test]
fn single_elimination_advances_for_every_team_count() {
    for count in 2..=64 {
        let ids = teams(count);
        let mut stage = Stage::new(
            "Плей-офф".into(),
            Format::Single,
            ids.clone(),
            3,
            Rules::default(),
        )
        .unwrap();
        complete(&mut stage);
        assert_eq!(
            stage.matches.iter().filter(|m| m.score.is_some()).count(),
            count - 1
        );
        assert_eq!(stage.qualifiers().unwrap()[0], ids[0]);
    }
}

#[test]
fn double_elimination_requires_two_losses_with_reset() {
    for count in 2..=64 {
        let mut stage = Stage::new(
            "Плей-офф".into(),
            Format::Double,
            teams(count),
            3,
            Rules {
                reset: true,
                ..Rules::default()
            },
        )
        .unwrap();
        let grand = stage.matches.len() - 2;
        for index in 0..stage.matches.len() {
            let status = stage.resolve()[index];
            if status.active && matches!(status.slots, [Slot::Team(a), Slot::Team(b)] if a != b) {
                stage
                    .set_score(index, Some(win(usize::from(grand == index))))
                    .unwrap();
            }
        }
        assert!(stage.matches[grand + 1].score.is_some());
        let champion = stage.qualifiers().unwrap()[0];
        let mut losses = HashMap::<Uuid, usize>::new();
        for status in stage.resolve() {
            if let Some([Slot::Team(_), Slot::Team(loser)]) = status.outcome {
                *losses.entry(loser).or_default() += 1;
            }
        }
        for id in &stage.entrants {
            assert_eq!(losses[id], if *id == champion { 1 } else { 2 });
        }
    }
}

#[test]
fn round_robin_unique_pairs_and_rounds() {
    for count in 2..=64 {
        let stage = Stage::new(
            "Лига".into(),
            Format::RoundRobin,
            teams(count),
            3,
            Rules {
                second_leg: true,
                ..Rules::default()
            },
        )
        .unwrap();
        let mut directed = HashSet::new();
        let mut in_round = HashSet::new();
        for game in &stage.matches {
            let [Source::Team(a), Source::Team(b)] = game.sources else {
                panic!("Expected two teams")
            };
            assert!(directed.insert((a, b)));
            assert!(in_round.insert((game.round, a)));
            assert!(in_round.insert((game.round, b)));
        }
        assert_eq!(directed.len(), count * (count - 1));
        assert!(stage.max_consecutive_matches() <= 2);
    }
}

#[test]
fn swiss_avoids_rematches_and_rotates_bye() {
    let mut stage = Stage::new(
        "Швейцарка".into(),
        Format::Swiss,
        teams(7),
        3,
        Rules::default(),
    )
    .unwrap();
    for _ in 0..3 {
        complete(&mut stage);
        stage.next_swiss_round(3).unwrap();
    }
    let mut byes = HashSet::new();
    let mut pairs = HashSet::new();
    for status in stage.resolve() {
        match status.slots {
            [Slot::Team(a), Slot::Team(b)] => assert!(pairs.insert((a.min(b), a.max(b)))),
            [Slot::Team(a), Slot::Bye] => assert!(byes.insert(a)),
            _ => panic!("Unexpected Swiss slot"),
        }
    }
}

#[test]
fn removing_result_cannot_change_played_match_participants() {
    let mut stage =
        Stage::new("Группа".into(), Format::Gsl, teams(4), 3, Rules::default()).unwrap();
    complete(&mut stage);
    assert_eq!(stage.qualifiers().unwrap().len(), 4);
    let before = stage.resolve()[4].slots;
    assert!(stage.set_score(0, Some(win(1))).is_err());
    assert_eq!(stage.resolve()[4].slots, before);
    assert!(stage.set_score(4, None).is_ok());
    assert!(stage.set_score(0, Some(win(1))).is_err());
}

#[test]
fn portable_round_trip_and_sqlite_history() {
    let mut project = Project::new("Кубок".into());
    project.teams = teams(5)
        .into_iter()
        .enumerate()
        .map(|(i, id)| Team {
            id,
            name: format!("Команда {}", i + 1),
            description: String::new(),
            players: vec![],
            logo_png: None,
        })
        .collect();
    let file = std::env::temp_dir().join(format!("bracketlab-{}.sqlite3", Uuid::new_v4()));
    let mut storage = Storage::open(&file).unwrap();
    storage.commit(&project, "Команды").unwrap();
    project.stages.push(
        Stage::new(
            "Кубок".into(),
            Format::Single,
            project.teams.iter().map(|t| t.id).collect(),
            3,
            Rules::default(),
        )
        .unwrap(),
    );
    storage.commit(&project, "Этап").unwrap();
    drop(storage);
    let mut storage = Storage::open(&file).unwrap();
    assert_eq!(storage.current().unwrap().unwrap().stages.len(), 1);
    assert_eq!(storage.step(false).unwrap().unwrap().stages.len(), 0);
    assert_eq!(storage.step(true).unwrap().unwrap().stages.len(), 1);
    storage.step(false).unwrap();
    storage.commit(&project, "Новая ветка").unwrap();
    assert!(storage.step(true).unwrap().is_none());
    assert!(storage
        .audit()
        .unwrap()
        .iter()
        .any(|(_, action)| action == "Этап"));
    let payload = serde_json::to_string(&project).unwrap();
    assert_eq!(decode(&payload).unwrap().stages[0].entrants.len(), 5);
    drop(storage);
    std::fs::remove_file(file).unwrap();
}

#[test]
fn reject_corrupt_edges_and_invalid_map_series() {
    let mut stage = Stage::new(
        "Плей-офф".into(),
        Format::Single,
        teams(4),
        3,
        Rules::default(),
    )
    .unwrap();
    let mut score = win(0);
    score.maps = vec![MapScore {
        name: "Mirage".into(),
        rounds: [13, 10],
    }];
    assert!(stage.set_score(0, Some(score.clone())).is_err());
    score.maps.push(MapScore {
        name: "Nuke".into(),
        rounds: [13, 8],
    });
    assert!(stage.set_score(0, Some(score)).is_ok());
    stage.matches[0].sources[1] = Source::Winner(2);
    assert!(stage.validate_graph().is_err());
}
