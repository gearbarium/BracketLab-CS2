use bracketlab::{storage::Storage, tournament::*};
use rusqlite::Connection;
use uuid::Uuid;

fn project(name: &str, format: Format, rules: Rules) -> Project {
    let mut project = Project::new(name.into());
    for number in 1..=8 {
        project.teams.push(Team {
            id: Uuid::from_u128(number),
            name: format!("Команда {number}"),
            description: String::new(),
            players: vec![],
            logo_png: None,
        });
    }
    project.stages.push(
        Stage::new(
            "Финал".into(),
            format,
            project.teams.iter().map(|t| t.id).collect(),
            3,
            rules,
        )
        .unwrap(),
    );
    project
}

fn finish(stage: &mut Stage) {
    for index in 0..stage.matches.len() {
        let status = stage.resolve()[index];
        if status.active && matches!(status.slots,[Slot::Team(a),Slot::Team(b)] if a!=b) {
            stage
                .set_score(
                    index,
                    Some(Score {
                        series: [2, 0],
                        technical: false,
                        maps: vec![],
                        note: String::new(),
                    }),
                )
                .unwrap();
        }
    }
}

#[test]
fn migrates_legacy_db_with_backup_and_keeps_projects_separate() {
    let root = std::env::temp_dir().join(format!("bracketlab-update-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("tour.sqlite3");
    let legacy = Connection::open(&path).unwrap();
    legacy
        .execute_batch(include_str!("../migrations/001.sql"))
        .unwrap();
    legacy
        .execute_batch(include_str!("../migrations/002.sql"))
        .unwrap();
    let mut saved = project("Старый кубок", Format::Single, Rules::default());
    saved.teams[0].players.push(Player {
        id: Uuid::new_v4(),
        name: "Игрок".into(),
        steam_id: "123".into(),
        note: String::new(),
    });
    finish(&mut saved.stages[0]);
    let mut payload = serde_json::to_value(&saved).unwrap();
    payload["teams"][0]["players"][0]
        .as_object_mut()
        .unwrap()
        .remove("id");
    legacy
        .execute(
            "INSERT INTO revision (id,action,payload) VALUES (1,'Первый турнир',?1)",
            [payload.to_string()],
        )
        .unwrap();
    legacy
        .execute("INSERT INTO current_revision VALUES (1,1)", [])
        .unwrap();
    drop(legacy);

    let mut db = Storage::open(&path).unwrap();
    let old = db.active_id().unwrap().to_owned();
    assert_eq!(
        db.current().unwrap().unwrap().teams[0].players[0].steam_id,
        "123"
    );
    let backup = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .find(|f| f.file_name().to_string_lossy().ends_with(".backup.sqlite3"))
        .unwrap()
        .path();
    let backup_db = Connection::open(&backup).unwrap();
    assert_eq!(
        backup_db
            .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        2
    );
    drop(backup_db);
    db.create(&Project::new("Второй".into()), "Новый турнир")
        .unwrap();
    let second = db.active_id().unwrap().to_owned();
    assert_ne!(old, second);
    db.rename(&old, "Архив".into()).unwrap();
    assert_eq!(db.current().unwrap().unwrap().name, "Второй");
    assert!(db.step(false).unwrap().is_none());
    let old_project = db.select(&old).unwrap();
    assert_eq!(old_project.name, "Архив");
    assert!(old_project.stages[0]
        .matches
        .iter()
        .all(|m| m.score.is_some()));
    assert_eq!(db.step(false).unwrap().unwrap().name, "Старый кубок");
    db.select(&second).unwrap();
    drop(db);
    let mut db = Storage::open(&path).unwrap();
    assert_eq!(db.current().unwrap().unwrap().name, "Второй");
    assert_eq!(db.catalog().unwrap().len(), 2);
    assert_eq!(db.delete(&second).unwrap().unwrap().name, "Старый кубок");
    assert_eq!(db.catalog().unwrap().len(), 1);
    drop(db);
    std::fs::remove_file(backup).unwrap();
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn podium_uses_played_placement_matches_and_respects_swiss_completion() {
    for (format, third) in [
        (Format::Single, false),
        (Format::Single, true),
        (Format::Double, false),
        (Format::RoundRobin, false),
        (Format::Swiss, false),
    ] {
        let mut project = project(
            "Кубок",
            format,
            Rules {
                third_place: third,
                reset: true,
                second_leg: false,
            },
        );
        assert_eq!(project.podium(), [None; 3]);
        finish(&mut project.stages[0]);
        if format == Format::Swiss {
            assert_eq!(project.podium(), [None; 3]);
            project.stages[0].swiss_finished = true;
        }
        let places = project.podium();
        assert!(places[0].is_some());
        assert!(places[1].is_some());
        assert_eq!(places[2].is_some(), format != Format::Single || third);
    }
}

#[test]
fn stage_deletion_rejects_dependents_and_shifts_remaining_links() {
    let mut project = project("Кубок", Format::Single, Rules::default());
    let mut source = project.stages[0].clone();
    finish(&mut source);
    let entrants = source.qualifiers().unwrap().into_iter().take(2).collect();
    project.stages.push(source);
    let mut final_stage = Stage::new(
        "Финал".into(),
        Format::Single,
        entrants,
        3,
        Rules::default(),
    )
    .unwrap();
    final_stage.qualification.push(Qualification {
        stage: 1,
        places: 2,
    });
    project.stages.push(final_stage);
    assert!(project.remove_stage(1).is_err());
    project.remove_stage(0).unwrap();
    assert_eq!(project.stages[1].qualification[0].stage, 0);
    project.validate().unwrap();
}

#[test]
fn partial_map_scores_do_not_advance_a_winner() {
    let mut project = project("Кубок", Format::Single, Rules::default());
    let stage = &mut project.stages[0];
    let score = Score {
        series: [1, 0],
        technical: false,
        maps: vec![MapScore {
            name: "Mirage".into(),
            rounds: [13, 8],
        }],
        note: String::new(),
    };
    stage.set_score(0, Some(score)).unwrap();
    assert!(stage.resolve()[0].outcome.is_none());
    assert!(stage.resolve()[4].slots.contains(&Slot::Pending));
    project.validate().unwrap();
}

#[test]
fn player_removal_uses_stable_identity() {
    let mut project = project("Кубок", Format::Single, Rules::default());
    let team = &mut project.teams[0];
    for i in 1..=5 {
        team.players.push(Player {
            id: Uuid::from_u128(i),
            name: "Одинаковое имя".into(),
            steam_id: String::new(),
            note: i.to_string(),
        });
    }
    for i in [1, 3, 5] {
        assert!(team.remove_player(Uuid::from_u128(i)));
    }
    assert_eq!(
        team.players
            .iter()
            .map(|player| player.note.as_str())
            .collect::<Vec<_>>(),
        ["2", "4"]
    );
    let round_trip =
        bracketlab::storage::decode(&serde_json::to_string(&project).unwrap()).unwrap();
    assert_eq!(round_trip.teams[0].players, project.teams[0].players);
}
