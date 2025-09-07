use std::collections::HashMap;

use tcrm_task::tasks::{
    async_tokio::spawner::TaskSpawner,
    state::{TaskState, TaskTerminateReason},
};
use tracing::warn;

use crate::monitor::{
    config::{TaskShell, TcrmTasks},
    depend::{build_depend_map, check_circular_dependencies},
    error::TaskMonitorError,
};

#[derive(Debug)]
pub struct TaskMonitor {
    pub tasks: TcrmTasks,
    pub tasks_spawner: HashMap<String, TaskSpawner>,
    pub dependencies: HashMap<String, Vec<String>>,
    pub dependents: HashMap<String, Vec<String>>,
}
impl TaskMonitor {
    pub fn new(mut tasks: TcrmTasks) -> Result<Self, TaskMonitorError> {
        let depen = build_depend_map(&tasks)?;
        let dependencies = depen.dependencies;
        let dependents = depen.dependents;
        check_circular_dependencies(&dependencies)?;
        shell_tasks(&mut tasks);
        let tasks_spawner: HashMap<String, TaskSpawner> = tasks
            .clone()
            .into_iter()
            .map(|(k, v)| (k.clone(), TaskSpawner::new(k, v.config)))
            .collect();

        Ok(Self {
            tasks,
            tasks_spawner,
            dependencies,
            dependents,
        })
    }
    pub(crate) async fn terminate_dependencies_if_all_dependent_finished(
        &mut self,
        task_name: &str,
    ) {
        let dependencies = match self.dependencies.get(task_name) {
            Some(d) => d,
            None => return,
        };
        for name in dependencies {
            let task = match self.tasks.get(name) {
                Some(t) => t,
                None => continue,
            };
            if !task.terminate_after_dependents_finished.unwrap_or_default() {
                continue;
            }

            let dependents = match self.dependents.get(name) {
                Some(d) => d,
                None => continue,
            };

            let mut all_finished = true;
            for dep_name in dependents {
                let dep_spawner = match self.tasks_spawner.get(dep_name) {
                    Some(s) => s,
                    None => {
                        all_finished = false;
                        break;
                    }
                };
                let stopped = dep_spawner.get_state().await == TaskState::Finished;
                if !stopped {
                    all_finished = false;
                    break;
                }
            }

            if all_finished {
                let spawner = match self.tasks_spawner.get_mut(name) {
                    Some(s) => s,
                    None => continue,
                };
                match spawner
                    .send_terminate_signal(TaskTerminateReason::DependenciesFinished)
                    .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        warn!(
                            error=%e,
                            "Terminating dependencies failed",
                        );
                    }
                }
            }
        }
    }
}
fn shell_tasks(tasks: &mut TcrmTasks) {
    for (_, task_spec) in tasks.iter_mut() {
        // Get shell setting, use default if None
        let shell = task_spec.shell.clone().unwrap_or_default();

        // Only modify if shell is not None
        if shell != TaskShell::None {
            let original_command = task_spec.config.command.clone();

            // Update command and args based on shell type
            match shell {
                TaskShell::None => {
                    // No changes needed
                }
                #[cfg(windows)]
                TaskShell::Cmd => {
                    task_spec.config.command = "cmd".to_string();
                    let mut new_args = vec!["/C".to_string(), original_command];
                    if let Some(existing_args) = task_spec.config.args.take() {
                        new_args.extend(existing_args);
                    }
                    task_spec.config.args = Some(new_args);
                }
                #[cfg(windows)]
                TaskShell::Powershell => {
                    task_spec.config.command = "powershell".to_string();
                    let mut new_args = vec!["-Command".to_string(), original_command];
                    if let Some(existing_args) = task_spec.config.args.take() {
                        new_args.extend(existing_args);
                    }
                    task_spec.config.args = Some(new_args);
                }
                #[cfg(unix)]
                TaskShell::Bash => {
                    task_spec.config.command = "bash".to_string();
                    let mut new_args = vec!["-c".to_string(), original_command];
                    if let Some(existing_args) = task_spec.config.args.take() {
                        new_args.extend(existing_args);
                    }
                    task_spec.config.args = Some(new_args);
                }
                TaskShell::Auto => {
                    #[cfg(windows)]
                    {
                        task_spec.config.command = "powershell".to_string();
                        let mut new_args = vec!["-Command".to_string(), original_command];
                        if let Some(existing_args) = task_spec.config.args.take() {
                            new_args.extend(existing_args);
                        }
                        task_spec.config.args = Some(new_args);
                    }
                    #[cfg(unix)]
                    {
                        task_spec.config.command = "bash".to_string();
                        let mut new_args = vec!["-c".to_string(), original_command];
                        if let Some(existing_args) = task_spec.config.args.take() {
                            new_args.extend(existing_args);
                        }
                        task_spec.config.args = Some(new_args);
                    }
                }
            }
        }
    }
}
