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
    let depend_map = build_depend_map(&tasks).unwrap();
    let result = check_circular_dependencies(&depend_map.dependencies);
    assert!(result.is_ok());
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
