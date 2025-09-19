use tcrm_monitor::flatbuffers::conversion::ToFlatbuffers;
use tcrm_monitor::flatbuffers::conversion::error::ConversionError;
use tcrm_monitor::monitor::config::{TaskShell, TaskSpec, TcrmTasks};
use tcrm_task::tasks::config::TaskConfig;

fn main() {
    println!("\n==============================");
    println!(" FlatBuffers Conversion Example ");
    println!("==============================\n");
    println!(
        "This example demonstrates how to use FlatBuffers conversions for TaskShell, TaskSpec, and TcrmTasks in tcrm-monitor.\n"
    );

    // Demonstrate TaskShell conversion
    shell_conversions();

    // Demonstrate TaskSpec conversion
    spec_conversions();

    // Demonstrate TcrmTasks conversion
    tasks_conversions();

    println!("\nAll FlatBuffers conversion examples completed!\n");
}

fn shell_conversions() {
    use tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TaskShell as FbTaskShell;

    println!("\n--- TaskShell Conversion Example ---");

    let shell = TaskShell::Auto;
    let fb_shell: FbTaskShell = shell.clone().into();
    let converted_back: Result<TaskShell, ConversionError> = fb_shell.try_into();

    match converted_back {
        Ok(converted) => {
            println!(
                "Converted TaskShell: {:?} -> FlatBuffers: {:?} -> Back: {:?}",
                shell, fb_shell, converted
            );
            if shell == converted {
                println!("Result: Conversion matches original value.");
            } else {
                println!("Result: Conversion does not match original value!");
            }
        }
        Err(e) => {
            println!("Error: TaskShell conversion failed: {}", e);
        }
    }

    // Direct read from byte value
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
        0 => TaskShell::None,
        1 => TaskShell::Auto,
        #[cfg(windows)]
        2 => TaskShell::Cmd,
        #[cfg(windows)]
        3 => TaskShell::Powershell,
        #[cfg(unix)]
        2 => TaskShell::Bash,
        _ => {
            println!("Unknown shell value: {}", raw_value);
            return;
        }
    };
    println!(
        "Direct byte value: {} maps to shell: {:?}",
        raw_value, direct_shell
    );
    if shell == direct_shell {
        println!("Direct mapping matches original shell value.");
    } else {
        println!("Direct mapping does not match original shell value!");
    }
}

fn spec_conversions() {
    use tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TaskShell as FbTaskShell;
    println!("\n--- TaskSpec Conversion Example ---");

    let task_config = TaskConfig {
        command: "echo".to_string(),
        args: Some(vec!["hello".to_string(), "world".to_string()]),
        working_dir: Some("/tmp".to_string()),
        timeout_ms: Some(5000),
        enable_stdin: Some(true),
        ready_indicator: Some("ready".to_string()),
        ready_indicator_source: Some(tcrm_task::tasks::config::StreamSource::Stdout),
        ..Default::default()
    };

    let spec = TaskSpec {
        config: task_config,
        shell: Some(TaskShell::Auto),
        dependencies: Some(vec!["dep1".to_string()]),
        terminate_after_dependents_finished: Some(true),
        ..Default::default()
    };

    let mut fbb = flatbuffers::FlatBufferBuilder::new();
    let fb_spec_offset = spec.to_flatbuffers(&mut fbb);
    fbb.finish(fb_spec_offset, None);
    let fb_data = fbb.finished_data();

    match flatbuffers::root::<
        tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TaskSpec,
    >(fb_data)
    {
        Ok(fb_spec) => {
            match TaskSpec::try_from(fb_spec) {
                Ok(converted_spec) => {
                    println!(
                        "Converted TaskSpec roundtrip: original command='{}', shell={:?}, dependencies={:?}",
                        spec.config.command, spec.shell, spec.dependencies
                    );
                    println!(
                        "Restored command='{}', shell={:?}, dependencies={:?}",
                        converted_spec.config.command,
                        converted_spec.shell,
                        converted_spec.dependencies
                    );
                    if converted_spec.config.command == spec.config.command {
                        println!("Result: Command matches after roundtrip.");
                    } else {
                        println!("Result: Command does not match after roundtrip!");
                    }
                }
                Err(e) => {
                    println!("Error: TaskSpec from_flatbuffers failed: {}", e);
                }
            }
            // Direct read: access fields directly from fb_spec
            let config = fb_spec.config();
            let direct_command = config.command().to_string();
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
            println!(
                "Direct FlatBuffer read: command='{}', shell_val={}",
                direct_command, direct_shell_val
            );
        }
        Err(e) => {
            println!("Error: Failed to parse flatbuffer: {:?}", e);
        }
    }
}

fn tasks_conversions() {
    println!("\n--- TcrmTasks Conversion Example ---");

    let mut tasks = TcrmTasks::new();

    let task_config = TaskConfig {
        command: "echo".to_string(),
        args: Some(vec!["test".to_string()]),
        ..Default::default()
    };

    let spec = TaskSpec {
        config: task_config,
        shell: Some(TaskShell::Auto),
        ..Default::default()
    };

    tasks.insert("test_task".to_string(), spec);

    let mut fbb2 = flatbuffers::FlatBufferBuilder::new();
    let fb_tasks_offset =
        tcrm_monitor::flatbuffers::conversion::tcrm_tasks_to_flatbuffers(&tasks, &mut fbb2);
    fbb2.finish(fb_tasks_offset, None);
    let fb_data = fbb2.finished_data();

    match flatbuffers::root::<
        tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TcrmTasks,
    >(fb_data)
    {
        Ok(fb_tasks) => {
            let converted_tasks: Result<TcrmTasks, _> = fb_tasks.try_into();
            match converted_tasks {
                Ok(converted_tasks) => {
                    println!(
                        "Converted TcrmTasks roundtrip: original len={}, restored len={}",
                        tasks.len(),
                        converted_tasks.len()
                    );
                    if converted_tasks.contains_key("test_task") {
                        println!("Result: Restored tasks contain 'test_task'.");
                    } else {
                        println!("Result: Restored tasks do NOT contain 'test_task'!");
                    }
                }
                Err(e) => {
                    println!("Error: TcrmTasks from_flatbuffers failed: {}", e);
                }
            }
            // Direct read: access entries directly from fb_tasks
            if let Some(entries) = fb_tasks.tasks() {
                for i in 0..entries.len() {
                    let entry = entries.get(i);
                    let name = entry.name();
                    let spec = entry.spec();
                    let config = spec.config();
                    let command = config.command();
                    println!(
                        "Direct FlatBuffer read: entry '{}' command='{}'",
                        name, command
                    );
                }
            }
        }
        Err(e) => {
            println!("Error: Failed to parse TcrmTasks flatbuffer: {:?}", e);
        }
    }
}
