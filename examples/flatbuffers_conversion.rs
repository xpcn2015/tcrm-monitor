use tcrm_monitor::flatbuffers::conversion::config::*;
use tcrm_monitor::flatbuffers::conversion::error::ConversionError;
use tcrm_monitor::monitor::config::{TaskShell, TaskSpec, TcrmTasks};
use tcrm_task::tasks::config::TaskConfig;

fn main() {
    println!("Testing flatbuffers conversions...");

    // Test TaskShell conversions
    test_shell_conversions();

    // Test TaskSpec conversions
    test_spec_conversions();

    // Test TcrmTasks conversions
    test_tasks_conversions();

    println!("All conversion tests completed successfully!");
}

fn test_shell_conversions() {
    use tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TaskShell as FbTaskShell;

    println!("Testing TaskShell conversions...");

    let shell = TaskShell::Auto;
    let fb_shell: FbTaskShell = shell.clone().into();
    let converted_back: Result<TaskShell, ConversionError> = fb_shell.try_into();

    match converted_back {
        Ok(converted) => {
            println!(
                "✓ TaskShell conversion successful: {:?} -> {:?}",
                shell, converted
            );
            assert_eq!(shell, converted);
        }
        Err(e) => {
            println!("✗ TaskShell conversion failed: {}", e);
            panic!("TaskShell conversion failed");
        }
    }
}

fn test_spec_conversions() {
    println!("Testing TaskSpec conversions...");

    let task_config = TaskConfig {
        command: "echo".to_string(),
        args: Some(vec!["hello".to_string(), "world".to_string()]),
        working_dir: Some("/tmp".to_string()),
        env: None,
        timeout_ms: Some(5000),
        enable_stdin: Some(true),
        ready_indicator: Some("ready".to_string()),
        ready_indicator_source: Some(tcrm_task::tasks::config::StreamSource::Stdout),
    };

    let spec = TaskSpec {
        config: task_config,
        shell: Some(TaskShell::Auto),
        dependencies: Some(vec!["dep1".to_string()]),
        terminate_after_dependents_finished: Some(true),
        ignore_dependencies_error: Some(false),
    };

    let mut fbb = flatbuffers::FlatBufferBuilder::new();
    match spec.to_flatbuffers(&mut fbb) {
        Ok(fb_spec_offset) => {
            fbb.finish(fb_spec_offset, None);
            let fb_data = fbb.finished_data();

            match flatbuffers::root::<
                tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TaskSpec,
            >(fb_data)
            {
                Ok(fb_spec) => match TaskSpec::from_flatbuffers(fb_spec) {
                    Ok(converted_spec) => {
                        println!("✓ TaskSpec roundtrip conversion successful");
                        assert_eq!(converted_spec.config.command, spec.config.command);
                        assert_eq!(converted_spec.shell, spec.shell);
                        assert_eq!(converted_spec.dependencies, spec.dependencies);
                    }
                    Err(e) => {
                        println!("✗ TaskSpec from_flatbuffers failed: {}", e);
                        panic!("TaskSpec conversion failed");
                    }
                },
                Err(e) => {
                    println!("✗ Failed to parse flatbuffer: {:?}", e);
                    panic!("Flatbuffer parsing failed");
                }
            }
        }
        Err(e) => {
            println!("✗ TaskSpec to_flatbuffers failed: {}", e);
            panic!("TaskSpec conversion failed");
        }
    }
}

fn test_tasks_conversions() {
    println!("Testing TcrmTasks conversions...");

    let mut tasks = TcrmTasks::new();

    let task_config = TaskConfig {
        command: "echo".to_string(),
        args: Some(vec!["test".to_string()]),
        working_dir: None,
        env: None,
        timeout_ms: None,
        enable_stdin: None,
        ready_indicator: None,
        ready_indicator_source: None,
    };

    let spec = TaskSpec {
        config: task_config,
        shell: Some(TaskShell::Auto),
        dependencies: None,
        terminate_after_dependents_finished: None,
        ignore_dependencies_error: None,
    };

    tasks.insert("test_task".to_string(), spec);

    let mut fbb2 = flatbuffers::FlatBufferBuilder::new();
    match tasks_to_flatbuffers(&tasks, &mut fbb2) {
        Ok(fb_tasks_offset) => {
            fbb2.finish(fb_tasks_offset, None);
            let fb_data = fbb2.finished_data();

            match flatbuffers::root::<
                tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TcrmTasks,
            >(fb_data)
            {
                Ok(fb_tasks) => match tasks_from_flatbuffers(fb_tasks) {
                    Ok(converted_tasks) => {
                        println!("✓ TcrmTasks roundtrip conversion successful");
                        assert_eq!(converted_tasks.len(), 1);
                        assert!(converted_tasks.contains_key("test_task"));
                    }
                    Err(e) => {
                        println!("✗ TcrmTasks from_flatbuffers failed: {}", e);
                        panic!("TcrmTasks conversion failed");
                    }
                },
                Err(e) => {
                    println!("✗ Failed to parse TcrmTasks flatbuffer: {:?}", e);
                    panic!("TcrmTasks flatbuffer parsing failed");
                }
            }
        }
        Err(e) => {
            println!("✗ TcrmTasks to_flatbuffers failed: {}", e);
            panic!("TcrmTasks conversion failed");
        }
    }
}
