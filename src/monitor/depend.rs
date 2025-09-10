use std::collections::{HashMap, HashSet};

use crate::monitor::{config::TcrmTasks, error::TaskMonitorError};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub struct TaskMonitorDependMap {
    pub dependencies: HashMap<String, Vec<String>>,
    pub dependents: HashMap<String, Vec<String>>,
}
pub fn build_depend_map(tasks: &TcrmTasks) -> Result<TaskMonitorDependMap, TaskMonitorError> {
    let mut dependencies = HashMap::new();
    let mut dependents = HashMap::new();
    // Collect and expand dependencies
    for (name, config) in tasks {
        let direct_deps = config.dependencies.clone().unwrap_or_default();

        // Expand to all transitive dependencies
        let mut all_deps = HashSet::new();
        let mut stack = direct_deps;

        while let Some(dep) = stack.pop() {
            if !tasks.contains_key(&dep) {
                return Err(TaskMonitorError::DependencyNotFound {
                    dep: dep.clone(),
                    task: name.clone(),
                });
            }
            if all_deps.insert(dep.clone()) {
                let config = match tasks.get(&dep) {
                    Some(c) => c,
                    None => {
                        return Err(TaskMonitorError::ConfigParse(format!(
                            "Unable to get task: {}",
                            dep
                        )));
                    }
                };
                if let Some(dep_cfg) = &config.dependencies {
                    stack.extend(dep_cfg.clone());
                }
            }
        }

        // Insert full dependency list
        let all_deps_vec = all_deps.into_iter().collect::<Vec<_>>();
        if all_deps_vec.is_empty() {
            continue;
        }
        dependencies.insert(name.clone(), all_deps_vec.clone());
        // Build reverse dependency map
        for dep in &all_deps_vec {
            dependents
                .entry(dep.clone())
                .or_insert_with(Vec::new)
                .push(name.clone());
        }
    }
    Ok(TaskMonitorDependMap {
        dependencies,
        dependents,
    })
}

/// Check circular dependencies in tasks, tasks with no dependencies will not check
pub fn check_circular_dependencies(
    dependencies: &HashMap<String, Vec<String>>,
) -> Result<(), TaskMonitorError> {
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();

    for task_name in dependencies.keys() {
        if !visited.contains(task_name.as_str()) {
            if has_cycle(&dependencies, task_name, &mut visited, &mut rec_stack) {
                return Err(TaskMonitorError::CircularDependency(task_name.clone()));
            }
        }
    }
    Ok(())
}
fn has_cycle<'a>(
    dependencies: &'a HashMap<String, Vec<String>>,
    task_name: &'a str,
    visited: &mut HashSet<&'a str>,
    rec_stack: &mut HashSet<&'a str>,
) -> bool {
    visited.insert(task_name);
    rec_stack.insert(task_name);

    if let Some(deps) = dependencies.get(task_name) {
        for dep in deps {
            if !visited.contains(dep.as_str()) {
                if has_cycle(dependencies, dep, visited, rec_stack) {
                    return true;
                }
            } else if rec_stack.contains(dep.as_str()) {
                return true;
            }
        }
    }

    rec_stack.remove(task_name);
    false
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use tcrm_task::tasks::config::TaskConfig;

    use crate::monitor::{
        config::TaskSpec,
        depend::{build_depend_map, check_circular_dependencies},
        error::TaskMonitorError,
    };

    #[test]
    fn functional() {
        let mut tasks = HashMap::new();

        tasks.insert(
            "task1".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["hello"])),
        );

        tasks.insert(
            "task2".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["world"])).dependencies(["task1"]),
        );

        tasks.insert(
            "task3".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["!"])).dependencies(["task2"]),
        );
        let result = build_depend_map(&tasks).unwrap();

        // task1 has no dependencies
        assert!(!result.dependencies.contains_key("task1"));

        // task2 depends on task1
        assert_eq!(
            result.dependencies.get("task2"),
            Some(&vec!["task1".to_string()])
        );

        // task3 depends transitively on both task1 and task2
        let task3_deps = result.dependencies.get("task3").unwrap();
        assert!(task3_deps.contains(&"task1".to_string()));
        assert!(task3_deps.contains(&"task2".to_string()));

        // Check dependents
        let task1_dependents = result.dependents.get("task1").unwrap();
        assert!(task1_dependents.contains(&"task2".to_string()));
        assert!(task1_dependents.contains(&"task3".to_string()));
    }
    #[test]
    fn missing_dependency() {
        let mut tasks = HashMap::new();
        tasks.insert(
            "task1".to_string(),
            TaskSpec::new(TaskConfig::new("echo")).dependencies(["missing_task"]),
        );

        let result = build_depend_map(&tasks);

        assert!(result.is_err());
        match result.unwrap_err() {
            TaskMonitorError::DependencyNotFound { dep, task } => {
                assert_eq!(dep, "missing_task");
                assert_eq!(task, "task1");
            }
            _ => panic!("Expected DependencyNotFound error"),
        }
    }

    #[test]
    fn no_cycle_in_linear_dependencies() {
        // Linear dependency chain: task1 -> task2 -> task3
        let mut tasks = HashMap::new();
        tasks.insert(
            "task1".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["hello"])),
        );
        tasks.insert(
            "task2".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["world"])).dependencies(["task1"]),
        );
        tasks.insert(
            "task3".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["!"])).dependencies(["task2"]),
        );
        let depend_map = build_depend_map(&tasks).unwrap();
        let result = check_circular_dependencies(&depend_map.dependencies);
        assert!(
            result.is_ok(),
            "Should not detect a cycle in linear dependencies"
        );
    }

    #[test]
    fn empty_tasks_should_work() {
        // No tasks at all
        let tasks: HashMap<String, TaskSpec> = HashMap::new();
        let depend_map = build_depend_map(&tasks).unwrap();
        assert!(depend_map.dependencies.is_empty());
        assert!(depend_map.dependents.is_empty());
        let result = check_circular_dependencies(&depend_map.dependencies);
        assert!(result.is_ok(), "Empty tasks should not have cycles");
    }

    #[test]
    fn single_task_no_dependencies() {
        let mut tasks = HashMap::new();
        tasks.insert(
            "solo".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["one"])),
        );
        let depend_map = build_depend_map(&tasks).unwrap();
        assert!(depend_map.dependencies.is_empty());
        assert_eq!(depend_map.dependents.get("solo"), None);
        let result = check_circular_dependencies(&depend_map.dependencies);
        assert!(result.is_ok(), "Single task should not have cycles");
    }

    #[test]
    fn multiple_independent_tasks() {
        let mut tasks = HashMap::new();
        for i in 1..=3 {
            let name = format!("task{}", i);
            tasks.insert(
                name.clone(),
                TaskSpec::new(TaskConfig::new("echo").args([name.clone()])),
            );
        }
        let depend_map = build_depend_map(&tasks).unwrap();
        assert!(depend_map.dependencies.is_empty());
        for i in 1..=3 {
            assert_eq!(depend_map.dependents.get(&format!("task{}", i)), None);
        }
        let result = check_circular_dependencies(&depend_map.dependencies);
        assert!(result.is_ok(), "Independent tasks should not have cycles");
    }

    #[test]
    fn duplicate_dependencies_should_not_cause_error() {
        let mut tasks = HashMap::new();
        tasks.insert(
            "task1".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["hello"])),
        );
        tasks.insert(
            "task2".to_string(),
            TaskSpec::new(TaskConfig::new("echo").args(["world"])).dependencies(["task1", "task1"]),
        );
        let depend_map = build_depend_map(&tasks).unwrap();
        // Should only have one dependency after deduplication
        assert_eq!(
            depend_map.dependencies.get("task2"),
            Some(&vec!["task1".to_string()])
        );
        let result = check_circular_dependencies(&depend_map.dependencies);
        assert!(
            result.is_ok(),
            "Duplicate dependencies should not cause cycles"
        );
    }
    #[test]
    fn should_catch_cycle_error() {
        let mut dependencies = HashMap::new();
        dependencies.insert("taskA".to_string(), vec!["taskB".to_string()]);
        dependencies.insert("taskB".to_string(), vec!["taskC".to_string()]);
        dependencies.insert("taskC".to_string(), vec!["taskA".to_string()]);

        let result = check_circular_dependencies(&dependencies);
        assert!(result.is_err());

        match result.unwrap_err() {
            TaskMonitorError::CircularDependency(task) => {
                assert!(["taskA", "taskB", "taskC"].contains(&task.as_str()));
            }
            _ => panic!("Expected CircularDependency error"),
        }
    }
    #[test]
    fn should_catch_self_reference() {
        let mut dependencies = HashMap::new();
        dependencies.insert("taskA".to_string(), vec!["taskA".to_string()]);

        let result = check_circular_dependencies(&dependencies);
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskMonitorError::CircularDependency(task) => {
                assert!(["taskA"].contains(&task.as_str()));
            }
            _ => panic!("Expected CircularDependency error"),
        }
    }
    #[test]
    fn detects_cycle_in_chain_b_c_d_b() {
        // a -> b -> c -> d -> b
        let mut dependencies = HashMap::new();
        dependencies.insert("a".to_string(), vec!["b".to_string()]);
        dependencies.insert("b".to_string(), vec!["c".to_string()]);
        dependencies.insert("c".to_string(), vec!["d".to_string()]);
        dependencies.insert("d".to_string(), vec!["b".to_string()]);
        let result = check_circular_dependencies(&dependencies);
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskMonitorError::CircularDependency(task) => {
                assert!(["a", "b", "c", "d"].contains(&task.as_str()));
            }
            _ => panic!("Expected CircularDependency error"),
        }
    }

    #[test]
    fn detects_cycle_in_chain_a_b_c_d_a() {
        // a -> b -> c -> d -> a
        let mut dependencies = HashMap::new();
        dependencies.insert("a".to_string(), vec!["b".to_string()]);
        dependencies.insert("b".to_string(), vec!["c".to_string()]);
        dependencies.insert("c".to_string(), vec!["d".to_string()]);
        dependencies.insert("d".to_string(), vec!["a".to_string()]);
        let result = check_circular_dependencies(&dependencies);
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskMonitorError::CircularDependency(task) => {
                assert!(task == "a" || task == "b" || task == "c" || task == "d");
            }
            _ => panic!("Expected CircularDependency error"),
        }
    }

    #[test]
    fn detects_self_cycle_c_c() {
        // a -> b -> c -> c
        let mut dependencies = HashMap::new();
        dependencies.insert("a".to_string(), vec!["b".to_string()]);
        dependencies.insert("b".to_string(), vec!["c".to_string()]);
        dependencies.insert("c".to_string(), vec!["c".to_string()]);
        let result = check_circular_dependencies(&dependencies);
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskMonitorError::CircularDependency(task) => {
                assert!(["a", "b", "c"].contains(&task.as_str()));
            }
            _ => panic!("Expected CircularDependency error"),
        }
    }

    #[test]
    fn detects_complex_cycle_graph() {
        // Graph:
        //   a: b, c
        //   c: d, e
        //   e: a, b
        //
        // Diagram:
        //     a
        //    / \
        //   b   c
        //      / \
        //     d   e
        //        / \
        //       a   b
        //
        let mut dependencies = HashMap::new();
        dependencies.insert("a".to_string(), vec!["b".to_string(), "c".to_string()]);
        dependencies.insert("c".to_string(), vec!["d".to_string(), "e".to_string()]);
        dependencies.insert("e".to_string(), vec!["a".to_string(), "b".to_string()]);
        let result = check_circular_dependencies(&dependencies);
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskMonitorError::CircularDependency(task) => {
                assert!(task == "a" || task == "c" || task == "e");
            }
            _ => panic!("Expected CircularDependency error"),
        }
    }
}
