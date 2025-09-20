use flatbuffers::{FlatBufferBuilder, WIPOffset};
use tcrm_task::flatbuffers::conversion::{ToFlatbuffers, ToFlatbuffersUnion};

use crate::flatbuffers::conversion::FromFlatbuffers;
use crate::flatbuffers::conversion::error::ConversionError;
use crate::flatbuffers::tcrm_monitor_generated::tcrm::monitor as fb;
use crate::monitor::error::{ControlCommandError, TaskMonitorError};
use crate::monitor::event::{TaskMonitorControlCommand, TaskMonitorControlEvent, TaskMonitorEvent};

impl<'a> ToFlatbuffers<'a> for TaskMonitorEvent {
    type Output = WIPOffset<fb::TaskMonitorEvent<'a>>;

    fn to_flatbuffers(&self, fbb: &mut FlatBufferBuilder<'a>) -> Self::Output {
        let (event_type, event_offset) = match self {
            TaskMonitorEvent::Task(task_event) => {
                let task_event_offset = task_event.to_flatbuffers(fbb);
                (
                    fb::TaskMonitorEventUnion::Task,
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
                    fb::TaskMonitorEventUnion::Started,
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
                    fb::TaskMonitorEventUnion::Completed,
                    completed_event.as_union_value(),
                )
            }
            TaskMonitorEvent::Control(control_event) => {
                let control_wrapper = control_event.to_flatbuffers(fbb);
                (
                    fb::TaskMonitorEventUnion::Control,
                    control_wrapper.as_union_value(),
                )
            }
            TaskMonitorEvent::Error(error) => {
                let error_wrapper = error.to_flatbuffers(fbb);
                (
                    fb::TaskMonitorEventUnion::Error,
                    error_wrapper.as_union_value(),
                )
            }
        };

        fb::TaskMonitorEvent::create(
            fbb,
            &fb::TaskMonitorEventArgs {
                event_type: event_type,
                event: Some(event_offset),
            },
        )
    }
}

impl FromFlatbuffers<fb::TaskMonitorEvent<'_>> for TaskMonitorEvent {
    fn from_flatbuffers(fb_event: fb::TaskMonitorEvent) -> Result<Self, ConversionError> {
        let event_type = fb_event.event_type();

        match event_type {
            fb::TaskMonitorEventUnion::Task => {
                if let Some(task_event) = fb_event.event_as_task() {
                    let task_event =
                        tcrm_task::tasks::event::TaskEvent::from_flatbuffers(task_event)?;
                    Ok(TaskMonitorEvent::Task(task_event))
                } else {
                    Err(ConversionError::MissingRequiredField("task event"))
                }
            }
            fb::TaskMonitorEventUnion::Started => {
                if let Some(started_event) = fb_event.event_as_started() {
                    Ok(TaskMonitorEvent::Started {
                        total_tasks: started_event.total_tasks() as usize,
                    })
                } else {
                    Err(ConversionError::MissingRequiredField("started event"))
                }
            }
            fb::TaskMonitorEventUnion::Completed => {
                if let Some(completed_event) = fb_event.event_as_completed() {
                    Ok(TaskMonitorEvent::Completed {
                        completed_tasks: completed_event.completed_tasks() as usize,
                        failed_tasks: completed_event.failed_tasks() as usize,
                    })
                } else {
                    Err(ConversionError::MissingRequiredField("completed event"))
                }
            }
            fb::TaskMonitorEventUnion::Control => {
                if let Some(control_event) = fb_event.event_as_control() {
                    let control = TaskMonitorControlEvent::from_flatbuffers(control_event)?;
                    Ok(TaskMonitorEvent::Control(control))
                } else {
                    Err(ConversionError::MissingRequiredField("control event"))
                }
            }
            fb::TaskMonitorEventUnion::Error => {
                if let Some(error_event) = fb_event.event_as_error() {
                    let error = TaskMonitorError::from_flatbuffers(error_event)?;
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

impl<'a> ToFlatbuffersUnion<'a, fb::TaskMonitorControlCommandUnion> for TaskMonitorControlCommand {
    fn to_flatbuffers_union(
        &self,
        fbb: &mut FlatBufferBuilder<'a>,
    ) -> (
        fb::TaskMonitorControlCommandUnion,
        WIPOffset<flatbuffers::UnionWIPOffset>,
    ) {
        match self {
            TaskMonitorControlCommand::TerminateAllTasks => {
                let terminate_all_command =
                    fb::TerminateAllTasksCommand::create(fbb, &fb::TerminateAllTasksCommandArgs {});
                (
                    fb::TaskMonitorControlCommandUnion::TerminateAllTasks,
                    terminate_all_command.as_union_value(),
                )
            }
            TaskMonitorControlCommand::TerminateTask { task_name } => {
                let task_name_offset = fbb.create_string(task_name);
                let terminate_task_command = fb::TerminateTaskCommand::create(
                    fbb,
                    &fb::TerminateTaskCommandArgs {
                        task_name: Some(task_name_offset),
                    },
                );
                (
                    fb::TaskMonitorControlCommandUnion::TerminateTask,
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
                    fb::TaskMonitorControlCommandUnion::SendStdin,
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

impl<'a> ToFlatbuffersUnion<'a, fb::TaskMonitorControlEventUnion> for TaskMonitorControlEvent {
    fn to_flatbuffers_union(
        &self,
        fbb: &mut FlatBufferBuilder<'a>,
    ) -> (
        fb::TaskMonitorControlEventUnion,
        WIPOffset<flatbuffers::UnionWIPOffset>,
    ) {
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
                (
                    fb::TaskMonitorControlEventUnion::ControlReceived,
                    control_received_event.as_union_value(),
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
                (
                    fb::TaskMonitorControlEventUnion::ControlProcessed,
                    control_processed_event.as_union_value(),
                )
            }
        }
    }
}

impl<'a> ToFlatbuffers<'a> for TaskMonitorControlEvent {
    type Output = WIPOffset<fb::TaskMonitorControlEvent<'a>>;

    fn to_flatbuffers(&self, fbb: &mut FlatBufferBuilder<'a>) -> Self::Output {
        let (control_event_type, control_event_offset) = self.to_flatbuffers_union(fbb);
        fb::TaskMonitorControlEvent::create(
            fbb,
            &fb::TaskMonitorControlEventArgs {
                control_event_type,
                control_event: Some(control_event_offset),
            },
        )
    }
}

impl FromFlatbuffers<fb::ControlReceivedEvent<'_>> for TaskMonitorControlEvent {
    fn from_flatbuffers(event: fb::ControlReceivedEvent) -> Result<Self, ConversionError> {
        let control = match event.control_type() {
            fb::TaskMonitorControlCommandUnion::TerminateAllTasks => {
                TaskMonitorControlCommand::TerminateAllTasks
            }
            fb::TaskMonitorControlCommandUnion::TerminateTask => {
                if let Some(cmd) = event.control_as_terminate_task() {
                    TaskMonitorControlCommand::TerminateTask {
                        task_name: cmd.task_name().to_string(),
                    }
                } else {
                    return Err(ConversionError::MissingRequiredField(
                        "TerminateTask command",
                    ));
                }
            }
            fb::TaskMonitorControlCommandUnion::SendStdin => {
                if let Some(cmd) = event.control_as_send_stdin() {
                    TaskMonitorControlCommand::SendStdin {
                        task_name: cmd.task_name().to_string(),
                        input: cmd.input().to_string(),
                    }
                } else {
                    return Err(ConversionError::MissingRequiredField("SendStdin command"));
                }
            }
            _ => {
                return Err(ConversionError::InvalidEnumValue(format!(
                    "Unknown TaskMonitorControlCommand variant: {:?}",
                    event.control_type()
                )));
            }
        };
        Ok(TaskMonitorControlEvent::ControlReceived { control })
    }
}

impl FromFlatbuffers<fb::ControlProcessedEvent<'_>> for TaskMonitorControlEvent {
    fn from_flatbuffers(event: fb::ControlProcessedEvent) -> Result<Self, ConversionError> {
        let control = match event.control_type() {
            fb::TaskMonitorControlCommandUnion::TerminateAllTasks => {
                TaskMonitorControlCommand::TerminateAllTasks
            }
            fb::TaskMonitorControlCommandUnion::TerminateTask => {
                if let Some(cmd) = event.control_as_terminate_task() {
                    TaskMonitorControlCommand::TerminateTask {
                        task_name: cmd.task_name().to_string(),
                    }
                } else {
                    return Err(ConversionError::MissingRequiredField(
                        "TerminateTask command",
                    ));
                }
            }
            fb::TaskMonitorControlCommandUnion::SendStdin => {
                if let Some(cmd) = event.control_as_send_stdin() {
                    TaskMonitorControlCommand::SendStdin {
                        task_name: cmd.task_name().to_string(),
                        input: cmd.input().to_string(),
                    }
                } else {
                    return Err(ConversionError::MissingRequiredField("SendStdin command"));
                }
            }
            _ => {
                return Err(ConversionError::InvalidEnumValue(format!(
                    "Unknown TaskMonitorControlCommand variant: {:?}",
                    event.control_type()
                )));
            }
        };
        Ok(TaskMonitorControlEvent::ControlProcessed { control })
    }
}

impl FromFlatbuffers<fb::TaskMonitorControlEvent<'_>> for TaskMonitorControlEvent {
    fn from_flatbuffers(wrapper: fb::TaskMonitorControlEvent) -> Result<Self, ConversionError> {
        match wrapper.control_event_type() {
            fb::TaskMonitorControlEventUnion::ControlReceived => {
                if let Some(event) = wrapper.control_event_as_control_received() {
                    TaskMonitorControlEvent::from_flatbuffers(event)
                } else {
                    Err(ConversionError::MissingRequiredField(
                        "ControlReceived event",
                    ))
                }
            }
            fb::TaskMonitorControlEventUnion::ControlProcessed => {
                if let Some(event) = wrapper.control_event_as_control_processed() {
                    TaskMonitorControlEvent::from_flatbuffers(event)
                } else {
                    Err(ConversionError::MissingRequiredField(
                        "ControlProcessed event",
                    ))
                }
            }
            _ => Err(ConversionError::InvalidEnumValue(format!(
                "Unknown TaskMonitorControlEvent variant: {:?}",
                wrapper.control_event_type()
            ))),
        }
    }
}

impl<'a> ToFlatbuffersUnion<'a, fb::TaskMonitorErrorUnion> for TaskMonitorError {
    fn to_flatbuffers_union(
        &self,
        fbb: &mut FlatBufferBuilder<'a>,
    ) -> (
        fb::TaskMonitorErrorUnion,
        WIPOffset<flatbuffers::UnionWIPOffset>,
    ) {
        match self {
            TaskMonitorError::ConfigParse(message) => {
                let message_offset = fbb.create_string(message);
                let config_parse_error = fb::ConfigParseError::create(
                    fbb,
                    &fb::ConfigParseErrorArgs {
                        message: Some(message_offset),
                    },
                );
                (
                    fb::TaskMonitorErrorUnion::ConfigParse,
                    config_parse_error.as_union_value(),
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
                (
                    fb::TaskMonitorErrorUnion::CircularDependency,
                    circular_dependency_error.as_union_value(),
                )
            }
            TaskMonitorError::DependencyNotFound {
                dependency_task_name,
                task_name,
            } => {
                let dependency_task_name_offset = fbb.create_string(dependency_task_name);
                let task_name_offset = fbb.create_string(task_name);
                let dependency_not_found_error = fb::DependencyNotFoundError::create(
                    fbb,
                    &fb::DependencyNotFoundErrorArgs {
                        dependency_task_name: Some(dependency_task_name_offset),
                        task_name: Some(task_name_offset),
                    },
                );
                (
                    fb::TaskMonitorErrorUnion::DependencyNotFound,
                    dependency_not_found_error.as_union_value(),
                )
            }
            TaskMonitorError::ControlError(control_error) => {
                let control_command_error = control_error.to_flatbuffers(fbb);
                (
                    fb::TaskMonitorErrorUnion::ControlError,
                    control_command_error.as_union_value(),
                )
            }
        }
    }
}

impl<'a> ToFlatbuffers<'a> for TaskMonitorError {
    type Output = WIPOffset<fb::TaskMonitorError<'a>>;

    fn to_flatbuffers(&self, fbb: &mut FlatBufferBuilder<'a>) -> Self::Output {
        let (error_type, error_offset) = self.to_flatbuffers_union(fbb);
        fb::TaskMonitorError::create(
            fbb,
            &fb::TaskMonitorErrorArgs {
                error_type,
                error: Some(error_offset),
            },
        )
    }
}

impl FromFlatbuffers<fb::ConfigParseError<'_>> for TaskMonitorError {
    fn from_flatbuffers(e: fb::ConfigParseError) -> Result<Self, ConversionError> {
        Ok(TaskMonitorError::ConfigParse(e.message().to_string()))
    }
}
impl FromFlatbuffers<fb::CircularDependencyError<'_>> for TaskMonitorError {
    fn from_flatbuffers(e: fb::CircularDependencyError) -> Result<Self, ConversionError> {
        Ok(TaskMonitorError::CircularDependency(
            e.task_name().to_string(),
        ))
    }
}
impl FromFlatbuffers<fb::DependencyNotFoundError<'_>> for TaskMonitorError {
    fn from_flatbuffers(e: fb::DependencyNotFoundError) -> Result<Self, ConversionError> {
        Ok(TaskMonitorError::DependencyNotFound {
            dependency_task_name: e.dependency_task_name().to_string(),
            task_name: e.task_name().to_string(),
        })
    }
}
impl FromFlatbuffers<fb::ControlCommandError<'_>> for TaskMonitorError {
    fn from_flatbuffers(e: fb::ControlCommandError) -> Result<Self, ConversionError> {
        Ok(TaskMonitorError::ControlError(
            ControlCommandError::try_from(e)?,
        ))
    }
}

impl FromFlatbuffers<tcrm_task::flatbuffers::tcrm_task_generated::tcrm::task::TaskEvent<'_>>
    for tcrm_task::tasks::event::TaskEvent
{
    fn from_flatbuffers(
        fb_task_event: tcrm_task::flatbuffers::tcrm_task_generated::tcrm::task::TaskEvent,
    ) -> Result<Self, ConversionError> {
        use tcrm_task::flatbuffers::tcrm_task_generated::tcrm::task as tcrm_task_fb;
        use tcrm_task::tasks::config::StreamSource;
        use tcrm_task::tasks::event::{TaskEvent, TaskEventStopReason, TaskTerminateReason};

        // Get the event type and use safe accessor methods
        let event_type = fb_task_event.event_type();

        match event_type {
            tcrm_task_fb::TaskEventUnion::Started => {
                if let Some(started_event) = fb_task_event.event_as_started() {
                    let task_name = started_event.task_name().to_string();
                    Ok(TaskEvent::Started { task_name })
                } else {
                    Err(ConversionError::MissingRequiredField("Started event"))
                }
            }
            tcrm_task_fb::TaskEventUnion::Output => {
                if let Some(output_event) = fb_task_event.event_as_output() {
                    let task_name = output_event.task_name().to_string();
                    let line = output_event.line().to_string();
                    let src = match output_event.src() {
                        tcrm_task_fb::StreamSource::Stdout => StreamSource::Stdout,
                        tcrm_task_fb::StreamSource::Stderr => StreamSource::Stderr,
                        _ => {
                            return Err(ConversionError::InvalidEnumValue(format!(
                                "Invalid StreamSource: {:?}",
                                output_event.src()
                            )));
                        }
                    };
                    Ok(TaskEvent::Output {
                        task_name,
                        line,
                        src,
                    })
                } else {
                    Err(ConversionError::MissingRequiredField("Output event"))
                }
            }
            tcrm_task_fb::TaskEventUnion::Ready => {
                if let Some(ready_event) = fb_task_event.event_as_ready() {
                    let task_name = ready_event.task_name().to_string();
                    Ok(TaskEvent::Ready { task_name })
                } else {
                    Err(ConversionError::MissingRequiredField("Ready event"))
                }
            }
            tcrm_task_fb::TaskEventUnion::Stopped => {
                if let Some(stopped_event) = fb_task_event.event_as_stopped() {
                    let task_name = stopped_event.task_name().to_string();
                    let exit_code = if stopped_event.exit_code() == 0 {
                        None
                    } else {
                        Some(stopped_event.exit_code())
                    };

                    // Convert the stop reason using safe accessor
                    let reason_type = stopped_event.reason_type();
                    let reason = match reason_type {
                        tcrm_task_fb::TaskEventStopReason::Finished => {
                            TaskEventStopReason::Finished
                        }
                        tcrm_task_fb::TaskEventStopReason::TerminatedTimeout => {
                            TaskEventStopReason::Terminated(TaskTerminateReason::Timeout)
                        }
                        tcrm_task_fb::TaskEventStopReason::TerminatedCleanup => {
                            TaskEventStopReason::Terminated(TaskTerminateReason::Cleanup)
                        }
                        tcrm_task_fb::TaskEventStopReason::TerminatedDependenciesFinished => {
                            TaskEventStopReason::Terminated(
                                TaskTerminateReason::DependenciesFinished,
                            )
                        }
                        tcrm_task_fb::TaskEventStopReason::TerminatedUserRequested => {
                            TaskEventStopReason::Terminated(TaskTerminateReason::UserRequested)
                        }
                        tcrm_task_fb::TaskEventStopReason::Error => {
                            if let Some(error_reason) = stopped_event.reason_as_error() {
                                let message = error_reason.message().to_string();
                                TaskEventStopReason::Error(message)
                            } else {
                                return Err(ConversionError::MissingRequiredField(
                                    "Error stop reason",
                                ));
                            }
                        }
                        _ => {
                            return Err(ConversionError::InvalidEnumValue(format!(
                                "Invalid TaskEventStopReason: {:?}",
                                reason_type
                            )));
                        }
                    };

                    Ok(TaskEvent::Stopped {
                        task_name,
                        exit_code,
                        reason,
                    })
                } else {
                    Err(ConversionError::MissingRequiredField("Stopped event"))
                }
            }
            tcrm_task_fb::TaskEventUnion::Error => {
                if let Some(error_event) = fb_task_event.event_as_error() {
                    let task_name = error_event.task_name().to_string();
                    let fb_error = error_event.error();
                    let message = fb_error.message().unwrap_or("").to_string();

                    // Create appropriate TaskError based on the error kind
                    match fb_error.kind() {
                        tcrm_task_fb::TaskErrorType::IO => Ok(TaskEvent::Error {
                            task_name,
                            error: tcrm_task::tasks::error::TaskError::IO(message),
                        }),
                        tcrm_task_fb::TaskErrorType::Handle => Ok(TaskEvent::Error {
                            task_name,
                            error: tcrm_task::tasks::error::TaskError::Handle(message),
                        }),
                        tcrm_task_fb::TaskErrorType::Channel => Ok(TaskEvent::Error {
                            task_name,
                            error: tcrm_task::tasks::error::TaskError::Channel(message),
                        }),
                        tcrm_task_fb::TaskErrorType::InvalidConfiguration => Ok(TaskEvent::Error {
                            task_name,
                            error: tcrm_task::tasks::error::TaskError::InvalidConfiguration(
                                message,
                            ),
                        }),
                        _ => Err(ConversionError::InvalidEnumValue(format!(
                            "Invalid TaskErrorType: {:?}",
                            fb_error.kind()
                        ))),
                    }
                } else {
                    Err(ConversionError::MissingRequiredField("Error event"))
                }
            }
            _ => Err(ConversionError::InvalidEnumValue(format!(
                "Invalid TaskEventUnion: {:?}",
                event_type
            ))),
        }
    }
}
