#[cfg(test)]
use proptest::prelude::*;
use std::collections::HashMap;
use tcrm_task::tasks::config::{StreamSource, TaskConfig};

use crate::{
    flatbuffers::{
        conversion::{ToFlatbuffers, error::ConversionError, tcrm_tasks_to_flatbuffers},
        tcrm_monitor_generated::tcrm::monitor,
    },
    monitor::config::{TaskShell, TaskSpec, TcrmTasks},
};

// Property-based test strategies

prop_compose! {
    fn arb_task_config()
        (command in "[a-zA-Z][a-zA-Z0-9_-]{0,50}",
         args in prop::option::of(prop::collection::vec("[a-zA-Z0-9_-]{0,20}", 0..10)),
         working_dir in prop::option::of("/[a-zA-Z0-9/_-]{0,100}"),
         env_size in 0..10usize,
         timeout_ms in prop::option::of(1u64..300_000),
         enable_stdin in any::<bool>(),
         ready_indicator in prop::option::of("[a-zA-Z0-9_-]{0,50}"),
         ready_indicator_source in prop::option::of(prop_oneof![
             Just(StreamSource::Stdout),
             Just(StreamSource::Stderr)
         ]))
        -> TaskConfig
    {
        let mut env = HashMap::new();
        for i in 0..env_size {
            env.insert(format!("VAR_{}", i), format!("value_{}", i));
        }
        let env = if env.is_empty() { None } else { Some(env) };

        let mut config = TaskConfig::new(command)
            .enable_stdin(enable_stdin);

        if let Some(args_vec) = args {
            if !args_vec.is_empty() {
                config = config.args(args_vec.iter().map(String::as_str));
            }
        }

        if let Some(wd) = working_dir {
            config = config.working_dir(wd);
        }
        if let Some(env_map) = env {
            config = config.env(env_map);
        }
        if let Some(timeout) = timeout_ms {
            config = config.timeout_ms(timeout);
        }
        if let Some(indicator) = ready_indicator {
            config = config.ready_indicator(indicator);
        }
        if let Some(source) = ready_indicator_source {
            config = config.ready_indicator_source(source);
        }
        config
    }
}

prop_compose! {
    fn arb_task_shell()
        (shell_type in 0..5u8)
        -> TaskShell
    {
        match shell_type {
            0 => TaskShell::None,
            1 => TaskShell::Auto,
            #[cfg(windows)]
            2 => TaskShell::Cmd,
            #[cfg(windows)]
            3 => TaskShell::Powershell,
            #[cfg(unix)]
            2 => TaskShell::Bash,
            _ => TaskShell::Auto,
        }
    }
}

prop_compose! {
    fn arb_task_spec()
        (config in arb_task_config(),
         shell in prop::option::of(arb_task_shell()),
         dependencies in prop::option::of(prop::collection::vec("[a-zA-Z][a-zA-Z0-9_-]{0,20}", 0..10)),
         terminate_after_dependents_finished in any::<bool>(),
         ignore_dependencies_error in any::<bool>())
        -> TaskSpec
    {
        TaskSpec {
            config,
            shell,
            dependencies,
            terminate_after_dependents_finished: Some(terminate_after_dependents_finished),
            ignore_dependencies_error: Some(ignore_dependencies_error),
        }
    }
}

prop_compose! {
    fn arb_tcrm_tasks()
        (tasks in prop::collection::btree_map("[a-zA-Z][a-zA-Z0-9_-]{0,20}", arb_task_spec(), 0..20))
        -> TcrmTasks
    {
        tasks.into_iter().collect()
    }
}

proptest! {
    #[test]
    fn test_taskshell_roundtrip_conversion(shell in arb_task_shell()) {
        use monitor::TaskShell as FbTaskShell;

        let fb_shell: FbTaskShell = shell.clone().into();
        let converted_back: Result<TaskShell, ConversionError> = fb_shell.try_into();

        match converted_back {
            Ok(result) => {
                // Cross-platform shells may be converted to Auto on unsupported platforms
                #[cfg(windows)]
                prop_assert_eq!(result, shell);
                #[cfg(unix)]
                prop_assert_eq!(result, shell);
                #[cfg(not(any(windows, unix)))]
                prop_assert!(matches!(result, TaskShell::Auto | TaskShell::None) || result == shell);
            }
            Err(_) => {
                // Should not fail for valid shell types
                prop_assert!(false, "TaskShell conversion should not fail for valid shells");
            }
        }
    }

    #[test]
    fn test_taskspec_roundtrip_conversion(spec in arb_task_spec()) {
        use flatbuffers::FlatBufferBuilder;

        let mut fbb = FlatBufferBuilder::new();
        let fb_spec_offset = spec.to_flatbuffers(&mut fbb);

        // No need to check is_ok() since to_flatbuffers returns WIPOffset directly
        fbb.finish(fb_spec_offset, None);

        let buf = fbb.finished_data();
        let fb_spec = flatbuffers::root::<monitor::TaskSpec>(buf);
        prop_assert!(fb_spec.is_ok(), "FlatBuffer parsing should succeed");

        let fb_spec = fb_spec.unwrap();
        let converted_spec = TaskSpec::try_from(fb_spec);
        prop_assert!(converted_spec.is_ok(), "TaskSpec try_from should succeed");

        let converted_spec = converted_spec.unwrap();

        // Verify core fields are preserved
        prop_assert_eq!(converted_spec.config.command, spec.config.command);
        prop_assert_eq!(converted_spec.config.args, spec.config.args);
        prop_assert_eq!(converted_spec.config.working_dir, spec.config.working_dir);
        prop_assert_eq!(converted_spec.config.timeout_ms, spec.config.timeout_ms);
        prop_assert_eq!(converted_spec.config.enable_stdin, spec.config.enable_stdin);
        prop_assert_eq!(converted_spec.config.ready_indicator, spec.config.ready_indicator);

        // Note: ready_indicator_source may get default value when None in flatbuffers
        // This is expected behavior from the tcrm_task library
        match (spec.config.ready_indicator_source, converted_spec.config.ready_indicator_source) {
            (None, Some(StreamSource::Stdout)) => {
                // This is expected - unset fields default to Stdout in tcrm_task library
            }
            (a, b) => prop_assert_eq!(a, b),
        }

        prop_assert_eq!(converted_spec.dependencies, spec.dependencies);
        prop_assert_eq!(converted_spec.terminate_after_dependents_finished, spec.terminate_after_dependents_finished);
        prop_assert_eq!(converted_spec.ignore_dependencies_error, spec.ignore_dependencies_error);

        // Environment may have different ordering, so check contents
        match (&converted_spec.config.env, &spec.config.env) {
            (Some(converted_env), Some(original_env)) => {
                prop_assert_eq!(converted_env.len(), original_env.len());
                for (key, value) in original_env {
                    prop_assert_eq!(converted_env.get(key), Some(value));
                }
            }
            (None, None) => {} // Both None, OK
            _ => prop_assert!(false, "Environment conversion mismatch"),
        }
    }

    #[test]
    fn test_tcrm_tasks_roundtrip_conversion(tasks in arb_tcrm_tasks()) {
        use flatbuffers::FlatBufferBuilder;

        let mut fbb = FlatBufferBuilder::new();
        let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);

        // No need to check is_ok() since the helper function returns WIPOffset directly
        fbb.finish(fb_tasks_offset, None);

        let buf = fbb.finished_data();
        let fb_tasks = flatbuffers::root::<monitor::TcrmTasks>(buf);
        prop_assert!(fb_tasks.is_ok(), "FlatBuffer parsing should succeed");

        let fb_tasks = fb_tasks.unwrap();
        let converted_tasks = TcrmTasks::try_from(fb_tasks);
        prop_assert!(converted_tasks.is_ok(), "TcrmTasks try_from should succeed");

        let converted_tasks = converted_tasks.unwrap();

        // Verify task count and names
        prop_assert_eq!(converted_tasks.len(), tasks.len());

        for (name, original_spec) in &tasks {
            let converted_spec = converted_tasks.get(name);
            prop_assert!(converted_spec.is_some(), "Task {} should exist after conversion", name);

            let converted_spec = converted_spec.unwrap();
            prop_assert_eq!(&converted_spec.config.command, &original_spec.config.command);
        }
    }

    #[test]
    fn test_conversion_preserves_data_integrity(spec in arb_task_spec()) {
        use flatbuffers::FlatBufferBuilder;

        // Test multiple roundtrips to ensure data integrity
        let original_command = spec.config.command.clone();
        let original_args = spec.config.args.clone();
        let original_dependencies = spec.dependencies.clone();
        let mut current_spec = spec;

        for _iteration in 0..3 {
            let mut fbb = FlatBufferBuilder::new();
            let fb_spec_offset = current_spec.to_flatbuffers(&mut fbb);
            fbb.finish(fb_spec_offset, None);

            let buf = fbb.finished_data();
            let fb_spec = flatbuffers::root::<monitor::TaskSpec>(buf).unwrap();
            current_spec = TaskSpec::try_from(fb_spec).unwrap();
        }

        // After multiple roundtrips, core data should be preserved
        prop_assert_eq!(current_spec.config.command, original_command);
        prop_assert_eq!(current_spec.config.args, original_args);
        prop_assert_eq!(current_spec.dependencies, original_dependencies);
    }

    #[test]
    fn test_empty_and_minimal_cases(
        command in "[a-zA-Z][a-zA-Z0-9_-]{1,10}"
    ) {
        use flatbuffers::FlatBufferBuilder;

        // Test minimal TaskConfig
        let minimal_config = TaskConfig::new(command.clone());

        let minimal_spec = TaskSpec::new(minimal_config);

        let mut fbb = FlatBufferBuilder::new();
        let fb_spec_offset = minimal_spec.to_flatbuffers(&mut fbb);
        fbb.finish(fb_spec_offset, None);

        let buf = fbb.finished_data();
        let fb_spec = flatbuffers::root::<monitor::TaskSpec>(buf).unwrap();
        let converted_spec = TaskSpec::try_from(fb_spec).unwrap();

        prop_assert_eq!(converted_spec.config.command, command);
        prop_assert_eq!(converted_spec.config.args, None);
        prop_assert_eq!(converted_spec.dependencies, None);
    }

    #[test]
    fn test_large_data_conversion(
        base_command in "[a-zA-Z][a-zA-Z0-9_-]{1,10}",
        arg_count in 1..100usize,
        env_count in 1..100usize,
        dep_count in 1..50usize
    ) {
        use flatbuffers::FlatBufferBuilder;

        // Create a TaskSpec with large amounts of data
        let args: Vec<String> = (0..arg_count).map(|i| format!("arg_{}", i)).collect();
        let mut env = HashMap::new();
        for i in 0..env_count {
            env.insert(format!("ENV_VAR_{}", i), format!("value_{}", i));
        }
        let dependencies: Vec<String> = (0..dep_count).map(|i| format!("dep_{}", i)).collect();

        let large_config = TaskConfig::new(base_command.clone())
            .args(args.clone())
            .working_dir("/very/long/path/to/working/directory")
            .env(env.clone())
            .timeout_ms(60000)
            .enable_stdin(true)
            .ready_indicator("READY_INDICATOR")
            .ready_indicator_source(StreamSource::Stderr)
            .use_process_group(true);

        let mut large_spec = TaskSpec::new(large_config)
            .shell(TaskShell::Auto)
            .dependencies(dependencies.clone());
        large_spec.terminate_after_dependents_finished = Some(true);
        large_spec.ignore_dependencies_error = Some(false);

        let mut fbb = FlatBufferBuilder::new();
        let fb_spec_offset = large_spec.to_flatbuffers(&mut fbb);
        fbb.finish(fb_spec_offset, None);

        let buf = fbb.finished_data();
        let fb_spec = flatbuffers::root::<monitor::TaskSpec>(buf).unwrap();
        let converted_spec = TaskSpec::try_from(fb_spec).unwrap();

        prop_assert_eq!(converted_spec.config.command, base_command);
        prop_assert_eq!(converted_spec.config.args.as_ref().unwrap().len(), arg_count);
        prop_assert_eq!(converted_spec.config.env.as_ref().unwrap().len(), env_count);
        prop_assert_eq!(converted_spec.dependencies.as_ref().unwrap().len(), dep_count);
    }
}

#[cfg(test)]
mod deterministic_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_specific_edge_cases() {
        // Test empty string command (should this be allowed?)
        let empty_command_config = TaskConfig::new("");

        let spec = TaskSpec::new(empty_command_config);

        use flatbuffers::FlatBufferBuilder;
        let mut fbb = FlatBufferBuilder::new();
        let _result = spec.to_flatbuffers(&mut fbb);
        // Empty command should be handled gracefully - no need to check is_ok()
    }

    #[test]
    fn test_unicode_handling() {
        let mut env = HashMap::new();
        env.insert("UNICODE_VAR".to_string(), "🚀 Rocket".to_string());

        let unicode_config = TaskConfig::new("echo")
            .args(["Hello 🌍", "مرحبا", "你好"])
            .working_dir("/tmp/测试目录")
            .env(env)
            .timeout_ms(5000)
            .ready_indicator("準備完了")
            .ready_indicator_source(StreamSource::Stdout);

        let spec = TaskSpec::new(unicode_config)
            .shell(TaskShell::Auto)
            .dependencies(["前置任务"]);

        use flatbuffers::FlatBufferBuilder;
        let mut fbb = FlatBufferBuilder::new();
        let fb_spec_offset = spec.to_flatbuffers(&mut fbb);
        fbb.finish(fb_spec_offset, None);

        let buf = fbb.finished_data();
        let fb_spec = flatbuffers::root::<monitor::TaskSpec>(buf).unwrap();
        let converted_spec = TaskSpec::try_from(fb_spec).unwrap();

        assert_eq!(converted_spec.config.args.as_ref().unwrap()[0], "Hello 🌍");
        assert_eq!(converted_spec.config.args.as_ref().unwrap()[1], "مرحبا");
        assert_eq!(converted_spec.config.args.as_ref().unwrap()[2], "你好");
        assert_eq!(
            converted_spec.config.working_dir.as_ref().unwrap(),
            "/tmp/测试目录"
        );
        assert_eq!(
            converted_spec.config.ready_indicator.as_ref().unwrap(),
            "準備完了"
        );
        assert_eq!(converted_spec.dependencies.as_ref().unwrap()[0], "前置任务");
    }

    #[test]
    fn test_boundary_values() {
        let boundary_config = TaskConfig::new("test")
            .timeout_ms(u64::MAX)
            .enable_stdin(true);

        let spec = TaskSpec::new(boundary_config)
            .shell(TaskShell::Auto)
            .terminate_after_dependents(true);

        use flatbuffers::FlatBufferBuilder;
        let mut fbb = FlatBufferBuilder::new();
        let fb_spec_offset = spec.to_flatbuffers(&mut fbb);
        fbb.finish(fb_spec_offset, None);

        let buf = fbb.finished_data();
        let fb_spec = flatbuffers::root::<monitor::TaskSpec>(buf).unwrap();
        let converted_spec = TaskSpec::try_from(fb_spec).unwrap();

        assert_eq!(converted_spec.config.timeout_ms, Some(u64::MAX));
    }
}
