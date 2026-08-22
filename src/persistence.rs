use crate::{domain::Automation, errors::EngineError};
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
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}
pub fn save(path: impl AsRef<Path>, config: &Config) -> Result<(), EngineError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?
    }
    fs::write(path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}
