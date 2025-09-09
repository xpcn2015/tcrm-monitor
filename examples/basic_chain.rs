use std::collections::HashMap;
use tcrm_monitor::monitor::{
    config::{TaskShell, TaskSpec},
    tasks::TaskMonitor,
};
use tcrm_task::tasks::config::TaskConfig;
use tokio::sync::mpsc;

/// Example: Basic task chain execution
/// Demonstrates a simple linear dependency chain: setup -> build -> test
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running basic task chain example...");

    let mut tasks = HashMap::new();

    // Define tasks with dependencies
    tasks.insert(
        "setup".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["Setting up project..."]))
            .shell(TaskShell::Auto),
    );

    tasks.insert(
        "build".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["Building project..."]))
            .dependencies(["setup"])
            .shell(TaskShell::Auto),
    );

    tasks.insert(
        "test".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["Running tests..."]))
            .dependencies(["build"])
            .shell(TaskShell::Auto),
    );

    // Set up event monitoring
    let (event_tx, mut event_rx) = mpsc::channel(1024);

    // Monitor events in background
    let monitor_handle = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                tcrm_task::tasks::event::TaskEvent::Started { task_name } => {
                    println!("Started: {}", task_name);
                }
                tcrm_task::tasks::event::TaskEvent::Stopped { task_name, .. } => {
                    println!("Completed: {}", task_name);
                }
                tcrm_task::tasks::event::TaskEvent::Error { task_name, error } => {
                    eprintln!("Failed: {} - {}", task_name, error);
                }
                _ => {}
            }
        }
    });

    // Create and execute the task monitor
    let mut monitor = TaskMonitor::new(tasks)?;
    monitor.execute_all_direct(Some(event_tx)).await;

    monitor_handle.abort();

    println!("All tasks completed successfully!");
    Ok(())
}
