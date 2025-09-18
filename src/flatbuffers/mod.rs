//! `FlatBuffers` serialization support for task monitoring types.
//!
//! This module provides efficient binary serialization and deserialization
//! of task monitoring types using `FlatBuffers`. This is useful for high-performance
//! scenarios where you need to serialize task configurations and results.
//!
//! ## Features
//!
//! - Conversion between Rust types and `FlatBuffers` representations
//! - Efficient binary serialization for task specifications and events
//! - Cross-language compatibility through `FlatBuffers` schema
//!
//! ## Usage
//!
//! Enable the `flatbuffers` feature in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! tcrm-monitor = { version = "0.1", features = ["flatbuffers"] }
//! ```
//!
//! ## Examples
//!
//! ```rust
//! use tcrm_monitor::flatbuffers::conversion::ToFlatbuffers;
//! use tcrm_monitor::monitor::config::{TaskSpec, TaskShell, TcrmTasks};
//! use tcrm_task::tasks::config::TaskConfig;
//! use flatbuffers::FlatBufferBuilder;
//! use std::collections::HashMap;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Convert tasks to FlatBuffers
//! let mut tasks = HashMap::new();
//! tasks.insert(
//!     "build".to_string(),
//!     TaskSpec::new(TaskConfig::new("cargo").args(["build"]))
//!         .shell(TaskShell::Auto)
//! );
//!
//! let mut fbb = FlatBufferBuilder::new();
//! let fb_tasks = tcrm_monitor::flatbuffers::conversion::tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
//! fbb.finish(fb_tasks, None);
//! let flatbuffer_data = fbb.finished_data().to_vec();
//!
//! // Convert back from FlatBuffers
//! let fb_root = flatbuffers::root::<tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TcrmTasks>(&flatbuffer_data)?;
//! let restored_tasks: TcrmTasks = fb_root.try_into()?;
//!
//! assert_eq!(restored_tasks.len(), 1);
//! assert!(restored_tasks.contains_key("build"));
//! # Ok(())
//! # }
//! ```

pub mod conversion;
#[allow(dead_code, unused_imports)]
#[path = "tcrm_monitor_generated.rs"]
pub mod tcrm_monitor_generated;

pub use tcrm_task::flatbuffers::tcrm_task_generated;

#[cfg(test)]
pub mod tests;
