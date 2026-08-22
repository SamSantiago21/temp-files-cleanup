use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct ActionDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub requires_admin: bool,
    pub configuration_schema: serde_json::Value,
}
#[derive(Debug, Clone, Serialize)]
pub struct TriggerDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
}
pub fn available_actions() -> Vec<ActionDefinition> {
    vec![
        ActionDefinition {
            id: "clean_temporary_files".into(),
            name: "Clean Temporary Files".into(),
            description: "Safely removes configured temporary files.".into(),
            category: "System".into(),
            requires_admin: true,
            configuration_schema: serde_json::json!({"type":"object"}),
        },
        ActionDefinition {
            id: "show_notification".into(),
            name: "Show Notification".into(),
            description: "Records a notification.".into(),
            category: "Feedback".into(),
            requires_admin: false,
            configuration_schema: serde_json::json!({"type":"object"}),
        },
    ]
}
pub fn available_triggers() -> Vec<TriggerDefinition> {
    [
        ("manual", "Manual"),
        ("hotkey", "Global Hotkey"),
        ("interval", "Interval"),
        ("daily", "Daily"),
    ]
    .into_iter()
    .map(|(id, name)| TriggerDefinition {
        id: id.into(),
        name: name.into(),
        description: format!("{name} trigger"),
    })
    .collect()
}
