//! Application-facing shell. The engine remains the source of truth for domain,
//! validation, execution and history; this module only adapts it to a desktop UI.

use crate::{
    domain::{Action, Automation, Condition, ConditionNode, Trigger},
    history::{ExecutionRecord, HistoryFilter, HistoryProvider, JsonlHistory},
    persistence::{self, Config},
    runtime::{RuntimeHandle, RuntimeStatus},
};
use eframe::egui::{self, Color32, RichText};
use std::{path::PathBuf, sync::Arc};

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
                .strong()
                .color(Color32::from_rgb(125, 170, 255)),
        );
        ui.label(
            RichText::new("Windows control center")
                .small()
                .color(Color32::GRAY),
        );
        ui.add_space(28.0);
        for (page, label, icon) in [
            (Page::Dashboard, "Dashboard", "⌂"),
            (Page::Automations, "Automations", "◇"),
            (Page::History, "History", "◷"),
            (Page::Settings, "Settings", "⚙"),
        ] {
            let selected = self.page == page;
            if ui
                .selectable_label(selected, format!("  {icon}  {label}"))
                .clicked()
            {
                self.page = page;
                self.editor = None;
            }
            ui.add_space(5.0);
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            let (label, color) = runtime_display(&self.runtime_status());
            ui.label(RichText::new(label).small().color(color));
            ui.label(
                RichText::new("BACKGROUND RUNTIME")
                    .small()
                    .color(Color32::GRAY),
            );
            ui.label(RichText::new("LOCAL MODE").small().color(Color32::GRAY));
        });
    }

    fn header(&mut self, ui: &mut egui::Ui, title: &str, subtitle: &str) {
        ui.horizontal(|ui| {
            ui.heading(title);
            ui.add_space(12.0);
            ui.label(RichText::new(subtitle).color(Color32::GRAY));
        });
        ui.add_space(16.0);
        if let Some(message) = self.notice.take() {
            ui.colored_label(Color32::from_rgb(115, 190, 140), message);
        }
        if let Some(message) = self.error.take() {
            ui.colored_label(Color32::from_rgb(230, 120, 110), message);
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
        let successful = history
            .iter()
            .filter(|record| record.result.success)
            .count();
        let failed = history
            .iter()
            .filter(|record| !record.result.success)
            .count();
        ui.horizontal(|ui| {
            stat(ui, "TOTAL AUTOMATIONS", total.to_string(), Color32::WHITE);
            stat(
                ui,
                "ENABLED",
                enabled.to_string(),
                Color32::from_rgb(115, 190, 140),
            );
            stat(ui, "DISABLED", (total - enabled).to_string(), Color32::GRAY);
            stat(
                ui,
                "RECENT RUNS",
                history.len().to_string(),
                Color32::from_rgb(125, 170, 255),
            );
            stat(
                ui,
                "SUCCESSFUL",
                successful.to_string(),
                Color32::from_rgb(115, 190, 140),
            );
            stat(
                ui,
                "FAILED",
                failed.to_string(),
                Color32::from_rgb(230, 120, 110),
            );
        });
        let (runtime_label, runtime_color) = runtime_display(&self.runtime_status());
        ui.colored_label(runtime_color, format!("Runtime: {runtime_label}"));
        ui.add_space(26.0);
        ui.heading("Recent activity");
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
            if ui.button("View history").clicked() {
                self.page = Page::History;
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
        let mut action: Option<(String, &'static str)> = None;
        for a in self.config.automations.iter().filter(|a| {
            query.is_empty()
                || a.name.to_lowercase().contains(&query)
                || a.id.to_lowercase().contains(&query)
        }) {
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
            egui::Window::new("Delete automation")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label("Delete this automation from the JSON configuration?");
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
        if self.config.automations.is_empty() {
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
            ui.label("Name");
            ui.text_edit_singleline(&mut a.name);
        });
        ui.add_space(8.0);
        ui.separator();
        ui.heading("Trigger");
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
        ui.heading("Condition");
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
        ui.heading("Actions");
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
            if ui.button("Save automation").clicked() {
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
        for record in records
            .iter()
            .rev()
            .filter(|r| q.is_empty() || format!("{:?}", r).to_lowercase().contains(&q))
        {
            history_row(
                ui,
                record,
                names
                    .iter()
                    .find(|a| a.id == record.result.automation_id)
                    .map(|a| a.name.as_str()),
            );
        }
        if records.is_empty() {
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
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("AUTOMATION DESK").strong());
                let (runtime_label, runtime_color) = runtime_display(&self.runtime_status());
                ui.colored_label(runtime_color, runtime_label);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (runtime_label, runtime_color) = runtime_display(&self.runtime_status());
                    ui.label(RichText::new(runtime_label).small().color(runtime_color));
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
        actions: vec![Action::CleanTemporaryFiles { directories: None }],
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
fn stat(ui: &mut egui::Ui, label: &str, value: String, color: Color32) {
    ui.group(|ui| {
        ui.label(RichText::new(label).small().color(Color32::GRAY));
        ui.label(RichText::new(value).size(26.0).color(color));
    });
}
fn empty(ui: &mut egui::Ui, title: &str, body: &str) {
    ui.add_space(12.0);
    ui.label(RichText::new(title).strong());
    ui.label(RichText::new(body).color(Color32::GRAY));
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
            ui.label(name.unwrap_or(&r.result.automation_id));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{}", r.timestamp))
            });
        });
        ui.label(
            RichText::new(format!("{}  ·  {}", r.result.action, r.result.message))
                .small()
                .color(Color32::GRAY),
        );
    });
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
