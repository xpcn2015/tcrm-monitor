use flatbuffers::{FlatBufferBuilder, WIPOffset};
use std::collections::HashMap;
use tcrm_task::tasks::config::TaskConfig;

use crate::flatbuffers::conversion::error::ConversionError;
use crate::flatbuffers::tcrm_monitor_generated::tcrm::monitor::{
    TaskEntry as FbTaskEntry, TaskEntryArgs, TaskShell as FbTaskShell, TaskSpec as FbTaskSpec,
    TaskSpecArgs, TcrmTasks as FbTcrmTasks, TcrmTasksArgs,
};
use crate::monitor::config::{
    TaskShell as ConfigTaskShell, TaskSpec as ConfigTaskSpec, TcrmTasks as ConfigTcrmTasks,
};

// TaskShell conversions
impl From<ConfigTaskShell> for FbTaskShell {
    fn from(shell: ConfigTaskShell) -> Self {
        match shell {
            ConfigTaskShell::None => FbTaskShell::None,
            ConfigTaskShell::Auto => FbTaskShell::Auto,
            #[cfg(windows)]
            ConfigTaskShell::Cmd => FbTaskShell::Cmd,
            #[cfg(windows)]
            ConfigTaskShell::Powershell => FbTaskShell::Powershell,
            #[cfg(unix)]
            ConfigTaskShell::Bash => FbTaskShell::Bash,
        }
    }
}

impl TryFrom<FbTaskShell> for ConfigTaskShell {
    type Error = ConversionError;

    fn try_from(shell: FbTaskShell) -> Result<Self, Self::Error> {
        match shell {
            FbTaskShell::None => Ok(ConfigTaskShell::None),
            FbTaskShell::Auto => Ok(ConfigTaskShell::Auto),
            #[cfg(windows)]
            FbTaskShell::Cmd => Ok(ConfigTaskShell::Cmd),
            #[cfg(windows)]
            FbTaskShell::Powershell => Ok(ConfigTaskShell::Powershell),
            #[cfg(unix)]
            FbTaskShell::Bash => Ok(ConfigTaskShell::Bash),
            // Handle cross-platform compatibility
            #[cfg(not(windows))]
            FbTaskShell::Cmd | FbTaskShell::Powershell => Ok(ConfigTaskShell::Auto),
            #[cfg(not(unix))]
            FbTaskShell::Bash => Ok(ConfigTaskShell::Auto),
            _ => Err(ConversionError::InvalidShell(shell.0)),
        }
    }
}

// TaskSpec conversions
impl ConfigTaskSpec {
    /// Convert to flatbuffers TaskSpec
    pub fn to_flatbuffers<'a>(
        &self,
        fbb: &mut FlatBufferBuilder<'a>,
    ) -> Result<WIPOffset<FbTaskSpec<'a>>, ConversionError> {
        // Convert TaskConfig to flatbuffers
        let config_offset = self.config.to_flatbuffers(fbb);

        // Convert dependencies to string vector
        let dependencies_offset = self.dependencies.as_ref().map(|deps| {
            let dep_strings: Vec<_> = deps.iter().map(|dep| fbb.create_string(dep)).collect();
            fbb.create_vector(&dep_strings)
        });

        // Build TaskSpec
        let args = TaskSpecArgs {
            config: Some(config_offset),
            shell: self.shell.clone().unwrap_or_default().into(),
            dependencies: dependencies_offset,
            terminate_after_dependents_finished: self
                .terminate_after_dependents_finished
                .unwrap_or(false),
            ignore_dependencies_error: self.ignore_dependencies_error.unwrap_or(false),
        };

        Ok(FbTaskSpec::create(fbb, &args))
    }

    /// Convert from flatbuffers TaskSpec
    pub fn from_flatbuffers(fb_spec: FbTaskSpec) -> Result<Self, ConversionError> {
        // Convert TaskConfig
        let config = TaskConfig::try_from(fb_spec.config())
            .map_err(|e| ConversionError::TaskConfigConversion(format!("{:?}", e)))?;

        // Convert dependencies
        let dependencies = fb_spec.dependencies().map(|deps| {
            (0..deps.len())
                .map(|i| deps.get(i).to_string())
                .collect::<Vec<String>>()
        });

        // Convert shell
        let shell = Some(fb_spec.shell().try_into()?);

        Ok(ConfigTaskSpec {
            config,
            shell,
            dependencies,
            terminate_after_dependents_finished: Some(
                fb_spec.terminate_after_dependents_finished(),
            ),
            ignore_dependencies_error: Some(fb_spec.ignore_dependencies_error()),
        })
    }
}

pub fn tasks_to_flatbuffers<'a>(
    tasks: &ConfigTcrmTasks,
    fbb: &mut FlatBufferBuilder<'a>,
) -> Result<WIPOffset<FbTcrmTasks<'a>>, ConversionError> {
    // Convert each task entry
    let entries: Result<Vec<_>, ConversionError> = tasks
        .iter()
        .map(|(name, spec)| {
            let name_str = fbb.create_string(name);
            let spec_offset = spec.to_flatbuffers(fbb)?;

            let entry_args = TaskEntryArgs {
                name: Some(name_str),
                spec: Some(spec_offset),
            };
            Ok(FbTaskEntry::create(fbb, &entry_args))
        })
        .collect();

    let entries = entries?;
    let tasks_vector = fbb.create_vector(&entries);

    let args = TcrmTasksArgs {
        tasks: Some(tasks_vector),
    };

    Ok(FbTcrmTasks::create(fbb, &args))
}

pub fn tasks_from_flatbuffers(fb_tasks: FbTcrmTasks) -> Result<ConfigTcrmTasks, ConversionError> {
    let mut tasks = HashMap::new();

    if let Some(entries) = fb_tasks.tasks() {
        for i in 0..entries.len() {
            let entry = entries.get(i);
            let name = entry.name().to_string();
            let spec = ConfigTaskSpec::from_flatbuffers(entry.spec())?;
            tasks.insert(name, spec);
        }
    }

    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::config::{
        TaskShell as ConfigTaskShell, TaskSpec as ConfigTaskSpec, TcrmTasks as ConfigTcrmTasks,
    };
    use flatbuffers::FlatBufferBuilder;
    use std::collections::HashMap;
    use tcrm_task::tasks::config::TaskConfig;

    #[test]
    fn test_taskshell_conversions() {
        // Test None conversion
        let shell = ConfigTaskShell::None;
        let fb_shell: FbTaskShell = shell.into();
        assert_eq!(fb_shell, FbTaskShell::None);

        let converted_back: ConfigTaskShell = fb_shell.try_into().unwrap();
        assert_eq!(converted_back, ConfigTaskShell::None);

        // Test Auto conversion
        let shell = ConfigTaskShell::Auto;
        let fb_shell: FbTaskShell = shell.into();
        assert_eq!(fb_shell, FbTaskShell::Auto);

        let converted_back: ConfigTaskShell = fb_shell.try_into().unwrap();
        assert_eq!(converted_back, ConfigTaskShell::Auto);

        // Test platform-specific shells
        #[cfg(windows)]
        {
            let shell = ConfigTaskShell::Cmd;
            let fb_shell: FbTaskShell = shell.into();
            assert_eq!(fb_shell, FbTaskShell::Cmd);

            let converted_back: ConfigTaskShell = fb_shell.try_into().unwrap();
            assert_eq!(converted_back, ConfigTaskShell::Cmd);

            let shell = ConfigTaskShell::Powershell;
            let fb_shell: FbTaskShell = shell.into();
            assert_eq!(fb_shell, FbTaskShell::Powershell);

            let converted_back: ConfigTaskShell = fb_shell.try_into().unwrap();
            assert_eq!(converted_back, ConfigTaskShell::Powershell);
        }

        #[cfg(unix)]
        {
            let shell = ConfigTaskShell::Bash;
            let fb_shell: FbTaskShell = shell.into();
            assert_eq!(fb_shell, FbTaskShell::Bash);

            let converted_back: ConfigTaskShell = fb_shell.try_into().unwrap();
            assert_eq!(converted_back, ConfigTaskShell::Bash);
        }
    }

    #[test]
    fn test_taskspec_conversions() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        env.insert("HOME".to_string(), "/home/user".to_string());

        let task_config = TaskConfig {
            command: "echo".to_string(),
            args: Some(vec!["hello".to_string(), "world".to_string()]),
            working_dir: Some("/tmp".to_string()),
            env: Some(env),
            timeout_ms: Some(5000),
            enable_stdin: Some(true),
            ready_indicator: Some("ready".to_string()),
            ready_indicator_source: Some(tcrm_task::tasks::config::StreamSource::Stdout),
        };

        let spec = ConfigTaskSpec {
            config: task_config,
            shell: Some(ConfigTaskShell::Auto),
            dependencies: Some(vec!["dep1".to_string(), "dep2".to_string()]),
            terminate_after_dependents_finished: Some(true),
            ignore_dependencies_error: Some(false),
        };

        // Test conversion to flatbuffers and back
        let mut fbb = FlatBufferBuilder::new();
        let fb_spec_offset = spec.to_flatbuffers(&mut fbb).unwrap();
        fbb.finish(fb_spec_offset, None);

        let buf = fbb.finished_data();
        let fb_spec = flatbuffers::root::<FbTaskSpec>(buf).unwrap();

        let converted_spec = ConfigTaskSpec::from_flatbuffers(fb_spec).unwrap();

        // Verify all fields are preserved
        assert_eq!(converted_spec.config.command, "echo");
        assert_eq!(
            converted_spec.config.args,
            Some(vec!["hello".to_string(), "world".to_string()])
        );
        assert_eq!(converted_spec.config.working_dir, Some("/tmp".to_string()));
        assert_eq!(converted_spec.config.timeout_ms, Some(5000));
        assert_eq!(converted_spec.config.enable_stdin, Some(true));
        assert_eq!(
            converted_spec.config.ready_indicator,
            Some("ready".to_string())
        );
        assert_eq!(converted_spec.shell, Some(ConfigTaskShell::Auto));
        assert_eq!(
            converted_spec.dependencies,
            Some(vec!["dep1".to_string(), "dep2".to_string()])
        );
        assert_eq!(
            converted_spec.terminate_after_dependents_finished,
            Some(true)
        );
        assert_eq!(converted_spec.ignore_dependencies_error, Some(false));

        // Check environment variables
        let env = converted_spec.config.env.unwrap();
        assert_eq!(env.get("PATH"), Some(&"/usr/bin".to_string()));
        assert_eq!(env.get("HOME"), Some(&"/home/user".to_string()));
    }

    #[test]
    fn test_taskspec_minimal() {
        // Test with minimal TaskSpec (only required fields)
        let task_config = TaskConfig {
            command: "ls".to_string(),
            args: None,
            working_dir: None,
            env: None,
            timeout_ms: None,
            enable_stdin: None,
            ready_indicator: None,
            ready_indicator_source: None,
        };

        let spec = ConfigTaskSpec {
            config: task_config,
            shell: None,
            dependencies: None,
            terminate_after_dependents_finished: None,
            ignore_dependencies_error: None,
        };

        // Test conversion
        let mut fbb = FlatBufferBuilder::new();
        let fb_spec_offset = spec.to_flatbuffers(&mut fbb).unwrap();
        fbb.finish(fb_spec_offset, None);

        let buf = fbb.finished_data();
        let fb_spec = flatbuffers::root::<FbTaskSpec>(buf).unwrap();

        let converted_spec = ConfigTaskSpec::from_flatbuffers(fb_spec).unwrap();

        assert_eq!(converted_spec.config.command, "ls");
        assert_eq!(converted_spec.config.args, None);
        assert_eq!(converted_spec.config.working_dir, None);
        assert_eq!(converted_spec.config.env, None);
        assert_eq!(converted_spec.config.timeout_ms, None);
        assert_eq!(converted_spec.dependencies, None);
    }

    #[test]
    fn test_tcrm_tasks_conversions() {
        let mut tasks = ConfigTcrmTasks::new();

        // Add first task
        let task_config1 = TaskConfig {
            command: "echo".to_string(),
            args: Some(vec!["task1".to_string()]),
            working_dir: None,
            env: None,
            timeout_ms: Some(1000),
            enable_stdin: Some(false),
            ready_indicator: None,
            ready_indicator_source: None,
        };

        let spec1 = ConfigTaskSpec {
            config: task_config1,
            shell: Some(ConfigTaskShell::Auto),
            dependencies: None,
            terminate_after_dependents_finished: Some(false),
            ignore_dependencies_error: Some(true),
        };

        // Add second task
        let task_config2 = TaskConfig {
            command: "sleep".to_string(),
            args: Some(vec!["1".to_string()]),
            working_dir: Some("/tmp".to_string()),
            env: None,
            timeout_ms: None,
            enable_stdin: None,
            ready_indicator: Some("done".to_string()),
            ready_indicator_source: Some(tcrm_task::tasks::config::StreamSource::Stderr),
        };

        let spec2 = ConfigTaskSpec {
            config: task_config2,
            shell: Some(ConfigTaskShell::None),
            dependencies: Some(vec!["task1".to_string()]),
            terminate_after_dependents_finished: Some(true),
            ignore_dependencies_error: Some(false),
        };

        tasks.insert("task1".to_string(), spec1);
        tasks.insert("task2".to_string(), spec2);

        // Test conversion to flatbuffers and back
        let mut fbb = FlatBufferBuilder::new();
        let fb_tasks_offset = tasks_to_flatbuffers(&tasks, &mut fbb).unwrap();
        fbb.finish(fb_tasks_offset, None);

        let buf = fbb.finished_data();
        let fb_tasks = flatbuffers::root::<FbTcrmTasks>(buf).unwrap();

        let converted_tasks = tasks_from_flatbuffers(fb_tasks).unwrap();

        // Verify task count
        assert_eq!(converted_tasks.len(), 2);

        // Verify task1
        let task1 = converted_tasks.get("task1").unwrap();
        assert_eq!(task1.config.command, "echo");
        assert_eq!(task1.config.args, Some(vec!["task1".to_string()]));
        assert_eq!(task1.config.timeout_ms, Some(1000));
        assert_eq!(task1.shell, Some(ConfigTaskShell::Auto));
        assert_eq!(task1.dependencies, None);
        assert_eq!(task1.terminate_after_dependents_finished, Some(false));
        assert_eq!(task1.ignore_dependencies_error, Some(true));

        // Verify task2
        let task2 = converted_tasks.get("task2").unwrap();
        assert_eq!(task2.config.command, "sleep");
        assert_eq!(task2.config.args, Some(vec!["1".to_string()]));
        assert_eq!(task2.config.working_dir, Some("/tmp".to_string()));
        assert_eq!(task2.config.ready_indicator, Some("done".to_string()));
        assert_eq!(task2.shell, Some(ConfigTaskShell::None));
        assert_eq!(task2.dependencies, Some(vec!["task1".to_string()]));
        assert_eq!(task2.terminate_after_dependents_finished, Some(true));
        assert_eq!(task2.ignore_dependencies_error, Some(false));
    }

    #[test]
    fn test_empty_tcrm_tasks() {
        let tasks = ConfigTcrmTasks::new();

        // Test conversion of empty tasks
        let mut fbb = FlatBufferBuilder::new();
        let fb_tasks_offset = tasks_to_flatbuffers(&tasks, &mut fbb).unwrap();
        fbb.finish(fb_tasks_offset, None);

        let buf = fbb.finished_data();
        let fb_tasks = flatbuffers::root::<FbTcrmTasks>(buf).unwrap();

        let converted_tasks = tasks_from_flatbuffers(fb_tasks).unwrap();

        assert_eq!(converted_tasks.len(), 0);
    }

    #[test]
    fn test_conversion_error_display() {
        let error = ConversionError::TaskConfigConversion("test error".to_string());
        assert_eq!(error.to_string(), "TaskConfig conversion error: test error");

        let error = ConversionError::InvalidShell(42);
        assert_eq!(error.to_string(), "Invalid TaskShell value: 42");

        let error = ConversionError::MissingRequiredField("test_field");
        assert_eq!(error.to_string(), "Missing required field: test_field");
    }
}
