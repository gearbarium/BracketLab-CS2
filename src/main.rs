#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod fixtures;
mod scene;

use base64::{engine::general_purpose::STANDARD, Engine};
use bracketlab::{
    storage::{self, Storage},
    tournament::*,
};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, FontId, Frame, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};
use egui_phosphor::regular as icons;
use scene::{curve_points, origin, short, Scene};
use std::{path::PathBuf, sync::mpsc};
use uuid::Uuid;

const BG: Color32 = Color32::from_rgb(5, 11, 18);
const PANEL: Color32 = Color32::from_rgb(8, 17, 27);
const CARD: Color32 = Color32::from_rgb(17, 33, 46);
const HOVER: Color32 = Color32::from_rgb(23, 44, 58);
const ACCENT: Color32 = Color32::from_rgb(255, 176, 46);
const TEXT: Color32 = Color32::from_rgb(241, 247, 251);
const MUTED: Color32 = Color32::from_rgb(156, 174, 187);
const LINE: Color32 = Color32::from_rgb(36, 55, 68);

fn main() -> eframe::Result {
    tracing_subscriber::fmt::init();
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--fixtures") {
        let output = args
            .get(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("dist/scenes"));
        if let Err(error) = fixtures::write(&output) {
            tracing::error!(%error, "Не удалось создать тестовые сцены");
            std::process::exit(1);
        }
        return Ok(());
    }
    if args.first().is_some_and(|arg| arg == "--check-project") {
        let result = args
            .get(1)
            .ok_or("Укажите путь к проекту".to_owned())
            .and_then(|path| std::fs::read_to_string(path).map_err(|e| e.to_string()))
            .and_then(|payload| storage::decode(&payload));
        if let Err(error) = result {
            tracing::error!(%error);
            std::process::exit(1);
        }
        return Ok(());
    }
    let portable = std::env::current_exe().ok().and_then(|exe| {
        let parent = exe.parent()?;
        parent
            .join("BracketLab.portable")
            .is_file()
            .then(|| parent.join("data"))
    });
    let dir = if args.first().is_some_and(|arg| arg == "--data-dir") {
        args.get(1).map(PathBuf::from)
    } else {
        portable
    }
    .unwrap_or_else(|| {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("BracketLab")
    });
    let capture = args.first().is_some_and(|arg| arg == "--capture").then(|| {
        args.get(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("screen.png"))
    });
    let store = if capture.is_some() || args.first().is_some_and(|arg| arg == "--preview") {
        Storage::open(std::path::Path::new(":memory:"))
            .map_err(|error| error.to_string())
            .and_then(|mut db| {
                let project = if capture.is_some() {
                    args.get(2)
                        .map(|path| {
                            std::fs::read_to_string(path)
                                .map_err(|e| e.to_string())
                                .and_then(|payload| storage::decode(&payload))
                        })
                        .transpose()?
                } else {
                    None
                };
                db.commit(&project.unwrap_or_else(fixtures::preview), "Тестовая сцена")?;
                Ok(db)
            })
    } else {
        std::fs::create_dir_all(&dir)
            .map_err(|error| error.to_string())
            .and_then(|_| {
                Storage::open(&dir.join("bracketlab.sqlite3")).map_err(|error| error.to_string())
            })
    };
    if capture.is_some() && store.is_err() {
        tracing::error!("Не удалось открыть сцену для снимка");
        std::process::exit(1);
    }
    let capture_stage = args
        .get(3)
        .and_then(|index| index.to_str())
        .and_then(|index| index.parse::<usize>().ok())
        .unwrap_or(0);
    let capture_zoom = args
        .get(4)
        .and_then(|value| value.to_str())
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| (0.1..=2.5).contains(value));
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_icon(window_icon())
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "BracketLab CS2",
        options,
        Box::new(move |cc| {
            let mut app = App::new(&cc.egui_ctx, store);
            app.capture = capture;
            app.capture_zoom = capture_zoom;
            if app.capture.is_some() {
                app.stage = capture_stage;
                app.refresh(true);
            }
            Ok(Box::new(app))
        }),
    )
}

fn theme(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Inter".into(),
        egui::FontData::from_static(include_bytes!("../assets/Inter-Regular.otf")).into(),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter".into());
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    fonts
        .families
        .insert(icon_family(), vec!["phosphor".into()]);
    ctx.set_fonts(fonts);
    egui_extras::install_image_loaders(ctx);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = BG;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.widgets.inactive.bg_fill = CARD;
    style.visuals.widgets.inactive.weak_bg_fill = CARD;
    style.visuals.widgets.hovered.bg_fill = HOVER;
    style.visuals.widgets.hovered.weak_bg_fill = HOVER;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    style.visuals.widgets.inactive.corner_radius = 7.into();
    style.visuals.widgets.hovered.corner_radius = 7.into();
    style.visuals.widgets.active.corner_radius = 7.into();
    style.visuals.widgets.open.corner_radius = 7.into();
    style.visuals.selection.bg_fill = HOVER;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    style.spacing.item_spacing = vec2(8.0, 8.0);
    style.spacing.button_padding = vec2(12.0, 8.0);
    style.spacing.interact_size.y = 34.0;
    for (kind, font) in &mut style.text_styles {
        font.size = match kind {
            egui::TextStyle::Heading => 21.0,
            egui::TextStyle::Small => 12.0,
            _ => 15.0,
        };
    }
    ctx.set_style(style);
}

pub fn status(game: &Match, state: Resolved) -> &'static str {
    match (state.active, state.outcome, game.score.as_ref()) {
        (false, _, _) => "УСЛОВНЫЙ",
        (_, _, Some(score)) if score.technical => "TECH",
        (_, None, Some(_)) => "ИДЁТ",
        (_, _, Some(_)) => "ЗАВЕРШЁН",
        (_, Some(_), _) => "BYE",
        _ if matches!(state.slots, [Slot::Team(_), Slot::Team(_)]) => "ГОТОВ",
        _ => "ОЖИДАНИЕ",
    }
}

fn label(ui: &mut egui::Ui, value: &str) {
    ui.label(RichText::new(value).size(11.0).strong().color(MUTED));
}

fn primary(ui: &mut egui::Ui, value: &str) -> bool {
    ui.add(
        egui::Button::new(RichText::new(value).strong().color(BG))
            .fill(ACCENT)
            .corner_radius(9)
            .min_size(vec2(0.0, 42.0)),
    )
    .clicked()
}

struct StageDraft {
    name: String,
    entrants: Vec<Uuid>,
    format: Format,
    rules: Rules,
    bo: u8,
    seed: Option<String>,
    qualify_count: usize,
    qualification: Vec<Qualification>,
    sources: Vec<usize>,
    locked: std::collections::HashSet<Uuid>,
    pot_size: usize,
    groups: usize,
    tie_break: TieBreak,
}

fn tool(ui: &mut egui::Ui, icon: &str, title: &str) -> bool {
    ui.add(
        egui::Button::new(RichText::new(icon).font(FontId::new(17.0, icon_family())))
            .min_size(vec2(32.0, 32.0)),
    )
    .on_hover_text(title)
    .clicked()
}

fn icon_family() -> egui::FontFamily {
    egui::FontFamily::Name("Phosphor".into())
}

fn icon_text(icon: &str, title: &str) -> egui::text::LayoutJob {
    let mut layout = egui::text::LayoutJob::default();
    layout.append(
        icon,
        0.0,
        egui::TextFormat {
            font_id: FontId::new(16.0, icon_family()),
            color: TEXT,
            ..Default::default()
        },
    );
    layout.append(
        title,
        6.0,
        egui::TextFormat {
            font_id: FontId::proportional(13.0),
            color: TEXT,
            ..Default::default()
        },
    );
    layout
}

fn window_icon() -> egui::IconData {
    let rgba = image::RgbaImage::from_fn(64, 64, |x, y| {
        let amber = ((x.abs_diff(16) <= 4 && (y.abs_diff(17) <= 4 || y.abs_diff(47) <= 4))
            || (x.abs_diff(48) <= 4 && y.abs_diff(32) <= 4))
            || (x.abs_diff(30) <= 1 && (17..=47).contains(&y))
            || ((16..=30).contains(&x) && (y.abs_diff(17) <= 1 || y.abs_diff(47) <= 1))
            || ((30..=48).contains(&x) && y.abs_diff(32) <= 1);
        image::Rgba(if amber {
            [255, 176, 46, 255]
        } else {
            [8, 17, 27, 255]
        })
    })
    .into_raw();
    egui::IconData {
        rgba,
        width: 64,
        height: 64,
    }
}

struct App {
    store: Option<Storage>,
    storage_job: Option<mpsc::Receiver<(Storage, StorageResult)>>,
    context: egui::Context,
    close_after_save: bool,
    project: Project,
    stage: usize,
    selected: Option<usize>,
    round_filter: Option<usize>,
    score: Option<Score>,
    scene: Option<Scene>,
    states: Vec<Resolved>,
    camera: Vec2,
    zoom: f32,
    camera_transition: Option<(Vec2, Vec2, f32, f32, f64)>,
    centered: bool,
    fit: bool,
    reduced_motion: bool,
    draft: Option<StageDraft>,
    teams_open: bool,
    team_draft: Option<Project>,
    export_open: bool,
    export_options: scene::ExportOptions,
    export_scope: u8,
    standings_open: bool,
    audit: Option<Vec<(String, String)>>,
    job: Option<mpsc::Receiver<Result<(), String>>>,
    logos: std::collections::HashMap<Uuid, (String, egui::TextureHandle)>,
    input: String,
    notice: String,
    error: bool,
    capture: Option<PathBuf>,
    capture_sent: bool,
    capture_zoom: Option<f32>,
    advance: Option<(usize, usize, f64)>,
    manager: Option<Vec<storage::TournamentEntry>>,
    delete_tournament: Option<(String, String)>,
    rename_tournament: Option<(String, String)>,
    delete_stage: Option<usize>,
    podium_open: bool,
}

enum StorageResult {
    Rename(Result<(Vec<storage::TournamentEntry>, Option<Project>), String>),
    Catalog(Result<Vec<storage::TournamentEntry>, String>),
    Switch(Result<Option<Project>, String>),
    Commit {
        result: Result<(), String>,
        previous: Project,
        previous_stage: usize,
        action: String,
    },
    Step(Result<Option<Project>, String>, bool),
    Audit(Result<Vec<(String, String)>, String>),
}

impl App {
    fn new(ctx: &egui::Context, store: Result<Storage, String>) -> Self {
        theme(ctx);
        let mut notice = String::new();
        let mut project = Project::new("Новый турнир".into());
        let store = match store {
            Ok(store) => match store.current() {
                Ok(current) => {
                    if let Some(saved) = current {
                        project = saved;
                    }
                    Some(store)
                }
                Err(error) => {
                    notice = error;
                    None
                }
            },
            Err(error) => {
                notice = error;
                None
            }
        };
        Self {
            store,
            storage_job: None,
            context: ctx.clone(),
            close_after_save: false,
            project,
            stage: 0,
            selected: None,
            round_filter: None,
            score: None,
            scene: None,
            states: vec![],
            camera: Vec2::ZERO,
            zoom: 1.0,
            camera_transition: None,
            centered: false,
            fit: true,
            reduced_motion: false,
            draft: None,
            teams_open: false,
            team_draft: None,
            export_open: false,
            export_options: Default::default(),
            export_scope: 0,
            standings_open: false,
            audit: None,
            job: None,
            logos: Default::default(),
            input: String::new(),
            error: !notice.is_empty(),
            notice,
            capture: None,
            capture_sent: false,
            capture_zoom: None,
            advance: None,
            manager: None,
            delete_tournament: None,
            rename_tournament: None,
            delete_stage: None,
            podium_open: true,
        }
        .with_scene()
    }

    fn with_scene(mut self) -> Self {
        self.refresh(true);
        self
    }

    fn active(&self) -> Option<&Stage> {
        self.project.stages.get(self.stage)
    }

    fn notify(&mut self, message: impl Into<String>, error: bool) {
        self.notice = message.into();
        self.error = error;
    }

    fn refresh(&mut self, fit: bool) {
        self.stage = self.stage.min(self.project.stages.len().saturating_sub(1));
        self.states = self.active().map(Stage::resolve).unwrap_or_default();
        self.scene = self.active().map(Scene::new);
        if fit {
            self.fit = true;
            self.centered = false;
            self.selected = None;
            self.round_filter = None;
            self.score = None;
        }
    }

    fn commit(&mut self, project: Project, action: &str, fit: bool) -> bool {
        if let Err(error) = project.validate() {
            self.notify(error, true);
            return false;
        }
        let snapshot = project.clone();
        let previous = self.project.clone();
        let previous_stage = self.stage;
        let action = action.to_owned();
        if !self.store_work(move |db| StorageResult::Commit {
            result: db.commit(&snapshot, &action),
            previous,
            previous_stage,
            action,
        }) {
            return false;
        }
        self.project = project;
        self.refresh(fit);
        self.notify("Сохранение…", false);
        true
    }

    fn step(&mut self, redo: bool) {
        self.store_work(move |db| StorageResult::Step(db.step(redo), redo));
    }

    fn store_work(
        &mut self,
        work: impl FnOnce(&mut Storage) -> StorageResult + Send + 'static,
    ) -> bool {
        let Some(mut store) = self.store.take() else {
            self.notify(
                if self.storage_job.is_some() {
                    "Сохранение ещё выполняется"
                } else {
                    "Хранилище недоступно"
                },
                true,
            );
            return false;
        };
        let context = self.context.clone();
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let result = work(&mut store);
            let _ = send.send((store, result));
            context.request_repaint();
        });
        self.storage_job = Some(receive);
        true
    }

    fn poll_storage(&mut self, ctx: &egui::Context) {
        if self.storage_job.is_some() && ctx.input(|input| input.viewport().close_requested()) {
            self.close_after_save = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        let Some(job) = &self.storage_job else {
            return;
        };
        match job.try_recv() {
            Ok((store, result)) => {
                self.storage_job = None;
                self.store = Some(store);
                match result {
                    StorageResult::Commit {
                        result: Ok(()),
                        action,
                        ..
                    } => self.notify(action, false),
                    StorageResult::Commit {
                        result: Err(error),
                        previous,
                        previous_stage,
                        ..
                    } => {
                        self.project = previous;
                        self.stage = previous_stage;
                        self.refresh(true);
                        self.notify(error, true);
                        self.close_after_save = false;
                    }
                    StorageResult::Step(Ok(Some(project)), redo) => {
                        self.project = project;
                        self.refresh(true);
                        self.notify(
                            if redo {
                                "Действие восстановлено"
                            } else {
                                "Действие отменено"
                            },
                            false,
                        );
                    }
                    StorageResult::Step(Ok(None), _) => self.notify("Нет действий", false),
                    StorageResult::Step(Err(error), _) | StorageResult::Audit(Err(error)) => {
                        self.notify(error, true)
                    }
                    StorageResult::Audit(Ok(events)) => self.audit = Some(events),
                    StorageResult::Catalog(Ok(entries)) => self.manager = Some(entries),
                    StorageResult::Rename(Ok((entries, project))) => {
                        self.manager = Some(entries);
                        if let Some(project) = project {
                            if let Some(draft) = &mut self.team_draft {
                                draft.name = project.name.clone();
                            }
                            self.project = project;
                        }
                        self.notify("Турнир переименован", false);
                    }
                    StorageResult::Rename(Err(error)) => self.notify(error, true),
                    StorageResult::Switch(Ok(project)) => {
                        self.project =
                            project.unwrap_or_else(|| Project::new("Новый турнир".into()));
                        self.stage = 0;
                        self.draft = None;
                        self.team_draft = None;
                        self.teams_open = false;
                        self.export_open = false;
                        self.standings_open = false;
                        self.audit = None;
                        self.advance = None;
                        self.input.clear();
                        self.refresh(true);
                        self.notify("Турнир открыт", false);
                    }
                    StorageResult::Catalog(Err(error)) | StorageResult::Switch(Err(error)) => {
                        self.notify(error, true)
                    }
                }
                if self.close_after_save {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.storage_job = None;
                self.close_after_save = false;
                self.notify("Рабочий поток хранилища завершился", true);
            }
        }
    }

    fn start_draft(&mut self) {
        if self.project.teams.len() < 2 {
            self.teams_open = true;
            return;
        }
        self.draft = Some(StageDraft {
            name: format!("Этап {}", self.project.stages.len() + 1),
            entrants: self.project.teams.iter().map(|t| t.id).collect(),
            format: Format::Single,
            rules: Rules::default(),
            bo: 3,
            seed: None,
            qualify_count: 2,
            qualification: vec![],
            sources: self.active().map(|_| vec![self.stage]).unwrap_or_default(),
            locked: Default::default(),
            pot_size: 0,
            groups: 1,
            tie_break: TieBreak::Seed,
        });
    }

    fn project_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(icon_text(icons::GEAR_SIX, "Проект"), |ui| {
            if ui.button("Новый турнир").clicked() {
                ui.close_menu();
                self.new_tournament(Project::new("Новый турнир".into()), "Создан турнир");
            }
            if ui.button("Менеджер турниров").clicked() {
                ui.close_menu();
                self.store_work(|db| StorageResult::Catalog(db.catalog()));
            }
            if ui.button("Импорт JSON").clicked() {
                ui.close_menu();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("BracketLab", &["json"])
                    .pick_file()
                {
                    match std::fs::read_to_string(path)
                        .map_err(|e| e.to_string())
                        .and_then(|data| storage::decode(&data))
                    {
                        Ok(project) => {
                            self.new_tournament(project, "Проект импортирован");
                        }
                        Err(error) => self.notify(error, true),
                    }
                }
            }
            if ui.button("Экспорт JSON").clicked() {
                ui.close_menu();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("BracketLab", &["json"])
                    .set_file_name("tournament.json")
                    .save_file()
                {
                    let result = serde_json::to_vec_pretty(&self.project)
                        .map_err(|e| e.to_string())
                        .and_then(|data| std::fs::write(path, data).map_err(|e| e.to_string()));
                    match result {
                        Ok(()) => self.notify("Проект экспортирован", false),
                        Err(error) => self.notify(error, true),
                    }
                }
            }
            ui.checkbox(&mut self.reduced_motion, "Уменьшить движение");
            if ui.button("История действий").clicked() {
                self.store_work(|db| {
                    StorageResult::Audit(db.audit().map_err(|error| error.to_string()))
                });
                ui.close_menu();
            }
        });
    }

    fn new_tournament(&mut self, project: Project, action: &'static str) {
        if self.store_work(move |db| {
            StorageResult::Switch(db.create(&project, action).map(|()| Some(project)))
        }) {
            self.manager = None;
        }
    }

    fn management_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(entries) = self.manager.take() {
            let mut open = true;
            let mut create = false;
            let mut select = None;
            let mut duplicate = None;
            egui::Window::new("Менеджер турниров")
                .open(&mut open)
                .default_width(520.0)
                .show(ctx, |ui| {
                    create = primary(ui, "Создать турнир");
                    egui::ScrollArea::vertical()
                        .max_height(ctx.screen_rect().height() * 0.65)
                        .show(ui, |ui| {
                            for entry in &entries {
                                ui.push_id(&entry.id, |ui| {
                                    ui.separator();
                                    ui.heading(&entry.project.name);
                                    let finished = entry.project.podium()[0].is_some()
                                        && entry.project.stages.iter().all(|stage| {
                                            stage.qualifiers().is_ok()
                                                && (stage.format != Format::Swiss
                                                    || stage.swiss_finished)
                                        });
                                    let status = if finished {
                                        "Завершён"
                                    } else if entry
                                        .project
                                        .stages
                                        .iter()
                                        .flat_map(|s| &s.matches)
                                        .any(|game| game.score.is_some())
                                    {
                                        "Идёт"
                                    } else {
                                        "Подготовка"
                                    };
                                    ui.label(format!(
                                        "Команд: {} · Этапов: {} · {}",
                                        entry.project.teams.len(),
                                        entry.project.stages.len(),
                                        status
                                    ));
                                    ui.label(format!(
                                        "{} · {} UTC",
                                        entry
                                            .project
                                            .stages
                                            .last()
                                            .map_or("Без этапов", |stage| stage
                                                .format
                                                .label()),
                                        entry.updated_at
                                    ));
                                    ui.horizontal(|ui| {
                                        if ui.button("Открыть").clicked() {
                                            select = Some(entry.id.clone());
                                        }
                                        if ui.button("Дублировать").clicked() {
                                            duplicate = Some(entry.project.clone());
                                        }
                                        if ui.button("Переименовать").clicked() {
                                            self.rename_tournament = Some((
                                                entry.id.clone(),
                                                entry.project.name.clone(),
                                            ));
                                        }
                                        if ui.button("Удалить").clicked() {
                                            self.delete_tournament = Some((
                                                entry.id.clone(),
                                                entry.project.name.clone(),
                                            ));
                                        }
                                    });
                                });
                            }
                        });
                });
            if open {
                self.manager = Some(entries);
            }
            if create {
                self.new_tournament(Project::new("Новый турнир".into()), "Создан турнир");
            }
            if let Some(mut project) = duplicate {
                project.name.push_str(" — копия");
                self.new_tournament(project, "Турнир продублирован");
            }
            if let Some(id) = select {
                if self.store_work(move |db| StorageResult::Switch(db.select(&id).map(Some))) {
                    self.manager = None;
                }
            }
        }
        if let Some((id, mut name)) = self.rename_tournament.take() {
            let mut save = false;
            let response = egui::Modal::new("rename_tournament".into()).show(ctx, |ui| {
                ui.heading("Переименовать турнир");
                ui.text_edit_singleline(&mut name);
                save = ui
                    .add_enabled(!name.trim().is_empty(), egui::Button::new("Сохранить"))
                    .clicked();
                ui.button("Отмена").clicked()
            });
            if save {
                let value = name.trim().to_owned();
                let target = id.clone();
                if !self.store_work(move |db| {
                    StorageResult::Rename(
                        db.rename(&target, value)
                            .and_then(|()| Ok((db.catalog()?, db.current()?))),
                    )
                }) {
                    self.rename_tournament = Some((id, name));
                }
            } else if !response.inner && !response.should_close() {
                self.rename_tournament = Some((id, name));
            }
        }
        if let Some((id, name)) = self.delete_tournament.clone() {
            let mut confirm = false;
            let modal = egui::Modal::new("confirm_tournament_delete".into()).show(ctx, |ui| {
                ui.heading(format!("Удалить турнир «{name}»?"));
                ui.label("Будут удалены команды, этапы, результаты и история турнира.");
                confirm = ui.button("Удалить турнир").clicked();
                ui.button("Отмена").clicked()
            });
            if modal.inner || modal.should_close() {
                self.delete_tournament = None;
            }
            if confirm && self.store_work(move |db| StorageResult::Switch(db.delete(&id))) {
                self.delete_tournament = None;
                self.manager = None;
            }
        }
        if let Some(index) = self.delete_stage {
            let mut project = self.project.clone();
            let Some(stage) = self.project.stages.get(index) else {
                self.delete_stage = None;
                return;
            };
            let error = project.remove_stage(index).err();
            let mut confirm = false;
            let modal = egui::Modal::new("confirm_stage_delete".into()).show(ctx, |ui| {
                ui.heading(format!("Удалить этап «{}»?", stage.name));
                ui.label(format!(
                    "Матчей: {} · Результатов: {}",
                    stage.matches.len(),
                    stage.matches.iter().filter(|m| m.score.is_some()).count()
                ));
                if let Some(error) = &error {
                    ui.colored_label(Color32::LIGHT_RED, error);
                }
                confirm = ui
                    .add_enabled(error.is_none(), egui::Button::new("Удалить этап"))
                    .clicked();
                ui.button("Отмена").clicked()
            });
            if modal.inner || modal.should_close() {
                self.delete_stage = None;
            }
            let next = self.stage - usize::from(self.stage > index);
            if confirm && self.commit(project, "Этап удалён", true) {
                self.stage = next;
                self.refresh(true);
                self.delete_stage = None;
            }
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, 10.0, Color32::from_rgb(49, 37, 24));
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                icons::GRAPH,
                FontId::new(23.0, icon_family()),
                ACCENT,
            );
            ui.vertical(|ui| {
                ui.heading(RichText::new("BracketLab").size(20.0).strong());
                ui.label(
                    RichText::new("CS2 · by gearbarium")
                        .size(10.0)
                        .color(ACCENT)
                        .strong(),
                );
            });
        });
        ui.add_space(20.0);
        label(ui, "ТЕКУЩИЙ ТУРНИР");
        ui.label(RichText::new(&self.project.name).size(19.0).strong());
        ui.add_space(14.0);
        if ui
            .button(icon_text(
                icons::USERS_THREE,
                &format!("Команды  ·  {}", self.project.teams.len()),
            ))
            .clicked()
        {
            self.teams_open = true;
        }
        self.project_menu(ui);
        ui.add_space(22.0);
        label(ui, "ЭТАПЫ");
        let mut next = None;
        for (index, stage) in self.project.stages.iter().enumerate() {
            let title = format!(
                "{:02}   {}\n{}",
                index + 1,
                short(&stage.name, 20),
                stage.format.label()
            );
            let response = ui.add(
                egui::Button::new(title)
                    .selected(self.stage == index)
                    .min_size(vec2(180.0, 44.0)),
            );
            if response.clicked() {
                next = Some(index);
            }
            response.context_menu(|ui| {
                if ui.button("Удалить этап").clicked() {
                    self.delete_stage = Some(index);
                    ui.close_menu();
                }
            });
        }
        if let Some(index) = next {
            self.stage = index;
            self.refresh(true);
        }
        ui.add_space(8.0);
        if primary(ui, "+  Новый этап") {
            self.start_draft();
        }
        ui.with_layout(egui::Layout::bottom_up(Align::Min), |ui| {
            ui.label(
                RichText::new(if self.store.is_some() || self.storage_job.is_some() {
                    "●  Локальное хранилище"
                } else {
                    "●  Хранилище недоступно"
                })
                .color(MUTED)
                .size(12.0),
            );
        });
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let area = ui.available_rect_before_wrap();
        let response = ui.interact(area, egui::Id::new("bracket_camera"), Sense::drag());
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 0.0, BG);
        let Some(scene) = &self.scene else {
            painter.text(
                area.center() - vec2(0.0, 20.0),
                Align2::CENTER_CENTER,
                "Соберите турнир",
                FontId::proportional(30.0),
                TEXT,
            );
            painter.text(
                area.center() + vec2(0.0, 20.0),
                Align2::CENTER_CENTER,
                "Добавьте команды и создайте первый этап",
                FontId::proportional(15.0),
                MUTED,
            );
            return;
        };
        let stage = self.active().unwrap();
        if self.fit {
            let bounds = if let Some(round) = self.round_filter {
                scene
                    .cards
                    .iter()
                    .filter(|card| stage.matches[card.index].round == round)
                    .fold(Rect::NOTHING, |bounds, card| bounds.union(card.rect))
            } else {
                scene.bounds
            };
            let target_zoom = self.capture_zoom.unwrap_or_else(|| {
                ((area.width() - 72.0) / bounds.width())
                    .min((area.height() - 72.0) / bounds.height())
                    .clamp(0.12, 1.0)
            });
            let mut target = (area.size() - bounds.size() * target_zoom) * 0.5
                - bounds.min.to_vec2() * target_zoom;
            if self.capture_zoom.is_some() {
                target = target.max(vec2(26.0, 22.0) - bounds.min.to_vec2() * target_zoom);
            }
            if self.centered && !self.reduced_motion {
                self.camera_transition = Some((
                    self.camera,
                    target,
                    self.zoom,
                    target_zoom,
                    ui.input(|i| i.time),
                ));
            } else {
                self.camera = target;
                self.zoom = target_zoom;
                self.camera_transition = None;
            }
            self.centered = true;
            self.fit = false;
        }
        if let Some((from, target, start_zoom, end_zoom, start)) = self.camera_transition {
            let fraction = ((ui.input(|i| i.time) - start) / 0.35).clamp(0.0, 1.0) as f32;
            let eased = 1.0 - (1.0 - fraction).powi(3);
            self.camera = from + (target - from) * eased;
            self.zoom = start_zoom + (end_zoom - start_zoom) * eased;
            if fraction < 1.0 && !self.reduced_motion {
                ui.ctx().request_repaint();
            } else {
                self.camera_transition = None;
            }
        }
        if response.dragged() {
            self.camera_transition = None;
            self.camera += response.drag_delta();
        }
        if let Some(pointer) = ui
            .input(|input| input.pointer.hover_pos())
            .filter(|_| ui.rect_contains_pointer(area) && !response.dragged())
        {
            let scroll = ui.input_mut(|i| std::mem::take(&mut i.smooth_scroll_delta.y));
            let pinch = ui.input(|i| i.zoom_delta());
            if scroll != 0.0 || pinch != 1.0 {
                self.camera_transition = None;
            }
            let previous = self.zoom;
            self.zoom = (self.zoom * (scroll * 0.0025).exp() * pinch).clamp(0.10, 2.5);
            let anchor = pointer - area.min;
            self.camera = anchor - (anchor - self.camera) * (self.zoom / previous);
        }
        let grid = (28.0 * self.zoom).max(14.0);
        let mut x = area.left() + self.camera.x.rem_euclid(grid);
        while x <= area.right() {
            let mut y = area.top() + self.camera.y.rem_euclid(grid);
            while y <= area.bottom() {
                painter.circle_filled(pos2(x, y), 0.8, LINE);
                y += grid;
            }
            x += grid;
        }
        let transform = |p: Pos2| area.min + self.camera + p.to_vec2() * self.zoom;
        let invert = |p: Pos2| Pos2::ZERO + (p - area.min - self.camera) / self.zoom;
        let view = Rect::from_min_max(invert(area.min), invert(area.max));
        for (point, heading) in &scene.headings {
            if self.round_filter.is_some() {
                break;
            }
            let point = transform(*point);
            if area.expand(80.0).contains(point) {
                painter.text(
                    point,
                    Align2::LEFT_BOTTOM,
                    heading,
                    FontId::proportional((13.0 * self.zoom).max(8.0)),
                    MUTED,
                );
            }
        }
        for (a, b, slot, loser, curve) in &scene.edges {
            if !view.intersects(scene.cards[*a].rect.union(scene.cards[*b].rect)) {
                continue;
            }
            let selected = self.selected.is_some_and(|id| id == *a || id == *b);
            let resolved = self.states[*a].outcome.is_some()
                && matches!(self.states[*b].slots[*slot], Slot::Team(_));
            let progress = self
                .advance
                .filter(|(stage, id, _)| *stage == self.stage && *id == *a && !self.reduced_motion)
                .map(|(_, _, start)| {
                    ((ui.input(|i| i.time) - start) / 0.52).clamp(0.0, 1.0) as f32
                });
            if progress.is_some_and(|p| p < 1.0) && resolved {
                let path: Vec<_> = curve_points(curve).map(transform).collect();
                let index = ((path.len() - 1) as f32 * progress.unwrap()).round() as usize;
                painter.circle_filled(path[index], 3.0, ACCENT);
                ui.ctx().request_repaint();
            }
            let color = if selected {
                ACCENT
            } else if resolved && !loser {
                Color32::from_rgb(145, 104, 49)
            } else {
                LINE
            };
            painter.add(egui::Shape::line(
                curve_points(curve).map(transform).collect(),
                Stroke::new(if selected { 2.0 } else { 1.35_f32 }, color),
            ));
        }
        let stage = self.active().unwrap();
        for transfer in &scene.transfers {
            let visible = transfer
                .points
                .windows(2)
                .any(|pair| view.intersects(Rect::from_two_pos(pair[0], pair[1]).expand(1.0)));
            if !visible {
                continue;
            }
            let selected = self
                .selected
                .is_some_and(|id| id == transfer.source || id == transfer.target);
            let ready = matches!(
                self.states[transfer.target].slots[transfer.slot],
                Slot::Team(_)
            );
            let color = if selected {
                ACCENT
            } else if ready && transfer.loser {
                Color32::from_rgb(82, 106, 121)
            } else if ready {
                Color32::from_rgb(145, 104, 49)
            } else {
                LINE
            };
            painter.add(egui::Shape::line(
                transfer.points.into_iter().map(transform).collect(),
                Stroke::new(1.25_f32, color),
            ));
        }
        let mut clicked = None;
        for card in scene.visible(view) {
            if self
                .round_filter
                .is_some_and(|round| stage.matches[card.index].round != round)
            {
                continue;
            }
            let rect = Rect::from_min_max(transform(card.rect.min), transform(card.rect.max));
            let interaction = ui.interact(
                rect,
                egui::Id::new(("match", self.stage, card.index)),
                Sense::click(),
            );
            let selected = self.selected == Some(card.index);
            let highlight = if self.reduced_motion {
                if interaction.hovered() {
                    1.0
                } else {
                    0.0
                }
            } else {
                ui.ctx()
                    .animate_bool_with_time(interaction.id, interaction.hovered(), 0.14)
            };
            let advance = self
                .advance
                .filter(|(stage, _, _)| *stage == self.stage && !self.reduced_motion)
                .and_then(|(_, source, start)| {
                    let fraction = ((ui.input(|i| i.time) - start) / 0.52).clamp(0.0, 1.0) as f32;
                    if fraction == 1.0 {
                        return None;
                    }
                    let dependent = game_depends(&stage.matches[card.index], source);
                    if source == card.index || dependent {
                        ui.ctx().request_repaint();
                        Some((1.0 - fraction) * 0.22)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0);
            let fill = CARD
                .lerp_to_gamma(HOVER, highlight)
                .lerp_to_gamma(ACCENT, advance);
            let stroke = if selected || interaction.has_focus() {
                ACCENT
            } else {
                LINE
            };
            painter.rect(
                rect,
                13.0 * self.zoom,
                fill,
                Stroke::new(1.0_f32, stroke),
                StrokeKind::Inside,
            );
            let game = &stage.matches[card.index];
            let state = self.states[card.index];
            if interaction.clicked() {
                clicked = Some(card.index);
            }
            let status_color = if game.score.is_some() {
                Color32::from_rgb(139, 202, 170)
            } else {
                MUTED
            };
            let font = |size: f32| FontId::proportional((size * self.zoom).max(6.0));
            let inset = 14.0 * self.zoom;
            painter.text(
                rect.min + vec2(inset, 17.0 * self.zoom),
                Align2::LEFT_CENTER,
                format!("M{:02}   /   BO{}", card.index + 1, game.bo),
                font(12.0),
                MUTED,
            );
            if self.zoom >= 0.40 {
                painter.text(
                    pos2(rect.right() - inset, rect.top() + 17.0 * self.zoom),
                    Align2::RIGHT_CENTER,
                    status(game, state),
                    font(10.0),
                    status_color,
                );
            }
            if self.zoom < 0.42 {
                continue;
            }
            let divider_y = rect.top() + 35.0 * self.zoom;
            painter.hline(
                rect.left() + inset..=rect.right() - inset,
                divider_y,
                Stroke::new(1.0_f32, LINE),
            );
            for slot in 0..2 {
                let y = rect.top() + (59.0 + slot as f32 * 36.0) * self.zoom;
                let text = if state.slots[slot] == Slot::Pending {
                    origin(game.sources[slot])
                } else {
                    self.project.team_name(state.slots[slot]).to_owned()
                };
                let won = state
                    .outcome
                    .is_some_and(|o| matches!(o[0], Slot::Team(_)) && o[0] == state.slots[slot]);
                let color = if won {
                    ACCENT
                } else if matches!(state.slots[slot], Slot::Team(_)) {
                    TEXT
                } else {
                    MUTED
                };
                let mut text_left = rect.left() + inset;
                if let Slot::Team(id) = state.slots[slot] {
                    let avatar = Rect::from_center_size(
                        pos2(text_left + 11.0 * self.zoom, y),
                        vec2(22.0, 22.0) * self.zoom,
                    );
                    if let Some((_, logo)) = self.logos.get(&id) {
                        let size = logo.size_vec2();
                        let fitted = size * (avatar.width() / size.x.max(size.y));
                        painter.image(
                            logo.id(),
                            Rect::from_center_size(avatar.center(), fitted),
                            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        painter.rect_filled(
                            avatar,
                            5.0 * self.zoom,
                            if won {
                                Color32::from_rgb(62, 48, 36)
                            } else {
                                HOVER
                            },
                        );
                        let initial = text
                            .chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .collect::<String>();
                        painter.text(
                            avatar.center(),
                            Align2::CENTER_CENTER,
                            initial,
                            font(11.0),
                            color,
                        );
                    }
                    text_left += 32.0 * self.zoom;
                }
                let team_galley = painter.layout_job({
                    let mut job = egui::text::LayoutJob::simple_singleline(text, font(15.0), color);
                    job.wrap = egui::text::TextWrapping {
                        max_width: rect.right() - text_left - 46.0 * self.zoom,
                        max_rows: 1,
                        break_anywhere: true,
                        overflow_character: Some('…'),
                    };
                    job
                });
                painter.galley(
                    pos2(text_left, y - team_galley.size().y / 2.0),
                    team_galley,
                    color,
                );
                if let Some(score) = &game.score {
                    painter.text(
                        pos2(rect.right() - inset, y),
                        Align2::RIGHT_CENTER,
                        score.series[slot],
                        FontId::monospace((20.0 * self.zoom).max(7.0)),
                        color,
                    );
                }
            }
            if self.zoom >= 0.65 {
                let rows = scene::map_rows(game);
                if !rows.is_empty() {
                    let alpha = ((self.zoom - 0.65) / 0.10).clamp(0.0, 1.0);
                    painter.hline(
                        rect.left() + inset..=rect.right() - inset,
                        rect.top() + 117.0 * self.zoom,
                        Stroke::new(1.0_f32, LINE.gamma_multiply(alpha)),
                    );
                    for (row, (name, score)) in rows.into_iter().enumerate() {
                        let y = rect.top() + (135.0 + row as f32 * scene::MAP_PITCH) * self.zoom;
                        let mut job = egui::text::LayoutJob::simple_singleline(
                            name,
                            font(13.0),
                            MUTED.gamma_multiply(alpha),
                        );
                        job.wrap = egui::text::TextWrapping {
                            max_width: rect.width() - 112.0 * self.zoom,
                            max_rows: 1,
                            break_anywhere: true,
                            overflow_character: Some('…'),
                        };
                        let galley = painter.layout_job(job);
                        painter.galley(
                            pos2(rect.left() + inset, y - galley.size().y / 2.0),
                            galley,
                            MUTED,
                        );
                        painter.text(
                            pos2(rect.right() - inset, y),
                            Align2::RIGHT_CENTER,
                            score,
                            FontId::monospace(14.0 * self.zoom),
                            TEXT.gamma_multiply(alpha),
                        );
                    }
                }
            }
        }
        if let Some(index) = clicked {
            let score = stage.matches[index].score.clone();
            self.selected = Some(index);
            self.score = Some(score.unwrap_or(Score {
                series: [0, 0],
                technical: false,
                maps: vec![],
                note: String::new(),
            }));
        }
    }

    fn match_panel(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.selected else {
            return;
        };
        let state = self.states[index];
        let game = &self.project.stages[self.stage].matches[index];
        ui.horizontal(|ui| {
            ui.heading(format!("Матч M{:02}", index + 1));
            if ui.button("Закрыть").clicked() {
                self.selected = None;
            }
        });
        label(ui, &format!("BO{}   /   {}", game.bo, status(game, state)));
        ui.add_space(12.0);
        for slot in 0..2 {
            ui.label(
                RichText::new(if state.slots[slot] == Slot::Pending {
                    origin(game.sources[slot])
                } else {
                    self.project.team_name(state.slots[slot]).into()
                })
                .size(16.0)
                .strong(),
            );
        }
        if !state.active || !matches!(state.slots, [Slot::Team(_), Slot::Team(_)]) {
            return;
        }
        let stage = &self.project.stages[self.stage];
        if stage.format == Format::Swiss
            && game.score.is_none()
            && stage
                .matches
                .last()
                .is_some_and(|last| last.round == game.round)
        {
            let mut change = None;
            ui.menu_button("Переставить участника", |ui| {
                for slot in 0..2 {
                    ui.menu_button(self.project.team_name(state.slots[slot]), |ui| {
                        for (i, other) in stage.matches.iter().enumerate() {
                            if i == index || other.round != game.round || other.score.is_some() {
                                continue;
                            }
                            if let [Source::Team(a), Source::Team(b)] = other.sources {
                                for id in [a, b] {
                                    if ui.button(self.project.team_name(Slot::Team(id))).clicked() {
                                        change = Some((slot, id));
                                    }
                                }
                            }
                        }
                    });
                }
            });
            if let Some((slot, id)) = change {
                let mut next = self.project.clone();
                match next.stages[self.stage].swap_swiss(index, slot, id) {
                    Ok(()) => {
                        self.commit(next, "Пары обновлены", false);
                    }
                    Err(error) => self.notify(error, true),
                }
                return;
            }
        }
        let Some(score) = &mut self.score else {
            return;
        };
        ui.separator();
        label(ui, "СЧЁТ СЕРИИ");
        ui.horizontal(|ui| {
            for value in &mut score.series {
                ui.add(egui::DragValue::new(value).range(0..=3).speed(0.1));
            }
        });
        ui.checkbox(&mut score.technical, "Технический результат");
        if !score.technical {
            let mut remove = None;
            for (i, map) in score.maps.iter_mut().enumerate() {
                ui.text_edit_singleline(&mut map.name);
                ui.horizontal(|ui| {
                    for value in &mut map.rounds {
                        ui.add(egui::DragValue::new(value).range(0..=99));
                    }
                    if ui.small_button("Удалить").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                score.maps.remove(i);
            }
            if score.maps.len() < usize::from(game.bo) && ui.button("+ Карта").clicked() {
                score.maps.push(MapScore {
                    name: String::new(),
                    rounds: [0, 0],
                });
            }
            if !score.maps.is_empty() {
                score.series = [0, 0];
                for map in &score.maps {
                    if map.rounds[0] != map.rounds[1] {
                        score.series[usize::from(map.rounds[1] > map.rounds[0])] += 1;
                    }
                }
            }
        }
        ui.add(egui::TextEdit::multiline(&mut score.note).hint_text("Примечание"));
        let save = primary(ui, "Подтвердить");
        let clear = game.score.is_some() && ui.button("Снять результат").clicked();
        if save || clear {
            let mut next = self.project.clone();
            let score = if save {
                let mut score = score.clone();
                if score.technical {
                    score.maps.clear();
                }
                Some(score)
            } else {
                None
            };
            match next.stages[self.stage].set_score(index, score) {
                Ok(()) => {
                    if self.commit(
                        next,
                        if save {
                            "Результат сохранён"
                        } else {
                            "Результат снят"
                        },
                        false,
                    ) {
                        if clear {
                            self.selected = None;
                        }
                        if save {
                            self.advance = Some((self.stage, index, ui.input(|i| i.time)));
                        }
                    }
                }
                Err(error) => self.notify(error, true),
            }
        }
    }

    fn podium(&self, ui: &mut egui::Ui) {
        ui.heading("Призовые места");
        ui.add_space(8.0);
        for (i, id) in self.project.podium().into_iter().enumerate() {
            let accent = [
                ACCENT,
                Color32::from_rgb(190, 203, 217),
                Color32::from_rgb(210, 154, 120),
            ][i];
            let progress = if self.reduced_motion {
                1.0
            } else {
                ui.ctx()
                    .animate_bool_with_time(ui.id().with((i, id)), id.is_some(), 0.28)
            };
            Frame::new()
                .fill(CARD.lerp_to_gamma(accent, 0.09 * progress))
                .stroke(Stroke::new(
                    1.0_f32,
                    LINE.lerp_to_gamma(accent, 0.16 + 0.39 * progress),
                ))
                .corner_radius(10)
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.set_min_width(150.0);
                    let mut heading = icon_text(icons::TROPHY, &format!("{} место", i + 1));
                    heading.sections[0].format.color = accent;
                    ui.label(heading);
                    ui.horizontal(|ui| {
                        if let Some(id) = id {
                            if let Some((_, texture)) = self.logos.get(&id) {
                                ui.add(
                                    egui::Image::new(texture)
                                        .fit_to_exact_size(vec2(28.0, 28.0))
                                        .maintain_aspect_ratio(true),
                                );
                            }
                            ui.label(
                                RichText::new(self.project.team_name(Slot::Team(id)))
                                    .strong()
                                    .color(accent),
                            );
                        } else {
                            ui.label(
                                RichText::new("Место пока не определено")
                                    .size(13.0)
                                    .color(MUTED),
                            );
                        }
                    });
                });
            ui.add_space(6.0);
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        self.management_dialogs(ctx);
        if self.teams_open {
            let mut open = true;
            let mut next = self
                .team_draft
                .take()
                .unwrap_or_else(|| self.project.clone());
            let mut save = false;
            let mut logo_error = None;
            egui::Window::new("Турнир и команды")
                .open(&mut open)
                .default_width(440.0)
                .min_width(340.0)
                .show(ctx, |ui| {
                    label(ui, "НАЗВАНИЕ ТУРНИРА");
                    ui.add(egui::TextEdit::singleline(&mut next.name).desired_width(f32::INFINITY));
                    egui::ScrollArea::vertical()
                        .max_height((ctx.screen_rect().height() * 0.65).max(200.0))
                        .show(ui, |ui| {
                            for team in &mut next.teams {
                                ui.push_id(team.id, |ui| {
                                    ui.collapsing(team.name.clone(), |ui| {
                                        ui.text_edit_singleline(&mut team.name);
                                        label(ui, "ЛОГОТИП КОМАНДЫ");
                                        ui.horizontal(|ui| {
                                            let key = team.logo_png.as_deref().map(logo_uri);
                                            logo_preview(
                                                ui,
                                                team.logo_png.as_deref(),
                                                key.as_deref(),
                                            );
                                            if ui.button("Изменить").clicked() {
                                                if let Some(path) = rfd::FileDialog::new()
                                                    .add_filter(
                                                        "Изображение",
                                                        &["png", "jpg", "jpeg", "webp"],
                                                    )
                                                    .pick_file()
                                                {
                                                    match load_logo(&path) {
                                                        Ok(logo) => {
                                                            if let Some(key) = &key {
                                                                ui.ctx().forget_image(key);
                                                            }
                                                            team.logo_png = Some(logo);
                                                        }
                                                        Err(error) => logo_error = Some(error),
                                                    }
                                                }
                                            }
                                            if team.logo_png.is_some()
                                                && ui.button("Удалить").clicked()
                                            {
                                                if let Some(key) = &key {
                                                    ui.ctx().forget_image(key);
                                                }
                                                team.logo_png = None;
                                            }
                                        });
                                        ui.add(
                                            egui::TextEdit::multiline(&mut team.description)
                                                .hint_text("Описание"),
                                        );
                                        let mut remove = None;
                                        for (index, player) in team.players.iter_mut().enumerate() {
                                            ui.push_id(player.id, |ui| {
                                                egui::Frame::new()
                                                    .fill(CARD)
                                                    .inner_margin(8)
                                                    .corner_radius(8)
                                                    .show(ui, |ui| {
                                                        let title = if player.name.trim().is_empty()
                                                        {
                                                            format!("Игрок {}", index + 1)
                                                        } else {
                                                            player.name.clone()
                                                        };
                                                        ui.horizontal(|ui| {
                                                            ui.strong(title);
                                                            if tool(
                                                                ui,
                                                                icons::TRASH,
                                                                "Удалить игрока",
                                                            ) {
                                                                remove = Some(player.id);
                                                            }
                                                        });
                                                        ui.collapsing(
                                                            "Редактировать",
                                                            |ui| {
                                                                ui.add(
                                                                    egui::TextEdit::singleline(
                                                                        &mut player.name,
                                                                    )
                                                                    .hint_text("ФИО"),
                                                                );
                                                                ui.add(
                                                                    egui::TextEdit::singleline(
                                                                        &mut player.steam_id,
                                                                    )
                                                                    .hint_text("SteamID"),
                                                                );
                                                                ui.add(
                                                                    egui::TextEdit::singleline(
                                                                        &mut player.note,
                                                                    )
                                                                    .hint_text("Примечание"),
                                                                );
                                                            },
                                                        );
                                                    })
                                            });
                                        }
                                        if let Some(id) = remove {
                                            team.remove_player(id);
                                        }
                                        if ui.button("+ Игрок").clicked() {
                                            team.players.push(Player {
                                                id: Uuid::new_v4(),
                                                name: String::new(),
                                                steam_id: String::new(),
                                                note: String::new(),
                                            });
                                        }
                                    });
                                });
                            }
                        });
                    ui.add(
                        egui::TextEdit::multiline(&mut self.input)
                            .desired_rows(4)
                            .hint_text("Одна команда на строку"),
                    );
                    save = primary(ui, "Сохранить команды");
                });
            if let Some(error) = logo_error {
                self.notify(error, true);
            }
            // Retain field edits across frames until the user applies them.
            if save {
                let mut candidate = next.clone();
                for name in self.input.lines().map(str::trim).filter(|s| !s.is_empty()) {
                    candidate.teams.push(Team {
                        id: Uuid::new_v4(),
                        name: name.into(),
                        description: String::new(),
                        players: vec![],
                        logo_png: None,
                    });
                }
                if next.name.trim().is_empty()
                    || next.teams.iter().any(|t| t.name.trim().is_empty())
                {
                    self.notify("Введите название турнира и каждой команды", true);
                } else if self.commit(candidate, "Турнир и команды сохранены", false)
                {
                    self.input.clear();
                    open = false;
                }
            }
            self.teams_open = open;
            self.team_draft = open.then_some(next);
        }
        if let Some(mut draft) = self.draft.take() {
            let mut open = true;
            let mut create = false;
            egui::Window::new("Настройка этапа")
                .open(&mut open)
                .default_width(480.0)
                .show(ctx, |ui| {
                    ui.text_edit_singleline(&mut draft.name);
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("format")
                            .selected_text(draft.format.label())
                            .show_ui(ui, |ui| {
                                for format in Format::ALL {
                                    ui.selectable_value(&mut draft.format, format, format.label());
                                }
                            });
                        for bo in [1, 3, 5] {
                            ui.selectable_value(&mut draft.bo, bo, format!("BO{bo}"));
                        }
                    });
                    match draft.format {
                        Format::Single => {
                            ui.checkbox(&mut draft.rules.third_place, "Матч за третье место");
                        }
                        Format::Double => {
                            ui.checkbox(&mut draft.rules.reset, "Повторный гранд-финал");
                        }
                        Format::RoundRobin => {
                            ui.checkbox(&mut draft.rules.second_leg, "Два круга");
                            egui::ComboBox::from_id_salt("tie_break")
                                .selected_text(draft.tie_break.label())
                                .show_ui(ui, |ui| {
                                    for option in TieBreak::ALL {
                                        ui.selectable_value(
                                            &mut draft.tie_break,
                                            option,
                                            option.label(),
                                        );
                                    }
                                });
                        }
                        _ => {}
                    }
                    if !self.project.stages.is_empty() {
                        ui.menu_button("Квалификация из этапов", |ui| {
                            for (index, stage) in self.project.stages.iter().enumerate() {
                                let mut checked = draft.sources.contains(&index);
                                if ui.checkbox(&mut checked, &stage.name).changed() {
                                    if checked {
                                        draft.sources.push(index);
                                    } else {
                                        draft.sources.retain(|id| *id != index);
                                    }
                                }
                            }
                            ui.horizontal(|ui| {
                                ui.label("Команд из каждого");
                                ui.add(
                                    egui::DragValue::new(&mut draft.qualify_count).range(1..=64),
                                );
                            });
                            if !draft.sources.is_empty()
                                && ui.button("Применить квалификацию").clicked()
                            {
                                let sources: Vec<_> = draft
                                    .sources
                                    .iter()
                                    .map(|&stage| Qualification {
                                        stage,
                                        places: draft.qualify_count,
                                    })
                                    .collect();
                                match self.project.qualified(self.project.stages.len(), &sources) {
                                    Ok(entrants) => {
                                        draft.entrants = entrants;
                                        draft.qualification = sources;
                                        draft.seed = None;
                                    }
                                    Err(error) => self.notify(error, true),
                                }
                            }
                        });
                    }
                    ui.menu_button("Состав", |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                for team in &self.project.teams {
                                    let mut present = draft.entrants.contains(&team.id);
                                    if ui.checkbox(&mut present, &team.name).changed() {
                                        if present {
                                            draft.entrants.push(team.id);
                                        } else {
                                            draft.entrants.retain(|id| *id != team.id);
                                        }
                                        draft.seed = None;
                                        draft.qualification.clear();
                                    }
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Групп");
                        ui.add_enabled(
                            draft.qualification.is_empty(),
                            egui::DragValue::new(&mut draft.groups).range(1..=32),
                        );
                        ui.label("Размер корзины");
                        ui.add(egui::DragValue::new(&mut draft.pot_size).range(0..=64));
                    });
                    if ui.button("Перемешать").clicked() {
                        let seed = rand::random::<u64>();
                        let size = match draft.pot_size {
                            0 if draft.groups > 1 => draft.groups,
                            0 => draft.entrants.len(),
                            size => size,
                        };
                        draw_pots(&mut draft.entrants, seed, size, &draft.locked);
                        draft.seed = Some(seed.to_string());
                    }
                    label(ui, "ПРЕДПРОСМОТР ПОСЕВА");
                    let mut movement = None;
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            for (i, id) in draft.entrants.iter().enumerate() {
                                ui.push_id(id, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(format!(
                                            "{:02}    {}",
                                            i + 1,
                                            self.project.team_name(Slot::Team(*id))
                                        ));
                                        let mut locked = draft.locked.contains(id);
                                        if ui
                                            .checkbox(&mut locked, "")
                                            .on_hover_text("Закрепить позицию")
                                            .changed()
                                        {
                                            if locked {
                                                draft.locked.insert(*id);
                                            } else {
                                                draft.locked.remove(id);
                                            }
                                        }
                                        if i > 0
                                            && !locked
                                            && !draft.locked.contains(&draft.entrants[i - 1])
                                            && ui.small_button("↑").clicked()
                                        {
                                            movement = Some((i, i - 1));
                                        }
                                        if i + 1 < draft.entrants.len()
                                            && !locked
                                            && !draft.locked.contains(&draft.entrants[i + 1])
                                            && ui.small_button("↓").clicked()
                                        {
                                            movement = Some((i, i + 1));
                                        }
                                    })
                                });
                            }
                        });
                    if let Some((a, b)) = movement {
                        draft.entrants.swap(a, b);
                        draft.seed = None;
                    }
                    create = primary(ui, "Создать сетку");
                });
            if create {
                let name = draft.name.trim();
                let generated = if draft.groups > 1 && draft.qualification.is_empty() {
                    group_stages(
                        name,
                        draft.format,
                        &draft.entrants,
                        draft.groups,
                        draft.bo,
                        draft.rules,
                    )
                } else {
                    Stage::new(
                        name.to_owned(),
                        draft.format,
                        draft.entrants.clone(),
                        draft.bo,
                        draft.rules,
                    )
                    .map(|stage| vec![stage])
                };
                match generated {
                    Ok(mut stages) if !name.is_empty() => {
                        for stage in &mut stages {
                            stage.draw_seed = draft.seed.clone();
                            stage.qualification = draft.qualification.clone();
                            if stage.format == Format::RoundRobin {
                                stage.tie_break = draft.tie_break;
                            }
                        }
                        let schedule_warning = stages.iter().any(|stage| {
                            stage.format == Format::RoundRobin
                                && stage.max_consecutive_matches() > 2
                        });
                        let mut next = self.project.clone();
                        let first = next.stages.len();
                        next.stages.extend(stages);
                        if self.commit(next, "Этап создан", true) {
                            self.stage = first;
                            self.refresh(true);
                            if schedule_warning {
                                self.notify(
                                    "В расписании есть серии из более чем двух встреч подряд",
                                    true,
                                );
                            }
                            open = false;
                        }
                    }
                    Ok(_) => self.notify("Введите название этапа", true),
                    Err(error) => self.notify(error, true),
                }
            }
            if open {
                self.draft = Some(draft);
            }
        }
        if self.export_open {
            let mut open = true;
            let mut save = false;
            egui::Window::new("Экспорт PNG")
                .open(&mut open)
                .default_width(400.0)
                .min_width(340.0)
                .show(ctx, |ui| {
                    ui.radio_value(&mut self.export_scope, 0, "Текущий этап");
                    ui.radio_value(&mut self.export_scope, 1, "Весь турнир");
                    ui.add_enabled_ui(
                        self.selected.is_some() || self.round_filter.is_some(),
                        |ui| {
                            ui.radio_value(&mut self.export_scope, 2, "Раунд выбранного матча");
                        },
                    );
                    ui.add_enabled_ui(self.selected.is_some(), |ui| {
                        ui.radio_value(
                            &mut self.export_scope,
                            3,
                            "Раунд выбранного матча и все последующие",
                        )
                        .on_disabled_hover_text("Выберите матч на сетке");
                    });
                    if let (3, Some(id), Some(stage)) =
                        (self.export_scope, self.selected, self.active())
                    {
                        let selected = scene::included(stage, scene::Slice::FromMatch(id));
                        let mut lanes = std::collections::BTreeMap::<
                            &str,
                            std::collections::BTreeSet<usize>,
                        >::new();
                        for (game, yes) in stage.matches.iter().zip(selected) {
                            if yes {
                                lanes.entry(&game.lane).or_default().insert(game.round + 1);
                            }
                        }
                        ui.separator();
                        label(ui, "В ЭКСПОРТЕ");
                        for (lane, rounds) in lanes {
                            ui.label(format!(
                                "{lane} · раунды {}",
                                rounds
                                    .into_iter()
                                    .map(|r| r.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ));
                        }
                    }
                    if self.export_scope == 1 {
                        ui.checkbox(&mut self.export_options.podium, "Включить призовые места");
                    }
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.export_options.scale, 1, "1×");
                        ui.selectable_value(&mut self.export_options.scale, 2, "2×");
                    });
                    ui.horizontal(|ui| {
                        ui.label("Поля, px");
                        ui.add(
                            egui::DragValue::new(&mut self.export_options.margin).range(16..=96),
                        );
                    });
                    ui.checkbox(&mut self.export_options.transparent, "Прозрачный фон");
                    let valid = self.export_scope < 2
                        || self.selected.is_some()
                        || self.export_scope == 2 && self.round_filter.is_some();
                    save = ui
                        .add_enabled_ui(valid, |ui| primary(ui, "Сохранить PNG"))
                        .inner;
                });
            if save {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PNG", &["png"])
                    .set_file_name("bracket.png")
                    .save_file()
                {
                    let selection: Vec<_> = match self.export_scope {
                        1 => (0..self.project.stages.len())
                            .map(|i| (i, scene::Slice::All))
                            .collect(),
                        2 => vec![(
                            self.stage,
                            scene::Slice::Round(
                                self.round_filter
                                    .or_else(|| {
                                        self.selected
                                            .map(|id| self.active().unwrap().matches[id].round)
                                    })
                                    .unwrap(),
                            ),
                        )],
                        3 => vec![(self.stage, scene::Slice::FromMatch(self.selected.unwrap()))],
                        _ => vec![(self.stage, scene::Slice::All)],
                    };
                    let project = self.project.clone();
                    let options = scene::ExportOptions {
                        podium: self.export_scope == 1 && self.export_options.podium,
                        ..self.export_options
                    };
                    let (send, receive) = mpsc::channel();
                    let ctx = ctx.clone();
                    std::thread::spawn(move || {
                        let _ = send.send(scene::export(&project, &selection, options, &path));
                        ctx.request_repaint();
                    });
                    self.job = Some(receive);
                    self.notify("Экспорт PNG…", false);
                    open = false;
                }
            }
            self.export_open = open;
        }
        if self.standings_open {
            let mut open = true;
            if let Some(stage) = self.active() {
                egui::Window::new("Таблица этапа")
                    .open(&mut open)
                    .show(ctx, |ui| {
                        if let Ok(places) = stage.qualifiers() {
                            if let Some(&id) = places.first() {
                                ui.heading(format!(
                                    "Лидер · {}",
                                    self.project.team_name(Slot::Team(id))
                                ));
                            }
                        }
                        egui::ScrollArea::vertical()
                            .max_height(440.0)
                            .show(ui, |ui| {
                                egui::Grid::new("standings")
                                    .striped(true)
                                    .min_col_width(48.0)
                                    .show(ui, |ui| {
                                        for heading in ["Место", "Команда", "Победы", "Поражения"]
                                        {
                                            label(ui, heading);
                                        }
                                        ui.end_row();
                                        for (index, (id, wins, losses)) in
                                            stage.standings().into_iter().enumerate()
                                        {
                                            ui.label((index + 1).to_string());
                                            ui.label(self.project.team_name(Slot::Team(id)));
                                            ui.label(wins.to_string());
                                            ui.label(losses.to_string());
                                            ui.end_row();
                                        }
                                    });
                            });
                    });
            }
            self.standings_open = open;
        }
        if let Some(events) = &self.audit {
            let mut open = true;
            egui::Window::new("История действий")
                .open(&mut open)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(420.0)
                        .show(ui, |ui| {
                            for (time, action) in events {
                                ui.label(format!("{time} UTC   {action}"));
                            }
                        });
                });
            if !open {
                self.audit = None;
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll_storage(ctx);
        self.logos.retain(|id, _| {
            self.project
                .teams
                .iter()
                .any(|team| team.id == *id && team.logo_png.is_some())
        });
        for team in &self.project.teams {
            let Some(logo) = &team.logo_png else {
                continue;
            };
            if self
                .logos
                .get(&team.id)
                .is_some_and(|(encoded, _)| encoded == logo)
            {
                continue;
            }
            self.logos.remove(&team.id);
            if let Ok(raw) = STANDARD.decode(logo) {
                if let Ok(image) = read_image(std::io::Cursor::new(raw)) {
                    let image = image.to_rgba8();
                    let pixels = egui::ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_raw(),
                    );
                    self.logos.insert(
                        team.id,
                        (
                            logo.clone(),
                            ctx.load_texture(
                                team.id.to_string(),
                                pixels,
                                egui::TextureOptions::LINEAR,
                            ),
                        ),
                    );
                }
            }
        }
        let export_message = self.job.as_ref().and_then(|job| match job.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("Не удалось завершить экспорт".into()))
            }
            Err(mpsc::TryRecvError::Empty) => None,
        });
        if let Some(result) = export_message {
            self.job = None;
            match result {
                Ok(()) => self.notify("PNG сохранён", false),
                Err(error) => self.notify(error, true),
            }
        }
        egui::SidePanel::left("navigation")
            .exact_width(220.0)
            .resizable(false)
            .frame(Frame::new().fill(PANEL).inner_margin(18))
            .show(ctx, |ui| self.sidebar(ui));
        egui::TopBottomPanel::top("toolbar")
            .frame(Frame::new().fill(PANEL).inner_margin(14))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(
                        RichText::new(
                            self.active()
                                .map(|s| s.name.as_str())
                                .unwrap_or("Рабочая область"),
                        )
                        .size(22.0),
                    );
                    ui.separator();
                    if tool(ui, icons::ARROW_COUNTER_CLOCKWISE, "Отменить") {
                        self.step(false);
                    }
                    if tool(ui, icons::ARROW_CLOCKWISE, "Повторить") {
                        self.step(true);
                    }
                    if self.scene.is_some() {
                        if tool(ui, icons::CORNERS_OUT, "Вписать сетку") {
                            self.fit = true;
                        }
                        if ui
                            .button(format!("{:.0}%", self.zoom * 100.0))
                            .on_hover_text("Масштаб 100%")
                            .clicked()
                        {
                            self.zoom = 1.0;
                            self.camera_transition = None;
                        }
                        if tool(ui, icons::LIST_NUMBERS, "Таблица этапа") {
                            self.standings_open = true;
                        }
                        if let Some(stage) = self.active().filter(|stage| {
                            matches!(stage.format, Format::RoundRobin | Format::Swiss)
                        }) {
                            let rounds: std::collections::BTreeSet<_> =
                                stage.matches.iter().map(|game| game.round).collect();
                            let before = self.round_filter;
                            egui::ComboBox::from_id_salt("visible_round")
                                .selected_text(before.map_or("Все туры".into(), |round| {
                                    format!("Тур {}", round + 1)
                                }))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.round_filter, None, "Все туры");
                                    for round in rounds {
                                        ui.selectable_value(
                                            &mut self.round_filter,
                                            Some(round),
                                            format!("Тур {}", round + 1),
                                        );
                                    }
                                });
                            if before != self.round_filter {
                                self.fit = true;
                                self.selected = None;
                            }
                        }
                        if self.job.is_none()
                            && self.storage_job.is_none()
                            && ui
                                .button(icon_text(icons::DOWNLOAD_SIMPLE, "Экспорт PNG"))
                                .clicked()
                        {
                            self.export_open = true;
                        }
                    }
                    if self
                        .active()
                        .is_some_and(|s| s.format == Format::Swiss && !s.swiss_finished)
                        && ui.button("Следующий тур").clicked()
                    {
                        let mut next = self.project.clone();
                        let stage = &mut next.stages[self.stage];
                        let bo = stage.matches[0].bo;
                        match stage.next_swiss_round(bo) {
                            Ok(()) => {
                                self.commit(next, "Тур добавлен", true);
                            }
                            Err(error) => self.notify(error, true),
                        }
                    }
                    if self.active().is_some_and(|s| {
                        s.format == Format::Swiss && !s.swiss_finished && s.qualifiers().is_ok()
                    }) && ui.button("Завершить Swiss").clicked()
                    {
                        let mut project = self.project.clone();
                        project.stages[self.stage].swiss_finished = true;
                        self.commit(project, "Этап Swiss завершён", false);
                    }
                    if self.scene.is_some()
                        && tool(ui, icons::TROPHY, "Показать или скрыть призовые места")
                    {
                        self.podium_open = !self.podium_open;
                        self.fit = true;
                    }
                })
            });
        egui::TopBottomPanel::bottom("status")
            .frame(Frame::new().fill(PANEL).inner_margin(10))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(if self.notice.is_empty() {
                        "Перетаскивание · перемещение     Колесо · масштаб"
                    } else {
                        &self.notice
                    })
                    .color(if self.error {
                        Color32::LIGHT_RED
                    } else {
                        MUTED
                    }),
                );
            });
        if self.podium_open && self.scene.is_some() {
            egui::SidePanel::right("podium")
                .default_width(210.0)
                .width_range(180.0..=280.0)
                .frame(Frame::new().fill(PANEL).inner_margin(14))
                .show(ctx, |ui| self.podium(ui));
        }
        if self.selected.is_some() {
            let mut open = true;
            egui::Window::new("Редактор матча")
                .open(&mut open)
                .default_width(490.0)
                .min_width(340.0)
                .default_pos(pos2((ctx.screen_rect().right() - 730.0).max(240.0), 84.0))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(ctx.screen_rect().height() * 0.66)
                        .show(ui, |ui| self.match_panel(ui));
                });
            if !open {
                self.selected = None;
            }
        }
        self.dialogs(ctx);
        egui::CentralPanel::default()
            .frame(Frame::new().fill(BG))
            .show(ctx, |ui| self.canvas(ui));
        if let Some(path) = &self.capture {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
            if !self.capture_sent && ctx.cumulative_pass_nr() >= 3 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.capture_sent = true;
            }
            let screenshot = ctx.input(|input| {
                input.events.iter().find_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(screenshot) = screenshot {
                let rgba: Vec<_> = screenshot
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                if let Err(error) = image::save_buffer(
                    path,
                    &rgba,
                    screenshot.size[0] as u32,
                    screenshot.size[1] as u32,
                    image::ColorType::Rgba8,
                ) {
                    tracing::error!(%error, "Не удалось сохранить снимок экрана");
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

fn game_depends(game: &Match, source: usize) -> bool {
    game.sources
        .iter()
        .any(|slot| matches!(slot, Source::Winner(id) | Source::Loser(id) if *id == source))
}

fn logo_uri(data: &str) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    format!("bytes://team-logo-{:016x}.png", hasher.finish())
}

fn logo_preview(ui: &mut egui::Ui, data: Option<&str>, key: Option<&str>) {
    let (square, _) = ui.allocate_exact_size(vec2(116.0, 116.0), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        square,
        8.0,
        CARD,
        Stroke::new(1.0_f32, LINE),
        StrokeKind::Inside,
    );
    if let (Some(data), Some(key)) = (data, key) {
        if ui.ctx().try_load_bytes(key).is_err() {
            if let Ok(bytes) = STANDARD.decode(data) {
                ui.ctx().include_bytes(key.to_owned(), bytes);
            }
        }
        let image = egui::Image::new(key);
        if let Ok(egui::load::TexturePoll::Ready { texture }) =
            image.load_for_size(ui.ctx(), vec2(104.0, 104.0))
        {
            let size = texture.size;
            image.paint_at(
                ui,
                Rect::from_center_size(square.center(), size * (104.0 / size.x.max(size.y))),
            );
            return;
        }
    }
    painter.text(
        square.center(),
        Align2::CENTER_CENTER,
        icons::IMAGE_SQUARE,
        FontId::new(30.0, icon_family()),
        MUTED,
    );
}

#[cfg(test)]
mod interaction_tests {
    use super::*;

    #[test]
    fn wheel_reaches_canvas_only_under_the_pointer() {
        let ctx = egui::Context::default();
        let mut store = Storage::open(std::path::Path::new(":memory:")).unwrap();
        store.commit(&fixtures::preview(), "Тест").unwrap();
        let mut app = App::new(&ctx, Ok(store));
        fn frame(ctx: &egui::Context, app: &mut App, point: Pos2, wheel: bool, overlay: u8) {
            let mut events = vec![egui::Event::PointerMoved(point)];
            if wheel {
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, 160.0),
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 800.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::SidePanel::left("test_sidebar")
                        .exact_width(150.0)
                        .show(ctx, |ui| {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for i in 0..80 {
                                    ui.label(format!("Строка {i}"));
                                }
                            });
                        });
                    if overlay == 1 {
                        egui::Window::new("Тестовая панель")
                            .fixed_pos(pos2(220.0, 100.0))
                            .fixed_size(vec2(280.0, 220.0))
                            .show(ctx, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(160.0)
                                    .show(ui, |ui| {
                                        for i in 0..80 {
                                            ui.label(format!("Строка {i}"));
                                        }
                                    });
                            });
                    }
                    if overlay == 2 {
                        egui::Modal::new("test_modal".into()).show(ctx, |ui| {
                            ui.label("Подтверждение");
                        });
                    }
                    egui::CentralPanel::default().show(ctx, |ui| app.canvas(ui));
                },
            );
        }
        for (point, overlay) in [
            (pos2(50.0, 150.0), 0),
            (pos2(260.0, 150.0), 1),
            (pos2(850.0, 620.0), 2),
        ] {
            for _ in 0..4 {
                frame(&ctx, &mut app, point, false, overlay);
            }
            let zoom = app.zoom;
            frame(&ctx, &mut app, point, true, overlay);
            assert_eq!(app.zoom, zoom, "overlay {overlay}");
        }
        let point = pos2(700.0, 600.0);
        for _ in 0..4 {
            frame(&ctx, &mut app, point, false, 0);
        }
        let zoom = app.zoom;
        frame(&ctx, &mut app, point, true, 0);
        assert!(app.zoom > zoom);
    }
}

fn read_image(
    source: impl std::io::BufRead + std::io::Seek,
) -> Result<image::DynamicImage, String> {
    let mut reader = image::ImageReader::new(source)
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(80 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|e| e.to_string())
}

fn load_logo(path: &std::path::Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let logo = read_image(std::io::BufReader::new(file))?.thumbnail(96, 96);
    let mut buffer = std::io::Cursor::new(Vec::new());
    logo.write_to(&mut buffer, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(STANDARD.encode(buffer.into_inner()))
}
