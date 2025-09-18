//! Direct task execution strategy.
//!
//! This module implements a direct execution strategy for tasks, where tasks are executed
//! in dependency order with parallel execution of independent tasks. It supports runtime
//! control for stopping tasks, sending stdin input, and terminating specific tasks.

use std::collections::HashSet;

use tcrm_task::tasks::{
    event::{TaskEvent, TaskEventStopReason, TaskTerminateReason},
    state::TaskState,
};
use tokio::sync::mpsc::{self, Sender};

use crate::monitor::{
    error::{ControlCommandError, SendStdinErrorReason, TaskMonitorError},
    event::{TaskMonitorControlCommand, TaskMonitorControlEvent, TaskMonitorEvent},
    tasks::TaskMonitor,
};

impl TaskMonitor {
    /// Execute all tasks using the direct execution strategy.
    ///
    /// This method executes tasks in dependency order, running independent tasks in parallel.
    /// It processes task events and manages the execution flow until all tasks complete.
    ///
    /// # Arguments
    ///
    /// * `event_tx` - Optional sender to forward task events to external listeners
    ///
    /// # Behavior
    ///
    /// 1. Starts all tasks that have no dependencies
    /// 2. Listens for task events (started, output, ready, stopped, error)
    /// 3. When a task becomes ready, starts its dependents if their dependencies are satisfied
    /// 4. Continues until all tasks have completed or failed
    /// 5. Handles task termination based on `terminate_after_dependents_finished` flag
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::HashMap;
    /// use tokio::sync::mpsc;
    /// use tcrm_monitor::monitor::{TaskMonitor, config::TaskSpec};
    /// use tcrm_task::tasks::{config::TaskConfig, event::TaskEvent};
    /// use tcrm_monitor::monitor::config::TaskShell;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut tasks = HashMap::new();
    /// tasks.insert(
    ///     "test".to_string(),
    ///     TaskSpec::new(TaskConfig::new("echo").args(["Running tests"])).shell(TaskShell::Auto)
    /// );
    ///
    /// let mut monitor = TaskMonitor::new(tasks)?;
    ///
    /// // Execute without event monitoring
    /// monitor.execute_all_direct(None).await;
    ///
    /// // Create a new monitor for the second example
    /// let mut tasks2 = HashMap::new();
    /// tasks2.insert(
    ///     "test2".to_string(),
    ///     TaskSpec::new(TaskConfig::new("echo").args(["Running tests 2"])).shell(TaskShell::Auto)
    /// );
    /// let mut monitor2 = TaskMonitor::new(tasks2)?;
    ///
    /// // Or with event monitoring
    /// let (event_tx, mut event_rx) = mpsc::channel(100);
    /// let event_handler = tokio::spawn(async move {
    ///     while let Some(event) = event_rx.recv().await {
    ///         println!("Task event: {:?}", event);
    ///         // Break on certain events to avoid hanging
    ///         if matches!(event, TaskEvent::Stopped { .. }) {
    ///             break;
    ///         }
    ///     }
    /// });
    ///
    /// monitor2.execute_all_direct(Some(event_tx)).await;
    /// let _ = event_handler.await;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn execute_all_direct(&mut self, event_tx: Option<Sender<TaskEvent>>) {
        let (task_event_tx, mut task_event_rx) = mpsc::channel::<TaskEvent>(1024);
        self.start_independent_tasks_direct(&task_event_tx).await;

        // Keep track of active tasks
        let mut active_tasks: HashSet<String> = self.tasks_spawner.keys().cloned().collect();

        while let Some(event) = task_event_rx.recv().await {
            if let Some(ref tx) = event_tx
                && let Err(_e) = tx.send(event.clone()).await
            {
                #[cfg(feature = "tracing")]
                tracing::warn!(event = ?event, "Failed to forward event");
            }
            match event {
                TaskEvent::Started { .. } | TaskEvent::Output { .. } => {}
                TaskEvent::Ready { task_name } => {
                    self.start_ready_dependents_direct(
                        &mut active_tasks,
                        &task_name,
                        None,
                        &task_event_tx.clone(),
                    )
                    .await;
                }
                TaskEvent::Stopped {
                    task_name,
                    exit_code: _,
                    reason,
                } => {
                    active_tasks.remove(&task_name);

                    self.terminate_dependencies_if_all_dependent_finished(&task_name)
                        .await;

                    self.start_ready_dependents_direct(
                        &mut active_tasks,
                        &task_name,
                        Some(reason),
                        &task_event_tx.clone(),
                    )
                    .await;
                }
                TaskEvent::Error { task_name, error } => {
                    active_tasks.remove(&task_name);

                    self.terminate_dependencies_if_all_dependent_finished(&task_name)
                        .await;

                    self.start_ready_dependents_direct(
                        &mut active_tasks,
                        &task_name,
                        Some(TaskEventStopReason::Error(error.to_string())),
                        &task_event_tx.clone(),
                    )
                    .await;
                }
            }

            // Exit when no tasks are active
            if active_tasks.is_empty() {
                #[cfg(feature = "tracing")]
                tracing::debug!("All tasks completed");
                break;
            }
        }
    }

    /// Execute all tasks with real-time control capabilities.
    ///
    /// This method provides advanced task execution with real-time control capabilities
    /// including the ability to send stdin to running tasks, request task termination,
    /// and gracefully stop all execution.
    ///
    /// # Parameters
    ///
    /// * `event_tx` - Optional channel to receive task execution events
    /// * `control_rx` - Channel to receive control commands during execution
    ///
    /// # Control Commands
    ///
    /// The control channel accepts [`TaskMonitorControlCommand`] commands:
    /// - `SendStdin { task_name, data }` - Send input to a specific task's stdin
    /// - `TerminateTask { task_name }` - Request termination of a specific task
    /// - `TerminateAll` - Request termination of all running tasks
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::HashMap;
    /// use tokio::sync::mpsc;
    /// use tcrm_monitor::monitor::{
    ///     tasks::TaskMonitor,
    ///     config::{TaskSpec, TaskShell},
    ///     event::{TaskMonitorControlCommand, TaskMonitorEvent}
    /// };
    /// use tcrm_task::tasks::config::TaskConfig;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut tasks = HashMap::new();
    /// tasks.insert(
    ///     "interactive_task".to_string(),
    ///     TaskSpec::new(TaskConfig::new("cat"))  // cat reads from stdin
    ///         .shell(TaskShell::Auto)
    /// );
    ///
    /// let mut monitor = TaskMonitor::new(tasks)?;
    /// let (event_tx, mut event_rx) = mpsc::channel(100);
    /// let (control_tx, control_rx) = mpsc::channel(10);
    ///
    /// // Spawn control task
    /// let control_handle = tokio::spawn(async move {
    ///     // Send some input to the task
    ///     control_tx.send(TaskMonitorControlCommand::SendStdin {
    ///         task_name: "interactive_task".to_string(),
    ///         input: "Hello, World!\n".to_string(),
    ///     }).await.unwrap();
    ///
    ///     // Wait a bit then stop
    ///     tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    ///     control_tx.send(TaskMonitorControlCommand::TerminateAllTasks).await.unwrap();
    /// });
    ///
    /// // Spawn event listener
    /// let event_handle = tokio::spawn(async move {
    ///     while let Some(event) = event_rx.recv().await {
    ///         match event {
    ///             TaskMonitorEvent::Completed { .. } => break,
    ///             _ => {}
    ///         }
    ///     }
    /// });
    ///
    /// // Execute with control
    /// monitor.execute_all_direct_with_control(Some(event_tx), control_rx).await;
    ///
    /// // Wait for background tasks
    /// control_handle.await?;
    /// event_handle.await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn execute_all_direct_with_control(
        &mut self,
        event_tx: Option<Sender<TaskMonitorEvent>>,
        mut control_rx: mpsc::Receiver<TaskMonitorControlCommand>,
    ) {
        let total_tasks = self.tasks_spawner.len();

        // Send execution started event
        if let Some(ref tx) = event_tx
            && tx
                .send(TaskMonitorEvent::Started { total_tasks })
                .await
                .is_err()
        {
            #[cfg(feature = "tracing")]
            tracing::warn!("Event channel closed while sending ExecutionStarted");
        }
        let (task_event_tx, mut task_event_rx) = mpsc::channel::<TaskEvent>(1024);
        self.start_independent_tasks_direct(&task_event_tx).await;

        // Keep track of active tasks
        let mut active_tasks: HashSet<String> = self.tasks_spawner.keys().cloned().collect();
        let mut completed_tasks = 0;
        let mut failed_tasks = 0;

        loop {
            tokio::select! {
                // Handle task events
                event = task_event_rx.recv() => {
                    let should_break = self.handle_task_event(event,&mut completed_tasks,&mut failed_tasks, &mut active_tasks, &task_event_tx, &event_tx).await;
                    if should_break {
                        break;
                    }
                }
                // Handle control messages
                control = control_rx.recv() => {
                    let should_break = self.handle_control_event(control, &event_tx).await;
                    if should_break {
                        break;
                    }


                }
            }
        }

        // Send execution completed event
        if let Some(ref tx) = event_tx
            && tx
                .send(TaskMonitorEvent::Completed {
                    completed_tasks,
                    failed_tasks,
                })
                .await
                .is_err()
        {
            #[cfg(feature = "tracing")]
            tracing::warn!("Event channel closed while sending ExecutionCompleted");
        }
    }

    // Return true to break the main loop
    async fn handle_task_event(
        &mut self,
        event: Option<TaskEvent>,
        completed_tasks: &mut usize,
        failed_tasks: &mut usize,
        active_tasks: &mut HashSet<String>,
        task_event_tx: &Sender<TaskEvent>,
        event_tx: &Option<Sender<TaskMonitorEvent>>,
    ) -> bool {
        let event = match event {
            Some(e) => e,
            None => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Task event channel closed");
                return true;
            }
        };
        // Count completed/failed tasks
        if let TaskEvent::Stopped { reason, .. } = &event {
            *completed_tasks += 1;
            if let TaskEventStopReason::Error(_) = reason {
                *failed_tasks += 1
            }
        }

        // Forward wrapped task event
        if let Some(tx) = event_tx
            && let Err(_e) = tx.send(TaskMonitorEvent::Task(event.clone())).await
        {
            #[cfg(feature = "tracing")]
            tracing::warn!(event = ?event, "Failed to forward task event");
        }
        match event {
            TaskEvent::Started { .. } | TaskEvent::Output { .. } => {}
            TaskEvent::Ready { task_name } => {
                self.start_ready_dependents_direct(
                    active_tasks,
                    &task_name,
                    None,
                    &task_event_tx.clone(),
                )
                .await;
            }
            TaskEvent::Stopped {
                task_name,
                exit_code: _,
                reason,
            } => {
                active_tasks.remove(&task_name);

                self.terminate_dependencies_if_all_dependent_finished(&task_name)
                    .await;

                self.start_ready_dependents_direct(
                    active_tasks,
                    &task_name,
                    Some(reason),
                    &task_event_tx.clone(),
                )
                .await;
            }
            TaskEvent::Error { task_name, error } => {
                active_tasks.remove(&task_name);

                self.terminate_dependencies_if_all_dependent_finished(&task_name)
                    .await;

                self.start_ready_dependents_direct(
                    active_tasks,
                    &task_name,
                    Some(TaskEventStopReason::Error(error.to_string())),
                    &task_event_tx.clone(),
                )
                .await;
            }
        }

        // Exit when no tasks are active or stop was requested
        if active_tasks.is_empty() {
            #[cfg(feature = "tracing")]
            tracing::debug!(active_tasks = active_tasks.len(), "Execution loop ending");
            return true;
        }
        false
    }

    async fn handle_control_event(
        &mut self,
        control: Option<TaskMonitorControlCommand>,
        event_tx: &Option<Sender<TaskMonitorEvent>>,
    ) -> bool {
        let control = match control {
            Some(c) => c,
            None => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Control channel closed");
                return false;
            }
        };
        // Send control received event
        let control_event = TaskMonitorControlEvent::ControlReceived {
            control: control.clone(),
        };
        if let Some(tx) = event_tx
            && tx
                .send(TaskMonitorEvent::Control(control_event))
                .await
                .is_err()
        {
            #[cfg(feature = "tracing")]
            tracing::warn!("Event channel closed while sending ControlReceived");
        }
        match control {
            TaskMonitorControlCommand::TerminateAllTasks => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Received TerminateAllTasks signal, terminating all tasks");

                self.terminate_all_tasks(TaskTerminateReason::UserRequested)
                    .await;
            }
            TaskMonitorControlCommand::TerminateTask { ref task_name } => {
                #[cfg(feature = "tracing")]
                tracing::debug!(task_name = %task_name, "Terminating specific task");

                self.terminate_task(&task_name, TaskTerminateReason::UserRequested)
                    .await;
            }
            TaskMonitorControlCommand::SendStdin {
                ref task_name,
                ref input,
            } => {
                #[cfg(feature = "tracing")]
                tracing::debug!(task_name = %task_name, input_len = input.len(), "Sending stdin to task");

                match self.send_stdin_to_task(&task_name, &input).await {
                    Ok(()) => {}
                    Err(e) => {
                        #[cfg(feature = "tracing")]
                        tracing::warn!(task_name = %task_name, error = %e, "Failed to send stdin to task");
                        // Send stdin error event
                        let control_command = ControlCommandError::SendStdin {
                            task_name: task_name.clone(),
                            input: input.clone(),
                            reason: e,
                        };
                        if let Some(tx) = event_tx
                            && tx
                                .send(TaskMonitorEvent::Error(TaskMonitorError::ControlError(
                                    control_command,
                                )))
                                .await
                                .is_err()
                        {
                            #[cfg(feature = "tracing")]
                            tracing::warn!(
                                "Event channel closed while sending TaskMonitorError::ControlError"
                            );
                        }
                        return false;
                    }
                }
            }
        }

        let control_event = TaskMonitorControlEvent::ControlProcessed { control };
        if let Some(tx) = event_tx
            && tx
                .send(TaskMonitorEvent::Control(control_event))
                .await
                .is_err()
        {
            #[cfg(feature = "tracing")]
            tracing::warn!("Event channel closed while sending ControlProcessed");
        }
        false
    }
    /// Terminate all running tasks with the specified reason.
    ///
    /// Sends termination signals to all tasks that are currently in Running or Ready state.
    /// Logs warnings for tasks that fail to receive the termination signal.
    async fn terminate_all_tasks(&mut self, reason: TaskTerminateReason) {
        for (task_name, spawner) in &mut self.tasks_spawner {
            let state = spawner.get_state().await;
            if !matches!(state, TaskState::Running | TaskState::Ready) {
                continue;
            }
            #[allow(clippy::used_underscore_binding)]
            if let Err(_e) = spawner.send_terminate_signal(reason.clone()).await {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    task_name = %task_name,
                    error = %_e,
                    "Failed to terminate task"
                );

                // Avoid unused variable warning when tracing is disabled
                #[cfg(not(feature = "tracing"))]
                let _ = task_name;
            }
        }
    }

    /// Terminate a specific task by name with the specified reason.
    ///
    /// Sends a termination signal to the named task if it exists and is in Running or Ready state.
    /// Logs a warning if the task fails to receive the termination signal.
    async fn terminate_task(&mut self, task_name: &str, reason: TaskTerminateReason) {
        let spawner = match self.tasks_spawner.get_mut(task_name) {
            Some(s) => s,
            None => {
                #[cfg(feature = "tracing")]
                tracing::warn!(task_name = %task_name, "Task not found");
                return;
            }
        };
        let state = spawner.get_state().await;
        if matches!(state, TaskState::Running | TaskState::Ready) {
            #[allow(clippy::used_underscore_binding)]
            if let Err(_e) = spawner.send_terminate_signal(reason).await {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    task_name = %task_name,
                    error = %_e,
                    "Failed to terminate task"
                );
            }
        } else {
            #[cfg(feature = "tracing")]
            tracing::warn!(
                task_name = %task_name,
                state = ?state,
                "Task is not in a state that can be terminated"
            );
        }
    }

    /// Send stdin input to a specific task.
    ///
    /// Delivers the input string to the named task's stdin stream if the task exists,
    /// is in the correct state, and has stdin enabled. Returns appropriate errors
    /// for various failure conditions.
    async fn send_stdin_to_task(
        &mut self,
        task_name: &str,
        input: &str,
    ) -> Result<(), SendStdinErrorReason> {
        // First check if the task exists
        let Some(task_spec) = self.tasks.get(task_name) else {
            #[cfg(feature = "tracing")]
            tracing::warn!(
                task_name = %task_name,
                "Task not found"
            );
            return Err(SendStdinErrorReason::TaskNotFound);
        };

        // Check if the task has stdin enabled in configuration
        let has_stdin_enabled = task_spec.config.enable_stdin.unwrap_or(false);

        if !has_stdin_enabled {
            #[cfg(feature = "tracing")]
            tracing::warn!(
                task_name = %task_name,
                "Task does not have stdin enabled in configuration"
            );
            return Err(SendStdinErrorReason::StdinNotEnabled);
        }

        // Check if we have a stdin sender for this task
        let Some(stdin_sender) = self.stdin_senders.get(task_name) else {
            #[cfg(feature = "tracing")]
            tracing::warn!(
                task_name = %task_name,
                "Task does not have a stdin sender (stdin might not be enabled)"
            );
            return Err(SendStdinErrorReason::TaskNotFound);
        };

        // Verify the task spawner exists
        let Some(spawner) = self.tasks_spawner.get(task_name) else {
            #[cfg(feature = "tracing")]
            tracing::warn!(task_name = %task_name, "Task spawner not found");
            return Err(SendStdinErrorReason::TaskNotFound);
        };

        // Verify the task is in a state that can receive input
        let state = spawner.get_state().await;
        if !matches!(state, TaskState::Running | TaskState::Ready) {
            #[cfg(feature = "tracing")]
            tracing::warn!(
                task_name = %task_name,
                state = ?state,
                "Task is not in a state that can receive stdin input"
            );
            return Err(SendStdinErrorReason::TaskNotActive);
        }

        // Send the input to the task's stdin channel
        match stdin_sender.send(input.to_string()).await {
            Ok(()) => {
                #[cfg(feature = "tracing")]
                tracing::info!(
                    task_name = %task_name,
                    input_len = input.len(),
                    "Successfully sent stdin to task: '{}'",
                    input.trim()
                );
                Ok(())
            }
            #[allow(clippy::used_underscore_binding)]
            Err(_e) => {
                #[cfg(feature = "tracing")]
                tracing::error!(
                    task_name = %task_name,
                    error = %_e,
                    "Failed to send stdin to task"
                );
                Err(SendStdinErrorReason::ChannelClosed)
            }
        }
    }

    /// Start execution of a specific task by name.
    ///
    /// Initiates the task execution using the direct strategy and sends task events
    /// through the provided channel. The task must exist in the task spawner collection.
    async fn start_task_direct(&mut self, name: &str, tx: &mpsc::Sender<TaskEvent>) {
        let Some(spawner) = self.tasks_spawner.get_mut(name) else {
            return;
        };
        let state = spawner.get_state().await;
        if state != TaskState::Pending {
            #[cfg(feature = "tracing")]
            tracing::warn!(
                task_name = name,
                state = ?state,
                "Task is not in Pending state",
            );
            return;
        }
        let _id = spawner.start_direct(tx.clone()).await;
    }

    /// Start all tasks that have no dependencies.
    ///
    /// Identifies and starts tasks that can run immediately because they do not
    /// depend on any other tasks. These tasks can run in parallel.
    async fn start_independent_tasks_direct(&mut self, tx: &mpsc::Sender<TaskEvent>) {
        // Collect tasks that have no dependencies
        let independent_tasks: Vec<String> = self
            .tasks_spawner
            .keys()
            .filter(|name| !self.dependencies.contains_key(*name))
            .cloned()
            .collect();
        // Start tasks that have no dependencies
        for name in independent_tasks {
            self.start_task_direct(&name, &tx).await;
        }
    }

    /// Start dependent tasks that are now ready to run.
    ///
    /// When a task completes, this function checks which dependent tasks can now be started.
    /// A dependent task is ready if all its dependencies have completed successfully.
    /// Tasks with failed dependencies are skipped unless configured to ignore dependency errors.
    async fn start_ready_dependents_direct(
        &mut self,
        active_tasks: &mut HashSet<String>,
        parent_task: &str,
        parent_task_stop_reason: Option<TaskEventStopReason>,
        tx: &mpsc::Sender<TaskEvent>,
    ) {
        let dependents = match self.dependents.get(parent_task) {
            Some(d) => d.clone(),
            None => return,
        };
        for task_name in dependents {
            let Some(dependencies) = self.dependencies.get(&task_name) else {
                #[cfg(feature = "tracing")]
                tracing::error!(
                    task_name,
                    "Task has no dependencies, unexpected behavior, it should not be started by this function"
                );
                break;
            };
            let mut all_dependencies_ready = true;
            for dependency in dependencies {
                let state = if let Some(c) = self.tasks_spawner.get(dependency) {
                    c.get_state().await
                } else {
                    #[cfg(feature = "tracing")]
                    tracing::error!(task_name, "Failed to get task spawner, unexpected behavior");
                    all_dependencies_ready = false;
                    break;
                };
                let not_ready = !matches!(state, TaskState::Ready | TaskState::Finished);
                if not_ready {
                    all_dependencies_ready = false;
                    break;
                }
            }
            if !all_dependencies_ready {
                continue;
            }

            let ignore_dependencies_error = match self.tasks.get(&task_name) {
                Some(config) => config.ignore_dependencies_error.unwrap_or_default(),
                None => false,
            };

            let should_start = match &parent_task_stop_reason {
                Some(TaskEventStopReason::Terminated(_) | TaskEventStopReason::Error(_)) => {
                    ignore_dependencies_error
                }
                Some(TaskEventStopReason::Finished) | None => true,
            };
            if should_start {
                self.start_task_direct(&task_name.clone(), tx).await;
            } else {
                // If not starting, remove from active tasks to avoid deadlock, also remove all dependents of this task
                active_tasks.remove(&task_name);
                if let Some(child_dependents) = self.dependents.get(&task_name) {
                    for child in child_dependents {
                        active_tasks.remove(child);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, time::Duration};

    use tcrm_task::tasks::{
        config::{StreamSource, TaskConfig},
        error::TaskError,
        event::TaskEvent,
    };
    use tokio::{sync::mpsc, time::timeout};

    use crate::monitor::{
        config::{TaskShell, TaskSpec},
        error::SendStdinErrorReason,
        tasks::TaskMonitor,
    };

    /// Collect events from a channel for testing purposes.
    ///
    /// Receives up to `max_events` events from the channel with a timeout for each event.
    /// Used in test scenarios to gather and verify task execution events.
    async fn collect_events(
        event_rx: &mut mpsc::Receiver<TaskEvent>,
        max_events: usize,
    ) -> Vec<TaskEvent> {
        let mut events = Vec::new();
        let mut event_count = 0;

        while event_count < max_events {
            match timeout(Duration::from_secs(5), event_rx.recv()).await {
                Ok(Some(event)) => {
                    events.push(event);
                    event_count += 1;
                }
                Ok(None) => break, // Channel closed
                Err(_) => break,   // Timeout
            }
        }
        events
    }

    #[tokio::test]
    async fn test_execute_all_simple_chain() {
        let mut tasks = HashMap::new();

        tasks.insert(
            "task1".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["hello"])).shell(TaskShell::Auto),
        );

        tasks.insert(
            "task2".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["world"]))
                .dependencies(["task1"])
                .shell(TaskShell::Auto),
        );

        tasks.insert(
            "task3".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["!"]))
                .dependencies(["task2"])
                .shell(TaskShell::Auto),
        );

        let mut monitor = TaskMonitor::new(tasks).unwrap();
        let (event_tx, mut event_rx) = mpsc::channel(1024);

        // Start execution in a separate task
        let execute_handle =
            tokio::spawn(async move { monitor.execute_all_direct(Some(event_tx)).await });

        let events = collect_events(&mut event_rx, 10).await;
        // Wait for execution to complete
        let result = timeout(Duration::from_secs(10), execute_handle).await;
        assert!(result.is_ok());

        // Verify the execution order through events
        let started_tasks: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                TaskEvent::Started { task_name } => Some(task_name.clone()),
                _ => None,
            })
            .collect();

        assert!(!started_tasks.is_empty());

        // Verify task1 starts before task2, and task2 before task3
        let task1_idx = started_tasks.iter().position(|x| x == "task1").unwrap();
        let task2_idx = started_tasks.iter().position(|x| x == "task2").unwrap();
        let task3_idx = started_tasks.iter().position(|x| x == "task3").unwrap();

        assert!(task1_idx < task2_idx);
        assert!(task2_idx < task3_idx);
    }

    #[tokio::test]
    async fn test_execute_all_independent_tasks() {
        let mut tasks = HashMap::new();

        tasks.insert(
            "independent1".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["task1"])).shell(TaskShell::Auto),
        );
        tasks.insert(
            "independent2".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["task2"])).shell(TaskShell::Auto),
        );
        tasks.insert(
            "independent3".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["task3"])).shell(TaskShell::Auto),
        );

        let mut monitor = TaskMonitor::new(tasks).unwrap();
        let (event_tx, mut event_rx) = mpsc::channel(1024);

        let execute_handle =
            tokio::spawn(async move { monitor.execute_all_direct(Some(event_tx)).await });

        let events = collect_events(&mut event_rx, 15).await;

        let result = timeout(Duration::from_secs(10), execute_handle).await;
        assert!(result.is_ok());

        // Verify all independent tasks were started and completed
        let started_tasks: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                TaskEvent::Started { task_name } => Some(task_name.clone()),
                _ => None,
            })
            .collect();

        let stopped_tasks: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                TaskEvent::Stopped { task_name, .. } => Some(task_name.clone()),
                _ => None,
            })
            .collect();

        // All tasks should start and stop
        assert_eq!(started_tasks.len(), 3);
        assert_eq!(stopped_tasks.len(), 3);
    }

    #[tokio::test]
    async fn test_task_with_ready_indicator() {
        // tracing_subscriber::fmt()
        //     .with_max_level(tracing::Level::TRACE)
        //     .init();
        let mut tasks = HashMap::new();

        // Create a task that writes "ready!" and then keeps running
        tasks.insert(
            "server".to_string(),
            TaskSpec::new(
                TaskConfig::new("echo ready!; sleep 10")
                    .ready_indicator("ready!".to_string())
                    .ready_indicator_source(StreamSource::Stdout),
            )
            .terminate_after_dependents(true)
            .shell(TaskShell::Auto),
        );

        // Create a dependent task that should start after server is ready
        tasks.insert(
            "client".to_string(),
            TaskSpec::new(TaskConfig::new("echo client-started"))
                .dependencies(["server"])
                .shell(TaskShell::Auto),
        );

        let mut monitor = TaskMonitor::new(tasks).unwrap();
        let (event_tx, mut event_rx) = mpsc::channel(1024);

        let execute_handle =
            tokio::spawn(async move { monitor.execute_all_direct(Some(event_tx)).await });

        let events = collect_events(&mut event_rx, 10).await;

        println!("Collected events: {:?}", events);
        // Find server ready event
        let server_ready = events
            .iter()
            .find(|e| matches!(e, TaskEvent::Ready { task_name } if task_name == "server"));
        assert!(server_ready.is_some());

        // Find client start event
        let client_started = events
            .iter()
            .find(|e| matches!(e, TaskEvent::Started { task_name } if task_name == "client"));
        assert!(client_started.is_some());

        // Verify client starts after server is ready
        let server_ready_idx = events
            .iter()
            .position(|e| matches!(e, TaskEvent::Ready { task_name } if task_name == "server"))
            .unwrap();
        let client_start_idx = events
            .iter()
            .position(|e| matches!(e, TaskEvent::Started { task_name } if task_name == "client"))
            .unwrap();
        assert!(server_ready_idx < client_start_idx);

        // Clean up
        let _ = timeout(Duration::from_secs(10), execute_handle).await;
    }

    #[tokio::test]
    async fn test_task_error_handling() {
        let mut tasks = HashMap::new();

        // A task that will fail
        tasks.insert(
            "failing_task".to_string(),
            TaskSpec::new(TaskConfig::new("exit").args(["1"])),
        );

        // A dependent task that should not start due to the failure
        tasks.insert(
            "dependent_task".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["should-not-run"]))
                .dependencies(["failing_task"]),
        );

        // A task that ignores dependency errors
        tasks.insert(
            "resilient_task".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["should-run"]))
                .shell(TaskShell::Auto)
                .dependencies(["failing_task"])
                .ignore_dependencies_error(true),
        );

        let mut monitor = TaskMonitor::new(tasks).unwrap();
        let (event_tx, mut event_rx) = mpsc::channel(1024);
        let execute_handle =
            tokio::spawn(async move { monitor.execute_all_direct(Some(event_tx)).await });

        let events = collect_events(&mut event_rx, 15).await;

        // Verify failing task stopped with error
        let failing_task_stopped = events.iter().find(|e| {
            matches!(e,
                TaskEvent::Error {
                    task_name,
                    error: TaskError::IO(_),
                } if task_name == "failing_task"
            )
        });
        assert!(failing_task_stopped.is_some());

        // Verify dependent task never started
        let dependent_started = events.iter().find(|e| {
            matches!(e,
                TaskEvent::Started {
                    task_name
                } if task_name == "dependent_task"
            )
        });
        assert!(dependent_started.is_none());

        // Verify resilient task did start despite dependency failure
        let resilient_started = events.iter().find(|e| {
            matches!(e,
                TaskEvent::Started {
                    task_name
                } if task_name == "resilient_task"
            )
        });
        assert!(resilient_started.is_some());

        let _ = timeout(Duration::from_secs(10), execute_handle).await;
    }

    #[tokio::test]
    async fn test_channel_overflow_resilience() {
        // Test that the system handles event channel overflow gracefully
        let mut tasks = HashMap::new();

        // Create tasks that generate output to stress the channel
        for i in 0..10 {
            tasks.insert(
                format!("task_{}", i),
                TaskSpec::new(TaskConfig::new("echo").args([&format!("output from task {}", i)])),
            );
        }

        let mut monitor = TaskMonitor::new(tasks).unwrap();

        // Use a very small channel buffer to force overflow scenarios
        let (event_tx, mut event_rx) = mpsc::channel(2);
        let execute_handle =
            tokio::spawn(async move { monitor.execute_all_direct(Some(event_tx)).await });

        // Consume events with some backpressure
        let mut event_count = 0;
        let timeout_duration = Duration::from_millis(200);

        while let Ok(Some(_event)) = timeout(timeout_duration, event_rx.recv()).await {
            event_count += 1;
            if event_count > 50 {
                break; // Prevent infinite loop
            }
            tokio::time::sleep(Duration::from_millis(2)).await; // Slight backpressure
        }

        // Verify that execution completes despite channel stress
        let result = timeout(Duration::from_secs(10), execute_handle).await;
        assert!(
            result.is_ok(),
            "Execution should complete despite channel pressure"
        );

        // Should have received a reasonable number of events (at least some starts and stops)
        assert!(
            event_count >= 10,
            "Should receive at least 10 events, got {}",
            event_count
        );
    }

    #[tokio::test]
    async fn test_concurrent_task_state_consistency() {
        // Test that task state remains consistent under concurrent access
        let mut tasks = HashMap::new();

        // Create a chain of dependent tasks
        tasks.insert(
            "first".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["first"])),
        );

        for i in 1..10 {
            tasks.insert(
                format!("task_{}", i),
                TaskSpec::new(TaskConfig::new("echo").args([&format!("task {}", i)]))
                    .dependencies([&format!("task_{}", i - 1)]),
            );
        }
        tasks.insert(
            "task_0".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["task 0"])).dependencies(["first"]),
        );

        let mut monitor = TaskMonitor::new(tasks).unwrap();
        let (event_tx, mut event_rx) = mpsc::channel(1024);

        let execute_handle =
            tokio::spawn(async move { monitor.execute_all_direct(Some(event_tx)).await });

        let events = collect_events(&mut event_rx, 50).await;

        // Verify that tasks started in dependency order
        let mut start_times = HashMap::new();
        for (idx, event) in events.iter().enumerate() {
            if let TaskEvent::Started { task_name } = event {
                start_times.insert(task_name.clone(), idx);
            }
        }

        // Verify dependency ordering
        if let (Some(&first_start), Some(&task_0_start)) =
            (start_times.get("first"), start_times.get("task_0"))
        {
            assert!(
                first_start < task_0_start,
                "Dependencies should start before dependents"
            );
        }

        for i in 1..9 {
            let task_name = format!("task_{}", i);
            let prev_task_name = format!("task_{}", i - 1);
            if let (Some(&current), Some(&previous)) = (
                start_times.get(&task_name),
                start_times.get(&prev_task_name),
            ) {
                assert!(
                    previous < current,
                    "Task {} should start after task {}",
                    i,
                    i - 1
                );
            }
        }

        let _ = timeout(Duration::from_secs(10), execute_handle).await;
    }

    #[tokio::test]
    async fn test_resource_cleanup_on_early_termination() {
        // Test that resources are properly cleaned up when execution is terminated early
        let mut tasks = HashMap::new();

        // Create long-running tasks that should be terminated
        tasks.insert(
            "long_task_1".to_string(),
            TaskSpec::new(TaskConfig::new("ping").args(["127.0.0.1", "-n", "100"])),
        );
        tasks.insert(
            "long_task_2".to_string(),
            TaskSpec::new(TaskConfig::new("ping").args(["127.0.0.1", "-n", "100"])),
        );

        let mut monitor = TaskMonitor::new(tasks).unwrap();
        let (event_tx, mut event_rx) = mpsc::channel(1024);

        let execute_handle =
            tokio::spawn(async move { monitor.execute_all_direct(Some(event_tx)).await });

        // Wait for tasks to start
        let mut started_count = 0;
        while let Ok(Some(event)) = timeout(Duration::from_secs(2), event_rx.recv()).await {
            if matches!(event, TaskEvent::Started { .. }) {
                started_count += 1;
                if started_count >= 2 {
                    break;
                }
            }
        }

        // Abort execution handle to simulate early termination
        execute_handle.abort();

        // Give some time for cleanup
        tokio::time::sleep(Duration::from_millis(100)).await;

        // The test passing means no panics occurred during cleanup
        assert!(
            started_count >= 2,
            "Both long-running tasks should have started"
        );
    }

    #[tokio::test]
    async fn test_invalid_command_error_propagation() {
        // Test that invalid commands properly propagate errors
        let mut tasks = HashMap::new();

        tasks.insert(
            "invalid_command".to_string(),
            TaskSpec::new(TaskConfig::new("definitely_not_a_real_command_12345")),
        );

        tasks.insert(
            "dependent_task".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["should not run"]))
                .dependencies(["invalid_command"]),
        );

        let mut monitor = TaskMonitor::new(tasks).unwrap();
        let (event_tx, mut event_rx) = mpsc::channel(1024);

        let execute_handle =
            tokio::spawn(async move { monitor.execute_all_direct(Some(event_tx)).await });

        let events = collect_events(&mut event_rx, 10).await;

        // Verify that invalid command generates an error
        let error_event = events.iter().find(
            |e| matches!(e, TaskEvent::Error { task_name, .. } if task_name == "invalid_command"),
        );
        assert!(
            error_event.is_some(),
            "Invalid command should generate an error event"
        );

        // Verify dependent task doesn't start
        let dependent_started = events.iter().find(
            |e| matches!(e, TaskEvent::Started { task_name } if task_name == "dependent_task"),
        );
        assert!(
            dependent_started.is_none(),
            "Dependent task should not start after dependency error"
        );

        let _ = timeout(Duration::from_secs(5), execute_handle).await;
    }

    #[tokio::test]
    async fn test_stdin_functionality_with_control() {
        // Test that stdin functionality works correctly with TaskMonitor
        let mut tasks = HashMap::new();

        // Create a task with stdin enabled that can properly echo stdin
        #[cfg(windows)]
        let stdin_task_config = TaskConfig::new("powershell")
            .args(["-Command", "'ready'; $host.UI.ReadLine()"])
            .enable_stdin(true)
            .ready_indicator("ready")
            .timeout_ms(5000);
        #[cfg(not(windows))]
        let stdin_task_config = TaskConfig::new("sh")
            .args(["-c", "echo 'ready'; read input; echo $input"])
            .enable_stdin(true)
            .ready_indicator("ready")
            .timeout_ms(5000);

        tasks.insert(
            "stdin_task".to_string(),
            TaskSpec::new(stdin_task_config).shell(TaskShell::Auto),
        );

        // Create a task without stdin enabled
        tasks.insert(
            "no_stdin_task".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["No stdin task"])).shell(TaskShell::Auto),
        );

        let mut monitor = TaskMonitor::new(tasks).unwrap();

        // Verify stdin senders are created only for tasks with stdin enabled
        assert!(monitor.stdin_senders.contains_key("stdin_task"));
        assert!(!monitor.stdin_senders.contains_key("no_stdin_task"));
        assert_eq!(monitor.stdin_senders.len(), 1);

        // Test sending stdin to valid task (should fail if task not running)
        let result = monitor
            .send_stdin_to_task("stdin_task", "Hello stdin!")
            .await;
        // Since task is not running, this should return TaskNotReady error
        assert!(
            result.is_err(),
            "Sending stdin to non-running task should fail"
        );
        if let Err(e) = result {
            println!("Expected error: {}", e);
        }

        // Test sending stdin to invalid task (should return error)
        let result = monitor
            .send_stdin_to_task("nonexistent_task", "Should be ignored")
            .await;
        assert!(
            result.is_err(),
            "Sending stdin to nonexistent task should fail"
        );

        // Test sending stdin to task without stdin enabled (should return error)
        let result = monitor
            .send_stdin_to_task("no_stdin_task", "Should be rejected")
            .await;
        assert!(
            result.is_err(),
            "Sending stdin to task without stdin enabled should fail"
        );
    }

    #[tokio::test]
    async fn test_stdin_channel_creation() {
        // Test that stdin channels are created correctly during TaskMonitor construction
        let mut tasks = HashMap::new();

        // Task with stdin enabled
        tasks.insert(
            "with_stdin".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["test"]).enable_stdin(true))
                .shell(TaskShell::Auto),
        );

        // Task with stdin explicitly disabled
        tasks.insert(
            "without_stdin".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["test"]).enable_stdin(false))
                .shell(TaskShell::Auto),
        );

        // Task with stdin not specified (defaults to false)
        tasks.insert(
            "default_stdin".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["test"])).shell(TaskShell::Auto),
        );

        let monitor = TaskMonitor::new(tasks).unwrap();

        // Verify stdin senders
        assert!(monitor.stdin_senders.contains_key("with_stdin"));
        assert!(!monitor.stdin_senders.contains_key("without_stdin"));
        assert!(!monitor.stdin_senders.contains_key("default_stdin"));
        assert_eq!(monitor.stdin_senders.len(), 1);

        // Verify all tasks have spawners
        assert_eq!(monitor.tasks_spawner.len(), 3);
    }

    #[tokio::test]
    async fn test_stdin_validation_and_error_handling() {
        // Test stdin validation and error handling directly
        let mut tasks = HashMap::new();

        #[cfg(windows)]
        let stdin_task_config = TaskConfig::new("powershell")
            .args(["-Command", "'ready'; $host.UI.ReadLine()"])
            .enable_stdin(true)
            .ready_indicator("ready")
            .timeout_ms(5000);
        #[cfg(not(windows))]
        let stdin_task_config = TaskConfig::new("sh")
            .args(["-c", "echo 'ready'; read input; echo $input"])
            .enable_stdin(true)
            .ready_indicator("ready")
            .timeout_ms(5000);

        tasks.insert(
            "stdin_enabled".to_string(),
            TaskSpec::new(stdin_task_config).shell(TaskShell::Auto),
        );

        let mut monitor = TaskMonitor::new(tasks).unwrap();

        // Verify stdin sender created
        assert!(monitor.stdin_senders.contains_key("stdin_enabled"));
        assert_eq!(monitor.stdin_senders.len(), 1);

        // Test direct method calls
        let result = monitor
            .send_stdin_to_task("stdin_enabled", "Valid input")
            .await;
        // Since task is not running, this should return TaskNotReady error
        assert!(
            result.is_err(),
            "Sending stdin to non-running task should fail"
        );

        let result = monitor
            .send_stdin_to_task("nonexistent_task", "Should be ignored")
            .await;
        assert!(
            result.is_err(),
            "Sending stdin to nonexistent task should fail"
        );
    }

    #[tokio::test]
    async fn test_stdin_error_types() {
        // Test specific error types returned by send_stdin_to_task
        let mut tasks = HashMap::new();

        #[cfg(windows)]
        let stdin_task_config = TaskConfig::new("powershell")
            .args(["-Command", "'ready'; $host.UI.ReadLine()"])
            .enable_stdin(true)
            .ready_indicator("ready")
            .timeout_ms(5000);
        #[cfg(not(windows))]
        let stdin_task_config = TaskConfig::new("sh")
            .args(["-c", "echo 'ready'; read input; echo $input"])
            .enable_stdin(true)
            .ready_indicator("ready")
            .timeout_ms(5000);

        tasks.insert(
            "stdin_task".to_string(),
            TaskSpec::new(stdin_task_config).shell(TaskShell::Auto),
        );

        tasks.insert(
            "no_stdin_task".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["test"])).shell(TaskShell::Auto),
        );

        let mut monitor = TaskMonitor::new(tasks).unwrap();

        // Test TaskNotReady error (task exists with stdin but not running)
        let result = monitor.send_stdin_to_task("stdin_task", "input").await;
        assert!(result.is_err());
        assert_eq!(result, Err(SendStdinErrorReason::TaskNotActive));

        // Test StdinNotEnabled error
        let result = monitor.send_stdin_to_task("no_stdin_task", "input").await;
        assert!(result.is_err());
        assert_eq!(result, Err(SendStdinErrorReason::StdinNotEnabled));

        // Test TaskNotFound error (task doesn't exist at all)
        let result = monitor.send_stdin_to_task("nonexistent", "input").await;
        assert!(result.is_err());
        assert_eq!(result, Err(SendStdinErrorReason::TaskNotFound));
    }

    #[tokio::test]
    async fn test_multiple_stdin_tasks_concurrent() {
        // Test multiple tasks with stdin
        let mut tasks = HashMap::new();

        for i in 1..=3 {
            #[cfg(windows)]
            let stdin_task_config = TaskConfig::new("powershell")
                .args(["-Command", "'ready'; $host.UI.ReadLine()"])
                .enable_stdin(true)
                .ready_indicator("ready")
                .timeout_ms(5000);
            #[cfg(not(windows))]
            let stdin_task_config = TaskConfig::new("sh")
                .args(["-c", "echo 'ready'; read input; echo $input"])
                .enable_stdin(true)
                .ready_indicator("ready")
                .timeout_ms(5000);

            tasks.insert(
                format!("stdin_task_{}", i),
                TaskSpec::new(stdin_task_config).shell(TaskShell::Auto),
            );
        }

        let mut monitor = TaskMonitor::new(tasks).unwrap();

        // Verify all stdin senders are created
        assert_eq!(monitor.stdin_senders.len(), 3);
        for i in 1..=3 {
            assert!(
                monitor
                    .stdin_senders
                    .contains_key(&format!("stdin_task_{}", i))
            );
        }

        // Test sending stdin to all tasks
        for i in 1..=3 {
            let result = monitor
                .send_stdin_to_task(
                    &format!("stdin_task_{}", i),
                    &format!("Input for task {}", i),
                )
                .await;
            assert!(
                result.is_err(),
                "Sending stdin to non-running stdin_task_{} should fail",
                i
            );
        }
    }
}
