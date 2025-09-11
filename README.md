# TCRM Monitor

Task monitor unit for the TCRM project
A task dependency management and execution library for Rust applications.

## Features
- **Task Dependency Management**
- **Parallel Execution**: Execute independent tasks concurrently while respecting dependencies
- **Termination Control**: Automatically terminate dependencies tasks when dependents finish
- **Event-Driven**: task execution events for monitoring and logging

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
tcrm-monitor = { version = "0.1.0" }

```

## Quick Start

```rust
use std::collections::HashMap;
use tcrm_monitor::monitor::{TaskMonitor, config::{TaskSpec, TaskShell}};
use tcrm_task::tasks::config::TaskConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tasks = HashMap::new();
    
    // Define a simple task chain: setup -> build -> test
    tasks.insert(
        "setup".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["Setting up..."]))
            .shell(TaskShell::Auto),
    );
    
    tasks.insert(
        "build".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["Building..."]))
            .dependencies(["setup"])
            .shell(TaskShell::Auto),
    );
    
    tasks.insert(
        "test".to_string(),
        TaskSpec::new(TaskConfig::new("echo").args(["Testing..."]))
            .dependencies(["build"])
            .shell(TaskShell::Auto),
    );
    
    // Create and execute the task monitor
    let mut monitor = TaskMonitor::new(tasks)?;
    monitor.execute_all_direct(None).await?;
    
    Ok(())
}
```

## Examples
See the `examples/` directory

## Event Monitoring

Monitor task execution with real-time events:

```rust
use tokio::sync::mpsc;

let (event_tx, mut event_rx) = mpsc::channel(1024);

// Start monitoring in background
tokio::spawn(async move {
    while let Some(event) = event_rx.recv().await {
        match event {
            TaskEvent::Started { task_name } => {
                println!("Task started: {}", task_name);
            }
            TaskEvent::Ready { task_name } => {
                println!("Task ready: {}", task_name);
            }
            TaskEvent::Stopped { task_name, .. } => {
                println!("Task completed: {}", task_name);
            }
            TaskEvent::Error { task_name, error } => {
                eprintln!("Task failed: {} - {}", task_name, error);
            }
            _ => {}
        }
    }
});

// Execute with event monitoring
monitor.execute_all_direct(Some(event_tx)).await?;
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
