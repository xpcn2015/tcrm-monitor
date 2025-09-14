use thiserror::Error;

/// Error types for `FlatBuffers` conversion operations.
///
/// These errors occur during the conversion between Rust types and `FlatBuffers`
/// binary format. They indicate various failure modes such as missing fields,
/// invalid enum values, or corruption in the serialized data.
#[derive(Debug, Error)]
pub enum ConversionError {
    /// `TaskConfig` conversion failed with the specified reason
    #[error("TaskConfig conversion error: {0}")]
    TaskConfigConversion(String),
    /// Invalid shell type value encountered during conversion
    #[error("Invalid TaskShell value: {0}")]
    InvalidShell(i8),
    /// Required field is missing from the `FlatBuffers` data
    #[error("Missing required field: {0}")]
    MissingRequiredField(&'static str),
    /// Named field is missing from the conversion input
    #[error("Missing field: {0}")]
    MissingField(String),
    /// Enum value is not recognized or valid
    #[error("Invalid enum value: {0}")]
    InvalidEnumValue(String),
    #[error("Unsupported event type: {0}")]
    UnsupportedEventType(String),
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
