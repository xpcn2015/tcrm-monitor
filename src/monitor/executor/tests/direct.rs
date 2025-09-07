use std::{collections::HashMap, time::Duration};

use tcrm_task::tasks::{config::TaskConfig, event::TaskEvent};
use tokio::{sync::mpsc, time::timeout};

use crate::monitor::{
    config::{TaskShell, TaskSpec},
    tasks::TaskMonitor,
};

#[tokio::test]
async fn test_execute_all_simple_chain() {
    let mut tasks = HashMap::new();

    tasks.insert(
        "task1".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["hello"])).shell(TaskShell::Auto),
    );

    tasks.insert(
        "task2".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["world"]))
            .dependencies(["task1"])
            .shell(TaskShell::Auto),
    );

    tasks.insert(
        "task3".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["!"]))
            .dependencies(["task2"])
            .shell(TaskShell::Auto),
    );

    let mut monitor = TaskMonitor::new(tasks).unwrap();

    let (event_tx, mut event_rx) = mpsc::channel(1024);

    // Start execution in a separate task
    let execute_handle =
        tokio::spawn(async move { monitor.execute_all_direct(Some(event_tx)).await });

    // Collect events
    let mut events = Vec::new();
    let mut event_count = 0;

    // Use timeout to avoid hanging tests
    while event_count < 10 {
        // Reasonable upper bound
        match timeout(Duration::from_secs(5), event_rx.recv()).await {
            Ok(Some(event)) => {
                events.push(event);
                event_count += 1;
            }
            Ok(None) => break, // Channel closed
            Err(_) => break,   // Timeout
        }
    }

    // Wait for execution to complete
    let result = timeout(Duration::from_secs(10), execute_handle).await;
    assert!(result.is_ok());
    assert!(result.unwrap().unwrap().is_ok());

    // Verify we got some events
    assert!(!events.is_empty());

    // Check that all tasks were started
    let started_tasks: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            TaskEvent::Started { task_name } => Some(task_name.clone()),
            _ => None,
        })
        .collect();

    assert!(!started_tasks.is_empty());
}
#[tokio::test]
async fn test_execute_all_independent_tasks() {
    let mut tasks = HashMap::new();

    tasks.insert(
        "independent1".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["task1"])).shell(TaskShell::Auto),
    );
    tasks.insert(
        "independent2".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["task2"])).shell(TaskShell::Auto),
    );
    tasks.insert(
        "independent3".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["task3"])).shell(TaskShell::Auto),
    );

    let mut monitor = TaskMonitor::new(tasks).unwrap();

    let (event_tx, mut event_rx) = mpsc::channel(1024);

    let execute_handle =
        tokio::spawn(async move { monitor.execute_all_direct(Some(event_tx)).await });

    let mut events = Vec::new();
    let mut event_count = 0;

    while event_count < 15 {
        // Allow for more events with independent tasks
        match timeout(Duration::from_secs(5), event_rx.recv()).await {
            Ok(Some(event)) => {
                events.push(event);
                event_count += 1;
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }

    let result = timeout(Duration::from_secs(10), execute_handle).await;
    assert!(result.is_ok());
    assert!(result.unwrap().unwrap().is_ok());

    // Verify all independent tasks were started
    let started_tasks: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            TaskEvent::Started { task_name } => Some(task_name.clone()),
            _ => None,
        })
        .collect();

    // All independent tasks should start
    assert!(started_tasks.len() >= 3);
}
