//! Event system for task monitoring.
//!
//! This module provides event types that allow monitoring the progress and status
//! of task execution in real-time. Events are emitted during various phases of
//! task monitoring, from initial setup to completion.

use tcrm_task::tasks::event::TaskEvent;

use crate::monitor::error::{SendStdinErrorReason, TaskMonitorError};

/// Events emitted by `TaskMonitor` during execution.
///
/// These events provide real-time information about task execution progress,
/// control message processing, and error conditions. They can be used to build
/// user interfaces, logging systems, or progress monitoring.
///
/// # Event Categories
///
/// - **Task Events**: Forwarded from individual tasks (started, output, stopped, etc.)
/// - **Execution Events**: Monitor-level events (started, completed)  
/// - **Control Events**: Events related to runtime control (stop, stdin, terminate)
/// - **Error Events**: Error conditions during monitoring
///
/// # Examples
///
/// ## Basic Event Monitoring
///
/// ```rust
/// use tokio::sync::mpsc;
/// use tcrm_monitor::monitor::event::TaskMonitorEvent;
///
/// # #[tokio::main]
/// # async fn main() {
/// let (event_tx, mut event_rx) = mpsc::channel(100);
///
/// // Spawn a task to handle events
/// tokio::spawn(async move {
///     while let Some(event) = event_rx.recv().await {
///         match event {
///             TaskMonitorEvent::ExecutionStarted { total_tasks } => {
///                 println!("Starting execution of {} tasks", total_tasks);
///             }
///             TaskMonitorEvent::Task(task_event) => {
///                 println!("Task event: {:?}", task_event);
///             }
///             TaskMonitorEvent::ExecutionCompleted { completed_tasks, failed_tasks } => {
///                 println!("Execution complete: {} completed, {} failed",
///                          completed_tasks, failed_tasks);
///                 break;
///             }
///             _ => {}
///         }
///     }
/// });
/// # }
/// ```
///
/// ## Filtering Events by Task
///
/// ```rust
/// use tcrm_monitor::monitor::event::TaskMonitorEvent;
///
/// fn handle_event(event: &TaskMonitorEvent) {
///     if let Some(task_name) = event.task_name() {
///         if task_name == "important_task" {
///             println!("Event for important task: {:?}", event);
///         }
///     }
/// }
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum TaskMonitorEvent {
    /// A task event from the underlying task system.
    ///
    /// This forwards events from individual tasks, including:
    /// - Task started
    /// - Task output (stdout/stderr)  
    /// - Task ready state changes
    /// - Task stopped/completed
    /// - Task errors
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::TaskMonitorEvent;
    /// use tcrm_task::tasks::event::TaskEvent;
    ///
    /// let event = TaskMonitorEvent::Task(TaskEvent::Started {
    ///     task_name: "build".to_string()
    /// });
    /// ```
    Task(TaskEvent),

    /// Monitor execution started.
    ///
    /// Emitted when task execution begins, providing the total number of tasks to execute.
    ///
    /// # Fields
    ///
    /// * `total_tasks` - Total number of tasks in the execution plan
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::TaskMonitorEvent;
    ///
    /// let event = TaskMonitorEvent::ExecutionStarted { total_tasks: 5 };
    /// ```
    ExecutionStarted {
        /// Total number of tasks to execute
        total_tasks: usize,
    },

    /// Monitor execution completed.
    ///
    /// Emitted when all tasks have finished (either successfully or with errors).
    ///
    /// # Fields
    ///
    /// * `completed_tasks` - Number of tasks that completed successfully
    /// * `failed_tasks` - Number of tasks that failed or were terminated
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::TaskMonitorEvent;
    ///
    /// let event = TaskMonitorEvent::ExecutionCompleted {
    ///     completed_tasks: 3,
    ///     failed_tasks: 1
    /// };
    /// ```
    ExecutionCompleted {
        /// Number of tasks completed successfully
        completed_tasks: usize,
        /// Number of tasks that failed
        failed_tasks: usize,
    },

    /// Control message received and being processed.
    ///
    /// Emitted when a control command (stop, stdin, terminate) is received.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::{TaskMonitorEvent, TaskMonitorControlType};
    ///
    /// let event = TaskMonitorEvent::ControlReceived {
    ///     control_type: TaskMonitorControlType::Stop
    /// };
    /// ```
    ControlReceived {
        /// Type of control message received
        control_type: TaskMonitorControlType,
    },

    /// Control message processed successfully.
    ///
    /// Emitted after a control command has been processed without errors.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::{TaskMonitorEvent, TaskMonitorControlType};
    ///
    /// let event = TaskMonitorEvent::ControlProcessed {
    ///     control_type: TaskMonitorControlType::SendStdin
    /// };
    /// ```
    ControlProcessed {
        /// Type of control message that was processed
        control_type: TaskMonitorControlType,
    },

    /// Error occurred while processing control message.
    ///
    /// Emitted when a control command fails to execute properly.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::{TaskMonitorEvent, TaskMonitorControlType};
    /// use tcrm_monitor::monitor::error::TaskMonitorError;
    ///
    /// let event = TaskMonitorEvent::ControlError {
    ///     control_type: TaskMonitorControlType::SendStdin,
    ///     error: TaskMonitorError::ConfigParse("Invalid input".to_string())
    /// };
    /// ```
    ControlError {
        /// Type of control message that failed
        control_type: TaskMonitorControlType,
        /// The error that occurred
        error: TaskMonitorError,
    },

    /// Stdin successfully sent to task.
    ///
    /// Emitted when input is successfully delivered to a task's stdin.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::TaskMonitorEvent;
    ///
    /// let event = TaskMonitorEvent::StdinSent {
    ///     task_name: "interactive".to_string(),
    ///     input_length: 42
    /// };
    /// ```
    StdinSent {
        /// Name of the task that received stdin
        task_name: String,
        /// Number of bytes sent
        input_length: usize,
    },

    /// Failed to send stdin to task.
    ///
    /// Emitted when an attempt to send stdin input to a task fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::TaskMonitorEvent;
    /// use tcrm_monitor::monitor::error::SendStdinErrorReason;
    ///
    /// let event = TaskMonitorEvent::StdinError {
    ///     task_name: "nonexistent".to_string(),
    ///     error: SendStdinErrorReason::TaskNotFound("nonexistent".to_string())
    /// };
    /// ```
    StdinError {
        /// Name of the target task
        task_name: String,
        /// Reason the stdin operation failed
        error: SendStdinErrorReason,
    },

    /// Task termination requested.
    ///
    /// Emitted when a specific task is requested to terminate.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::TaskMonitorEvent;
    ///
    /// let event = TaskMonitorEvent::TaskTerminationRequested {
    ///     task_name: "long_running_task".to_string()
    /// };
    /// ```
    TaskTerminationRequested {
        /// Name of the task to terminate
        task_name: String,
    },

    /// All tasks termination requested (stop signal).
    ///
    /// Emitted when a stop-all command is issued, requesting termination of all running tasks.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::TaskMonitorEvent;
    ///
    /// let event = TaskMonitorEvent::AllTasksTerminationRequested;
    /// ```
    AllTasksTerminationRequested,
}

/// Type of control message for event reporting.
///
/// Used in control-related events to identify what type of control operation
/// is being performed or has been performed.
///
/// # Examples
///
/// ```rust
/// use tcrm_monitor::monitor::event::TaskMonitorControlType;
///
/// let control_type = TaskMonitorControlType::Stop;
/// assert_eq!(control_type, TaskMonitorControlType::Stop);
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum TaskMonitorControlType {
    /// Stop all running tasks gracefully
    Stop,
    /// Terminate a specific task forcefully  
    TerminateTask,
    /// Send input to a task's stdin
    SendStdin,
}

impl TaskMonitorEvent {
    /// Extract the task name if this event is related to a specific task.
    ///
    /// Many events are associated with specific tasks. This method provides a convenient
    /// way to extract the task name when it's available, useful for filtering events
    /// or routing them to task-specific handlers.
    ///
    /// # Returns
    ///
    /// * `Some(&str)` - The task name if this event is task-specific
    /// * `None` - If this is a monitor-level event not associated with a specific task
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::TaskMonitorEvent;
    /// use tcrm_task::tasks::event::TaskEvent;
    ///
    /// // Task-specific event
    /// let event = TaskMonitorEvent::Task(TaskEvent::Started {
    ///     task_name: "build".to_string()
    /// });
    /// assert_eq!(event.task_name(), Some("build"));
    ///
    /// // Monitor-level event
    /// let event = TaskMonitorEvent::ExecutionStarted { total_tasks: 5 };
    /// assert_eq!(event.task_name(), None);
    ///
    /// // Stdin event
    /// let event = TaskMonitorEvent::StdinSent {
    ///     task_name: "interactive".to_string(),
    ///     input_length: 10
    /// };
    /// assert_eq!(event.task_name(), Some("interactive"));
    /// ```
    #[must_use]
    pub fn task_name(&self) -> Option<&str> {
        match self {
            TaskMonitorEvent::Task(task_event) => match task_event {
                TaskEvent::Started { task_name }
                | TaskEvent::Output { task_name, .. }
                | TaskEvent::Ready { task_name }
                | TaskEvent::Stopped { task_name, .. }
                | TaskEvent::Error { task_name, .. } => Some(task_name),
            },
            TaskMonitorEvent::StdinSent { task_name, .. }
            | TaskMonitorEvent::StdinError { task_name, .. }
            | TaskMonitorEvent::TaskTerminationRequested { task_name } => Some(task_name),
            _ => None,
        }
    }

    /// Check if this is a task-level event.
    ///
    /// Task-level events are those that originate from individual tasks,
    /// as opposed to monitor-level events that relate to the overall execution.
    ///
    /// # Returns
    ///
    /// * `true` - If this is a [`TaskMonitorEvent::Task`] variant
    /// * `false` - If this is any other type of event
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::TaskMonitorEvent;
    /// use tcrm_task::tasks::event::TaskEvent;
    ///
    /// // Task event
    /// let event = TaskMonitorEvent::Task(TaskEvent::Ready {
    ///     task_name: "test".to_string()
    /// });
    /// assert!(event.is_task_event());
    ///
    /// // Monitor event
    /// let event = TaskMonitorEvent::ExecutionStarted { total_tasks: 1 };
    /// assert!(!event.is_task_event());
    /// ```
    #[must_use]
    pub fn is_task_event(&self) -> bool {
        matches!(self, TaskMonitorEvent::Task(_))
    }

    /// Check if this is a monitor-level event.
    ///
    /// Monitor-level events relate to the overall task execution process,
    /// such as starting, completion, or control operations.
    ///
    /// # Returns
    ///
    /// * `true` - If this is not a [`TaskMonitorEvent::Task`] variant
    /// * `false` - If this is a task-level event
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::TaskMonitorEvent;
    /// use tcrm_task::tasks::event::TaskEvent;
    ///
    /// // Monitor event
    /// let event = TaskMonitorEvent::ExecutionCompleted {
    ///     completed_tasks: 3,
    ///     failed_tasks: 1
    /// };
    /// assert!(event.is_monitor_event());
    ///
    /// // Task event
    /// let event = TaskMonitorEvent::Task(TaskEvent::Ready {
    ///     task_name: "test".to_string()
    /// });
    /// assert!(!event.is_monitor_event());
    /// ```
    #[must_use]
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
