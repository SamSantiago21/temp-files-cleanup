use crate::errors::EngineError;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::Path,
    sync::Mutex,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub automation_id: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub timestamp: u64,
    #[serde(flatten)]
    pub result: ExecutionResult,
}
#[derive(Debug, Clone, Default)]
pub struct HistoryFilter {
    pub automation_id: Option<String>,
}
pub trait HistoryProvider: Send + Sync {
    fn record_execution(&self, r: &ExecutionResult) -> Result<(), EngineError>;
    fn get_history(&self, f: HistoryFilter) -> Result<Vec<ExecutionRecord>, EngineError>;
}
pub struct JsonlHistory {
    file: Mutex<File>,
}
impl JsonlHistory {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, EngineError> {
        let p = path.as_ref();
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?
        }
        Ok(Self {
            file: Mutex::new(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .read(true)
                    .open(p)?,
            ),
        })
    }
}
impl HistoryProvider for JsonlHistory {
    fn record_execution(&self, r: &ExecutionResult) -> Result<(), EngineError> {
        let rec = ExecutionRecord {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            result: r.clone(),
        };
        let mut file = self
            .file
            .lock()
            .map_err(|_| EngineError::Action("history lock poisoned".into()))?;
        writeln!(file, "{}", serde_json::to_string(&rec)?)?;
        Ok(())
    }
    fn get_history(&self, f: HistoryFilter) -> Result<Vec<ExecutionRecord>, EngineError> {
        let file = self
            .file
            .lock()
            .map_err(|_| EngineError::Action("history lock poisoned".into()))?
            .try_clone()?;
        let mut out = vec![];
        for line in BufReader::new(file).lines() {
            let rec: ExecutionRecord = serde_json::from_str(&line?)?;
            if f.automation_id
                .as_ref()
                .is_none_or(|id| id == &rec.result.automation_id)
            {
                out.push(rec)
            }
        }
        Ok(out)
    }
}
