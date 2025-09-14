//! Task monitoring and execution module.
//!
//! This module provides the core functionality for managing and executing tasks with dependency relationships.
//! It includes configuration types, error handling, event monitoring, and execution strategies.
//!
//! ## Core Components
//!
//! - [`tasks::TaskMonitor`]: Main struct for managing task execution
//! - [`config::TaskSpec`]: Task configuration with dependencies and execution options
//! - [`config::TaskShell`]: Cross-platform shell configuration
//! - [`event::TaskMonitorEvent`]: Event types for monitoring task execution
//! - [`error::TaskMonitorError`]: Error types for task monitoring operations
//! - [`executor`]: Task execution strategies (currently direct execution)
//!
//! ## Usage
//!
//! The typical workflow involves:
//!
//! 1. Create task specifications with dependencies
//! 2. Build a `TaskMonitor` with the task collection
//! 3. Execute tasks either directly or with event monitoring
//!
//! ```rust
//! use std::collections::HashMap;
//! use tcrm_monitor::monitor::{tasks::TaskMonitor, config::{TaskSpec, TaskShell}};
//! use tcrm_task::tasks::config::TaskConfig;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut tasks = HashMap::new();
//!
//! tasks.insert(
//!     "compile".to_string(),
//!     TaskSpec::new(TaskConfig::new("cargo").args(["build"]))
//!         .shell(TaskShell::Auto)
//! );
//!
//! let mut monitor = TaskMonitor::new(tasks)?;
//! monitor.execute_all_direct(None).await;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod depend;
pub mod error;
pub mod event;
pub mod executor;
pub mod tasks;

// Re-export key types for easier access
pub use tasks::TaskMonitor;
