//! Event system for task monitoring.
//!
//! This module provides event types that allow monitoring the progress and status
//! of task execution in real-time. Events are emitted during various phases of
//! task monitoring, from initial setup to completion.

use tcrm_task::tasks::event::TaskEvent;

use crate::monitor::error::TaskMonitorError;

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
///             TaskMonitorEvent::Started { total_tasks } => {
///                 println!("Starting execution of {} tasks", total_tasks);
///             }
///             TaskMonitorEvent::Task(task_event) => {
///                 println!("Task event: {:?}", task_event);
///             }
///             TaskMonitorEvent::Completed { completed_tasks, failed_tasks } => {
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
    /// let event = TaskMonitorEvent::Started { total_tasks: 5 };
    /// ```
    Started {
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
    /// let event = TaskMonitorEvent::Completed {
    ///     completed_tasks: 3,
    ///     failed_tasks: 1
    /// };
    /// ```
    Completed {
        /// Number of tasks completed successfully
        completed_tasks: usize,
        /// Number of tasks that failed
        failed_tasks: usize,
    },

    Control(TaskMonitorControlEvent),

    Error(TaskMonitorError),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum TaskMonitorControlEvent {
    ControlReceived { control: TaskMonitorControlCommand },
    ControlProcessed { control: TaskMonitorControlCommand },
}

/// Control message for `TaskMonitor` execution.
///
/// These control messages allow runtime interaction with task execution,
/// providing the ability to stop tasks, send input, or terminate specific tasks.
///
/// # Examples
///
/// ## Terminating All Tasks
///
/// ```rust
/// use tcrm_monitor::monitor::event::TaskMonitorControlCommand;
///
/// let control = TaskMonitorControlCommand::TerminateAllTasks;
/// ```
///
/// ## Sending Stdin to a Task
///
/// ```rust
/// use tcrm_monitor::monitor::event::TaskMonitorControlCommand;
///
/// let control = TaskMonitorControlCommand::SendStdin {
///     task_name: "interactive_task".to_string(),
///     input: "y\n".to_string()
/// };
/// ```
///
/// ## Terminating a Specific Task
///
/// ```rust
/// use tcrm_monitor::monitor::event::TaskMonitorControlCommand;
///
/// let control = TaskMonitorControlCommand::TerminateTask {
///     task_name: "runaway_task".to_string()
/// };
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub enum TaskMonitorControlCommand {
    /// Stop all tasks gracefully and terminate execution.
    ///
    /// This will attempt to stop all running tasks in an orderly fashion,
    /// waiting for them to complete their current operations before terminating.
    TerminateAllTasks,

    /// Terminate a specific task by name.
    ///
    /// This forcefully terminates the specified task without waiting for
    /// it to complete naturally.
    ///
    /// # Fields
    ///
    /// * `task_name` - Name of the task to terminate
    TerminateTask {
        /// Name of the task to terminate
        task_name: String,
    },

    /// Send stdin input to a specific task.
    ///
    /// Only works if the target task was configured with `enable_stdin(true)`.
    /// The input will be delivered to the task's stdin stream.
    ///
    /// # Fields
    ///
    /// * `task_name` - Name of the target task
    /// * `input` - String input to send to the task's stdin
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::event::TaskMonitorControlCommand;
    ///
    /// // Send "yes" followed by newline to a task
    /// let control = TaskMonitorControlCommand::SendStdin {
    ///     task_name: "confirmation_task".to_string(),
    ///     input: "yes\n".to_string()
    /// };
    /// ```
    SendStdin {
        /// Name of the target task
        task_name: String,
        /// Input string to send to stdin
        input: String,
    },
}
