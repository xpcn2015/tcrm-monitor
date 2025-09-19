use flatbuffers::{FlatBufferBuilder, WIPOffset};
use std::collections::HashMap;
use tcrm_task::flatbuffers::conversion::ToFlatbuffers as TcrmToFlatbuffers;
use tcrm_task::tasks::config::TaskConfig;

use crate::flatbuffers::conversion::ToFlatbuffers;
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
            #[cfg(unix)]
            ConfigTaskShell::Sh => FbTaskShell::Sh,
            #[cfg(unix)]
            ConfigTaskShell::Zsh => FbTaskShell::Zsh,
            #[cfg(unix)]
            ConfigTaskShell::Fish => FbTaskShell::Fish,
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
            #[cfg(unix)]
            FbTaskShell::Sh => Ok(ConfigTaskShell::Sh),
            #[cfg(unix)]
            FbTaskShell::Zsh => Ok(ConfigTaskShell::Zsh),
            #[cfg(unix)]
            FbTaskShell::Fish => Ok(ConfigTaskShell::Fish),
            // Handle cross-platform compatibility
            #[cfg(not(windows))]
            FbTaskShell::Cmd | FbTaskShell::Powershell => Ok(ConfigTaskShell::Auto),
            #[cfg(not(unix))]
            FbTaskShell::Bash | FbTaskShell::Sh | FbTaskShell::Zsh | FbTaskShell::Fish => {
                Ok(ConfigTaskShell::Auto)
            }
            _ => Err(ConversionError::InvalidShell(shell.0)),
        }
    }
}

// TaskSpec conversions
impl<'a> ToFlatbuffers<'a> for ConfigTaskSpec {
    type Output = WIPOffset<FbTaskSpec<'a>>;

    fn to_flatbuffers(&self, fbb: &mut FlatBufferBuilder<'a>) -> Self::Output {
        // Use tcrm_task's to_flatbuffers for TaskConfig
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

        FbTaskSpec::create(fbb, &args)
    }
}

impl TryFrom<FbTaskSpec<'_>> for ConfigTaskSpec {
    type Error = ConversionError;

    fn try_from(fb_spec: FbTaskSpec) -> Result<Self, Self::Error> {
        // Convert TaskConfig
        let config = TaskConfig::try_from(fb_spec.config())
            .map_err(|e| ConversionError::TaskConfigConversion(format!("{e:?}")))?;

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

impl TryFrom<FbTcrmTasks<'_>> for ConfigTcrmTasks {
    type Error = ConversionError;

    fn try_from(fb_tasks: FbTcrmTasks) -> Result<Self, Self::Error> {
        let mut tasks = HashMap::new();
        if let Some(entries) = fb_tasks.tasks() {
            for i in 0..entries.len() {
                let entry = entries.get(i);
                let name = entry.name().to_string();
                let spec = ConfigTaskSpec::try_from(entry.spec())?;
                tasks.insert(name, spec);
            }
        }
        Ok(tasks)
    }
}

// Helper function for converting TcrmTasks to FlatBuffers since we can't implement
// ToFlatbuffers for HashMap due to orphan rules

/// Convert a `TcrmTasks` (HashMap of task specs) to FlatBuffers.
///
/// This helper function serializes a `TcrmTasks` (HashMap<String, TaskSpec>)
/// into a FlatBuffers binary representation, producing a `WIPOffset<FbTcrmTasks>`.
/// It is required because Rust's orphan rules prevent implementing the `ToFlatbuffers`
/// trait directly for `HashMap` types from external crates.
///
/// # Parameters
/// - `tasks`: The map of task names to task specifications to serialize.
/// - `fbb`: The FlatBufferBuilder used to build the FlatBuffers data.
///
/// # Returns
/// A FlatBuffers offset to the serialized `TcrmTasks` table.
///
/// # Example
/// ```rust
/// use tcrm_monitor::flatbuffers::conversion::tcrm_tasks_to_flatbuffers;
/// use tcrm_monitor::monitor::config::{TaskSpec, TaskShell, TcrmTasks};
/// use tcrm_task::tasks::config::TaskConfig;
/// use flatbuffers::FlatBufferBuilder;
///
/// let mut tasks = TcrmTasks::new();
/// tasks.insert(
///     "build".to_string(),
///     TaskSpec::new(TaskConfig::new("cargo").args(["build"]))
///         .shell(TaskShell::Auto)
/// );
/// let mut fbb = FlatBufferBuilder::new();
/// let fb_offset = tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
/// fbb.finish(fb_offset, None);
/// let data = fbb.finished_data();
/// // ...
/// ```
pub fn tcrm_tasks_to_flatbuffers<'a>(
    tasks: &ConfigTcrmTasks,
    fbb: &mut FlatBufferBuilder<'a>,
) -> WIPOffset<FbTcrmTasks<'a>> {
    // Convert each task entry
    let entries: Vec<_> = tasks
        .iter()
        .map(|(name, spec)| {
            let name_str = fbb.create_string(name);
            let spec_offset = spec.to_flatbuffers(fbb);

            let entry_args = TaskEntryArgs {
                name: Some(name_str),
                spec: Some(spec_offset),
            };
            FbTaskEntry::create(fbb, &entry_args)
        })
        .collect();

    let tasks_vector = fbb.create_vector(&entries);

    let args = TcrmTasksArgs {
        tasks: Some(tasks_vector),
    };

    FbTcrmTasks::create(fbb, &args)
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

        // Direct byte read test
        let shell = ConfigTaskShell::Auto;
        let fb_shell: FbTaskShell = shell.clone().into();
        let raw_value: i8 = match fb_shell {
            FbTaskShell::None => 0,
            FbTaskShell::Auto => 1,
            #[cfg(windows)]
            FbTaskShell::Cmd => 2,
            #[cfg(windows)]
            FbTaskShell::Powershell => 3,
            #[cfg(unix)]
            FbTaskShell::Bash => 2,
            _ => -1,
        };
        let direct_shell = match raw_value {
            0 => ConfigTaskShell::None,
            1 => ConfigTaskShell::Auto,
            #[cfg(windows)]
            2 => ConfigTaskShell::Cmd,
            #[cfg(windows)]
            3 => ConfigTaskShell::Powershell,
            #[cfg(unix)]
            2 => ConfigTaskShell::Bash,
            _ => panic!("Unknown shell value: {}", raw_value),
        };
        assert_eq!(shell, direct_shell);
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
            ..Default::default()
        };

        let spec = ConfigTaskSpec {
            config: task_config,
            shell: Some(ConfigTaskShell::Auto),
            dependencies: Some(vec!["dep1".to_string(), "dep2".to_string()]),
            terminate_after_dependents_finished: Some(true),
            ..Default::default()
        };

        // Test conversion to flatbuffers and back
        let mut fbb = FlatBufferBuilder::new();
        let fb_spec_offset = spec.to_flatbuffers(&mut fbb);
        fbb.finish(fb_spec_offset, None);

        let buf = fbb.finished_data();
        let fb_spec = flatbuffers::root::<FbTaskSpec>(buf).unwrap();

        let converted_spec = ConfigTaskSpec::try_from(fb_spec).unwrap();

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

        // Direct byte read test for TaskSpec: compare all fields to original
        let config = fb_spec.config();
        assert_eq!(config.command(), spec.config.command);
        assert_eq!(config.working_dir(), spec.config.working_dir.as_deref());
        assert_eq!(config.timeout_ms(), spec.config.timeout_ms.unwrap_or(0));
        assert_eq!(
            config.enable_stdin(),
            spec.config.enable_stdin.unwrap_or(false)
        );
        assert_eq!(
            config.ready_indicator(),
            spec.config.ready_indicator.as_deref()
        );
        // ready_indicator_source: compare as i8
        let orig_source = spec.config.ready_indicator_source.map(|s| match s {
            tcrm_task::tasks::config::StreamSource::Stdout => 0,
            tcrm_task::tasks::config::StreamSource::Stderr => 1,
        });
        let fb_source = match config.ready_indicator_source().0 {
            0 => 0,         // Stdout
            1 => 1,         // Stderr
            other => other, // fallback for unexpected values
        };
        if let Some(orig_source) = orig_source {
            assert_eq!(fb_source, orig_source);
        }
        // args
        let orig_args = spec
            .config
            .args
            .as_ref()
            .map(|v| v.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        let fb_args = config
            .args()
            .map(|v| (0..v.len()).map(|i| v.get(i)).collect::<Vec<_>>());
        assert_eq!(fb_args, orig_args);
        // env
        let orig_env = spec.config.env.as_ref().map(|env| {
            env.iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect::<Vec<_>>()
        });
        let fb_env = config.env().map(|v| {
            (0..v.len())
                .map(|i| (v.get(i).key(), v.get(i).value()))
                .collect::<Vec<_>>()
        });
        assert_eq!(fb_env, orig_env);
        // shell
        let direct_shell_val = match fb_spec.shell() {
            FbTaskShell::None => 0,
            FbTaskShell::Auto => 1,
            #[cfg(windows)]
            FbTaskShell::Cmd => 2,
            #[cfg(windows)]
            FbTaskShell::Powershell => 3,
            #[cfg(unix)]
            FbTaskShell::Bash => 2,
            _ => -1,
        };
        let orig_shell_val = match spec.shell.unwrap_or(ConfigTaskShell::None) {
            ConfigTaskShell::None => 0,
            ConfigTaskShell::Auto => 1,
            #[cfg(windows)]
            ConfigTaskShell::Cmd => 2,
            #[cfg(windows)]
            ConfigTaskShell::Powershell => 3,
            #[cfg(unix)]
            ConfigTaskShell::Bash => 2,
        };
        assert_eq!(direct_shell_val, orig_shell_val);
        // dependencies
        let orig_deps = spec
            .dependencies
            .as_ref()
            .map(|v| v.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        let fb_deps = fb_spec
            .dependencies()
            .map(|v| (0..v.len()).map(|i| v.get(i)).collect::<Vec<_>>());
        assert_eq!(fb_deps, orig_deps);
        // terminate_after_dependents_finished
        assert_eq!(
            fb_spec.terminate_after_dependents_finished(),
            spec.terminate_after_dependents_finished.unwrap_or(false)
        );
        // ignore_dependencies_error
        assert_eq!(
            fb_spec.ignore_dependencies_error(),
            spec.ignore_dependencies_error.unwrap_or(false)
        );
    }

    #[test]
    fn test_taskspec_minimal() {
        // Test with minimal TaskSpec (only required fields)
        let task_config = TaskConfig {
            command: "ls".to_string(),
            ..Default::default()
        };

        let spec = ConfigTaskSpec {
            config: task_config,
            ..Default::default()
        };

        // Test conversion
        let mut fbb = FlatBufferBuilder::new();
        let fb_spec_offset = spec.to_flatbuffers(&mut fbb);
        fbb.finish(fb_spec_offset, None);

        let buf = fbb.finished_data();
        let fb_spec = flatbuffers::root::<FbTaskSpec>(buf).unwrap();

        let converted_spec = ConfigTaskSpec::try_from(fb_spec).unwrap();

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
            timeout_ms: Some(1000),
            ..Default::default()
        };

        let spec1 = ConfigTaskSpec {
            config: task_config1,
            shell: Some(ConfigTaskShell::Auto),
            ignore_dependencies_error: Some(true),
            ..Default::default()
        };

        // Add second task
        let task_config2 = TaskConfig {
            command: "sleep".to_string(),
            args: Some(vec!["1".to_string()]),
            working_dir: Some("/tmp".to_string()),
            ready_indicator: Some("done".to_string()),
            ready_indicator_source: Some(tcrm_task::tasks::config::StreamSource::Stderr),
            ..Default::default()
        };

        let spec2 = ConfigTaskSpec {
            config: task_config2,
            shell: Some(ConfigTaskShell::None),
            dependencies: Some(vec!["task1".to_string()]),
            terminate_after_dependents_finished: Some(true),
            ..Default::default()
        };

        tasks.insert("task1".to_string(), spec1);
        tasks.insert("task2".to_string(), spec2);

        // Test conversion to flatbuffers and back
        let mut fbb = FlatBufferBuilder::new();
        let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
        fbb.finish(fb_tasks_offset, None);

        let buf = fbb.finished_data();
        let fb_tasks = flatbuffers::root::<FbTcrmTasks>(buf).unwrap();

        let converted_tasks: ConfigTcrmTasks = fb_tasks.try_into().unwrap();

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
        let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
        fbb.finish(fb_tasks_offset, None);

        let buf = fbb.finished_data();
        let fb_tasks = flatbuffers::root::<FbTcrmTasks>(buf).unwrap();

        let converted_tasks: ConfigTcrmTasks = fb_tasks.try_into().unwrap();

        assert_eq!(converted_tasks.len(), 0);
    }
}
