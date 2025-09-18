use std::collections::HashMap;
use tcrm_monitor::monitor::config::TaskShell;
use tcrm_monitor::monitor::{
    config::TaskSpec, event::TaskMonitorControlCommand, tasks::TaskMonitor,
};
use tcrm_task::tasks::config::TaskConfig;
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("TaskMonitor Control Example");

    // Create tasks with different configurations
    let mut tasks = HashMap::new();

    // A long-running task that can be terminated
    #[cfg(windows)]
    tasks.insert(
        "long_task".to_string(),
        TaskSpec::new(TaskConfig::new("ping").args(["127.0.0.1", "-n", "100"]))
            .shell(TaskShell::Auto),
    );

    #[cfg(not(windows))]
    tasks.insert(
        "long_task".to_string(),
        TaskSpec::new(TaskConfig::new("ping").args(["127.0.0.1", "-c", "100"]))
            .shell(TaskShell::Auto),
    );

    // A task with stdin enabled - use the proper commands for stdin interaction
    #[cfg(windows)]
    tasks.insert(
        "stdin_task".to_string(),
        TaskSpec::new(
            TaskConfig::new("powershell")
                .args(["-Command", "'ready'; $host.UI.ReadLine()"])
                .enable_stdin(true)
                .ready_indicator("ready")
                .timeout_ms(5000),
        )
        .shell(TaskShell::Auto),
    );

    #[cfg(not(windows))]
    tasks.insert(
        "stdin_task".to_string(),
        TaskSpec::new(
            TaskConfig::new("sh")
                .args(["-c", "echo 'ready'; read input; echo $input"])
                .enable_stdin(true)
                .ready_indicator("ready")
                .timeout_ms(5000),
        )
        .shell(TaskShell::Auto),
    );

    // A simple task
    tasks.insert(
        "simple_task".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["Hello", "World!"])).shell(TaskShell::Auto),
    );

    let mut monitor = TaskMonitor::new(tasks)?;

    // Create channels for events and control
    let (event_tx, mut event_rx) = mpsc::channel(1024);
    let (control_tx, control_rx) = mpsc::channel(32);

    // Start the monitor in a separate task
    let monitor_handle = tokio::spawn(async move {
        monitor
            .execute_all_direct_with_control(Some(event_tx), control_rx)
            .await;
    });

    // Handle events and demonstrate control
    let mut events_received = 0;
    let max_events = 10; // Limit events to prevent infinite output

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            events_received += 1;
            println!("Event {}: {:?}", events_received, event);

            // Stop after receiving some events to demonstrate control
            if events_received >= max_events {
                break;
            }
        }
    });

    // Demonstrate various control commands
    println!("\n--- Demonstrating Task Control ---");

    // Wait a bit for tasks to start
    sleep(Duration::from_secs(2)).await;

    // 1. Send stdin to a task (demonstrates validation)
    println!("1. Sending stdin to stdin_task...");
    let stdin_control = TaskMonitorControlCommand::SendStdin {
        task_name: "stdin_task".to_string(),
        input: "Hello from stdin!\n".to_string(),
    };
    control_tx.send(stdin_control).await?;

    sleep(Duration::from_millis(500)).await;

    // 2. Try to send stdin to a task without stdin enabled (will be rejected)
    println!("2. Trying to send stdin to long_task (should be rejected)...");
    let invalid_stdin_control = TaskMonitorControlCommand::SendStdin {
        task_name: "long_task".to_string(),
        input: "This should be rejected\n".to_string(),
    };
    control_tx.send(invalid_stdin_control).await?;

    sleep(Duration::from_millis(500)).await;

    // 3. Terminate a specific task
    println!("3. Terminating long_task...");
    let terminate_control = TaskMonitorControlCommand::TerminateTask {
        task_name: "long_task".to_string(),
    };
    control_tx.send(terminate_control).await?;

    sleep(Duration::from_secs(1)).await;

    // 4. Stop all tasks
    println!("4. Stopping all remaining tasks...");
    let stop_control = TaskMonitorControlCommand::TerminateAllTasks;
    control_tx.send(stop_control).await?;

    // Wait for the monitor to finish
    sleep(Duration::from_secs(2)).await;

    println!("\n--- Control Example Complete ---");
    println!("The monitor should have stopped all tasks gracefully.");

    // Clean up
    monitor_handle.abort();

    Ok(())
}
