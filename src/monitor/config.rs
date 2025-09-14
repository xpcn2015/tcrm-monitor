//! Task configuration types and shell definitions.
//!
//! This module provides configuration types for tasks, including dependency management
//! and cross-platform shell support.

use std::collections::HashMap;

use tcrm_task::tasks::config::TaskConfig;

/// Type alias for a collection of named task specifications.
///
/// Maps task names to their corresponding [`TaskSpec`] configurations.
/// Used as the primary input for [`TaskMonitor`](crate::monitor::tasks::TaskMonitor).
///
/// # Examples
///
/// ```rust
/// use std::collections::HashMap;
/// use tcrm_monitor::monitor::config::{TcrmTasks, TaskSpec, TaskShell};
/// use tcrm_task::tasks::config::TaskConfig;
///
/// let mut tasks: TcrmTasks = HashMap::new();
/// tasks.insert(
///     "build".to_string(),
///     TaskSpec::new(TaskConfig::new("cargo").args(["build"]))
///         .shell(TaskShell::Auto)
/// );
/// ```
pub type TcrmTasks = HashMap<String, TaskSpec>;

/// Task specification with dependency management and execution configuration.
///
/// Wraps a [`TaskConfig`] with additional metadata for dependency management,
/// shell selection, and execution behavior in a task graph.
///
/// ## Examples
///
/// ### Simple Task
///
/// ```rust
/// use tcrm_monitor::monitor::config::{TaskSpec, TaskShell};
/// use tcrm_task::tasks::config::TaskConfig;
///
/// let task = TaskSpec::new(
///     TaskConfig::new("echo").args(["Hello, World!"])
/// );
/// ```
///
/// ### Task with Dependencies
///
/// ```rust
/// use tcrm_monitor::monitor::config::{TaskSpec, TaskShell};
/// use tcrm_task::tasks::config::TaskConfig;
///
/// let build_task = TaskSpec::new(
///     TaskConfig::new("cargo").args(["build", "--release"])
/// )
/// .dependencies(["test", "lint"])
/// .shell(TaskShell::Auto)
/// .terminate_after_dependents(true);
/// ```
///
/// ### Task with Custom Configuration
///
/// ```rust
/// use tcrm_monitor::monitor::config::{TaskSpec, TaskShell};
/// use tcrm_task::tasks::config::TaskConfig;
///
/// let server_task = TaskSpec::new(
///     TaskConfig::new("node")
///         .args(["server.js"])
///         .working_dir("/app")
///         .enable_stdin(true)
///         .timeout_ms(0) // No timeout
/// )
/// .shell(TaskShell::Auto)
/// .ignore_dependencies_error(true);
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct TaskSpec {
    /// The underlying task configuration containing command, arguments, and execution options
    pub config: TaskConfig,

    /// Shell to use for task execution. None means direct execution without shell
    pub shell: Option<TaskShell>,

    /// List of task names this task depends on. Must complete before this task starts
    pub dependencies: Option<Vec<String>>,

    /// Whether to terminate this task when all dependent tasks finish.
    /// Useful for long-running services that should stop when their dependents complete
    pub terminate_after_dependents_finished: Option<bool>,

    /// Whether to ignore errors from dependency tasks and continue execution anyway
    pub ignore_dependencies_error: Option<bool>,
}

impl Default for TaskSpec {
    fn default() -> Self {
        Self {
            config: TaskConfig::default(),
            shell: Some(TaskShell::None),
            dependencies: None,
            terminate_after_dependents_finished: Some(false),
            ignore_dependencies_error: Some(false),
        }
    }
}

impl TaskSpec {
    /// Creates a new task specification from a task configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The underlying task configuration
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::config::TaskSpec;
    /// use tcrm_task::tasks::config::TaskConfig;
    ///
    /// let task = TaskSpec::new(
    ///     TaskConfig::new("ls").args(["-la"])
    /// );
    /// ```
    #[must_use]
    pub fn new(config: TaskConfig) -> Self {
        TaskSpec {
            config,
            ..Default::default()
        }
    }

    /// Sets the shell for task execution.
    ///
    /// # Arguments
    ///
    /// * `shell` - The shell type to use
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::config::{TaskSpec, TaskShell};
    /// use tcrm_task::tasks::config::TaskConfig;
    ///
    /// let task = TaskSpec::new(TaskConfig::new("echo").args(["test"]))
    ///     .shell(TaskShell::Auto);
    /// ```
    #[must_use]
    pub fn shell(mut self, shell: TaskShell) -> Self {
        self.shell = Some(shell);
        self
    }

    /// Adds dependencies to this task.
    ///
    /// Dependencies must complete successfully before this task can start.
    ///
    /// # Arguments
    ///
    /// * `dependencies` - An iterator of task names this task depends on
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::config::TaskSpec;
    /// use tcrm_task::tasks::config::TaskConfig;
    ///
    /// let task = TaskSpec::new(TaskConfig::new("cargo").args(["build"]))
    ///     .dependencies(["test", "lint"]);
    /// ```
    #[must_use]
    pub fn dependencies<I, S>(mut self, dependencies: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.dependencies = Some(dependencies.into_iter().map(Into::into).collect());
        self
    }

    /// Sets whether to terminate this task when its dependents complete.
    ///
    /// Useful for long-running services that should stop when their dependents finish.
    ///
    /// # Arguments
    ///
    /// * `terminate` - Whether to terminate after dependents complete
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::config::TaskSpec;
    /// use tcrm_task::tasks::config::TaskConfig;
    ///
    /// // A database server that should stop when tests finish
    /// let db_task = TaskSpec::new(TaskConfig::new("postgres").args(["-D", "/data"]))
    ///     .terminate_after_dependents(true);
    /// ```
    #[must_use]
    pub fn terminate_after_dependents(mut self, terminate: bool) -> Self {
        self.terminate_after_dependents_finished = Some(terminate);
        self
    }

    /// Sets whether to ignore errors from dependency tasks.
    ///
    /// When true, this task will run even if its dependencies fail.
    ///
    /// # Arguments
    ///
    /// * `ignore` - Whether to ignore dependency errors
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::config::TaskSpec;
    /// use tcrm_task::tasks::config::TaskConfig;
    ///
    /// // Cleanup task that should run regardless of test results
    /// let cleanup = TaskSpec::new(TaskConfig::new("rm").args(["-rf", "/tmp/test"]))
    ///     .dependencies(["test"])
    ///     .ignore_dependencies_error(true);
    /// ```
    #[must_use]
    pub fn ignore_dependencies_error(mut self, ignore: bool) -> Self {
        self.ignore_dependencies_error = Some(ignore);
        self
    }
}

/// Shell type for task execution.
///
/// Supports multiple shell types across Unix and Windows platforms with automatic detection.
///
/// ## Platform Support
///
/// - **Unix**: Bash, Sh, Zsh, Fish
/// - **Windows**: Cmd, Powershell
/// - **Cross-platform**: Auto (detects the best available shell), None (direct execution)
///
/// ## Examples
///
/// ```rust
/// use tcrm_monitor::monitor::config::TaskShell;
///
/// // Use automatic shell detection
/// let auto_shell = TaskShell::Auto;
///
/// // Execute without shell
/// let direct = TaskShell::None;
///
/// // Platform-specific shells
/// #[cfg(unix)]
/// let bash_shell = TaskShell::Bash;
///
/// #[cfg(windows)]
/// let powershell = TaskShell::Powershell;
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum TaskShell {
    /// Execute command directly without shell
    None,
    /// Automatic shell detection based on platform and availability
    Auto,
    /// Windows Command Prompt (Windows only)
    #[cfg(windows)]
    Cmd,
    /// Windows `PowerShell` (Windows only)
    #[cfg(windows)]
    Powershell,
    /// Bourne Again Shell (Unix only)
    #[cfg(unix)]
    Bash,
    /// POSIX Shell (Unix only)
    #[cfg(unix)]
    Sh,
    /// Z Shell (Unix only)
    #[cfg(unix)]
    Zsh,
    /// Fish Shell (Unix only)
    #[cfg(unix)]
    Fish,
}

impl Default for TaskShell {
    fn default() -> Self {
        Self::None
    }
}

impl TaskShell {
    /// Returns true if this shell is available on Unix systems.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::config::TaskShell;
    ///
    /// assert!(TaskShell::Auto.is_cross_platform());
    /// assert!(TaskShell::None.is_cross_platform());
    ///
    /// #[cfg(unix)]
    /// {
    ///     assert!(TaskShell::Bash.is_unix());
    ///     assert!(TaskShell::Zsh.is_unix());
    /// }
    ///
    /// #[cfg(windows)]
    /// {
    ///     assert!(!TaskShell::Cmd.is_unix());
    /// }
    /// ```
    #[must_use]
    pub fn is_unix(&self) -> bool {
        match self {
            #[cfg(unix)]
            TaskShell::Bash | TaskShell::Sh | TaskShell::Zsh | TaskShell::Fish => true,
            _ => false,
        }
    }

    /// Returns true if this shell is available on Windows systems.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::config::TaskShell;
    ///
    /// #[cfg(windows)]
    /// {
    ///     assert!(TaskShell::Powershell.is_windows());
    ///     assert!(TaskShell::Cmd.is_windows());
    /// }
    ///
    /// #[cfg(unix)]
    /// {
    ///     assert!(!TaskShell::Bash.is_windows());
    /// }
    /// ```
    #[must_use]
    pub fn is_windows(&self) -> bool {
        match self {
            #[cfg(windows)]
            TaskShell::Powershell | TaskShell::Cmd => true,
            _ => false,
        }
    }

    /// Returns true if this shell is available on all platforms.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::config::TaskShell;
    ///
    /// assert!(TaskShell::Auto.is_cross_platform());
    /// assert!(TaskShell::None.is_cross_platform());
    /// ```
    #[must_use]
    pub fn is_cross_platform(&self) -> bool {
        matches!(self, TaskShell::Auto | TaskShell::None)
    }

    /// Returns the command name used to invoke this shell, if applicable.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tcrm_monitor::monitor::config::TaskShell;
    ///
    /// assert_eq!(TaskShell::Auto.command_name(), None);
    /// assert_eq!(TaskShell::None.command_name(), None);
    ///
    /// #[cfg(unix)]
    /// assert_eq!(TaskShell::Bash.command_name(), Some("bash"));
    ///
    /// #[cfg(windows)]
    /// assert_eq!(TaskShell::Powershell.command_name(), Some("powershell"));
    /// ```
    #[must_use]
    pub fn command_name(&self) -> Option<&str> {
        match self {
            TaskShell::None | TaskShell::Auto => None,
            #[cfg(windows)]
            TaskShell::Cmd => Some("cmd"),
            #[cfg(windows)]
            TaskShell::Powershell => Some("powershell"),
            #[cfg(unix)]
            TaskShell::Bash => Some("bash"),
            #[cfg(unix)]
            TaskShell::Sh => Some("sh"),
            #[cfg(unix)]
            TaskShell::Zsh => Some("zsh"),
            #[cfg(unix)]
            TaskShell::Fish => Some("fish"),
        }
    }
}
