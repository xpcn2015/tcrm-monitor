use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaskMonitorError {
    #[error("Config parse error: {0}")]
    ConfigParse(String),
    #[error("Circular dependency detected involving task '{0}'")]
    CircularDependency(String),
    #[error("Dependency '{dep}' not found for task '{task}'")]
    DependencyNotFound { dep: String, task: String },
}
