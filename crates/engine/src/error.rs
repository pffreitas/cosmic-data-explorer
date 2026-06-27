use thiserror::Error;

pub type Result<T> = std::result::Result<T, EngineError>;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("validation error: {0}")]
    Validation(String),

    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),

    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(&'static str),

    #[error("credential store error: {0}")]
    Credential(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("query timed out after {0} ms")]
    Timeout(u64),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("time parse error: {0}")]
    TimeParse(#[from] chrono::ParseError),

    #[error("highlight error: {0}")]
    Highlight(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(target_os = "macos")]
impl From<keyring::Error> for EngineError {
    fn from(value: keyring::Error) -> Self {
        Self::Credential(value.to_string())
    }
}
