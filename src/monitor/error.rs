use thiserror::Error;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Error, Debug, Clone)]
pub enum SendStdinErrorReason {
    #[error("Task '{0}' not found")]
    TaskNotFound(String),
    #[error("Task '{0}' does not have stdin enabled")]
    StdinNotEnabled(String),
    #[error("Task '{0}' is not in a state that can receive stdin")]
    TaskNotReady(String),
    #[error("Failed to send stdin to task '{0}': channel closed")]
    ChannelClosed(String),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Error, Debug, Clone)]
pub enum TaskMonitorError {
    #[error("Config parse error: {0}")]
    ConfigParse(String),
    #[error("Circular dependency detected involving task '{0}'")]
    CircularDependency(String),
    #[error("Dependency '{dep}' not found for task '{task}'")]
    DependencyNotFound { dep: String, task: String },
    #[error("Send stdin error: {0}")]
    SendStdinError(#[from] SendStdinErrorReason),
}
