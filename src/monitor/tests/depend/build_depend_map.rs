use std::collections::HashMap;

use tcrm_task::tasks::config::TaskConfig;

use crate::monitor::{config::TaskSpec, depend::build_depend_map, error::TaskMonitorError};

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
