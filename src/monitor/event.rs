use tcrm_task::tasks::event::TaskEvent;

use crate::monitor::error::{SendStdinErrorReason, TaskMonitorError};

/// Events emitted by TaskMonitor during execution
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum TaskMonitorEvent {
    /// A task event from the underlying task system
    Task(TaskEvent),
    /// Monitor execution started
    ExecutionStarted { total_tasks: usize },
    /// Monitor execution completed (all tasks finished or stopped)
    ExecutionCompleted {
        completed_tasks: usize,
        failed_tasks: usize,
    },
    /// Control message received and being processed
    ControlReceived {
        control_type: TaskMonitorControlType,
    },
    /// Control message processed successfully
    ControlProcessed {
        control_type: TaskMonitorControlType,
    },
    /// Error occurred while processing control message
    ControlError {
        control_type: TaskMonitorControlType,
        error: TaskMonitorError,
    },
    /// Stdin successfully sent to task
    StdinSent {
        task_name: String,
        input_length: usize,
    },
    /// Failed to send stdin to task
    StdinError {
        task_name: String,
        error: SendStdinErrorReason,
    },
    /// Task termination requested
    TaskTerminationRequested { task_name: String },
    /// All tasks termination requested (stop signal)
    AllTasksTerminationRequested,
}

/// Type of control message for event reporting
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum TaskMonitorControlType {
    Stop,
    TerminateTask,
    SendStdin,
}

impl TaskMonitorEvent {
    /// Extract the task name if this event is related to a specific task
    pub fn task_name(&self) -> Option<&str> {
        match self {
            TaskMonitorEvent::Task(task_event) => match task_event {
                TaskEvent::Started { task_name } => Some(task_name),
                TaskEvent::Output { task_name, .. } => Some(task_name),
                TaskEvent::Ready { task_name } => Some(task_name),
                TaskEvent::Stopped { task_name, .. } => Some(task_name),
                TaskEvent::Error { task_name, .. } => Some(task_name),
            },
            TaskMonitorEvent::StdinSent { task_name, .. } => Some(task_name),
            TaskMonitorEvent::StdinError { task_name, .. } => Some(task_name),
            TaskMonitorEvent::TaskTerminationRequested { task_name } => Some(task_name),
            _ => None,
        }
    }

    /// Check if this is a task-level event
    pub fn is_task_event(&self) -> bool {
        matches!(self, TaskMonitorEvent::Task(_))
    }

    /// Check if this is a monitor-level event
    pub fn is_monitor_event(&self) -> bool {
        !self.is_task_event()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcrm_task::tasks::config::StreamSource;

    #[test]
    fn test_task_name_extraction() {
        // Test task events
        let event = TaskMonitorEvent::Task(TaskEvent::Started {
            task_name: "test_task".to_string(),
        });
        assert_eq!(event.task_name(), Some("test_task"));

        let event = TaskMonitorEvent::Task(TaskEvent::Output {
            task_name: "test_task".to_string(),
            line: "output".to_string(),
            src: StreamSource::Stdout,
        });
        assert_eq!(event.task_name(), Some("test_task"));

        // Test monitor events with task names
        let event = TaskMonitorEvent::StdinSent {
            task_name: "test_task".to_string(),
            input_length: 10,
        };
        assert_eq!(event.task_name(), Some("test_task"));

        // Test monitor events without task names
        let event = TaskMonitorEvent::ExecutionStarted { total_tasks: 5 };
        assert_eq!(event.task_name(), None);
    }

    #[test]
    fn test_event_type_classification() {
        let task_event = TaskMonitorEvent::Task(TaskEvent::Ready {
            task_name: "test".to_string(),
        });
        assert!(task_event.is_task_event());
        assert!(!task_event.is_monitor_event());

        let monitor_event = TaskMonitorEvent::ExecutionStarted { total_tasks: 1 };
        assert!(!monitor_event.is_task_event());
        assert!(monitor_event.is_monitor_event());
    }
}
