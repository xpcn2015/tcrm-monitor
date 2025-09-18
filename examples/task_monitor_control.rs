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
    // Initialize tracing subscriber to print logs to terminal
    #[cfg(feature = "tracing")]
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();

    // Create tasks with different configurations
    let mut tasks = HashMap::new();

    // A long-running task that can be terminated
    #[cfg(windows)]
    tasks.insert(
        "long_task".to_string(),
        TaskSpec::new(TaskConfig::new("ping").args(["127.0.0.1", "-n", "15"])),
    );

    #[cfg(unix)]
    tasks.insert(
        "long_task".to_string(),
        TaskSpec::new(TaskConfig::new("ping").args(["-c", "15", "127.0.0.1"])),
    );

    // Another long-running task that should be terminated by TerminateAllTasks command
    #[cfg(windows)]
    tasks.insert(
        "long_task_2".to_string(),
        TaskSpec::new(TaskConfig::new("ping").args(["127.0.0.1", "-n", "15"])),
    );

    #[cfg(unix)]
    tasks.insert(
        "long_task_2".to_string(),
        TaskSpec::new(TaskConfig::new("ping").args(["-c", "15", "127.0.0.1"])),
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
        ),
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
        ),
    );

    // A simple task
    tasks.insert(
        "simple_task".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["Hello", "World!"])).shell(TaskShell::Auto),
    );

    let mut monitor = TaskMonitor::new(tasks)?;

    // Create channels for events and control
    let (event_tx, mut event_rx) = mpsc::channel(100);
    let (control_tx, control_rx) = mpsc::channel(32);

    // Start the monitor in a separate task
    let monitor_handle = tokio::spawn(async move {
        monitor
            .execute_all_direct_with_control(Some(event_tx), control_rx)
            .await;
    });

    let event_handle = tokio::spawn(async move {
        let mut events_received = 0;
        let max_events = 100; // Limit events received

        while let Some(event) = event_rx.recv().await {
            events_received += 1;
            println!("Event {}: {:?}", events_received, event);

            if events_received >= max_events {
                break;
            }
        }
    });

    println!("\n--- Demonstrating Task Control ---");

    // Wait a bit for tasks to start
    sleep(Duration::from_secs(2)).await;

    // 1. Send stdin to a task
    println!("1. Sending stdin to stdin_task...");
    let stdin_control = TaskMonitorControlCommand::SendStdin {
        task_name: "stdin_task".to_string(),
        input: "Hello from stdin!\n".to_string(),
    };
    control_tx.send(stdin_control).await?;

    // 2. Try to send stdin to a task without stdin enabled (will be rejected)
    println!("2. Trying to send stdin to long_task (should be rejected)...");
    let invalid_stdin_control = TaskMonitorControlCommand::SendStdin {
        task_name: "long_task".to_string(),
        input: "This should be rejected\n".to_string(),
    };
    control_tx.send(invalid_stdin_control).await?;

    // 3. Terminate a specific task
    println!("3. Terminating long_task...");
    let terminate_control = TaskMonitorControlCommand::TerminateTask {
        task_name: "long_task".to_string(),
    };
    control_tx.send(terminate_control).await?;

    // 4. Stop all tasks
    println!("4. Stopping all remaining tasks...");
    let stop_control = TaskMonitorControlCommand::TerminateAllTasks;
    control_tx.send(stop_control).await?;

    // Clean up
    let _ = monitor_handle.await;
    let _ = event_handle.await;

    println!("\n--- Control Example Complete ---");
    println!("The monitor should have stopped all tasks gracefully.");

    Ok(())
}
