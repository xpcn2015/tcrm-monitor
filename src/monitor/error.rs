//! Error types for task monitoring operations.
//!
//! This module defines the error types that can occur during task monitoring,
//! including dependency validation errors and stdin communication errors.

use thiserror::Error;

/// Reasons why sending stdin to a task might fail.
///
/// These errors provide specific context about stdin operation failures,
/// helping users understand why their stdin input couldn't be delivered.
///
/// # Examples
///
/// ```rust
/// use tcrm_monitor::monitor::error::SendStdinErrorReason;
///
/// let error = SendStdinErrorReason::TaskNotFound("nonexistent".to_string());
/// println!("Error: {}", error);
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Error, Debug, Clone)]
pub enum SendStdinErrorReason {
    /// The specified task does not exist in the task collection.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::error::SendStdinErrorReason;
    ///
    /// let error = SendStdinErrorReason::TaskNotFound("missing_task".to_string());
    /// assert!(error.to_string().contains("missing_task"));
    /// ```
    #[error("Task '{0}' not found")]
    TaskNotFound(String),

    /// The task exists but does not have stdin enabled.
    ///
    /// Tasks must be configured with `enable_stdin(true)` to receive input.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::error::SendStdinErrorReason;
    ///
    /// let error = SendStdinErrorReason::StdinNotEnabled("readonly_task".to_string());
    /// assert!(error.to_string().contains("does not have stdin enabled"));
    /// ```
    #[error("Task '{0}' does not have stdin enabled")]
    StdinNotEnabled(String),

    /// The task is not in a state that can receive stdin input.
    ///
    /// For example, the task might not be running yet or has already finished.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::error::SendStdinErrorReason;
    ///
    /// let error = SendStdinErrorReason::TaskNotReady("finished_task".to_string());
    /// assert!(error.to_string().contains("not in a state"));
    /// ```
    #[error("Task '{0}' is not in a state that can receive stdin")]
    TaskNotReady(String),

    /// The stdin channel for the task is closed or unavailable.
    ///
    /// This typically happens when the task has terminated or crashed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::error::SendStdinErrorReason;
    ///
    /// let error = SendStdinErrorReason::ChannelClosed("crashed_task".to_string());
    /// assert!(error.to_string().contains("channel closed"));
    /// ```
    #[error("Failed to send stdin to task '{0}': channel closed")]
    ChannelClosed(String),
}

/// Main error type for task monitoring operations.
///
/// Covers various error conditions that can occur during task monitoring setup and execution,
/// including configuration parsing, dependency validation, and runtime communication errors.
///
/// # Examples
///
/// ## Handling Circular Dependencies
///
/// ```rust,should_panic
/// use std::collections::HashMap;
/// use tcrm_monitor::monitor::{TaskMonitor, config::TaskSpec, error::TaskMonitorError};
/// use tcrm_task::tasks::config::TaskConfig;
///
/// let mut tasks = HashMap::new();
/// tasks.insert(
///     "a".to_string(),
///     TaskSpec::new(TaskConfig::new("echo").args(["a"]))
///         .dependencies(["b"])
/// );
/// tasks.insert(
///     "b".to_string(),
///     TaskSpec::new(TaskConfig::new("echo").args(["b"]))
///         .dependencies(["a"])
/// );
///
/// match TaskMonitor::new(tasks) {
///     Err(TaskMonitorError::CircularDependency(task)) => {
///         println!("Circular dependency involving: {}", task);
///         panic!("Expected circular dependency error");
///     }
///     _ => panic!("Expected error"),
/// }
/// ```
///
/// ## Handling Missing Dependencies
///
/// ```rust,should_panic
/// use std::collections::HashMap;
/// use tcrm_monitor::monitor::{TaskMonitor, config::TaskSpec, error::TaskMonitorError};
/// use tcrm_task::tasks::config::TaskConfig;
///
/// let mut tasks = HashMap::new();
/// tasks.insert(
///     "build".to_string(),
///     TaskSpec::new(TaskConfig::new("cargo").args(["build"]))
///         .dependencies(["nonexistent"])
/// );
///
/// match TaskMonitor::new(tasks) {
///     Err(TaskMonitorError::DependencyNotFound { task, dep }) => {
///         println!("Task '{}' depends on missing task '{}'", task, dep);
///         panic!("Expected missing dependency error");
///     }
///     _ => panic!("Expected error"),
/// }
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Error, Debug, Clone)]
pub enum TaskMonitorError {
    /// Configuration parsing failed.
    ///
    /// This error occurs when task configuration cannot be parsed or contains invalid values.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::error::TaskMonitorError;
    ///
    /// let error = TaskMonitorError::ConfigParse("Invalid timeout value".to_string());
    /// assert!(error.to_string().contains("Config parse error"));
    /// ```
    #[error("Config parse error: {0}")]
    ConfigParse(String),

    /// A circular dependency was detected in the task graph.
    ///
    /// Contains the name of a task involved in the circular dependency.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::error::TaskMonitorError;
    ///
    /// let error = TaskMonitorError::CircularDependency("task_a".to_string());
    /// assert!(error.to_string().contains("Circular dependency"));
    /// ```
    #[error("Circular dependency detected involving task '{0}'")]
    CircularDependency(String),

    /// A task depends on another task that doesn't exist.
    ///
    /// This error is returned during task graph validation when a dependency
    /// is specified that doesn't correspond to any defined task.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::error::TaskMonitorError;
    ///
    /// let error = TaskMonitorError::DependencyNotFound {
    ///     task: "build".to_string(),
    ///     dep: "nonexistent".to_string()
    /// };
    /// assert!(error.to_string().contains("not found"));
    /// ```
    #[error("Dependency '{dep}' not found for task '{task}'")]
    DependencyNotFound {
        /// Name of the missing dependency task
        dep: String,
        /// Name of the task that has the missing dependency
        task: String,
    },

    /// Failed to send stdin input to a task.
    ///
    /// This error wraps a [`SendStdinErrorReason`] with additional context about
    /// the stdin operation failure.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::error::{TaskMonitorError, SendStdinErrorReason};
    ///
    /// let stdin_error = SendStdinErrorReason::TaskNotFound("missing".to_string());
    /// let error = TaskMonitorError::SendStdinError(stdin_error);
    /// assert!(error.to_string().contains("Send stdin error"));
    /// ```
    #[error("Send stdin error: {0}")]
    SendStdinError(#[from] SendStdinErrorReason),
}
