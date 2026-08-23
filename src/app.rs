//! Application-facing shell. The engine remains the source of truth for domain,
//! validation, execution and history; this module only adapts it to a desktop UI.

use crate::{
    domain::{Action, Automation, Condition, ConditionNode, Trigger},
    history::{ExecutionRecord, HistoryFilter, HistoryProvider, JsonlHistory},
    persistence::{self, Config},
    runtime::{RuntimeHandle, RuntimeStatus},
};
use eframe::egui::{self, Color32, RichText};
use std::{path::PathBuf, sync::Arc, time::Duration};

const BLUE: Color32 = Color32::from_rgb(112, 166, 255);
const GREEN: Color32 = Color32::from_rgb(108, 194, 145);
const RED: Color32 = Color32::from_rgb(232, 116, 108);
const MUTED: Color32 = Color32::from_rgb(145, 155, 169);
const CARD: Color32 = Color32::from_rgb(27, 33, 42);
const CARD_HOVER: Color32 = Color32::from_rgb(34, 42, 53);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Dashboard,
    Automations,
    History,
    Settings,
}

pub struct DesktopApp {
    config_path: PathBuf,
    history: Arc<JsonlHistory>,
    runtime: RuntimeHandle,
    config: Config,
    page: Page,
    editor: Option<Automation>,
    search: String,
    history_search: String,
    error: Option<String>,
    notice: Option<String>,
    confirm_delete: Option<String>,
}

impl DesktopApp {
    pub fn open(config_path: PathBuf) -> Result<Self, String> {
        let config = if config_path.exists() {
            persistence::load(&config_path)
        } else {
            let config = Config::default();
            persistence::save(&config_path, &config).map_err(|e| e.to_string())?;
            Ok(config)
        }
        .map_err(|e| e.to_string())?;
        let history_path = directories::ProjectDirs::from("com", "temp-files-cleanup", "engine")
            .ok_or_else(|| "Cannot resolve the application data directory".to_string())?
            .data_dir()
            .join("execution.jsonl");
        let history = Arc::new(JsonlHistory::open(history_path).map_err(|e| e.to_string())?);
        let runtime = RuntimeHandle::start(config.automations.clone(), history.clone());
        Ok(Self {
            config_path,
            history,
            runtime,
            config,
            page: Page::Dashboard,
            editor: None,
            search: String::new(),
            history_search: String::new(),
            error: None,
            notice: None,
            confirm_delete: None,
        })
    }

    fn save(&mut self) -> bool {
        match persistence::save(&self.config_path, &self.config) {
            Ok(()) => {
                self.notice = Some("Automation configuration saved".into());
                self.error = None;
                self.refresh_runtime();
                true
            }
            Err(e) => {
                self.error = Some(user_error(e.to_string()));
                false
            }
        }
    }

    fn refresh_runtime(&mut self) {
        if let Err(error) = self.runtime.refresh(self.config.automations.clone()) {
            self.error = Some(user_error(error));
        }
    }

    fn run(&mut self, id: &str) {
        match self.runtime.trigger(id) {
            Ok(()) => self.notice = Some("Automation queued".into()),
            Err(error) => self.error = Some(user_error(error)),
        }
    }

    fn runtime_status(&self) -> RuntimeStatus {
        self.runtime.status()
    }

    fn navigation(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("AUTOMATION DESK")
                .size(17.0)
                .strong()
                .color(BLUE),
        );
        ui.label(RichText::new("Windows control center").small().color(MUTED));
        ui.add_space(28.0);
        for (page, label, icon) in [
            (Page::Dashboard, "Dashboard", "⌂"),
            (Page::Automations, "Automations", "◇"),
            (Page::History, "History", "◷"),
            (Page::Settings, "Settings", "⚙"),
        ] {
            let selected = self.page == page;
            let response = ui.add_sized(
                [ui.available_width(), 34.0],
                egui::Button::new(
                    RichText::new(format!("  {icon}  {label}")).color(if selected {
                        Color32::WHITE
                    } else {
                        MUTED
                    }),
                )
                .fill(if selected {
                    Color32::from_rgb(39, 71, 112)
                } else {
                    Color32::TRANSPARENT
                })
                .stroke(egui::Stroke::NONE),
            );
            if response.clicked() {
                self.page = page;
                self.editor = None;
            }
            ui.add_space(5.0);
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            let (label, color) = runtime_display(&self.runtime_status());
            status_chip(ui, label, color);
            ui.label(RichText::new("BACKGROUND RUNTIME").small().color(MUTED));
            ui.label(RichText::new("LOCAL MODE").small().color(MUTED));
        });
    }

    fn header(&mut self, ui: &mut egui::Ui, title: &str, subtitle: &str) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).size(26.0).strong());
            ui.add_space(12.0);
            ui.label(RichText::new(subtitle).color(MUTED));
        });
        ui.add_space(16.0);
        if let Some(message) = self.notice.take() {
            ui.colored_label(GREEN, message);
        }
        if let Some(message) = self.error.take() {
            ui.colored_label(RED, message);
        }
    }

    fn dashboard(&mut self, ui: &mut egui::Ui) {
        self.header(ui, "Dashboard", "A quick read on your automation system");
        let total = self.config.automations.len();
        let enabled = self.config.automations.iter().filter(|a| a.enabled).count();
        let history = self
            .history
            .get_history(HistoryFilter::default())
            .unwrap_or_default();
        ui.columns(2, |columns| {
            let (runtime_label, runtime_color) = runtime_display(&self.runtime_status());
            panel(&mut columns[0], |ui| {
                section_title(ui, "Runtime", "The local automation service");
                status_chip(ui, runtime_label, runtime_color);
                ui.add_space(6.0);
                ui.label(
                    RichText::new(match self.runtime_status() {
                        RuntimeStatus::Running => "Ready for schedules, hotkeys, and manual runs.",
                        RuntimeStatus::Starting => "Starting the local runtime…",
                        RuntimeStatus::Stopped => "The runtime is stopped.",
                        RuntimeStatus::Error(_) => "The runtime needs attention before it can run.",
                    })
                    .color(MUTED),
                );
            });
            panel(&mut columns[1], |ui| {
                section_title(ui, "Automation overview", "Your local system at a glance");
                ui.horizontal(|ui| {
                    stat(ui, "TOTAL", total.to_string(), Color32::WHITE);
                    stat(ui, "ENABLED", enabled.to_string(), GREEN);
                    stat(ui, "DISABLED", (total - enabled).to_string(), MUTED);
                });
            });
        });
        ui.add_space(18.0);
        ui.add_space(8.0);
        section_title(ui, "Recent activity", "The latest automation results");
        ui.add_space(8.0);
        if history.is_empty() {
            empty(
                ui,
                "No execution history yet",
                "Run an automation and its result will appear here.",
            );
        }
        for record in history.iter().rev().take(6) {
            history_row(
                ui,
                record,
                self.config
                    .automations
                    .iter()
                    .find(|a| a.id == record.result.automation_id)
                    .map(|a| a.name.as_str()),
            );
        }
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            if ui.button("＋  New automation").clicked() {
                self.editor = Some(new_automation());
                self.page = Page::Automations;
            }
            if secondary_button(ui, "View history").clicked() {
                self.page = Page::History;
            }
            if secondary_button(ui, "View automations").clicked() {
                self.page = Page::Automations;
            }
        });
    }

    fn automations(&mut self, ui: &mut egui::Ui) {
        if let Some(mut automation) = self.editor.take() {
            self.editor_ui(ui, &mut automation);
            return;
        }
        self.header(ui, "Automations", "Manage what runs and when");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search automations…")
                    .desired_width(280.0),
            );
            if ui.button("＋  New automation").clicked() {
                self.editor = Some(new_automation());
            }
        });
        ui.add_space(12.0);
        let query = self.search.to_lowercase();
        let history = self
            .history
            .get_history(HistoryFilter::default())
            .unwrap_or_default();
        let visible: Vec<Automation> = self
            .config
            .automations
            .iter()
            .filter(|a| {
                query.is_empty()
                    || a.name.to_lowercase().contains(&query)
                    || a.id.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        let mut action: Option<(String, &'static str)> = None;
        for a in &visible {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        if a.enabled {
                            Color32::from_rgb(115, 190, 140)
                        } else {
                            Color32::GRAY
                        },
                        if a.enabled { "●" } else { "○" },
                    );
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&a.name).strong());
                        ui.label(
                            RichText::new(format!("{}  ·  {}", trigger_name(&a.trigger), a.id))
                                .small()
                                .color(Color32::GRAY),
                        );
                        if let Some(last) = history
                            .iter()
                            .rev()
                            .find(|record| record.result.automation_id == a.id)
                        {
                            ui.label(
                                RichText::new(format!(
                                    "Last: {} ({})",
                                    last.timestamp,
                                    if last.result.success {
                                        "success"
                                    } else {
                                        "failed"
                                    }
                                ))
                                .small()
                                .color(if last.result.success {
                                    Color32::from_rgb(115, 190, 140)
                                } else {
                                    Color32::from_rgb(230, 120, 110)
                                }),
                            );
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Edit").clicked() {
                            action = Some((a.id.clone(), "edit"));
                        }
                        if ui.button("Run").clicked() {
                            action = Some((a.id.clone(), "run"));
                        }
                        let label = if a.enabled { "Disable" } else { "Enable" };
                        if ui.button(label).clicked() {
                            action = Some((a.id.clone(), "toggle"));
                        }
                        if ui.button("Delete").clicked() {
                            action = Some((a.id.clone(), "delete"));
                        }
                    });
                });
            });
            ui.add_space(7.0);
        }
        if let Some((id, kind)) = action {
            match kind {
                "edit" => {
                    self.editor = self.config.automations.iter().find(|a| a.id == id).cloned()
                }
                "run" => self.run(&id),
                "toggle" => {
                    if let Some(a) = self.config.automations.iter_mut().find(|a| a.id == id) {
                        a.enabled = !a.enabled;
                    }
                    self.save();
                }
                "delete" => self.confirm_delete = Some(id),
                _ => {}
            }
        }
        if let Some(id) = self.confirm_delete.clone() {
            let name = self
                .config
                .automations
                .iter()
                .find(|a| a.id == id)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| "this automation".into());
            egui::Window::new("Delete automation")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Delete '{name}' from the JSON configuration?"));
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            self.config.automations.retain(|a| a.id != id);
                            self.confirm_delete = None;
                            self.save();
                            self.notice = Some("Automation deleted".into());
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirm_delete = None;
                        }
                    });
                });
        }
        if visible.is_empty() && !query.is_empty() {
            ui.horizontal(|ui| {
                empty(
                    ui,
                    "No automations match your search",
                    "Try a different name or clear the filter.",
                );
                if secondary_button(ui, "Clear search").clicked() {
                    self.search.clear();
                }
            });
        } else if self.config.automations.is_empty() {
            empty(
                ui,
                "No automations",
                "Create your first automation to start managing local tasks.",
            );
        }
    }

    fn editor_ui(&mut self, ui: &mut egui::Ui, a: &mut Automation) {
        self.header(
            ui,
            if self.config.automations.iter().any(|x| x.id == a.id) {
                "Edit automation"
            } else {
                "New automation"
            },
            "Configure the existing engine model",
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("Name").strong());
            ui.text_edit_singleline(&mut a.name);
            ui.checkbox(&mut a.enabled, "Enabled");
        });
        if a.name.trim().is_empty() {
            ui.colored_label(
                Color32::from_rgb(230, 120, 110),
                "Give this automation a name before saving.",
            );
        }
        ui.add_space(8.0);
        ui.separator();
        ui.heading(RichText::new("WHEN").color(Color32::from_rgb(125, 170, 255)));
        ui.label(
            RichText::new("Choose when this automation should be considered.").color(Color32::GRAY),
        );
        ui.horizontal(|ui| {
            ui.label("Type");
            egui::ComboBox::from_id_salt("trigger")
                .selected_text(trigger_name(&a.trigger))
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(matches!(a.trigger, Trigger::Manual), "Manual")
                        .clicked()
                    {
                        a.trigger = Trigger::Manual;
                    }
                    if ui
                        .selectable_label(matches!(a.trigger, Trigger::Interval { .. }), "Interval")
                        .clicked()
                    {
                        a.trigger = Trigger::Interval { seconds: 60 };
                    }
                    if ui
                        .selectable_label(matches!(a.trigger, Trigger::Daily { .. }), "Daily")
                        .clicked()
                    {
                        a.trigger = Trigger::Daily {
                            time_hh_mm: "09:00".into(),
                        };
                    }
                    if ui
                        .selectable_label(matches!(a.trigger, Trigger::Hotkey { .. }), "Hotkey")
                        .clicked()
                    {
                        a.trigger = Trigger::Hotkey {
                            combination: "Ctrl+Alt+T".into(),
                        };
                    }
                });
            match &mut a.trigger {
                Trigger::Interval { seconds } => {
                    ui.label("Seconds");
                    ui.add(egui::DragValue::new(seconds).range(1..=86400));
                }
                Trigger::Daily { time_hh_mm } => {
                    ui.label("Time (HH:MM)");
                    ui.text_edit_singleline(time_hh_mm);
                }
                Trigger::Hotkey { combination } => {
                    ui.label("Combination");
                    ui.text_edit_singleline(combination);
                }
                Trigger::Manual => {}
            }
        });
        ui.separator();
        ui.heading(RichText::new("ONLY IF").color(Color32::from_rgb(205, 170, 95)));
        ui.label(
            RichText::new("Optional conditions that must be true before running.")
                .color(Color32::GRAY),
        );
        condition_editor(ui, &mut a.conditions);
        ui.separator();
        ui.heading("Execution settings");
        ui.horizontal(|ui| {
            ui.label("Concurrency");
            egui::ComboBox::from_id_salt("concurrency-policy")
                .selected_text(match a.settings.concurrency_policy {
                    crate::domain::ConcurrencyPolicy::Allow => "Allow concurrent runs",
                    crate::domain::ConcurrencyPolicy::SkipIfRunning => "Skip if running",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut a.settings.concurrency_policy,
                        crate::domain::ConcurrencyPolicy::Allow,
                        "Allow concurrent runs",
                    );
                    ui.selectable_value(
                        &mut a.settings.concurrency_policy,
                        crate::domain::ConcurrencyPolicy::SkipIfRunning,
                        "Skip if running",
                    );
                });
            ui.label("Failure");
            egui::ComboBox::from_id_salt("failure-policy")
                .selected_text(match a.settings.failure_policy {
                    crate::domain::FailurePolicy::Continue => "Continue",
                    crate::domain::FailurePolicy::Stop => "Stop",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut a.settings.failure_policy,
                        crate::domain::FailurePolicy::Continue,
                        "Continue",
                    );
                    ui.selectable_value(
                        &mut a.settings.failure_policy,
                        crate::domain::FailurePolicy::Stop,
                        "Stop on failure",
                    );
                });
        });
        ui.separator();
        ui.heading(RichText::new("THEN").color(Color32::from_rgb(115, 190, 140)));
        ui.label(RichText::new("Actions run in the order shown below.").color(Color32::GRAY));
        let mut remove_action = None;
        for (index, action) in a.actions.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                if action_editor(ui, action) {
                    remove_action = Some(index);
                }
            });
        }
        if let Some(index) = remove_action {
            a.actions.remove(index);
        }
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("new-action")
                .selected_text("Choose action to add")
                .show_ui(ui, |ui| {
                    if ui.button("Clean temporary files").clicked() {
                        a.actions
                            .push(Action::CleanTemporaryFiles { directories: None });
                    }
                    if ui.button("Launch application").clicked() {
                        a.actions.push(Action::LaunchApplication {
                            executable: String::new(),
                            args: vec![],
                        });
                    }
                    if ui.button("Show notification").clicked() {
                        a.actions.push(Action::ShowNotification {
                            title: String::new(),
                            message: String::new(),
                        });
                    }
                });
            ui.label("Actions execute in order.");
        });
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            if primary_button(ui, "Save automation").clicked() {
                if a.name.trim().is_empty() {
                    self.error = Some("Automation name cannot be empty".into());
                } else if a.actions.is_empty() {
                    self.error = Some("Add at least one action".into());
                } else {
                    let mut candidate = self.config.clone();
                    if let Some(existing) = candidate.automations.iter_mut().find(|x| x.id == a.id)
                    {
                        *existing = a.clone();
                    } else {
                        candidate.automations.push(a.clone());
                    }
                    match persistence::validate(&candidate) {
                        Ok(()) => {
                            self.config = candidate;
                            if self.save() {
                                self.editor = None;
                            }
                        }
                        Err(error) => self.error = Some(user_error(error.to_string())),
                    }
                }
            }
            if ui.button("Cancel").clicked() {
                self.editor = None;
            }
        });
    }

    fn history(&mut self, ui: &mut egui::Ui) {
        self.header(
            ui,
            "Execution history",
            "JSONL records written by the engine",
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.history_search)
                .hint_text("Search by automation, action or result…")
                .desired_width(360.0),
        );
        ui.add_space(12.0);
        let names = self.config.automations.clone();
        let records = self
            .history
            .get_history(HistoryFilter::default())
            .unwrap_or_default();
        let q = self.history_search.to_lowercase();
        let mut shown = false;
        for record in records
            .iter()
            .rev()
            .filter(|r| q.is_empty() || format!("{:?}", r).to_lowercase().contains(&q))
        {
            shown = true;
            history_row(
                ui,
                record,
                names
                    .iter()
                    .find(|a| a.id == record.result.automation_id)
                    .map(|a| a.name.as_str()),
            );
        }
        if !shown && !q.is_empty() {
            empty(
                ui,
                "No matching executions",
                "Clear the search to return to the full history.",
            );
        } else if records.is_empty() {
            empty(
                ui,
                "No history",
                "Execution history will appear after an automation runs.",
            );
        }
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        self.header(ui, "Settings", "Storage and engine details");
        ui.group(|ui| {
            ui.heading("Storage");
            ui.label(format!("Configuration\n{}", self.config_path.display()));
            ui.add_space(5.0);
            ui.label("History\nApplication data\\execution.jsonl");
        });
        ui.add_space(12.0);
        ui.group(|ui| {
            ui.heading("Engine");
            ui.label("Status");
            let (runtime_label, runtime_color) = runtime_display(&self.runtime_status());
            ui.colored_label(runtime_color, runtime_label);
            ui.label("The desktop app owns the local background runtime and JSON persistence.");
        });
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_theme(ctx);
        ctx.request_repaint_after(Duration::from_millis(500));
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(page_name(self.page)).strong());
                ui.label(RichText::new("/  Automation Desk").small().color(MUTED));
                let (runtime_label, runtime_color) = runtime_display(&self.runtime_status());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    status_chip(ui, runtime_label, runtime_color);
                });
            });
        });
        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(190.0)
            .show(ctx, |ui| self.navigation(ui));
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_max_width(1100.0);
            match self.page {
                Page::Dashboard => self.dashboard(ui),
                Page::Automations => self.automations(ui),
                Page::History => self.history(ui),
                Page::Settings => self.settings(ui),
            }
        });
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 9.0);
    style.spacing.button_padding = egui::vec2(13.0, 8.0);
    style.spacing.indent = 18.0;
    style.visuals = egui::Visuals::dark();
    style.visuals.window_fill = Color32::from_rgb(25, 29, 36);
    style.visuals.panel_fill = Color32::from_rgb(19, 23, 29);
    style.visuals.faint_bg_color = CARD;
    style.visuals.widgets.inactive.bg_fill = CARD;
    style.visuals.widgets.hovered.bg_fill = CARD_HOVER;
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(43, 75, 116);
    style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(23, 28, 35);
    style.visuals.widgets.inactive.fg_stroke.color = MUTED;
    style.visuals.widgets.hovered.fg_stroke.color = Color32::WHITE;
    style.visuals.extreme_bg_color = Color32::from_rgb(13, 16, 21);
    style.visuals.selection.bg_fill = Color32::from_rgb(45, 82, 130);
    style.visuals.hyperlink_color = BLUE;
    ctx.set_style(style);
}

fn panel(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::none()
        .fill(CARD)
        .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(49, 59, 73)))
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, content);
}

fn section_title(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).size(16.0).strong());
    ui.label(RichText::new(subtitle).small().color(MUTED));
    ui.add_space(10.0);
}

fn status_chip(ui: &mut egui::Ui, label: &str, color: Color32) {
    egui::Frame::none()
        .fill(color.linear_multiply(0.16))
        .rounding(egui::Rounding::same(5.0))
        .inner_margin(egui::Margin::symmetric(8.0, 4.0))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("●  {label}"))
                    .small()
                    .strong()
                    .color(color),
            );
        });
}

fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).strong().color(Color32::WHITE))
            .fill(Color32::from_rgb(44, 91, 150)),
    )
}

fn secondary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(egui::Button::new(label).fill(CARD))
}

fn new_automation() -> Automation {
    Automation {
        id: format!(
            "automation-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ),
        name: "New automation".into(),
        enabled: true,
        trigger: Trigger::Manual,
        conditions: ConditionNode::Empty,
        actions: vec![],
        settings: Default::default(),
    }
}
fn user_error(e: String) -> String {
    format!("Could not complete operation: {e}")
}
fn runtime_display(status: &RuntimeStatus) -> (&'static str, Color32) {
    match status {
        RuntimeStatus::Starting => ("STARTING", Color32::YELLOW),
        RuntimeStatus::Running => ("RUNNING", Color32::from_rgb(115, 190, 140)),
        RuntimeStatus::Stopped => ("STOPPED", Color32::GRAY),
        RuntimeStatus::Error(_) => ("ERROR", Color32::from_rgb(230, 120, 110)),
    }
}
fn trigger_name(t: &Trigger) -> &'static str {
    match t {
        Trigger::Manual => "Manual",
        Trigger::Hotkey { .. } => "Hotkey",
        Trigger::Interval { .. } => "Interval",
        Trigger::Daily { .. } => "Daily",
    }
}
fn page_name(page: Page) -> &'static str {
    match page {
        Page::Dashboard => "Dashboard",
        Page::Automations => "Automations",
        Page::History => "History",
        Page::Settings => "Settings",
    }
}
fn stat(ui: &mut egui::Ui, label: &str, value: String, color: Color32) {
    ui.group(|ui| {
        ui.set_min_width(142.0);
        ui.label(RichText::new(label).small().color(Color32::GRAY));
        ui.add_space(3.0);
        ui.label(RichText::new(value).size(28.0).strong().color(color));
    });
}
fn empty(ui: &mut egui::Ui, title: &str, body: &str) {
    ui.group(|ui| {
        ui.add_space(6.0);
        ui.label(RichText::new(title).size(16.0).strong());
        ui.label(RichText::new(body).color(Color32::GRAY));
        ui.add_space(6.0);
    });
}
fn history_row(ui: &mut egui::Ui, r: &ExecutionRecord, name: Option<&str>) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.colored_label(
                if r.result.success {
                    Color32::from_rgb(115, 190, 140)
                } else {
                    Color32::from_rgb(230, 120, 110)
                },
                if r.result.success {
                    "SUCCESS"
                } else {
                    "FAILED"
                },
            );
            ui.label(RichText::new(name.unwrap_or(&r.result.automation_id)).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(human_timestamp(r.timestamp))
                        .small()
                        .color(Color32::GRAY),
                )
            });
        });
        ui.label(
            RichText::new(format!("{}  ·  {}", r.result.action, r.result.message))
                .small()
                .color(Color32::GRAY),
        );
    });
}

fn human_timestamp(timestamp: u64) -> String {
    let Some((year, month, day, hour, minute)) = local_datetime(timestamp) else {
        return "Unknown time".into();
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let Some((today_year, today_month, today_day, _, _)) = local_datetime(now) else {
        return format!("{year:04}-{month:02}-{day:02} · {hour:02}:{minute:02}");
    };
    let clock = format!("{hour:02}:{minute:02}");
    if (year, month, day) == (today_year, today_month, today_day) {
        format!("Today · {clock}")
    } else if timestamp.saturating_add(86_400) >= now
        && (year, month, day) != (today_year, today_month, today_day)
    {
        format!("Yesterday · {clock}")
    } else {
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        format!(
            "{} {day} · {clock}",
            MONTHS
                .get(month.saturating_sub(1) as usize)
                .unwrap_or(&"Unknown")
        )
    }
}

#[cfg(windows)]
fn local_datetime(timestamp: u64) -> Option<(u16, u16, u16, u16, u16)> {
    use windows::Win32::{
        Foundation::{FILETIME, SYSTEMTIME},
        System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime},
    };
    let ticks = timestamp
        .checked_mul(10_000)?
        .checked_add(116_444_736_000_000_000)?;
    let utc_file = FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    };
    let mut utc = SYSTEMTIME::default();
    let mut local = SYSTEMTIME::default();
    unsafe { FileTimeToSystemTime(&utc_file, &mut utc).ok()? };
    unsafe { SystemTimeToTzSpecificLocalTime(None, &utc, &mut local).ok()? };
    Some((
        local.wYear,
        local.wMonth,
        local.wDay,
        local.wHour,
        local.wMinute,
    ))
}

#[cfg(not(windows))]
fn local_datetime(timestamp: u64) -> Option<(u16, u16, u16, u16, u16)> {
    let days = timestamp / 86_400;
    let seconds = timestamp % 86_400;
    let (year, month, day) = civil_date(days as i64);
    Some((
        year as u16,
        month as u16,
        day as u16,
        (seconds / 3600) as u16,
        ((seconds % 3600) / 60) as u16,
    ))
}

// Gregorian UTC fallback for non-Windows builds; Windows uses the local timezone API above.
#[cfg(not(windows))]
fn civil_date(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}
fn condition_editor(ui: &mut egui::Ui, node: &mut ConditionNode) {
    let mut kind = match node {
        ConditionNode::Empty => 0,
        ConditionNode::Leaf {
            condition: Condition::TimeRange { .. },
        } => 1,
        ConditionNode::Leaf {
            condition: Condition::BatteryBelow { .. },
        } => 2,
        ConditionNode::And { .. } => 3,
        ConditionNode::Or { .. } => 4,
        ConditionNode::Not { .. } => 5,
    };
    egui::ComboBox::from_id_salt("condition")
        .selected_text(
            [
                "None",
                "Time range",
                "Battery below",
                "All (AND)",
                "Any (OR)",
                "Not",
            ][kind],
        )
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut kind, 0, "None");
            ui.selectable_value(&mut kind, 1, "Time range");
            ui.selectable_value(&mut kind, 2, "Battery below");
            ui.selectable_value(&mut kind, 3, "All (AND)");
            ui.selectable_value(&mut kind, 4, "Any (OR)");
            ui.selectable_value(&mut kind, 5, "Not");
        });
    match (kind, node) {
        (0, n) => *n = ConditionNode::Empty,
        (1, n) => {
            if !matches!(
                n,
                ConditionNode::Leaf {
                    condition: Condition::TimeRange { .. }
                }
            ) {
                *n = ConditionNode::Leaf {
                    condition: Condition::TimeRange {
                        start_hh_mm: "09:00".into(),
                        end_hh_mm: "17:00".into(),
                    },
                }
            }
            if let ConditionNode::Leaf {
                condition:
                    Condition::TimeRange {
                        start_hh_mm,
                        end_hh_mm,
                    },
            } = n
            {
                ui.horizontal(|ui| {
                    ui.label("From");
                    ui.text_edit_singleline(start_hh_mm);
                    ui.label("to");
                    ui.text_edit_singleline(end_hh_mm);
                });
            }
        }
        (2, n) => {
            if !matches!(
                n,
                ConditionNode::Leaf {
                    condition: Condition::BatteryBelow { .. }
                }
            ) {
                *n = ConditionNode::Leaf {
                    condition: Condition::BatteryBelow { percentage: 20 },
                }
            }
            if let ConditionNode::Leaf {
                condition: Condition::BatteryBelow { percentage },
            } = n
            {
                ui.add(egui::Slider::new(percentage, 0..=100).text("Battery %"));
            }
        }
        (3, n) | (4, n) => {
            let is_and = kind == 3;
            if (is_and && !matches!(n, ConditionNode::And { .. }))
                || (!is_and && !matches!(n, ConditionNode::Or { .. }))
            {
                *n = if is_and {
                    ConditionNode::And {
                        children: vec![ConditionNode::Empty],
                    }
                } else {
                    ConditionNode::Or {
                        children: vec![ConditionNode::Empty],
                    }
                };
            }
            let children = match n {
                ConditionNode::And { children } | ConditionNode::Or { children } => children,
                _ => unreachable!(),
            };
            let mut remove = None;
            for (index, child) in children.iter_mut().enumerate() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("Condition {}", index + 1));
                        if ui.button("Remove").clicked() {
                            remove = Some(index);
                        }
                    });
                    ui.push_id(index, |ui| condition_editor(ui, child));
                });
            }
            if let Some(index) = remove {
                children.remove(index);
            }
            if ui.button("Add nested condition").clicked() {
                children.push(ConditionNode::Empty);
            }
        }
        (5, n) => {
            if !matches!(n, ConditionNode::Not { .. }) {
                *n = ConditionNode::Not {
                    child: Box::new(ConditionNode::Empty),
                };
            }
            if let ConditionNode::Not { child } = n {
                ui.push_id("not-child", |ui| condition_editor(ui, child));
            }
        }
        _ => {}
    }
}
fn action_editor(ui: &mut egui::Ui, action: &mut Action) -> bool {
    let mut remove = false;
    ui.group(|ui| {
        if ui.button("Remove action").clicked() {
            remove = true;
        }
        egui::ComboBox::from_id_salt(format!("action{:?}", action))
            .selected_text(match action {
                Action::CleanTemporaryFiles { .. } => "Clean temporary files",
                Action::LaunchApplication { .. } => "Launch application",
                Action::ShowNotification { .. } => "Show notification",
            })
            .show_ui(ui, |ui| {
                if ui.button("Clean temporary files").clicked() {
                    *action = Action::CleanTemporaryFiles { directories: None }
                }
                if ui.button("Launch application").clicked() {
                    *action = Action::LaunchApplication {
                        executable: String::new(),
                        args: vec![],
                    }
                }
                if ui.button("Show notification").clicked() {
                    *action = Action::ShowNotification {
                        title: String::new(),
                        message: String::new(),
                    }
                }
            });
        let _: () = match action {
            Action::CleanTemporaryFiles { directories } => {
                ui.label(
                    "Cleanup removes the contents of each root; the roots themselves are retained.",
                );
                match directories {
                    Some(paths) => {
                        let mut remove = None;
                        for (index, path) in paths.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label("Directory");
                                ui.text_edit_singleline(path);
                                if ui.button("Remove").clicked() {
                                    remove = Some(index);
                                }
                            });
                        }
                        if let Some(index) = remove {
                            paths.remove(index);
                        }
                        if ui.button("Add directory").clicked() {
                            paths.push(String::new());
                        }
                        if paths.is_empty()
                            && ui.button("Use default Windows directories").clicked()
                        {
                            *directories = None;
                        }
                    }
                    None => {
                        ui.label("Uses TEMP, WINDIR\\Temp, and WINDIR\\Prefetch when available.");
                        if ui.button("Use custom directories").clicked() {
                            *directories = Some(vec![String::new()]);
                        }
                    }
                }
            }
            Action::LaunchApplication { executable, args } => {
                ui.horizontal(|ui| {
                    ui.label("Executable");
                    ui.text_edit_singleline(executable);
                });
                let mut joined = args.join(" ");
                if ui
                    .horizontal(|ui| {
                        ui.label("Arguments");
                        ui.text_edit_singleline(&mut joined)
                    })
                    .inner
                    .changed()
                {
                    *args = joined.split_whitespace().map(str::to_owned).collect();
                }
                let _ = ui.label("");
            }
            Action::ShowNotification { title, message } => {
                ui.horizontal(|ui| {
                    ui.label("Title");
                    ui.text_edit_singleline(title);
                });
                ui.horizontal(|ui| {
                    ui.label("Message");
                    ui.text_edit_singleline(message);
                });
                let _ = ui.label("");
            }
        };
    });
    remove
}
