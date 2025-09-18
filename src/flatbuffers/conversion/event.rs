use flatbuffers::{FlatBufferBuilder, WIPOffset};
use tcrm_task::flatbuffers::conversion::{ToFlatbuffers, ToFlatbuffersUnion};

use crate::flatbuffers::conversion::error::ConversionError;
use crate::flatbuffers::conversion::FromFlatbuffers;
use crate::flatbuffers::tcrm_monitor_generated::tcrm::monitor as fb;
use crate::monitor::error::{TaskMonitorError, ControlCommandError};
use crate::monitor::event::{TaskMonitorControlEvent, TaskMonitorEvent, TaskMonitorControlCommand};

impl<'a> ToFlatbuffers<'a> for TaskMonitorEvent {
    type Output = WIPOffset<fb::TaskMonitorEventMessage<'a>>;

    fn to_flatbuffers(&self, fbb: &mut FlatBufferBuilder<'a>) -> Self::Output {
        let (event_type, event_offset) = match self {
            TaskMonitorEvent::Task(task_event) => {
                let task_event_offset = task_event.to_flatbuffers(fbb);
                (
                    fb::TaskMonitorEvent::Task,
                    task_event_offset.as_union_value(),
                )
            }
            TaskMonitorEvent::Started { total_tasks } => {
                let started_event = fb::ExecutionStartedEvent::create(
                    fbb,
                    &fb::ExecutionStartedEventArgs {
                        total_tasks: *total_tasks as u64,
                    },
                );
                (
                    fb::TaskMonitorEvent::Started,
                    started_event.as_union_value(),
                )
            }
            TaskMonitorEvent::Completed {
                completed_tasks,
                failed_tasks,
            } => {
                let completed_event = fb::ExecutionCompletedEvent::create(
                    fbb,
                    &fb::ExecutionCompletedEventArgs {
                        completed_tasks: *completed_tasks as u64,
                        failed_tasks: *failed_tasks as u64,
                    },
                );
                (
                    fb::TaskMonitorEvent::Completed,
                    completed_event.as_union_value(),
                )
            }
            TaskMonitorEvent::Control(control_event) => {
                let control_wrapper = control_event.to_flatbuffers(fbb);
                (
                    fb::TaskMonitorEvent::Control,
                    control_wrapper.as_union_value(),
                )
            }
            TaskMonitorEvent::Error(error) => {
                let error_wrapper = error.to_flatbuffers(fbb);
                (
                    fb::TaskMonitorEvent::Error,
                    error_wrapper.as_union_value(),
                )
            }
        };

        fb::TaskMonitorEventMessage::create(
            fbb,
            &fb::TaskMonitorEventMessageArgs {
                event_type,
                event: Some(event_offset),
            },
        )
    }
}

impl FromFlatbuffers<fb::TaskMonitorEventMessage<'_>> for TaskMonitorEvent {
    fn from_flatbuffers(
        fb_event_message: fb::TaskMonitorEventMessage,
    ) -> Result<Self, ConversionError> {
        let event_type = fb_event_message.event_type();

        match event_type {
            fb::TaskMonitorEvent::Task => {
                if let Some(task_event) = fb_event_message.event_as_task() {
                    // Convert FlatBuffers TaskEvent to tcrm_task TaskEvent
                    let task_event = convert_flatbuffers_to_task_event(task_event)?;
                    Ok(TaskMonitorEvent::Task(task_event))
                } else {
                    Err(ConversionError::MissingRequiredField("task event"))
                }
            }
            fb::TaskMonitorEvent::Started => {
                if let Some(started_event) = fb_event_message.event_as_started() {
                    Ok(TaskMonitorEvent::Started {
                        total_tasks: started_event.total_tasks() as usize,
                    })
                } else {
                    Err(ConversionError::MissingRequiredField("started event"))
                }
            }
            fb::TaskMonitorEvent::Completed => {
                if let Some(completed_event) = fb_event_message.event_as_completed() {
                    Ok(TaskMonitorEvent::Completed {
                        completed_tasks: completed_event.completed_tasks() as usize,
                        failed_tasks: completed_event.failed_tasks() as usize,
                    })
                } else {
                    Err(ConversionError::MissingRequiredField("completed event"))
                }
            }
            fb::TaskMonitorEvent::Control => {
                if let Some(control_wrapper) = fb_event_message.event_as_control() {
                    let control_event = TaskMonitorControlEvent::from_flatbuffers(control_wrapper)?;
                    Ok(TaskMonitorEvent::Control(control_event))
                } else {
                    Err(ConversionError::MissingRequiredField("control event"))
                }
            }
            fb::TaskMonitorEvent::Error => {
                if let Some(error_wrapper) = fb_event_message.event_as_error() {
                    let error = TaskMonitorError::from_flatbuffers(error_wrapper)?;
                    Ok(TaskMonitorEvent::Error(error))
                } else {
                    Err(ConversionError::MissingRequiredField("error event"))
                }
            }
            _ => Err(ConversionError::UnsupportedEventType(format!(
                "Unsupported TaskMonitorEvent variant: {:?}",
                event_type
            ))),
        }
    }
}

impl<'a> ToFlatbuffersUnion<'a, fb::TaskMonitorControlCommand> for TaskMonitorControlCommand {
    fn to_flatbuffers_union(
        &self,
        fbb: &mut FlatBufferBuilder<'a>,
    ) -> (fb::TaskMonitorControlCommand, WIPOffset<flatbuffers::UnionWIPOffset>) {
        match self {
            TaskMonitorControlCommand::TerminateAllTasks => {
                let reason_offset = fbb.create_string("User requested termination");
                let terminate_all_command = fb::TerminateAllTasksCommand::create(
                    fbb,
                    &fb::TerminateAllTasksCommandArgs {
                        reason: Some(reason_offset),
                    },
                );
                (
                    fb::TaskMonitorControlCommand::TerminateAllTasks,
                    terminate_all_command.as_union_value(),
                )
            }
            TaskMonitorControlCommand::TerminateTask { task_name } => {
                let task_name_offset = fbb.create_string(task_name);
                let reason_offset = fbb.create_string("User requested termination");
                let terminate_task_command = fb::TerminateTaskCommand::create(
                    fbb,
                    &fb::TerminateTaskCommandArgs {
                        task_name: Some(task_name_offset),
                        reason: Some(reason_offset),
                    },
                );
                (
                    fb::TaskMonitorControlCommand::TerminateTask,
                    terminate_task_command.as_union_value(),
                )
            }
            TaskMonitorControlCommand::SendStdin { task_name, input } => {
                let task_name_offset = fbb.create_string(task_name);
                let input_offset = fbb.create_string(input);
                let send_stdin_command = fb::SendStdinCommand::create(
                    fbb,
                    &fb::SendStdinCommandArgs {
                        task_name: Some(task_name_offset),
                        input: Some(input_offset),
                    },
                );
                (
                    fb::TaskMonitorControlCommand::SendStdin,
                    send_stdin_command.as_union_value(),
                )
            }
        }
    }
}

impl<'a> ToFlatbuffers<'a> for TaskMonitorControlCommand {
    type Output = WIPOffset<flatbuffers::UnionWIPOffset>;

    fn to_flatbuffers(&self, fbb: &mut FlatBufferBuilder<'a>) -> Self::Output {
        let (_discriminant, offset) = self.to_flatbuffers_union(fbb);
        offset
    }
}

impl<'a> ToFlatbuffers<'a> for TaskMonitorControlEvent {
    type Output = WIPOffset<fb::TaskMonitorControlEventWrapper<'a>>;

    fn to_flatbuffers(&self, fbb: &mut FlatBufferBuilder<'a>) -> Self::Output {
        match self {
            TaskMonitorControlEvent::ControlReceived { control } => {
                let (control_type, control_union) = control.to_flatbuffers_union(fbb);
                let control_received_event = fb::ControlReceivedEvent::create(
                    fbb,
                    &fb::ControlReceivedEventArgs {
                        control_type,
                        control: Some(control_union),
                    },
                );

                fb::TaskMonitorControlEventWrapper::create(
                    fbb,
                    &fb::TaskMonitorControlEventWrapperArgs {
                        control_event_type: fb::TaskMonitorControlEvent::ControlReceived,
                        control_event: Some(control_received_event.as_union_value()),
                    },
                )
            }
            TaskMonitorControlEvent::ControlProcessed { control } => {
                let (control_type, control_union) = control.to_flatbuffers_union(fbb);
                let control_processed_event = fb::ControlProcessedEvent::create(
                    fbb,
                    &fb::ControlProcessedEventArgs {
                        control_type,
                        control: Some(control_union),
                    },
                );

                fb::TaskMonitorControlEventWrapper::create(
                    fbb,
                    &fb::TaskMonitorControlEventWrapperArgs {
                        control_event_type: fb::TaskMonitorControlEvent::ControlProcessed,
                        control_event: Some(control_processed_event.as_union_value()),
                    },
                )
            }
        }
    }
}

impl FromFlatbuffers<fb::TaskMonitorControlEventWrapper<'_>> for TaskMonitorControlEvent {
    fn from_flatbuffers(wrapper: fb::TaskMonitorControlEventWrapper) -> Result<Self, ConversionError> {
        match wrapper.control_event_type() {
            fb::TaskMonitorControlEvent::ControlReceived => {
                if let Some(control_received_event) = wrapper.control_event_as_control_received() {
                    let control = convert_control_command_from_flatbuffers(
                        control_received_event.control_type(),
                        control_received_event.control()
                    )?;
                    Ok(TaskMonitorControlEvent::ControlReceived { control })
                } else {
                    Err(ConversionError::InvalidEnumValue("ControlReceived event variant mismatch".to_string()))
                }
            }
            fb::TaskMonitorControlEvent::ControlProcessed => {
                if let Some(control_processed_event) = wrapper.control_event_as_control_processed() {
                    let control = convert_control_command_from_flatbuffers(
                        control_processed_event.control_type(),
                        control_processed_event.control()
                    )?;
                    Ok(TaskMonitorControlEvent::ControlProcessed { control })
                } else {
                    Err(ConversionError::InvalidEnumValue("ControlProcessed event variant mismatch".to_string()))
                }
            }
            _ => Err(ConversionError::InvalidEnumValue(format!("Unknown TaskMonitorControlEvent variant: {:?}", wrapper.control_event_type()))),
        }
    }
}

impl<'a> ToFlatbuffers<'a> for TaskMonitorError {
    type Output = WIPOffset<fb::TaskMonitorErrorWrapper<'a>>;

    fn to_flatbuffers(&self, fbb: &mut FlatBufferBuilder<'a>) -> Self::Output {
        match self {
            TaskMonitorError::ConfigParse(message) => {
                let message_offset = fbb.create_string(message);
                let config_parse_error = fb::ConfigParseError::create(
                    fbb,
                    &fb::ConfigParseErrorArgs {
                        message: Some(message_offset),
                    },
                );
                
                fb::TaskMonitorErrorWrapper::create(
                    fbb,
                    &fb::TaskMonitorErrorWrapperArgs {
                        error_type: fb::TaskMonitorError::ConfigParse,
                        error: Some(config_parse_error.as_union_value()),
                    },
                )
            }
            TaskMonitorError::CircularDependency(task_name) => {
                let task_name_offset = fbb.create_string(task_name);
                let circular_dependency_error = fb::CircularDependencyError::create(
                    fbb,
                    &fb::CircularDependencyErrorArgs {
                        task_name: Some(task_name_offset),
                    },
                );
                
                fb::TaskMonitorErrorWrapper::create(
                    fbb,
                    &fb::TaskMonitorErrorWrapperArgs {
                        error_type: fb::TaskMonitorError::CircularDependency,
                        error: Some(circular_dependency_error.as_union_value()),
                    },
                )
            }
            TaskMonitorError::DependencyNotFound { dependency_task_name, task_name } => {
                let dependency_task_name_offset = fbb.create_string(dependency_task_name);
                let task_name_offset = fbb.create_string(task_name);
                let dependency_not_found_error = fb::DependencyNotFoundError::create(
                    fbb,
                    &fb::DependencyNotFoundErrorArgs {
                        dependency_task_name: Some(dependency_task_name_offset),
                        task_name: Some(task_name_offset),
                    },
                );
                
                fb::TaskMonitorErrorWrapper::create(
                    fbb,
                    &fb::TaskMonitorErrorWrapperArgs {
                        error_type: fb::TaskMonitorError::DependencyNotFound,
                        error: Some(dependency_not_found_error.as_union_value()),
                    },
                )
            }
            TaskMonitorError::ControlError(control_error) => {
                let control_command_error = control_error.to_flatbuffers(fbb);
                
                fb::TaskMonitorErrorWrapper::create(
                    fbb,
                    &fb::TaskMonitorErrorWrapperArgs {
                        error_type: fb::TaskMonitorError::ControlError,
                        error: Some(control_command_error.as_union_value()),
                    },
                )
            }
        }
    }
}

impl FromFlatbuffers<fb::TaskMonitorErrorWrapper<'_>> for TaskMonitorError {
    fn from_flatbuffers(wrapper: fb::TaskMonitorErrorWrapper) -> Result<Self, ConversionError> {
        match wrapper.error_type() {
            fb::TaskMonitorError::ConfigParse => {
                if let Some(config_parse_error) = wrapper.error_as_config_parse() {
                    let message = config_parse_error.message().to_string();
                    Ok(TaskMonitorError::ConfigParse(message))
                } else {
                    Err(ConversionError::InvalidEnumValue("ConfigParse error variant mismatch".to_string()))
                }
            }
            fb::TaskMonitorError::CircularDependency => {
                if let Some(circular_dependency_error) = wrapper.error_as_circular_dependency() {
                    let task_name = circular_dependency_error.task_name().to_string();
                    Ok(TaskMonitorError::CircularDependency(task_name))
                } else {
                    Err(ConversionError::InvalidEnumValue("CircularDependency error variant mismatch".to_string()))
                }
            }
            fb::TaskMonitorError::DependencyNotFound => {
                if let Some(dependency_not_found_error) = wrapper.error_as_dependency_not_found() {
                    let dependency_task_name = dependency_not_found_error.dependency_task_name().to_string();
                    let task_name = dependency_not_found_error.task_name().to_string();
                    Ok(TaskMonitorError::DependencyNotFound { dependency_task_name, task_name })
                } else {
                    Err(ConversionError::InvalidEnumValue("DependencyNotFound error variant mismatch".to_string()))
                }
            }
            fb::TaskMonitorError::ControlError => {
                if let Some(control_command_error) = wrapper.error_as_control_error() {
                    let control_error = ControlCommandError::try_from(control_command_error)?;
                    Ok(TaskMonitorError::ControlError(control_error))
                } else {
                    Err(ConversionError::InvalidEnumValue("ControlError error variant mismatch".to_string()))
                }
            }
            _ => Err(ConversionError::InvalidEnumValue(format!("Unknown TaskMonitorError variant: {:?}", wrapper.error_type()))),
        }
    }
}

/// Converts a FlatBuffers TaskEvent to tcrm_task TaskEvent
fn convert_flatbuffers_to_task_event(
    fb_task_event: tcrm_task::flatbuffers::tcrm_task_generated::tcrm::task::TaskEvent,
) -> Result<tcrm_task::tasks::event::TaskEvent, ConversionError> {
    use tcrm_task::flatbuffers::tcrm_task_generated::tcrm::task as tcrm_task_fb;
    use tcrm_task::tasks::config::StreamSource;
    use tcrm_task::tasks::event::{TaskEvent, TaskEventStopReason, TaskTerminateReason};

    // Get the event type and event union
    let event_type = fb_task_event.event_type();
    let event = fb_task_event.event().ok_or(ConversionError::MissingRequiredField("event"))?;

    match event_type {
        tcrm_task_fb::TaskEventUnion::Started => {
            let started_event = unsafe { tcrm_task_fb::StartedEvent::init_from_table(event) };
            let task_name = started_event.task_name().to_string();
            Ok(TaskEvent::Started { task_name })
        }
        tcrm_task_fb::TaskEventUnion::Output => {
            let output_event = unsafe { tcrm_task_fb::OutputEvent::init_from_table(event) };
            let task_name = output_event.task_name().to_string();
            let line = output_event.line().to_string();
            let src = match output_event.src() {
                tcrm_task_fb::StreamSource::Stdout => StreamSource::Stdout,
                tcrm_task_fb::StreamSource::Stderr => StreamSource::Stderr,
                _ => {
                    return Err(ConversionError::InvalidEnumValue(format!(
                        "Invalid StreamSource: {:?}",
                        output_event.src()
                    )))
                }
            };
            Ok(TaskEvent::Output {
                task_name,
                line,
                src,
            })
        }
        tcrm_task_fb::TaskEventUnion::Ready => {
            let ready_event = unsafe { tcrm_task_fb::ReadyEvent::init_from_table(event) };
            let task_name = ready_event.task_name().to_string();
            Ok(TaskEvent::Ready { task_name })
        }
        tcrm_task_fb::TaskEventUnion::Stopped => {
            let stopped_event = unsafe { tcrm_task_fb::StoppedEvent::init_from_table(event) };
            let task_name = stopped_event.task_name().to_string();
            let exit_code = if stopped_event.exit_code() == 0 {
                None
            } else {
                Some(stopped_event.exit_code())
            };

            // Convert the stop reason
            let reason_type = stopped_event.reason_type();
            let reason = match reason_type {
                tcrm_task_fb::TaskEventStopReason::Finished => TaskEventStopReason::Finished,
                tcrm_task_fb::TaskEventStopReason::TerminatedTimeout => {
                    TaskEventStopReason::Terminated(TaskTerminateReason::Timeout)
                }
                tcrm_task_fb::TaskEventStopReason::TerminatedCleanup => {
                    TaskEventStopReason::Terminated(TaskTerminateReason::Cleanup)
                }
                tcrm_task_fb::TaskEventStopReason::TerminatedDependenciesFinished => {
                    TaskEventStopReason::Terminated(TaskTerminateReason::DependenciesFinished)
                }
                tcrm_task_fb::TaskEventStopReason::TerminatedUserRequested => {
                    TaskEventStopReason::Terminated(TaskTerminateReason::UserRequested)
                }
                tcrm_task_fb::TaskEventStopReason::Error => {
                    let reason_table = stopped_event.reason();
                    let error_reason = unsafe { tcrm_task_fb::ErrorStopReason::init_from_table(reason_table) };
                    let message = error_reason.message().to_string();
                    TaskEventStopReason::Error(message)
                }
                _ => {
                    return Err(ConversionError::InvalidEnumValue(format!(
                        "Invalid TaskEventStopReason: {:?}",
                        reason_type
                    )))
                }
            };

            Ok(TaskEvent::Stopped {
                task_name,
                exit_code,
                reason,
            })
        }
        tcrm_task_fb::TaskEventUnion::Error => {
            let error_event = unsafe { tcrm_task_fb::ErrorEvent::init_from_table(event) };
            let task_name = error_event.task_name().to_string();

            let fb_error = error_event.error();

            let message = fb_error.message().unwrap_or("").to_string();

            // For now, create a simple error - TaskError construction needs investigation
            match fb_error.kind() {
                tcrm_task_fb::TaskErrorType::IO => {
                    Ok(TaskEvent::Error { 
                        task_name, 
                        error: tcrm_task::tasks::error::TaskError::IO(message) 
                    })
                }
                tcrm_task_fb::TaskErrorType::Handle => {
                    Ok(TaskEvent::Error { 
                        task_name, 
                        error: tcrm_task::tasks::error::TaskError::Handle(message) 
                    })
                }
                tcrm_task_fb::TaskErrorType::Channel => {
                    Ok(TaskEvent::Error { 
                        task_name, 
                        error: tcrm_task::tasks::error::TaskError::Channel(message) 
                    })
                }
                tcrm_task_fb::TaskErrorType::InvalidConfiguration => {
                    Ok(TaskEvent::Error { 
                        task_name, 
                        error: tcrm_task::tasks::error::TaskError::InvalidConfiguration(message) 
                    })
                }
                _ => {
                    Err(ConversionError::InvalidEnumValue(format!(
                        "Invalid TaskErrorType: {:?}",
                        fb_error.kind()
                    )))
                }
            }
        }
        _ => Err(ConversionError::InvalidEnumValue(format!(
            "Invalid TaskEventUnion: {:?}",
            event_type
        ))),
    }
}

/// Helper function to convert FlatBuffers control command to TaskMonitorControlCommand
fn convert_control_command_from_flatbuffers(
    control_type: fb::TaskMonitorControlCommand,
    control_table: flatbuffers::Table,
) -> Result<TaskMonitorControlCommand, ConversionError> {
    match control_type {
        fb::TaskMonitorControlCommand::TerminateAllTasks => {
            Ok(TaskMonitorControlCommand::TerminateAllTasks)
        }
        fb::TaskMonitorControlCommand::TerminateTask => {
            let terminate_task_command = unsafe { fb::TerminateTaskCommand::init_from_table(control_table) };
            let task_name = terminate_task_command.task_name().to_string();
            Ok(TaskMonitorControlCommand::TerminateTask { task_name })
        }
        fb::TaskMonitorControlCommand::SendStdin => {
            let send_stdin_command = unsafe { fb::SendStdinCommand::init_from_table(control_table) };
            let task_name = send_stdin_command.task_name().to_string();
            let input = send_stdin_command.input().to_string();
            Ok(TaskMonitorControlCommand::SendStdin { task_name, input })
        }
        _ => Err(ConversionError::InvalidEnumValue(format!("Unknown TaskMonitorControlCommand variant: {:?}", control_type))),
    }
}
