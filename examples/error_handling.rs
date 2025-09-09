use std::collections::HashMap;
use tcrm_monitor::monitor::{
    config::{TaskShell, TaskSpec},
    tasks::TaskMonitor,
};
use tcrm_task::tasks::config::TaskConfig;
use tokio::sync::mpsc;

/// Example: Error handling
/// Demonstrates how tasks handle failures and dependency error propagation
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running error handling example...");

    let mut tasks = HashMap::new();

    // Task that will succeed
    tasks.insert(
        "success_task".to_string(),
        TaskSpec::new(TaskConfig::new("echo 'This task will succeed'")).shell(TaskShell::Auto),
    );

    // Task that will fail (exit code 1)
    tasks.insert(
        "failing_task".to_string(),
        TaskSpec::new(TaskConfig::new("echo 'This task will fail'; exit 1")).shell(TaskShell::Auto),
    );

    // Task that depends on the failing task - will not run by default
    tasks.insert(
        "blocked_task".to_string(),
        TaskSpec::new(TaskConfig::new("echo 'This task will be blocked'"))
            .dependencies(["failing_task"])
            .shell(TaskShell::Auto),
    );

    // Task that ignores dependency errors and will run anyway
    tasks.insert(
        "resilient_task".to_string(),
        TaskSpec::new(TaskConfig::new("echo 'This task runs despite failures'"))
            .dependencies(["failing_task"])
            .ignore_dependencies_error(true)
            .shell(TaskShell::Auto),
    );

    // Independent task that should run regardless
    tasks.insert(
        "independent_task".to_string(),
        TaskSpec::new(TaskConfig::new("echo 'This task runs independently'"))
            .shell(TaskShell::Auto),
    );

    // Set up event monitoring
    let (event_tx, mut event_rx) = mpsc::channel(1024);

    // Monitor events in background
    let monitor_handle = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                tcrm_task::tasks::event::TaskEvent::Started { task_name } => {
                    println!("🏁 Started: {}", task_name);
                }
                tcrm_task::tasks::event::TaskEvent::Stopped {
                    task_name,
                    exit_code,
                    reason,
                } => match exit_code {
                    Some(0) => println!("✅ Completed successfully: {} ({:?})", task_name, reason),
                    Some(code) => println!(
                        "💥 Failed with exit code {}: {} ({:?})",
                        code, task_name, reason
                    ),
                    None => println!(
                        "🏁 Task stopped without exit code: {} ({:?})",
                        task_name, reason
                    ),
                },
                tcrm_task::tasks::event::TaskEvent::Error { task_name, error } => {
                    eprintln!("🚨 Error in {}: {}", task_name, error);
                }
                tcrm_task::tasks::event::TaskEvent::Output {
                    task_name,
                    line,
                    src,
                } => {
                    println!("📣 Output from {}: {} (src: {:?})", task_name, line, src);
                }
                _ => {}
            }
        }
    });

    // Create and execute the task monitor
    let mut monitor = TaskMonitor::new(tasks)?;
    monitor.execute_all_direct(Some(event_tx)).await;

    // Wait a moment for monitor to finish
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    monitor_handle.abort();

    println!("🏁 Error handling example completed!");
    println!("Notice how:");
    println!("  - The failing task ran and failed");
    println!("  - The blocked task didn't run (dependency failed)");
    println!("  - The resilient task ran anyway (ignore_dependencies_error = true)");
    println!("  - The independent task ran normally");
    Ok(())
}
