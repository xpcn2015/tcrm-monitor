use std::collections::{HashMap, HashSet};

use crate::monitor::{config::TcrmTasks, error::TaskMonitorError};

/// Dependency mapping structure for task execution ordering.
///
/// This structure contains bidirectional dependency mappings that enable
/// efficient task scheduling and dependency resolution. It maps both
/// dependencies (what a task depends on) and dependents (what depends on a task).
///
/// # Fields
///
/// * `dependencies` - Maps task names to lists of tasks they depend on
/// * `dependents` - Maps task names to lists of tasks that depend on them
///
/// # Examples
///
/// ```rust
/// use std::collections::HashMap;
/// use tcrm_monitor::monitor::{
///     depend::{build_depend_map, TaskMonitorDependMap},
///     config::{TaskSpec, TcrmTasks}
/// };
/// use tcrm_task::tasks::config::TaskConfig;
///
/// let mut tasks = HashMap::new();
/// tasks.insert(
///     "compile".to_string(),
///     TaskSpec::new(TaskConfig::new("cargo").args(["build"]))
/// );
/// tasks.insert(
///     "test".to_string(),
///     TaskSpec::new(TaskConfig::new("cargo").args(["test"]))
///         .dependencies(["compile"])
/// );
///
/// let depend_map = build_depend_map(&tasks)?;
///
/// // Check dependencies: test depends on compile
/// assert_eq!(depend_map.dependencies["test"], vec!["compile"]);
///
/// // Check dependents: compile has test as dependent
/// assert_eq!(depend_map.dependents["compile"], vec!["test"]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub struct TaskMonitorDependMap {
    /// Maps each task name to the list of tasks it depends on
    pub dependencies: HashMap<String, Vec<String>>,
    /// Maps each task name to the list of tasks that depend on it
    pub dependents: HashMap<String, Vec<String>>,
}

/// Build a bidirectional dependency map from task specifications.
///
/// This function analyzes all tasks and their dependencies to create a comprehensive
/// dependency mapping that includes both direct and transitive dependencies. It also
/// validates that all referenced dependencies exist and detects circular dependencies.
///
/// # Parameters
///
/// * `tasks` - Collection of task specifications to analyze
///
/// # Returns
///
/// Returns a [`TaskMonitorDependMap`] containing bidirectional dependency mappings,
/// or a [`TaskMonitorError`] if validation fails.
///
/// # Errors
///
/// * [`TaskMonitorError::DependencyNotFound`] - If a task references a non-existent dependency
/// * [`TaskMonitorError::CircularDependency`] - If circular dependencies are detected
///
/// # Examples
///
/// ```rust
/// use std::collections::HashMap;
/// use tcrm_monitor::monitor::{
///     depend::build_depend_map,
///     config::{TaskSpec, TcrmTasks}
/// };
/// use tcrm_task::tasks::config::TaskConfig;
///
/// let mut tasks = HashMap::new();
///
/// // Create a dependency chain: build -> test -> deploy
/// tasks.insert(
///     "build".to_string(),
///     TaskSpec::new(TaskConfig::new("cargo").args(["build"]))
/// );
/// tasks.insert(
///     "test".to_string(),
///     TaskSpec::new(TaskConfig::new("cargo").args(["test"]))
///         .dependencies(["build"])
/// );
/// tasks.insert(
///     "deploy".to_string(),
///     TaskSpec::new(TaskConfig::new("deploy_script"))
///         .dependencies(["test"])
/// );
///
/// let depend_map = build_depend_map(&tasks)?;
///
/// // Check transitive dependencies - deploy depends on both test and build
/// assert!(depend_map.dependencies["deploy"].contains(&"test".to_string()));
/// assert!(depend_map.dependencies["deploy"].contains(&"build".to_string()));
///
/// // Check dependents - build has both test and deploy as dependents
/// assert!(depend_map.dependents["build"].contains(&"test".to_string()));
/// assert!(depend_map.dependents["build"].contains(&"deploy".to_string()));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
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

/// Check for circular dependencies in a task dependency graph.
///
/// This function performs a depth-first search to detect circular dependencies
/// in the task graph. It uses a recursive stack to track the current path and
/// detect back edges that would indicate a cycle.
///
/// # Parameters
///
/// * `dependencies` - Map of task names to their dependency lists
///
/// # Returns
///
/// Returns `Ok(())` if no circular dependencies are found, or a
/// [`TaskMonitorError::CircularDependency`] containing the name of a task
/// involved in a circular dependency.
///
/// # Examples
///
/// ```rust
/// use std::collections::HashMap;
/// use tcrm_monitor::monitor::{
///     depend::check_circular_dependencies,
///     error::TaskMonitorError
/// };
///
/// // Valid dependency graph: A -> B -> C
/// let mut valid_deps = HashMap::new();
/// valid_deps.insert("A".to_string(), vec!["B".to_string()]);
/// valid_deps.insert("B".to_string(), vec!["C".to_string()]);
/// valid_deps.insert("C".to_string(), vec![]);
///
/// assert!(check_circular_dependencies(&valid_deps).is_ok());
///
/// // Invalid dependency graph: A -> B -> A (circular)
/// let mut circular_deps = HashMap::new();
/// circular_deps.insert("A".to_string(), vec!["B".to_string()]);
/// circular_deps.insert("B".to_string(), vec!["A".to_string()]);
///
/// let result = check_circular_dependencies(&circular_deps);
/// assert!(matches!(result, Err(TaskMonitorError::CircularDependency(_))));
/// ```
///
/// # Algorithm
///
/// Uses depth-first search with a recursion stack to detect back edges:
/// 1. Maintains a `visited` set to track processed nodes
/// 2. Maintains a `rec_stack` to track the current path
/// 3. A back edge (pointing to a node in the current path) indicates a cycle
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

/// Recursive helper function to detect cycles in dependency graph.
///
/// Performs depth-first search to detect back edges that indicate circular dependencies.
/// Uses a visited set to track processed nodes and a recursion stack to identify
/// cycles during traversal.
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
