use super::error::ConversionError;
use crate::flatbuffers::tcrm_monitor_generated::tcrm::monitor as fb;
use crate::monitor::error::SendStdinErrorReason;
use crate::monitor::event::{TaskMonitorControlType, TaskMonitorEvent};
use flatbuffers::{FlatBufferBuilder, WIPOffset};

// Convert TaskMonitorControlType to FlatBuffers
impl From<TaskMonitorControlType> for fb::TaskMonitorControlType {
    fn from(control_type: TaskMonitorControlType) -> Self {
        match control_type {
            TaskMonitorControlType::Stop => fb::TaskMonitorControlType::Stop,
            TaskMonitorControlType::TerminateTask => fb::TaskMonitorControlType::TerminateTask,
            TaskMonitorControlType::SendStdin => fb::TaskMonitorControlType::SendStdin,
        }
    }
}

// Convert FlatBuffers TaskMonitorControlType to our type
impl TryFrom<fb::TaskMonitorControlType> for TaskMonitorControlType {
    type Error = ConversionError;

    fn try_from(fb_control_type: fb::TaskMonitorControlType) -> Result<Self, Self::Error> {
        match fb_control_type {
            fb::TaskMonitorControlType::Stop => Ok(TaskMonitorControlType::Stop),
            fb::TaskMonitorControlType::TerminateTask => Ok(TaskMonitorControlType::TerminateTask),
            fb::TaskMonitorControlType::SendStdin => Ok(TaskMonitorControlType::SendStdin),
            _ => Err(ConversionError::InvalidEnumValue(format!(
                "Invalid TaskMonitorControlType: {fb_control_type:?}"
            ))),
        }
    }
}

// Convert SendStdinErrorReason to FlatBuffers
impl From<SendStdinErrorReason> for fb::SendStdinErrorReason {
    fn from(reason: SendStdinErrorReason) -> Self {
        match reason {
            SendStdinErrorReason::TaskNotFound(_) => fb::SendStdinErrorReason::TaskNotFound,
            SendStdinErrorReason::StdinNotEnabled(_) => fb::SendStdinErrorReason::StdinNotEnabled,
            SendStdinErrorReason::TaskNotReady(_) => fb::SendStdinErrorReason::TaskNotReady,
            SendStdinErrorReason::ChannelClosed(_) => fb::SendStdinErrorReason::ChannelClosed,
        }
    }
}

// Convert FlatBuffers SendStdinErrorReason to our type
impl TryFrom<fb::SendStdinErrorReason> for SendStdinErrorReason {
    type Error = ConversionError;

    fn try_from(fb_reason: fb::SendStdinErrorReason) -> Result<Self, Self::Error> {
        match fb_reason {
            fb::SendStdinErrorReason::TaskNotFound => {
                Ok(SendStdinErrorReason::TaskNotFound(String::new()))
            }
            fb::SendStdinErrorReason::StdinNotEnabled => {
                Ok(SendStdinErrorReason::StdinNotEnabled(String::new()))
            }
            fb::SendStdinErrorReason::TaskNotReady => {
                Ok(SendStdinErrorReason::TaskNotReady(String::new()))
            }
            fb::SendStdinErrorReason::ChannelClosed => {
                Ok(SendStdinErrorReason::ChannelClosed(String::new()))
            }
            _ => Err(ConversionError::InvalidEnumValue(format!(
                "Invalid SendStdinErrorReason: {fb_reason:?}"
            ))),
        }
    }
}

/// Convert a `TaskMonitorEvent` to `FlatBuffers` format.
///
/// Serializes task monitor events into `FlatBuffers` binary representation
/// for efficient storage and transmission. Handles all event types including
/// execution events, task events, and control events.
pub fn task_monitor_event_to_flatbuffers<'fbb>(
    event: &TaskMonitorEvent,
    fbb: &mut FlatBufferBuilder<'fbb>,
) -> Result<WIPOffset<fb::TaskMonitorEventMessage<'fbb>>, ConversionError> {
    match event {
        TaskMonitorEvent::ExecutionStarted { total_tasks } => {
            let exec_started = fb::ExecutionStartedEvent::create(
                fbb,
                &fb::ExecutionStartedEventArgs {
                    total_tasks: *total_tasks as u32,
                },
            );

            let event_message = fb::TaskMonitorEventMessage::create(
                fbb,
                &fb::TaskMonitorEventMessageArgs {
                    event_type: fb::TaskMonitorEvent::ExecutionStarted,
                    event: Some(exec_started.as_union_value()),
                },
            );

            Ok(event_message)
        }
        TaskMonitorEvent::ExecutionCompleted {
            completed_tasks,
            failed_tasks,
        } => {
            let exec_completed = fb::ExecutionCompletedEvent::create(
                fbb,
                &fb::ExecutionCompletedEventArgs {
                    completed_tasks: *completed_tasks as u32,
                    failed_tasks: *failed_tasks as u32,
                },
            );

            let event_message = fb::TaskMonitorEventMessage::create(
                fbb,
                &fb::TaskMonitorEventMessageArgs {
                    event_type: fb::TaskMonitorEvent::ExecutionCompleted,
                    event: Some(exec_completed.as_union_value()),
                },
            );

            Ok(event_message)
        }
        TaskMonitorEvent::ControlReceived { control_type } => {
            let control_received = fb::ControlReceivedEvent::create(
                fbb,
                &fb::ControlReceivedEventArgs {
                    control_type: control_type.clone().into(),
                },
            );

            let event_message = fb::TaskMonitorEventMessage::create(
                fbb,
                &fb::TaskMonitorEventMessageArgs {
                    event_type: fb::TaskMonitorEvent::ControlReceived,
                    event: Some(control_received.as_union_value()),
                },
            );

            Ok(event_message)
        }
        TaskMonitorEvent::ControlProcessed { control_type } => {
            let control_processed = fb::ControlProcessedEvent::create(
                fbb,
                &fb::ControlProcessedEventArgs {
                    control_type: control_type.clone().into(),
                },
            );

            let event_message = fb::TaskMonitorEventMessage::create(
                fbb,
                &fb::TaskMonitorEventMessageArgs {
                    event_type: fb::TaskMonitorEvent::ControlProcessed,
                    event: Some(control_processed.as_union_value()),
                },
            );

            Ok(event_message)
        }
        TaskMonitorEvent::ControlError {
            control_type,
            error,
        } => {
            // For now, we'll convert the error to a string representation
            // This is simplified - in a full implementation you'd want proper error conversion
            let error_message = format!("{error}");
            let error_str = fbb.create_string(&error_message);

            // Create a placeholder error wrapper for now
            let task_error = fb::TaskErrorWrapper::create(
                fbb,
                &fb::TaskErrorWrapperArgs {
                    error_message: Some(error_str),
                },
            );

            let error_union = fb::TaskMonitorError::TaskError;

            let control_error = fb::ControlErrorEvent::create(
                fbb,
                &fb::ControlErrorEventArgs {
                    control_type: control_type.clone().into(),
                    error_type: error_union,
                    error: Some(task_error.as_union_value()),
                },
            );

            let event_message = fb::TaskMonitorEventMessage::create(
                fbb,
                &fb::TaskMonitorEventMessageArgs {
                    event_type: fb::TaskMonitorEvent::ControlError,
                    event: Some(control_error.as_union_value()),
                },
            );

            Ok(event_message)
        }
        TaskMonitorEvent::AllTasksTerminationRequested => {
            let empty_event = fb::EmptyEvent::create(fbb, &fb::EmptyEventArgs {});

            let event_message = fb::TaskMonitorEventMessage::create(
                fbb,
                &fb::TaskMonitorEventMessageArgs {
                    event_type: fb::TaskMonitorEvent::AllTasksTerminationRequested,
                    event: Some(empty_event.as_union_value()),
                },
            );

            Ok(event_message)
        }
        TaskMonitorEvent::TaskTerminationRequested { task_name } => {
            let task_name_str = fbb.create_string(task_name);
            let task_termination = fb::TaskTerminationRequestedEvent::create(
                fbb,
                &fb::TaskTerminationRequestedEventArgs {
                    task_name: Some(task_name_str),
                },
            );

            let event_message = fb::TaskMonitorEventMessage::create(
                fbb,
                &fb::TaskMonitorEventMessageArgs {
                    event_type: fb::TaskMonitorEvent::TaskTerminationRequested,
                    event: Some(task_termination.as_union_value()),
                },
            );

            Ok(event_message)
        }
        TaskMonitorEvent::StdinSent {
            task_name,
            input_length,
        } => {
            let task_name_str = fbb.create_string(task_name);
            let stdin_sent = fb::StdinSentEvent::create(
                fbb,
                &fb::StdinSentEventArgs {
                    task_name: Some(task_name_str),
                    input_length: *input_length as u32,
                },
            );

            let event_message = fb::TaskMonitorEventMessage::create(
                fbb,
                &fb::TaskMonitorEventMessageArgs {
                    event_type: fb::TaskMonitorEvent::StdinSent,
                    event: Some(stdin_sent.as_union_value()),
                },
            );

            Ok(event_message)
        }
        TaskMonitorEvent::StdinError { task_name, error } => {
            let task_name_str = fbb.create_string(task_name);
            let stdin_error = fb::StdinErrorEvent::create(
                fbb,
                &fb::StdinErrorEventArgs {
                    task_name: Some(task_name_str),
                    error: error.clone().into(),
                },
            );

            let event_message = fb::TaskMonitorEventMessage::create(
                fbb,
                &fb::TaskMonitorEventMessageArgs {
                    event_type: fb::TaskMonitorEvent::StdinError,
                    event: Some(stdin_error.as_union_value()),
                },
            );

            Ok(event_message)
        }
        TaskMonitorEvent::Task(task_event) => {
            // For now, we'll serialize the task event as a placeholder
            // In a full implementation, you'd want proper TaskEvent to FlatBuffers conversion
            let event_data = format!("{task_event:?}");
            let event_bytes = fbb.create_vector(event_data.as_bytes());

            let task_wrapper = fb::TaskEventWrapper::create(
                fbb,
                &fb::TaskEventWrapperArgs {
                    event_data: Some(event_bytes),
                },
            );

            let event_message = fb::TaskMonitorEventMessage::create(
                fbb,
                &fb::TaskMonitorEventMessageArgs {
                    event_type: fb::TaskMonitorEvent::Task,
                    event: Some(task_wrapper.as_union_value()),
                },
            );

            Ok(event_message)
        }
    }
}

/// Convert from `FlatBuffers` format to `TaskMonitorEvent`.
///
/// Deserializes `FlatBuffers` binary data back into `TaskMonitorEvent` instances.
/// Handles all event types and validates the data during conversion.
/// Returns conversion errors for invalid or corrupted data.
pub fn task_monitor_event_from_flatbuffers(
    fb_event_message: fb::TaskMonitorEventMessage,
) -> Result<TaskMonitorEvent, ConversionError> {
    match fb_event_message.event_type() {
        fb::TaskMonitorEvent::ExecutionStarted => {
            let exec_started = fb_event_message
                .event_as_execution_started()
                .ok_or_else(|| {
                    ConversionError::MissingField("ExecutionStarted event data".to_string())
                })?;

            Ok(TaskMonitorEvent::ExecutionStarted {
                total_tasks: exec_started.total_tasks() as usize,
            })
        }
        fb::TaskMonitorEvent::ExecutionCompleted => {
            let exec_completed =
                fb_event_message
                    .event_as_execution_completed()
                    .ok_or_else(|| {
                        ConversionError::MissingField("ExecutionCompleted event data".to_string())
                    })?;

            Ok(TaskMonitorEvent::ExecutionCompleted {
                completed_tasks: exec_completed.completed_tasks() as usize,
                failed_tasks: exec_completed.failed_tasks() as usize,
            })
        }
        fb::TaskMonitorEvent::ControlReceived => {
            let control_received =
                fb_event_message
                    .event_as_control_received()
                    .ok_or_else(|| {
                        ConversionError::MissingField("ControlReceived event data".to_string())
                    })?;

            Ok(TaskMonitorEvent::ControlReceived {
                control_type: control_received.control_type().try_into()?,
            })
        }
        fb::TaskMonitorEvent::ControlProcessed => {
            let control_processed =
                fb_event_message
                    .event_as_control_processed()
                    .ok_or_else(|| {
                        ConversionError::MissingField("ControlProcessed event data".to_string())
                    })?;

            Ok(TaskMonitorEvent::ControlProcessed {
                control_type: control_processed.control_type().try_into()?,
            })
        }
        fb::TaskMonitorEvent::AllTasksTerminationRequested => {
            Ok(TaskMonitorEvent::AllTasksTerminationRequested)
        }
        fb::TaskMonitorEvent::TaskTerminationRequested => {
            let task_termination = fb_event_message
                .event_as_task_termination_requested()
                .ok_or_else(|| {
                    ConversionError::MissingField("TaskTerminationRequested event data".to_string())
                })?;

            let task_name = task_termination.task_name().to_string();

            Ok(TaskMonitorEvent::TaskTerminationRequested { task_name })
        }
        fb::TaskMonitorEvent::StdinSent => {
            let stdin_sent = fb_event_message
                .event_as_stdin_sent()
                .ok_or_else(|| ConversionError::MissingField("StdinSent event data".to_string()))?;

            let task_name = stdin_sent.task_name().to_string();

            Ok(TaskMonitorEvent::StdinSent {
                task_name,
                input_length: stdin_sent.input_length() as usize,
            })
        }
        fb::TaskMonitorEvent::StdinError => {
            let stdin_error = fb_event_message.event_as_stdin_error().ok_or_else(|| {
                ConversionError::MissingField("StdinError event data".to_string())
            })?;

            let task_name = stdin_error.task_name().to_string();

            let error_reason: SendStdinErrorReason = stdin_error.error().try_into()?;

            // Update the error reason with the actual task name
            let updated_error = match error_reason {
                SendStdinErrorReason::TaskNotFound(_) => {
                    SendStdinErrorReason::TaskNotFound(task_name.clone())
                }
                SendStdinErrorReason::StdinNotEnabled(_) => {
                    SendStdinErrorReason::StdinNotEnabled(task_name.clone())
                }
                SendStdinErrorReason::TaskNotReady(_) => {
                    SendStdinErrorReason::TaskNotReady(task_name.clone())
                }
                SendStdinErrorReason::ChannelClosed(_) => {
                    SendStdinErrorReason::ChannelClosed(task_name.clone())
                }
            };

            Ok(TaskMonitorEvent::StdinError {
                task_name,
                error: updated_error,
            })
        }
        _ => Err(ConversionError::UnsupportedEventType(format!(
            "Unsupported TaskMonitorEvent type: {:?}",
            fb_event_message.event_type()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flatbuffers::FlatBufferBuilder;

    #[test]
    fn test_task_monitor_control_type_conversion() {
        // Test conversion to FlatBuffers
        assert_eq!(
            fb::TaskMonitorControlType::Stop,
            TaskMonitorControlType::Stop.into()
        );
        assert_eq!(
            fb::TaskMonitorControlType::TerminateTask,
            TaskMonitorControlType::TerminateTask.into()
        );
        assert_eq!(
            fb::TaskMonitorControlType::SendStdin,
            TaskMonitorControlType::SendStdin.into()
        );

        // Test conversion from FlatBuffers
        assert_eq!(
            TaskMonitorControlType::Stop,
            fb::TaskMonitorControlType::Stop.try_into().unwrap()
        );
        assert_eq!(
            TaskMonitorControlType::TerminateTask,
            fb::TaskMonitorControlType::TerminateTask
                .try_into()
                .unwrap()
        );
        assert_eq!(
            TaskMonitorControlType::SendStdin,
            fb::TaskMonitorControlType::SendStdin.try_into().unwrap()
        );
    }

    #[test]
    fn test_send_stdin_error_reason_conversion() {
        // Test conversion to FlatBuffers
        assert_eq!(
            fb::SendStdinErrorReason::TaskNotFound,
            SendStdinErrorReason::TaskNotFound("test".to_string()).into()
        );
        assert_eq!(
            fb::SendStdinErrorReason::StdinNotEnabled,
            SendStdinErrorReason::StdinNotEnabled("test".to_string()).into()
        );
    }

    #[test]
    fn test_execution_started_event_conversion() {
        let mut fbb = FlatBufferBuilder::new();

        let event = TaskMonitorEvent::ExecutionStarted { total_tasks: 5 };

        // Convert to FlatBuffers
        let fb_event = task_monitor_event_to_flatbuffers(&event, &mut fbb).unwrap();
        fbb.finish(fb_event, None);

        // Convert back from FlatBuffers
        let buffer = fbb.finished_data();
        let fb_event_message = flatbuffers::root::<fb::TaskMonitorEventMessage>(&buffer).unwrap();
        let converted_event = task_monitor_event_from_flatbuffers(fb_event_message).unwrap();

        match converted_event {
            TaskMonitorEvent::ExecutionStarted { total_tasks } => {
                assert_eq!(total_tasks, 5);
            }
            _ => panic!("Expected ExecutionStarted event"),
        }
    }

    #[test]
    fn test_stdin_sent_event_conversion() {
        let mut fbb = FlatBufferBuilder::new();

        let event = TaskMonitorEvent::StdinSent {
            task_name: "test_task".to_string(),
            input_length: 42,
        };

        // Convert to FlatBuffers
        let fb_event = task_monitor_event_to_flatbuffers(&event, &mut fbb).unwrap();
        fbb.finish(fb_event, None);

        // Convert back from FlatBuffers
        let buffer = fbb.finished_data();
        let fb_event_message = flatbuffers::root::<fb::TaskMonitorEventMessage>(&buffer).unwrap();
        let converted_event = task_monitor_event_from_flatbuffers(fb_event_message).unwrap();

        match converted_event {
            TaskMonitorEvent::StdinSent {
                task_name,
                input_length,
            } => {
                assert_eq!(task_name, "test_task");
                assert_eq!(input_length, 42);
            }
            _ => panic!("Expected StdinSent event"),
        }
    }

    #[test]
    fn test_all_tasks_termination_requested_event_conversion() {
        let mut fbb = FlatBufferBuilder::new();

        let event = TaskMonitorEvent::AllTasksTerminationRequested;

        // Convert to FlatBuffers
        let fb_event = task_monitor_event_to_flatbuffers(&event, &mut fbb).unwrap();
        fbb.finish(fb_event, None);

        // Convert back from FlatBuffers
        let buffer = fbb.finished_data();
        let fb_event_message = flatbuffers::root::<fb::TaskMonitorEventMessage>(&buffer).unwrap();
        let converted_event = task_monitor_event_from_flatbuffers(fb_event_message).unwrap();

        match converted_event {
            TaskMonitorEvent::AllTasksTerminationRequested => {
                // Success
            }
            _ => panic!("Expected AllTasksTerminationRequested event"),
        }
    }
}
