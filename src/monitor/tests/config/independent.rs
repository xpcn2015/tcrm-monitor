use std::collections::HashMap;

use tcrm_task::tasks::config::TaskConfig;

use crate::monitor::{
    config::{TaskShell, TaskSpec},
    tasks::TaskMonitor,
};

#[test]
fn correct_command() {
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
fn single_task_no_dependencies() {
    let mut tasks = HashMap::new();
    tasks.insert(
        "single".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["hello"])),
    );

    let monitor = TaskMonitor::new(tasks);
    assert!(monitor.is_ok());

    let monitor = monitor.unwrap();
    assert_eq!(monitor.tasks.len(), 1);
    assert!(monitor.dependencies.is_empty()); // No dependencies
    assert!(monitor.dependents.is_empty()); // No dependents
}
