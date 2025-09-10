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

#[cfg(test)]
mod tests {
    use crate::flatbuffers::conversion::error::ConversionError;

    #[test]
    fn test_conversion_error_display() {
        let error = ConversionError::TaskConfigConversion("test error".to_string());
        assert_eq!(error.to_string(), "TaskConfig conversion error: test error");

        let error = ConversionError::InvalidShell(42);
        assert_eq!(error.to_string(), "Invalid TaskShell value: 42");

        let error = ConversionError::MissingRequiredField("test_field");
        assert_eq!(error.to_string(), "Missing required field: test_field");
    }
}
