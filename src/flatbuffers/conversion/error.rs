use flatbuffers::{FlatBufferBuilder, WIPOffset};
use tcrm_task::flatbuffers::conversion::ToFlatbuffers;
use thiserror::Error;

use crate::flatbuffers::tcrm_monitor_generated::tcrm::monitor::{
    ControlCommandError as FbControlCommandError, ControlCommandErrorArgs,
    SendStdinErrorReason as FbSendStdinErrorReason,
};
use crate::monitor::error::{ControlCommandError, SendStdinErrorReason};

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

// SendStdinErrorReason conversions
impl From<SendStdinErrorReason> for FbSendStdinErrorReason {
    fn from(reason: SendStdinErrorReason) -> Self {
        match reason {
            SendStdinErrorReason::TaskNotFound => FbSendStdinErrorReason::TaskNotFound,
            SendStdinErrorReason::StdinNotEnabled => FbSendStdinErrorReason::StdinNotEnabled,
            SendStdinErrorReason::TaskNotActive => FbSendStdinErrorReason::TaskNotActive,
            SendStdinErrorReason::ChannelClosed => FbSendStdinErrorReason::ChannelClosed,
        }
    }
}

impl TryFrom<FbSendStdinErrorReason> for SendStdinErrorReason {
    type Error = ConversionError;

    fn try_from(reason: FbSendStdinErrorReason) -> Result<Self, Self::Error> {
        match reason {
            FbSendStdinErrorReason::TaskNotFound => Ok(SendStdinErrorReason::TaskNotFound),
            FbSendStdinErrorReason::StdinNotEnabled => Ok(SendStdinErrorReason::StdinNotEnabled),
            FbSendStdinErrorReason::TaskNotActive => Ok(SendStdinErrorReason::TaskNotActive),
            FbSendStdinErrorReason::ChannelClosed => Ok(SendStdinErrorReason::ChannelClosed),
            _ => Err(ConversionError::InvalidEnumValue(format!(
                "Invalid SendStdinErrorReason: {:?}",
                reason
            ))),
        }
    }
}

// ControlCommandError conversions
impl<'a> ToFlatbuffers<'a> for ControlCommandError {
    type Output = WIPOffset<FbControlCommandError<'a>>;

    fn to_flatbuffers(&self, fbb: &mut FlatBufferBuilder<'a>) -> Self::Output {
        let (
            terminate_all_tasks_reason,
            terminate_task_name,
            terminate_task_reason,
            send_stdin_task_name,
            send_stdin_input,
            send_stdin_reason,
        ) = match self {
            ControlCommandError::TerminateAllTasks { reason } => (
                Some(fbb.create_string(reason)),
                None,
                None,
                None,
                None,
                FbSendStdinErrorReason::TaskNotFound,
            ),
            ControlCommandError::TerminateTask { task_name, reason } => (
                None,
                Some(fbb.create_string(task_name)),
                Some(fbb.create_string(reason)),
                None,
                None,
                FbSendStdinErrorReason::TaskNotFound,
            ),
            ControlCommandError::SendStdin {
                task_name,
                input,
                reason,
            } => (
                None,
                None,
                None,
                Some(fbb.create_string(task_name)),
                Some(fbb.create_string(input)),
                reason.clone().into(),
            ),
        };

        let args = ControlCommandErrorArgs {
            terminate_all_tasks_reason,
            terminate_task_name,
            terminate_task_reason,
            send_stdin_task_name,
            send_stdin_input,
            send_stdin_reason,
        };

        FbControlCommandError::create(fbb, &args)
    }
}

impl TryFrom<FbControlCommandError<'_>> for ControlCommandError {
    type Error = ConversionError;

    fn try_from(fb_error: FbControlCommandError) -> Result<Self, Self::Error> {
        if let Some(reason) = fb_error.terminate_all_tasks_reason() {
            return Ok(ControlCommandError::TerminateAllTasks {
                reason: reason.to_string(),
            });
        }

        if let (Some(task_name), Some(reason)) = (
            fb_error.terminate_task_name(),
            fb_error.terminate_task_reason(),
        ) {
            return Ok(ControlCommandError::TerminateTask {
                task_name: task_name.to_string(),
                reason: reason.to_string(),
            });
        }

        if let (Some(task_name), Some(input)) =
            (fb_error.send_stdin_task_name(), fb_error.send_stdin_input())
        {
            return Ok(ControlCommandError::SendStdin {
                task_name: task_name.to_string(),
                input: input.to_string(),
                reason: fb_error.send_stdin_reason().try_into()?,
            });
        }

        Err(ConversionError::MissingRequiredField(
            "control command error variant",
        ))
    }
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
