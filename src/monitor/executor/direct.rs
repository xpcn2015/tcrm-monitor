use std::collections::HashSet;

use tcrm_task::tasks::{
    event::{TaskEvent, TaskEventStopReason},
    state::TaskState,
};
use tokio::sync::mpsc::{self, Sender};

use crate::monitor::tasks::TaskMonitor;

impl TaskMonitor {
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
    pub async fn execute_all_direct(&mut self, event_tx: Option<Sender<TaskEvent>>) {
        let (task_event_tx, mut task_event_rx) = mpsc::channel::<TaskEvent>(1024);
        self.start_independent_tasks_direct(&task_event_tx).await;

        // Keep track of active tasks
        let mut active_tasks: HashSet<String> = self.tasks_spawner.keys().cloned().collect();

        while let Some(event) = task_event_rx.recv().await {
            if let Some(ref tx) = event_tx {
                if let Err(_e) = tx.send(event.clone()).await {
                    #[cfg(feature = "tracing")]
                    tracing::warn!(event = ?event, "Failed to forward event");
                }
            }
            match event {
                TaskEvent::Started { .. } => {}
                TaskEvent::Output { .. } => {}
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

    async fn start_task_direct(&mut self, name: &str, tx: &mpsc::Sender<TaskEvent>) {
        let spawner = match self.tasks_spawner.get_mut(name) {
            Some(spawner) => spawner,
            None => return,
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
            self.start_task_direct(&name, &tx.clone()).await;
        }
    }

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
            let dependencies = match self.dependencies.get(&task_name) {
                Some(c) => c,
                None => {
                    #[cfg(feature = "tracing")]
                    tracing::error!(
                        task_name,
                        "Task has no dependencies, unexpected behavior, it should not be started by this function"
                    );
                    break;
                }
            };
            let mut all_dependencies_ready = true;
            for dependency in dependencies {
                let state = match self.tasks_spawner.get(dependency) {
                    Some(c) => c.get_state().await,
                    None => {
                        #[cfg(feature = "tracing")]
                        tracing::error!(
                            task_name,
                            "Failed to get task spawner, unexpected behavior"
                        );
                        all_dependencies_ready = false;
                        break;
                    }
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
                Some(TaskEventStopReason::Finished) => true,
                Some(TaskEventStopReason::Terminated(_)) => ignore_dependencies_error,
                Some(TaskEventStopReason::Error(_)) => ignore_dependencies_error,
                None => true,
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
        tasks::TaskMonitor,
    };

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
}
