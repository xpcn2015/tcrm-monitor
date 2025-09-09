use std::collections::HashMap;

use tcrm_task::tasks::{
    async_tokio::spawner::TaskSpawner,
    state::{TaskState, TaskTerminateReason},
};

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
                        #[cfg(feature = "tracing")]
                        tracing::warn!(
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

#[cfg(test)]
mod tests {

    mod dependency_tests {
        use std::collections::HashMap;

        use tcrm_task::tasks::config::TaskConfig;

        use crate::monitor::{
            config::{TaskShell, TaskSpec, TcrmTasks},
            error::TaskMonitorError,
            tasks::TaskMonitor,
        };

        #[test]
        fn test_valid_dependencies() {
            let mut tasks: TcrmTasks = HashMap::new();

            tasks.insert(
                "taskA".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["A"])).shell(TaskShell::Auto),
            );

            tasks.insert(
                "taskB".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["B"]))
                    .dependencies(["taskA"])
                    .shell(TaskShell::Auto),
            );

            tasks.insert(
                "taskC".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["C"]))
                    .dependencies(["taskA"])
                    .shell(TaskShell::Auto),
            );

            let monitor = TaskMonitor::new(tasks);
            assert!(monitor.is_ok());
        }

        #[test]
        fn test_circular_dependency() {
            let mut tasks = HashMap::new();

            tasks.insert(
                "taskA".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["A"]))
                    .dependencies(["taskB"])
                    .shell(TaskShell::Auto),
            );

            tasks.insert(
                "taskB".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["B"]))
                    .dependencies(["taskC"])
                    .shell(TaskShell::Auto),
            );

            tasks.insert(
                "taskC".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["C"]))
                    .dependencies(["taskA"])
                    .shell(TaskShell::Auto),
            );

            let monitor = TaskMonitor::new(tasks);
            assert!(monitor.is_err());

            match monitor.unwrap_err() {
                TaskMonitorError::CircularDependency(task) => {
                    assert!(["taskA", "taskB", "taskC"].contains(&task.as_str()));
                }
                _ => panic!("Expected CircularDependency error"),
            }
        }

        #[test]
        fn test_missing_dependency() {
            let mut tasks = HashMap::new();

            tasks.insert(
                "taskA".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["A"])).shell(TaskShell::Auto),
            );

            tasks.insert(
                "taskC".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["C"]))
                    .dependencies(["nonexistent_task"])
                    .shell(TaskShell::Auto),
            );

            let monitor = TaskMonitor::new(tasks);
            assert!(monitor.is_err());

            match monitor.unwrap_err() {
                TaskMonitorError::DependencyNotFound { dep, task } => {
                    assert_eq!(dep, "nonexistent_task");
                    assert_eq!(task, "taskC");
                }
                _ => panic!("Expected DependencyNotFound error"),
            }
        }

        #[test]
        fn test_complex_dependency_tree() {
            let mut tasks = HashMap::new();

            // Create a complex dependency tree:
            //     task1
            //    /     \
            //  task2   task3
            //    |       |
            //  task4   task5
            //    \     /
            //     task6

            tasks.insert(
                "task1".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["1"])),
            );
            tasks.insert(
                "task2".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["2"])).dependencies(["task1"]),
            );
            tasks.insert(
                "task3".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["3"])).dependencies(["task1"]),
            );
            tasks.insert(
                "task4".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["4"])).dependencies(["task2"]),
            );
            tasks.insert(
                "task5".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["5"])).dependencies(["task3"]),
            );
            tasks.insert(
                "task6".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["6"])).dependencies(["task4", "task5"]),
            );

            let monitor = TaskMonitor::new(tasks).unwrap();

            // Check dependencies are built correctly
            assert!(!monitor.dependencies.contains_key("task1")); // No dependencies
            assert_eq!(
                monitor.dependencies.get("task2"),
                Some(&vec!["task1".to_string()])
            );
            assert_eq!(
                monitor.dependencies.get("task3"),
                Some(&vec!["task1".to_string()])
            );

            // task6 should depend on all tasks (transitive dependencies)
            let task6_deps = monitor.dependencies.get("task6").unwrap();
            assert!(task6_deps.contains(&"task1".to_string()));
            assert!(task6_deps.contains(&"task2".to_string()));
            assert!(task6_deps.contains(&"task3".to_string()));
            assert!(task6_deps.contains(&"task4".to_string()));
            assert!(task6_deps.contains(&"task5".to_string()));
        }

        #[test]
        fn test_multiple_independent_chains() {
            let mut tasks = HashMap::new();

            // Chain 1: A -> B
            tasks.insert(
                "A".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["A"])),
            );
            tasks.insert(
                "B".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["B"])).dependencies(["A"]),
            );

            // Chain 2: X -> Y -> Z
            tasks.insert(
                "X".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["X"])),
            );
            tasks.insert(
                "Y".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["Y"])).dependencies(["X"]),
            );
            tasks.insert(
                "Z".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["Z"])).dependencies(["Y"]),
            );

            let monitor = TaskMonitor::new(tasks).unwrap();
            assert_eq!(monitor.tasks.len(), 5);

            // Verify independent chains don't interfere
            assert!(!monitor.dependencies.contains_key("A"));
            assert!(!monitor.dependencies.contains_key("X"));
            assert!(
                monitor
                    .dependencies
                    .get("B")
                    .unwrap()
                    .contains(&"A".to_string())
            );
            assert!(
                monitor
                    .dependencies
                    .get("Z")
                    .unwrap()
                    .contains(&"X".to_string())
            );
            assert!(
                monitor
                    .dependencies
                    .get("Z")
                    .unwrap()
                    .contains(&"Y".to_string())
            );
        }
    }

    mod shell_tests {
        use std::collections::HashMap;

        use tcrm_task::tasks::config::TaskConfig;

        use crate::monitor::{
            config::{TaskShell, TaskSpec},
            tasks::TaskMonitor,
        };

        #[test]
        fn test_auto_shell_configuration() {
            let mut tasks = HashMap::new();

            tasks.insert(
                "echo_hello".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["hello"])).shell(TaskShell::Auto),
            );

            tasks.insert(
                "echo_hi".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["hi"])).shell(TaskShell::Auto),
            );

            let monitor = TaskMonitor::new(tasks);
            assert!(monitor.is_ok());
        }

        #[test]
        fn test_none_shell_configuration() {
            let mut tasks = HashMap::new();
            tasks.insert(
                "raw_command".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["test"])).shell(TaskShell::None),
            );

            let monitor = TaskMonitor::new(tasks).unwrap();
            let task = monitor.tasks.get("raw_command").unwrap();
            assert_eq!(task.config.command, "echo");
        }

        #[cfg(windows)]
        #[test]
        fn test_windows_specific_shells() {
            let mut tasks = HashMap::new();

            // Test PowerShell
            tasks.insert(
                "powershell_task".to_string(),
                TaskSpec::new(TaskConfig::new("Write-Host").args(["test"]))
                    .shell(TaskShell::Powershell),
            );

            // Test CMD
            tasks.insert(
                "cmd_task".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["test"])).shell(TaskShell::Cmd),
            );

            let monitor = TaskMonitor::new(tasks).unwrap();

            let ps_task = monitor.tasks.get("powershell_task").unwrap();
            assert_eq!(ps_task.config.command, "powershell");

            let cmd_task = monitor.tasks.get("cmd_task").unwrap();
            assert_eq!(cmd_task.config.command, "cmd");
        }

        #[cfg(unix)]
        #[test]
        fn test_unix_specific_shells() {
            let mut tasks = HashMap::new();

            // Test Bash
            tasks.insert(
                "bash_task".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["test"])).shell(TaskShell::Bash),
            );

            let monitor = TaskMonitor::new(tasks).unwrap();
            let bash_task = monitor.tasks.get("bash_task").unwrap();
            assert_eq!(bash_task.config.command, "bash");
        }
    }

    mod task_lifecycle_tests {
        use std::collections::HashMap;

        use tcrm_task::tasks::{config::TaskConfig, state::TaskState};

        use crate::monitor::{config::TaskSpec, tasks::TaskMonitor};

        #[test]
        fn test_single_task_no_dependencies() {
            let mut tasks = HashMap::new();
            tasks.insert(
                "single".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["hello"])),
            );

            let monitor = TaskMonitor::new(tasks).unwrap();
            assert_eq!(monitor.tasks.len(), 1);
            assert!(monitor.dependencies.is_empty());
            assert!(monitor.dependents.is_empty());
        }

        #[test]
        fn test_empty_task_list() {
            let tasks = HashMap::new();
            let monitor = TaskMonitor::new(tasks);
            assert!(monitor.is_ok());
            assert_eq!(monitor.unwrap().tasks.len(), 0);
        }

        #[tokio::test]
        async fn test_terminate_after_dependents_finished() {
            let mut tasks = HashMap::new();

            // Setup a parent task that will be terminated when dependents finish
            tasks.insert(
                "parent".to_string(),
                TaskSpec::new(TaskConfig::new("sleep").args(["10"]))
                    .terminate_after_dependents(true),
            );

            // Add dependent tasks
            tasks.insert(
                "child1".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["test"])).dependencies(["parent"]),
            );

            tasks.insert(
                "child2".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["test"])).dependencies(["parent"]),
            );

            let mut monitor = TaskMonitor::new(tasks).unwrap();

            // Verify initial state
            assert!(monitor.dependencies.contains_key("child1"));
            assert!(monitor.dependencies.contains_key("child2"));
            assert!(monitor.dependents.contains_key("parent"));

            // Get state before marking as finished
            let parent_spawner = monitor.tasks_spawner.get("parent").unwrap();
            let initial_state = parent_spawner.get_state().await;
            assert_ne!(initial_state, TaskState::Finished);

            // Verify dependent task termination works
            monitor
                .terminate_dependencies_if_all_dependent_finished("child1")
                .await;
            monitor
                .terminate_dependencies_if_all_dependent_finished("child2")
                .await;
        }
    }
}
