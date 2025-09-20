use flatbuffers::{FlatBufferBuilder, WIPOffset};
use tcrm_task::flatbuffers::conversion::ToFlatbuffers;
use thiserror::Error;

use crate::flatbuffers::conversion::FromFlatbuffers;

use crate::flatbuffers::tcrm_monitor_generated::tcrm::monitor as fb;
use crate::monitor::error::{ControlCommandError, SendStdinErrorReason, TaskMonitorError};

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
impl From<SendStdinErrorReason> for fb::SendStdinErrorReason {
    fn from(reason: SendStdinErrorReason) -> Self {
        match reason {
            SendStdinErrorReason::TaskNotFound => fb::SendStdinErrorReason::TaskNotFound,
            SendStdinErrorReason::StdinNotEnabled => fb::SendStdinErrorReason::StdinNotEnabled,
            SendStdinErrorReason::TaskNotActive => fb::SendStdinErrorReason::TaskNotActive,
            SendStdinErrorReason::ChannelClosed => fb::SendStdinErrorReason::ChannelClosed,
        }
    }
}

impl TryFrom<fb::SendStdinErrorReason> for SendStdinErrorReason {
    type Error = ConversionError;

    fn try_from(reason: fb::SendStdinErrorReason) -> Result<Self, Self::Error> {
        match reason {
            fb::SendStdinErrorReason::TaskNotFound => Ok(SendStdinErrorReason::TaskNotFound),
            fb::SendStdinErrorReason::StdinNotEnabled => Ok(SendStdinErrorReason::StdinNotEnabled),
            fb::SendStdinErrorReason::TaskNotActive => Ok(SendStdinErrorReason::TaskNotActive),
            fb::SendStdinErrorReason::ChannelClosed => Ok(SendStdinErrorReason::ChannelClosed),
            _ => Err(ConversionError::InvalidEnumValue(format!(
                "Invalid SendStdinErrorReason: {:?}",
                reason
            ))),
        }
    }
}

// ControlCommandError conversions (new schema)

impl<'a> ToFlatbuffers<'a> for ControlCommandError {
    type Output = WIPOffset<fb::ControlCommandError<'a>>;

    fn to_flatbuffers(&self, fbb: &mut FlatBufferBuilder<'a>) -> Self::Output {
        let (error_type, error_offset) = match self {
            ControlCommandError::TerminateAllTasks { reason } => {
                let reason_str = fbb.create_string(reason);
                let args = fb::TerminateAllTasksErrorArgs {
                    reason: Some(reason_str),
                };
                let offset = fb::TerminateAllTasksError::create(fbb, &args).as_union_value();
                (fb::ControlCommandErrorUnion::TerminateAllTasks, offset)
            }
            ControlCommandError::TerminateTask { task_name, reason } => {
                let task_name_str = fbb.create_string(task_name);
                let reason_str = fbb.create_string(reason);
                let args = fb::TerminateTaskErrorArgs {
                    task_name: Some(task_name_str),
                    reason: Some(reason_str),
                };
                let offset = fb::TerminateTaskError::create(fbb, &args).as_union_value();
                (fb::ControlCommandErrorUnion::TerminateTask, offset)
            }
            ControlCommandError::SendStdin {
                task_name,
                input,
                reason,
            } => {
                let task_name_str = fbb.create_string(task_name);
                let input_str = fbb.create_string(input);
                let args = fb::SendStdinErrorArgs {
                    task_name: Some(task_name_str),
                    input: Some(input_str),
                    reason: reason.clone().into(),
                };
                let offset = fb::SendStdinError::create(fbb, &args).as_union_value();
                (fb::ControlCommandErrorUnion::SendStdin, offset)
            }
        };
        let args = fb::ControlCommandErrorArgs {
            error_type,
            error: Some(error_offset),
        };
        fb::ControlCommandError::create(fbb, &args)
    }
}

impl TryFrom<fb::ControlCommandError<'_>> for ControlCommandError {
    type Error = ConversionError;

    fn try_from(fb_error: fb::ControlCommandError) -> Result<Self, Self::Error> {
        match fb_error.error_type() {
            fb::ControlCommandErrorUnion::TerminateAllTasks => {
                let tat = fb_error.error_as_terminate_all_tasks().ok_or(
                    ConversionError::MissingRequiredField("TerminateAllTasksError"),
                )?;
                let reason = tat.reason();
                Ok(ControlCommandError::TerminateAllTasks {
                    reason: reason.to_string(),
                })
            }
            fb::ControlCommandErrorUnion::TerminateTask => {
                let tt = fb_error
                    .error_as_terminate_task()
                    .ok_or(ConversionError::MissingRequiredField("TerminateTaskError"))?;
                let task_name = tt.task_name();
                let reason = tt.reason();
                Ok(ControlCommandError::TerminateTask {
                    task_name: task_name.to_string(),
                    reason: reason.to_string(),
                })
            }
            fb::ControlCommandErrorUnion::SendStdin => {
                let ss = fb_error
                    .error_as_send_stdin()
                    .ok_or(ConversionError::MissingRequiredField("SendStdinError"))?;
                let task_name = ss.task_name();
                let input = ss.input();
                let reason = ss.reason();
                Ok(ControlCommandError::SendStdin {
                    task_name: task_name.to_string(),
                    input: input.to_string(),
                    reason: reason.try_into()?,
                })
            }
            _ => Err(ConversionError::MissingRequiredField(
                "control command error variant",
            )),
        }
    }
}

impl FromFlatbuffers<fb::TaskMonitorError<'_>> for TaskMonitorError {
    fn from_flatbuffers(wrapper: fb::TaskMonitorError) -> Result<Self, ConversionError> {
        match wrapper.error_type() {
            fb::TaskMonitorErrorUnion::ConfigParse => {
                if let Some(error) = wrapper.error_as_config_parse() {
                    TaskMonitorError::from_flatbuffers(error)
                } else {
                    Err(ConversionError::MissingRequiredField("ConfigParse error"))
                }
            }
            fb::TaskMonitorErrorUnion::CircularDependency => {
                if let Some(error) = wrapper.error_as_circular_dependency() {
                    TaskMonitorError::from_flatbuffers(error)
                } else {
                    Err(ConversionError::MissingRequiredField(
                        "CircularDependency error",
                    ))
                }
            }
            fb::TaskMonitorErrorUnion::DependencyNotFound => {
                if let Some(error) = wrapper.error_as_dependency_not_found() {
                    TaskMonitorError::from_flatbuffers(error)
                } else {
                    Err(ConversionError::MissingRequiredField(
                        "DependencyNotFound error",
                    ))
                }
            }
            fb::TaskMonitorErrorUnion::ControlError => {
                if let Some(error) = wrapper.error_as_control_error() {
                    TaskMonitorError::from_flatbuffers(error)
                } else {
                    Err(ConversionError::MissingRequiredField("ControlError error"))
                }
            }
            _ => Err(ConversionError::InvalidEnumValue(format!(
                "Unknown TaskMonitorError variant: {:?}",
                wrapper.error_type()
            ))),
        }
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
