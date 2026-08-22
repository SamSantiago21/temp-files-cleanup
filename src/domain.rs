use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Automation {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub trigger: Trigger,
    #[serde(default)]
    pub conditions: ConditionNode,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub settings: AutomationSettings,
}
fn default_true() -> bool {
    true
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Trigger {
    Manual,
    Hotkey { combination: String },
    Interval { seconds: u64 },
    Daily { time_hh_mm: String },
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConditionNode {
    And {
        children: Vec<ConditionNode>,
    },
    Or {
        children: Vec<ConditionNode>,
    },
    Not {
        child: Box<ConditionNode>,
    },
    Leaf {
        condition: Condition,
    },
    #[default]
    Empty,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    TimeRange {
        start_hh_mm: String,
        end_hh_mm: String,
    },
    BatteryBelow {
        percentage: u8,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    CleanTemporaryFiles {
        directories: Option<Vec<String>>,
    },
    LaunchApplication {
        executable: String,
        #[serde(default)]
        args: Vec<String>,
    },
    ShowNotification {
        title: String,
        message: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutomationSettings {
    #[serde(default)]
    pub concurrency_policy: ConcurrencyPolicy,
    #[serde(default)]
    pub failure_policy: FailurePolicy,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConcurrencyPolicy {
    #[default]
    Allow,
    SkipIfRunning,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FailurePolicy {
    #[default]
    Continue,
    Stop,
}
#[derive(Debug, Clone)]
pub struct TriggerFired {
    pub automation_id: String,
    pub source: String,
}
pub const SHUTDOWN_SOURCE: &str = "__engine_shutdown__";
