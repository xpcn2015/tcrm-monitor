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
//! use tcrm_monitor::flatbuffers::conversion::ToFlatbuffers;
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

use flatbuffers::FlatBufferBuilder;

use crate::flatbuffers::conversion::error::ConversionError;

/// Trait for converting from `FlatBuffers` format back to Rust types.
///
/// This trait provides a standardized interface for deserializing `FlatBuffers`
/// data back into Rust types. It handles validation and error reporting during
/// the conversion process.
///
/// # Examples
///
/// ```rust
/// use tcrm_monitor::flatbuffers::conversion::FromFlatbuffers;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Assuming you have FlatBuffers data and the appropriate generated type
/// // let fb_data = /* ... */;
/// // let rust_value = MyType::from_flatbuffers(fb_data)?;
/// # Ok(())
/// # }
/// ```
pub trait FromFlatbuffers<T> {
    /// Convert from `FlatBuffers` format to Rust type.
    ///
    /// # Parameters
    ///
    /// * `fb_data` - The `FlatBuffers` data to convert from
    ///
    /// # Returns
    ///
    /// Returns the Rust type, or a [`ConversionError`] if conversion fails.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError`] if the `FlatBuffers` data is invalid or corrupted.
    fn from_flatbuffers(fb_data: T) -> Result<Self, ConversionError>
    where
        Self: Sized;
}

/// Trait for converting Rust types to `FlatBuffers` format.
///
/// This trait provides a standardized interface for converting task monitor
/// configuration types into their `FlatBuffers` representation. It handles
/// serialization into a compact, cross-platform binary format.
///
/// # Type Parameters
///
/// * `'a` - Lifetime parameter for the `FlatBufferBuilder` reference
///
/// # Associated Types
///
/// * `Output` - The `FlatBuffers` type that this conversion produces
///
/// # Examples
///
/// ```rust
/// use tcrm_monitor::flatbuffers::conversion::ToFlatbuffers;
/// use tcrm_monitor::monitor::config::{TaskSpec, TaskShell};
/// use tcrm_task::tasks::config::TaskConfig;
/// use flatbuffers::FlatBufferBuilder;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let task_spec = TaskSpec::new(TaskConfig::new("cargo").args(["test"]))
///     .shell(TaskShell::Auto);
///
/// let mut fbb = FlatBufferBuilder::new();
/// let fb_task_spec = task_spec.to_flatbuffers(&mut fbb)?;
///
/// // The resulting fb_task_spec can now be used to build the final FlatBuffer
/// # Ok(())
/// # }
/// ```
pub trait ToFlatbuffers<'a> {
    /// The `FlatBuffers` type produced by this conversion
    type Output;

    /// Convert this type to its `FlatBuffers` representation.
    ///
    /// # Parameters
    ///
    /// * `fbb` - Mutable reference to the `FlatBufferBuilder` for serialization
    ///
    /// # Returns
    ///
    /// Returns the `FlatBuffers` offset for the serialized data, or a
    /// [`ConversionError`] if serialization fails.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError`] if the `FlatBuffers` data is invalid or corrupted.
    fn to_flatbuffers(
        &self,
        fbb: &mut FlatBufferBuilder<'a>,
    ) -> Result<Self::Output, ConversionError>;
}
