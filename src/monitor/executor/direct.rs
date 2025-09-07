use std::collections::HashSet;

use tcrm_task::tasks::{
    event::{TaskEvent, TaskEventStopReason},
    state::TaskState,
};
use tokio::sync::mpsc::{self, Sender};
use tracing::{debug, warn};

use crate::monitor::{error::TaskMonitorError, tasks::TaskMonitor};

impl TaskMonitor {
    pub async fn execute_all_direct(
        &mut self,
        event_tx: Option<Sender<TaskEvent>>,
    ) -> Result<(), TaskMonitorError> {
        let (task_event_tx, mut task_event_rx) = mpsc::channel::<TaskEvent>(1024);
        self.start_independent_tasks_direct(&task_event_tx).await?;

        // Keep track of active tasks
        let mut active_tasks = HashSet::new();
        for name in self.tasks_spawner.keys() {
            active_tasks.insert(name.clone());
        }

        while let Some(event) = task_event_rx.recv().await {
            if let Some(ref tx) = event_tx {
                if let Err(e) = tx.send(event.clone()).await {
                    warn!("Failed to forward event: {}", e);
                }
            }
            match event {
                TaskEvent::Started { task_name } => {
                    debug!("Task [{}] started", task_name);
                }
                TaskEvent::Output {
                    task_name: _,
                    line: _,
                    src: _,
                } => {}
                TaskEvent::Ready { task_name } => {
                    debug!("Task [{}] ready", task_name);
                }
                TaskEvent::Stopped {
                    task_name,
                    exit_code: _,
                    reason,
                } => {
                    debug!("Task [{}] stopped", task_name);
                    active_tasks.remove(&task_name);

                    let ignore_dependencies_error = match self.tasks.get(&task_name) {
                        Some(config) => config.ignore_dependencies_error.unwrap_or(false),
                        None => true,
                    };

                    self.terminate_dependencies_if_all_dependent_finished(&task_name)
                        .await;

                    if reason != TaskEventStopReason::Finished && !ignore_dependencies_error {
                        continue;
                    }
                    self.start_child_tasks_direct(&task_name, &task_event_tx.clone())
                        .await?;
                }
                TaskEvent::Error { task_name, error } => {
                    warn!("Task [{}] error: {}", task_name, error);
                    break;
                }
            }

            // Exit when no tasks are active
            if active_tasks.is_empty() {
                debug!("All tasks completed");
                break;
            }
        }

        Ok(())
    }

    async fn start_task_direct(
        &mut self,
        name: &str,
        tx: &mpsc::Sender<TaskEvent>,
    ) -> Result<(), TaskMonitorError> {
        if let Some(spawner) = self.tasks_spawner.get_mut(name) {
            if spawner.get_state().await != TaskState::Pending {
                return Ok(());
            }
            let _ = spawner.start_direct(tx.clone()).await?;
        }
        Ok(())
    }

    async fn start_independent_tasks_direct(
        &mut self,
        tx: &mpsc::Sender<TaskEvent>,
    ) -> Result<(), TaskMonitorError> {
        // Collect tasks that have no dependencies
        let independent_tasks: Vec<String> = self
            .tasks_spawner
            .keys()
            .filter(|name| !self.dependencies.contains_key(*name))
            .cloned()
            .collect();
        // Start tasks that have no dependencies
        for name in independent_tasks {
            self.start_task_direct(&name, &tx.clone()).await?;
        }
        Ok(())
    }

    async fn start_child_tasks_direct(
        &mut self,
        task: &str,
        tx: &mpsc::Sender<TaskEvent>,
    ) -> Result<(), TaskMonitorError> {
        let dependents = match self.dependents.get(task) {
            Some(d) => d.clone(),
            None => return Ok(()),
        };
        for name in dependents {
            let dependencies = match self.dependencies.get(&name) {
                Some(c) => c,
                None => continue,
            };
            let mut all_parent_ready = true;
            for parent_name in dependencies {
                let state = match self.tasks_spawner.get(parent_name) {
                    Some(c) => c.get_state().await,
                    None => continue,
                };
                let not_ready = !matches!(state, TaskState::Ready | TaskState::Finished);
                if not_ready {
                    all_parent_ready = false;
                    break;
                }
            }
            if all_parent_ready {
                self.start_task_direct(&name.clone(), tx).await?;
            }
        }
        Ok(())
    }
}
