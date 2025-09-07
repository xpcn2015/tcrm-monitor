use std::collections::HashMap;

use tcrm_task::tasks::config::TaskConfig;

use crate::monitor::{
    config::{TaskShell, TaskSpec, TcrmTasks},
    error::TaskMonitorError,
    tasks::TaskMonitor,
};

#[test]
fn correct_command() {
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
fn circular_dependency() {
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
fn missing_dependency() {
    let mut tasks = HashMap::new();

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
fn complex_dependency_tree() {
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

    let config = tasks;
    let monitor = TaskMonitor::new(config);
    assert!(monitor.is_ok());

    let monitor = monitor.unwrap();

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
fn multiple_independent_chains() {
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

    let config = tasks;
    let monitor = TaskMonitor::new(config);
    assert!(monitor.is_ok());

    let monitor = monitor.unwrap();
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
