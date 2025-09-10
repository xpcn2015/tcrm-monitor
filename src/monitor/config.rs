use std::collections::HashMap;

use tcrm_task::tasks::config::TaskConfig;

pub type TcrmTasks = HashMap<String, TaskSpec>;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct TaskSpec {
    /// Task configuration
    pub config: TaskConfig,

    /// Shell options if the command should run in a shell
    pub shell: Option<TaskShell>,

    /// Using pseudo-terminal to run this task
    // pub pty: Option<bool>,

    /// List of task names that this task depends on
    pub dependencies: Option<Vec<String>>,

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
            // pty: Some(false),
            dependencies: None,
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
    // pub fn pty(mut self, pty: bool) -> Self {
    //     self.pty = Some(pty);
    //     self
    // }
    pub fn dependencies<I, S>(mut self, dependencies: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.dependencies = Some(dependencies.into_iter().map(Into::into).collect());
        self
    }

    pub fn terminate_after_dependents(mut self, terminate: bool) -> Self {
        self.terminate_after_dependents_finished = Some(terminate);
        self
    }

    pub fn ignore_dependencies_error(mut self, ignore: bool) -> Self {
        self.ignore_dependencies_error = Some(ignore);
        self
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
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
