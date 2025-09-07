use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tcrm_task::tasks::config::{StreamSource, TaskConfig};

pub type TcrmTasks = HashMap<String, TaskSpec>;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TaskSpec {
    /// Task configuration
    pub config: TaskConfig,

    /// Shell options if the command should run in a shell
    pub shell: Option<TaskShell>,

    /// Using pseudo-terminal to run this task
    pub pty: Option<bool>,

    /// List of task names that this task depends on
    pub dependencies: Option<Vec<String>>,

    /// Optional string to indicate the task is ready (for long-running processes like servers)
    pub ready_indicator: Option<String>,

    /// Source of the ready indicator string (stdout/stderr)
    pub ready_indicator_source: Option<StreamSource>,

    /// Automatically terminate this task after all dependent tasks finish
    pub terminate_after_dependents_finished: Option<bool>,

    /// Continue starting this task even if dependencies have error
    pub ignore_dependencies_error: Option<bool>,
}
impl Default for TaskSpec {
    fn default() -> Self {
        Self {
            config: TaskConfig::default(),
            shell: Some(TaskShell::None),
            pty: Some(false),
            dependencies: None,
            ready_indicator: None,
            ready_indicator_source: None,
            terminate_after_dependents_finished: Some(false),
            ignore_dependencies_error: Some(false),
        }
    }
}
impl TaskSpec {
    pub fn new(config: TaskConfig) -> Self {
        TaskSpec {
            config,
            ..Default::default()
        }
    }

    pub fn shell(mut self, shell: TaskShell) -> Self {
        self.shell = Some(shell);
        self
    }
    pub fn pty(mut self, pty: bool) -> Self {
        self.pty = Some(pty);
        self
    }
    pub fn dependencies<I, S>(mut self, dependencies: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.dependencies = Some(dependencies.into_iter().map(Into::into).collect());
        self
    }

    pub fn ready_indicator(mut self, signal: impl Into<String>) -> Self {
        self.ready_indicator = Some(signal.into());
        self
    }
    pub fn ready_indicator_source(mut self, source: StreamSource) -> Self {
        self.ready_indicator_source = Some(source);
        self
    }

    pub fn terminate_after_dependents(mut self, terminate: bool) -> Self {
        self.terminate_after_dependents_finished = Some(terminate);
        self
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskShell {
    None,
    Auto,
    #[cfg(windows)]
    Cmd,
    #[cfg(windows)]
    Powershell,
    #[cfg(unix)]
    Bash,
}
impl Default for TaskShell {
    fn default() -> Self {
        Self::None
    }
}
