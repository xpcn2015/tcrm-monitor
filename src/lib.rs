//! # TCRM Monitor
//!
//! A task dependency management and execution library for Rust applications.
//!
//! ## Overview
//!
//! TCRM Monitor provides tools for defining, managing, and executing tasks with complex dependency relationships.
//! It supports parallel execution of independent tasks while ensuring dependencies are respected,
//! real-time event monitoring, and runtime control of task execution.
//!
//! ## Core Features
//!
//! - **Dependency Management**: Define task graphs with automatic dependency validation and circular dependency detection
//! - **Parallel Execution**: Execute independent tasks concurrently while respecting dependency order
//! - **Shell Integration**: Support for running tasks from various shells (Bash, Sh, PowerShell, CMD) based on OS
//! - **Real-time Events**: Monitor task execution with detailed event streams
//! - **Runtime Control**: Stop tasks, send stdin input, and terminate specific tasks during execution
//! - **Serialization**: Optional flatbuffers and serde support for task configuration
//!
//! ## Quick Start
//!
//! ```rust
//! use std::collections::HashMap;
//! use tcrm_monitor::monitor::{tasks::TaskMonitor, config::{TaskSpec, TaskShell}};
//! use tcrm_task::tasks::config::TaskConfig;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut tasks = HashMap::new();
//!     
//!     // Define a task with dependencies
//!     tasks.insert(
//!         "setup".to_string(),
//!         TaskSpec::new(TaskConfig::new("echo").args(["Setting up..."]))
//!             .shell(TaskShell::Auto),
//!     );
//!     
//!     tasks.insert(
//!         "build".to_string(),
//!         TaskSpec::new(TaskConfig::new("echo").args(["Building..."]))
//!             .dependencies(["setup"])
//!             .shell(TaskShell::Auto),
//!     );
//!     
//!     // Create and execute
//!     let mut monitor = TaskMonitor::new(tasks)?;
//!     monitor.execute_all_direct(None).await;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Event Monitoring
//!
//! Monitor task execution in real-time:
//!
//! ```rust
//! use tokio::sync::mpsc;
//! use std::collections::HashMap;
//! use tcrm_monitor::monitor::{tasks::TaskMonitor, config::{TaskSpec, TaskShell}, event::{TaskMonitorEvent, TaskMonitorControlCommand}};
//! use tcrm_task::tasks::config::TaskConfig;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut tasks = HashMap::new();
//! tasks.insert(
//!     "test".to_string(),
//!     TaskSpec::new(TaskConfig::new("echo").args(["Hello"]))
//!         .shell(TaskShell::Auto)
//! );
//! let mut monitor = TaskMonitor::new(tasks)?;
//! let (event_tx, mut event_rx) = mpsc::channel(100);
//! let (control_tx, control_rx) = mpsc::channel(10);
//!
//! // Start monitoring events
//! let event_handler = tokio::spawn(async move {
//!     while let Some(event) = event_rx.recv().await {
//!         match event {
//!             TaskMonitorEvent::Started { total_tasks } => {
//!                 println!("Started execution of {} tasks", total_tasks);
//!             }
//!             TaskMonitorEvent::Task(task_event) => {
//!                 println!("Task event: {:?}", task_event);
//!             }
//!             TaskMonitorEvent::Control(control_event) => {
//!                 println!("Control event: {:?}", control_event);
//!             }
//!             TaskMonitorEvent::Completed { .. } => {
//!                 break; // Exit when execution completes
//!             }
//!             _ => {}
//!         }
//!     }
//! });
//!
//! // Execute with control
//! monitor.execute_all_direct_with_control(Some(event_tx), control_rx).await;
//!
//! // Wait for event handler to finish
//! let _ = event_handler.await;
//! # Ok(())
//! # }
//! ```
//!
//! ## Task Configuration
//!
//! Tasks are configured using [`TaskSpec`](monitor::config::TaskSpec) which wraps a
//! [`TaskConfig`](https://docs.rs/tcrm-task/latest/tcrm_task/tasks/config/struct.TaskConfig.html)
//! with additional dependency and execution options:
//!
//! ```rust
//! use tcrm_monitor::monitor::config::{TaskSpec, TaskShell};
//! use tcrm_task::tasks::config::TaskConfig;
//!
//! let task = TaskSpec::new(
//!     TaskConfig::new("cargo")
//!         .args(["build", "--release"])
//!         .working_dir("/project")
//!         .timeout_ms(300_000)
//!         .enable_stdin(true)
//! )
//! .shell(TaskShell::Auto)
//! .dependencies(["setup", "test"])
//! .terminate_after_dependents(true)
//! .ignore_dependencies_error(false);
//! ```
//!
//! ## Features
//!
//! The library supports several optional features:
//!
//! - `serde`: Enable serde serialization for task configurations
//! - `flatbuffers`: Enable flatbuffers serialization for efficient data exchange
//! - `tracing`: Enable structured logging with the tracing crate
//!
//! Enable features in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! tcrm-monitor = { version = "0.1", features = ["serde", "tracing"] }
//! ```

#[cfg(feature = "flatbuffers")]
pub mod flatbuffers;
pub mod monitor;

pub use tcrm_task::tasks as tcrm_tasks;
