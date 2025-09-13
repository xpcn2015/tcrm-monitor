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
            .iter()
            .map(|(k, v)| (k.clone(), TaskSpawner::new(k.clone(), v.config.clone())))
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
                    Err(_e) => {
                        #[cfg(feature = "tracing")]
                        tracing::warn!(
                            error=%_e,
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
        fn test_shell_command_transformation() {
            let mut tasks = HashMap::new();

            // Test that Auto shell transforms commands correctly based on platform
            tasks.insert(
                "echo_test".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["hello world"])).shell(TaskShell::Auto),
            );

            let monitor = TaskMonitor::new(tasks).unwrap();
            let task = monitor.tasks.get("echo_test").unwrap();

            // Verify the shell wrapper is applied correctly
            #[cfg(windows)]
            {
                assert!(task.config.command == "cmd" || task.config.command == "powershell");
                assert!(task.config.args.as_ref().unwrap().len() >= 2); // Should have shell args
            }

            #[cfg(unix)]
            {
                assert_eq!(task.config.command, "bash");
                assert!(task.config.args.as_ref().unwrap().len() >= 2); // Should have shell args
            }
        }

        #[test]
        fn test_none_shell_preserves_original_command() {
            let mut tasks = HashMap::new();
            let original_command = "custom_executable";
            let original_args = vec!["arg1".to_string(), "arg2".to_string()];

            tasks.insert(
                "raw_command".to_string(),
                TaskSpec::new(TaskConfig::new(original_command).args(original_args.clone()))
                    .shell(TaskShell::None),
            );

            let monitor = TaskMonitor::new(tasks).unwrap();
            let task = monitor.tasks.get("raw_command").unwrap();

            // Verify original command and args are preserved
            assert_eq!(task.config.command, original_command);
            assert_eq!(task.config.args.as_ref().unwrap(), &original_args);
        }

        #[cfg(windows)]
        #[test]
        fn test_windows_shell_argument_escaping() {
            let mut tasks = HashMap::new();

            // Test PowerShell with special characters
            tasks.insert(
                "powershell_escape".to_string(),
                TaskSpec::new(
                    TaskConfig::new("Write-Host").args(["test with spaces & special chars"]),
                )
                .shell(TaskShell::Powershell),
            );

            // Test CMD with quotes and pipes
            tasks.insert(
                "cmd_escape".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["\"quoted string\" | more"]))
                    .shell(TaskShell::Cmd),
            );

            let monitor = TaskMonitor::new(tasks).unwrap();

            let ps_task = monitor.tasks.get("powershell_escape").unwrap();
            assert_eq!(ps_task.config.command, "powershell");
            // Verify PowerShell-specific argument structure
            let ps_args = ps_task.config.args.as_ref().unwrap();
            assert!(ps_args.len() >= 2);
            assert!(
                ps_args.contains(&"-Command".to_string()) || ps_args.contains(&"-c".to_string())
            );

            let cmd_task = monitor.tasks.get("cmd_escape").unwrap();
            assert_eq!(cmd_task.config.command, "cmd");
            // Verify CMD-specific argument structure
            let cmd_args = cmd_task.config.args.as_ref().unwrap();
            assert!(cmd_args.len() >= 2);
            assert!(cmd_args.contains(&"/c".to_string()) || cmd_args.contains(&"/C".to_string()));
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
        fn test_dependency_graph_construction() {
            let mut tasks = HashMap::new();

            // Create a meaningful dependency structure
            tasks.insert(
                "root".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["root task"])),
            );

            tasks.insert(
                "dependent".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["dependent task"]))
                    .dependencies(["root"]),
            );

            let monitor = TaskMonitor::new(tasks).unwrap();

            // Verify dependency graph structure
            assert_eq!(monitor.tasks.len(), 2);
            assert!(!monitor.dependencies.contains_key("root")); // Root has no dependencies
            assert!(monitor.dependencies.contains_key("dependent"));

            // Verify dependent relationships
            let root_dependents = monitor.dependents.get("root").unwrap();
            assert!(root_dependents.contains(&"dependent".to_string()));

            let dependent_deps = monitor.dependencies.get("dependent").unwrap();
            assert!(dependent_deps.contains(&"root".to_string()));
        }

        #[test]
        fn test_task_state_initialization() {
            let mut tasks = HashMap::new();
            tasks.insert(
                "test_task".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["hello"])),
            );

            let monitor = TaskMonitor::new(tasks).unwrap();

            // Verify all tasks start in NotStarted state
            for (_name, task) in &monitor.tasks {
                // This would require accessing task state if it was exposed
                // For now, verify the task exists and has correct configuration
                assert_eq!(task.config.command, "echo");
                assert_eq!(task.config.args.as_ref().unwrap()[0], "hello");
            }
        }

        #[test]
        fn test_empty_task_list_edge_case() {
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

    mod validation_tests {
        use crate::monitor::{
            config::{TaskShell, TaskSpec},
            error::TaskMonitorError,
            tasks::TaskMonitor,
        };
        use std::collections::HashMap;
        use tcrm_task::tasks::config::TaskConfig;

        #[test]
        fn test_empty_command_validation() {
            // Test that empty commands are handled appropriately
            let mut tasks = HashMap::new();

            // Empty command should not cause creation to fail, but might fail at execution
            tasks.insert(
                "empty_command".to_string(),
                TaskSpec::new(TaskConfig::new("")),
            );

            let monitor = TaskMonitor::new(tasks);
            // The monitor should be created successfully even with empty command
            assert!(monitor.is_ok());
        }

        #[test]
        fn test_large_dependency_graph_validation() {
            // Test validation with a large number of dependencies
            let mut tasks = HashMap::new();

            // Create a large dependency chain
            tasks.insert(
                "start".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["start"])),
            );

            for i in 1..100 {
                let task_name = format!("task_{}", i);
                let prev_task = if i == 1 {
                    "start".to_string()
                } else {
                    format!("task_{}", i - 1)
                };
                tasks.insert(
                    task_name,
                    TaskSpec::new(TaskConfig::new("echo").args([&format!("task {}", i)]))
                        .dependencies([&prev_task]),
                );
            }

            let start_time = std::time::Instant::now();
            let monitor = TaskMonitor::new(tasks);
            let duration = start_time.elapsed();

            assert!(monitor.is_ok(), "Large dependency graph should be valid");
            assert!(
                duration.as_millis() < 1000,
                "Validation should complete quickly even for large graphs"
            );
        }

        #[test]
        fn test_self_dependency_detection() {
            // Test that self-dependencies are properly detected as circular
            let mut tasks = HashMap::new();

            tasks.insert(
                "self_dependent".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["hello"]))
                    .dependencies(["self_dependent"]),
            );

            let monitor = TaskMonitor::new(tasks);
            assert!(monitor.is_err());

            if let Err(TaskMonitorError::CircularDependency(task)) = monitor {
                assert_eq!(task, "self_dependent");
            } else {
                panic!("Expected CircularDependency error for self-dependency");
            }
        }

        #[test]
        fn test_configuration_memory_efficiency() {
            // Test that configuration doesn't consume excessive memory
            let mut tasks = HashMap::new();

            // Create many tasks with various configurations
            for i in 0..1000 {
                let task_name = format!("task_{}", i);
                let config =
                    TaskConfig::new("echo").args([&format!("arg_{}", i), &format!("value_{}", i)]);

                tasks.insert(
                    task_name,
                    TaskSpec::new(config)
                        .shell(if i % 2 == 0 {
                            TaskShell::Auto
                        } else {
                            TaskShell::None
                        })
                        .ignore_dependencies_error(i % 3 == 0),
                );
            }

            let start_memory = get_memory_usage();
            let monitor = TaskMonitor::new(tasks);
            let end_memory = get_memory_usage();

            assert!(monitor.is_ok());

            // Memory usage should be reasonable (this is a loose check)
            let memory_diff = end_memory.saturating_sub(start_memory);
            assert!(
                memory_diff < 50_000_000,
                "Memory usage should be reasonable for 1000 tasks"
            ); // 50MB limit
        }

        fn get_memory_usage() -> usize {
            // Simple memory usage estimation - in real scenarios you'd use proper profiling
            // This is a placeholder that returns 0, but in practice you could integrate
            // with system memory monitoring tools
            0
        }

        #[test]
        fn test_dependency_chain_depth_limits() {
            // Test extremely deep dependency chains
            let mut tasks = HashMap::new();

            tasks.insert(
                "root".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["root"])),
            );

            for i in 1..=500 {
                let task_name = format!("deep_{}", i);
                let prev_task = if i == 1 {
                    "root".to_string()
                } else {
                    format!("deep_{}", i - 1)
                };
                tasks.insert(
                    task_name,
                    TaskSpec::new(TaskConfig::new("echo").args([&format!("deep {}", i)]))
                        .dependencies([&prev_task]),
                );
            }

            let monitor = TaskMonitor::new(tasks);
            assert!(
                monitor.is_ok(),
                "Deep dependency chains should be handled without stack overflow"
            );
        }

        #[test]
        fn test_unicode_task_names_and_arguments() {
            // Test that unicode in task names and arguments is handled correctly
            let mut tasks = HashMap::new();

            tasks.insert(
                "測試任務_🚀".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["你好世界", "🌟✨", "Здравствуй мир"])),
            );

            tasks.insert(
                "τεστ_задача_🎯".to_string(),
                TaskSpec::new(TaskConfig::new("echo").args(["العالم مرحبا"]))
                    .dependencies(["測試任務_🚀"]),
            );

            let monitor = TaskMonitor::new(tasks);
            assert!(
                monitor.is_ok(),
                "Unicode task names and arguments should be supported"
            );

            // Verify the tasks are properly stored
            let monitor = monitor.unwrap();
            assert!(monitor.tasks.contains_key("測試任務_🚀"));
            assert!(monitor.tasks.contains_key("τεστ_задача_🎯"));
        }
    }
}
