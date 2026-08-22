use thiserror::Error;
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("configuration error: {0}")]
    Configuration(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid time: {0}")]
    InvalidTime(String),
    #[error("action failed: {0}")]
    Action(String),
    #[error("trigger channel closed")]
    ChannelClosed,
}
