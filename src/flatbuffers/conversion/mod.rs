//! Conversion traits and implementations for `FlatBuffers` serialization.
//!
//! This module provides conversion traits and implementations for converting
//! between Rust types and their `FlatBuffers` representations. It supports
//! bidirectional conversion with proper error handling.
//!
//! ## Core Traits
//!
//! - `ToFlatbuffers` - Convert Rust types to `FlatBuffers` binary format
//! - `FromFlatbuffers` - Convert `FlatBuffers` binary format back to Rust types
//!
//! ## Supported Types
//!
//! - [`TaskSpec`](crate::monitor::config::TaskSpec) - Task specifications
//! - [`TcrmTasks`](crate::monitor::config::TcrmTasks) - Collections of tasks
//! - [`TaskShell`](crate::monitor::config::TaskShell) - Shell configurations
//! - Various event and error types
//!
//! ## Examples
//!
//! ```rust
//! use tcrm_monitor::flatbuffers::conversion::config::ToFlatbuffers;
//! use tcrm_monitor::monitor::config::{TaskSpec, TaskShell, TcrmTasks};
//! use tcrm_task::tasks::config::TaskConfig;
//! use flatbuffers::FlatBufferBuilder;
//! use std::collections::HashMap;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create some tasks
//! let mut tasks = HashMap::new();
//! tasks.insert(
//!     "test".to_string(),
//!     TaskSpec::new(TaskConfig::new("cargo").args(["test"]))
//!         .shell(TaskShell::Auto)
//! );
//!
//! // Convert to FlatBuffers
//! let mut fbb = FlatBufferBuilder::new();
//! let fb_tasks = tasks.to_flatbuffers(&mut fbb)?;
//! fbb.finish(fb_tasks, None);
//! let flatbuffer_data = fbb.finished_data().to_vec();
//!
//! // Convert back to Rust types
//! let fb_root = flatbuffers::root::<tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TcrmTasks>(&flatbuffer_data)?;
//! let restored_tasks: TcrmTasks = fb_root.try_into()?;
//!
//! assert_eq!(restored_tasks.len(), 1);
//! assert!(restored_tasks.contains_key("test"));
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod error;
pub mod event;
