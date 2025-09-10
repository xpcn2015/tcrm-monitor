use thiserror::Error;

/// Conversion error for flatbuffers operations
#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("TaskConfig conversion error: {0}")]
    TaskConfigConversion(String),
    #[error("Invalid TaskShell value: {0}")]
    InvalidShell(i8),
    #[error("Missing required field: {0}")]
    MissingRequiredField(&'static str),
}
