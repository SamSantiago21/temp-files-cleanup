use crate::{
    conditions::parse_minutes,
    domain::{Action, Automation, Condition, ConditionNode, Trigger},
    errors::EngineError,
};
use std::{fs, path::Path};
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub schema_version: u32,
    pub automations: Vec<Automation>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: 1,
            automations: vec![],
        }
    }
}
pub fn load(path: impl AsRef<Path>) -> Result<Config, EngineError> {
    let config: Config = serde_json::from_str(&fs::read_to_string(path)?)?;
    validate(&config)?;
    Ok(config)
}
pub fn save(path: impl AsRef<Path>, config: &Config) -> Result<(), EngineError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?
    }
    validate(config)?;
    fs::write(path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}

pub fn validate(config: &Config) -> Result<(), EngineError> {
    if config.schema_version != 1 {
        return Err(EngineError::InvalidConfiguration(format!(
            "unsupported schema version {}",
            config.schema_version
        )));
    }
    let mut ids = std::collections::HashSet::new();
    for automation in &config.automations {
        if automation.id.trim().is_empty() || !ids.insert(&automation.id) {
            return Err(EngineError::InvalidConfiguration(
                "automation IDs must be non-empty and unique".into(),
            ));
        }
        if automation.actions.is_empty() {
            return Err(EngineError::InvalidConfiguration(format!(
                "automation '{}' has no actions",
                automation.id
            )));
        }
        match &automation.trigger {
            Trigger::Interval { seconds: 0 } => {
                return Err(EngineError::InvalidConfiguration(format!(
                    "automation '{}' has a zero interval",
                    automation.id
                )));
            }
            Trigger::Daily { time_hh_mm } => {
                parse_minutes(time_hh_mm)?;
            }
            Trigger::Hotkey { combination } if combination.trim().is_empty() => {
                return Err(EngineError::InvalidConfiguration(format!(
                    "automation '{}' has an empty hotkey",
                    automation.id
                )));
            }
            _ => {}
        }
        validate_conditions(&automation.conditions)?;
        for action in &automation.actions {
            if let Action::LaunchApplication { executable, .. } = action
                && executable.trim().is_empty()
            {
                return Err(EngineError::InvalidConfiguration(format!(
                    "automation '{}' has an empty executable",
                    automation.id
                )));
            }
        }
    }
    Ok(())
}

fn validate_conditions(node: &ConditionNode) -> Result<(), EngineError> {
    match node {
        ConditionNode::And { children } | ConditionNode::Or { children } => {
            for child in children {
                validate_conditions(child)?;
            }
        }
        ConditionNode::Not { child } => validate_conditions(child)?,
        ConditionNode::Leaf {
            condition:
                Condition::TimeRange {
                    start_hh_mm,
                    end_hh_mm,
                },
        } => {
            parse_minutes(start_hh_mm)?;
            parse_minutes(end_hh_mm)?;
        }
        ConditionNode::Leaf {
            condition: Condition::BatteryBelow { percentage },
        } if *percentage > 100 => {
            return Err(EngineError::InvalidConfiguration(
                "battery percentage must be between 0 and 100".into(),
            ));
        }
        ConditionNode::Empty | ConditionNode::Leaf { .. } => {}
    }
    Ok(())
}
