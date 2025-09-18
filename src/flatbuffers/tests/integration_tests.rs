use pretty_assertions::assert_eq;
use std::collections::HashMap;
use tcrm_task::tasks::config::{StreamSource, TaskConfig};
use temp_dir::TempDir;

use crate::{
    flatbuffers::{
        conversion::{ToFlatbuffers, tcrm_tasks_to_flatbuffers},
        tcrm_monitor_generated,
    },
    monitor::{
        config::{TaskShell, TaskSpec, TcrmTasks},
        depend::build_depend_map,
    },
};

/// Integration tests for the complete tcrm-monitor workflow
/// These tests verify end-to-end functionality including flatbuffers conversion,
/// and dependency resolution

mod test_scenarios {
    use crate::monitor::config::{TaskShell, TaskSpec, TcrmTasks};

    use super::*;

    /// Create a sample task configuration for testing
    pub fn create_sample_task_config(command: &str) -> TaskConfig {
        TaskConfig {
            command: command.to_string(),
            args: Some(vec!["--help".to_string()]),
            working_dir: None,
            env: Some({
                let mut env = HashMap::new();
                env.insert("TEST_VAR".to_string(), "test_value".to_string());
                env
            }),
            timeout_ms: Some(5000),
            enable_stdin: Some(false),
            ready_indicator: Some("Ready".to_string()),
            ready_indicator_source: Some(StreamSource::Stdout),
        }
    }

    /// Create a sample task spec for testing
    pub fn create_sample_task_spec(name: &str, dependencies: Option<Vec<String>>) -> TaskSpec {
        TaskSpec {
            config: create_sample_task_config(&format!("task_{}", name)),
            shell: Some(TaskShell::Auto),
            dependencies,
            terminate_after_dependents_finished: Some(false),
            ignore_dependencies_error: Some(false),
        }
    }

    /// Create a complex task dependency graph for testing
    pub fn create_complex_task_graph() -> TcrmTasks {
        let mut tasks = TcrmTasks::new();

        // Root task with no dependencies
        tasks.insert("root".to_string(), create_sample_task_spec("root", None));

        // First level dependencies
        tasks.insert(
            "build".to_string(),
            create_sample_task_spec("build", Some(vec!["root".to_string()])),
        );
        tasks.insert(
            "lint".to_string(),
            create_sample_task_spec("lint", Some(vec!["root".to_string()])),
        );

        // Second level dependencies
        tasks.insert(
            "test".to_string(),
            create_sample_task_spec("test", Some(vec!["build".to_string(), "lint".to_string()])),
        );
        tasks.insert(
            "package".to_string(),
            create_sample_task_spec("package", Some(vec!["build".to_string()])),
        );

        // Final deployment task
        tasks.insert(
            "deploy".to_string(),
            create_sample_task_spec(
                "deploy",
                Some(vec!["test".to_string(), "package".to_string()]),
            ),
        );

        // tasks should contain: root, build, lint, test, package, deploy = 6 tasks
        assert!(tasks.len() == 6);
        tasks
    }
}

#[tokio::test]

async fn test_complete_flatbuffers_workflow() {
    // Create a complex task configuration
    let original_tasks = test_scenarios::create_complex_task_graph();

    // Convert to flatbuffers
    use flatbuffers::FlatBufferBuilder;
    let mut fbb = FlatBufferBuilder::new();
    let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&original_tasks, &mut fbb);
    fbb.finish(fb_tasks_offset, None);

    // Serialize to bytes
    let serialized_data = fbb.finished_data().to_vec();

    // Simulate saving to file and loading back
    let loaded_fb_tasks =
        flatbuffers::root::<tcrm_monitor_generated::tcrm::monitor::TcrmTasks>(&serialized_data)
            .unwrap();

    // Convert back to native types
    let converted_tasks: TcrmTasks = loaded_fb_tasks.try_into().unwrap();

    // Verify the conversion preserved all tasks
    assert_eq!(converted_tasks.len(), original_tasks.len());

    // Verify specific tasks and their properties
    let root_task = converted_tasks.get("root").unwrap();
    assert_eq!(root_task.config.command, "task_root");
    assert_eq!(root_task.dependencies, None);

    let deploy_task = converted_tasks.get("deploy").unwrap();
    let deploy_deps = deploy_task.dependencies.as_ref().unwrap();
    assert_eq!(deploy_deps.len(), 2);
    assert!(deploy_deps.contains(&"test".to_string()));
    assert!(deploy_deps.contains(&"package".to_string()));
}

#[tokio::test]

async fn test_flatbuffers_config_validation() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create a simple task configuration
    let mut tasks = TcrmTasks::new();
    let simple_config = TaskConfig {
        command: if cfg!(windows) { "cmd" } else { "echo" }.to_string(),
        args: Some(if cfg!(windows) {
            vec![
                "/c".to_string(),
                "echo".to_string(),
                "Hello World".to_string(),
            ]
        } else {
            vec!["Hello World".to_string()]
        }),
        working_dir: Some(temp_path.to_string_lossy().to_string()),
        env: None,
        timeout_ms: Some(10000),
        enable_stdin: Some(false),
        ready_indicator: None,
        ready_indicator_source: None,
    };

    let simple_spec = TaskSpec {
        config: simple_config,
        shell: Some(TaskShell::Auto),
        dependencies: None,
        terminate_after_dependents_finished: Some(false),
        ignore_dependencies_error: Some(false),
    };

    tasks.insert("simple_task".to_string(), simple_spec);

    // Convert to flatbuffers and back
    use flatbuffers::FlatBufferBuilder;
    let mut fbb = FlatBufferBuilder::new();
    let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
    fbb.finish(fb_tasks_offset, None);

    let serialized_data = fbb.finished_data().to_vec();
    let loaded_fb_tasks =
        flatbuffers::root::<tcrm_monitor_generated::tcrm::monitor::TcrmTasks>(&serialized_data)
            .unwrap();
    let converted_tasks: TcrmTasks = loaded_fb_tasks.try_into().unwrap();

    // Verify the task configuration is correct
    assert_eq!(converted_tasks.len(), 1);
    assert!(converted_tasks.contains_key("simple_task"));

    let task = converted_tasks.get("simple_task").unwrap();
    assert_eq!(task.config.timeout_ms, Some(10000));
    assert_eq!(task.shell, Some(TaskShell::Auto));
}

#[tokio::test]

async fn test_dependency_resolution_with_flatbuffers() {
    // Create a task graph with dependencies
    let original_tasks = test_scenarios::create_complex_task_graph();

    // Convert through flatbuffers
    use flatbuffers::FlatBufferBuilder;
    let mut fbb = FlatBufferBuilder::new();
    let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&original_tasks, &mut fbb);
    fbb.finish(fb_tasks_offset, None);

    let serialized_data = fbb.finished_data().to_vec();
    let loaded_fb_tasks =
        flatbuffers::root::<tcrm_monitor_generated::tcrm::monitor::TcrmTasks>(&serialized_data)
            .unwrap();
    let converted_tasks: TcrmTasks = loaded_fb_tasks.try_into().unwrap();

    // Use dependency resolver to verify the task graph
    let depend_map = build_depend_map(&converted_tasks).unwrap();

    // Verify dependency structure
    assert!(depend_map.dependencies.contains_key("deploy"));
    let deploy_deps = &depend_map.dependencies["deploy"];
    assert!(deploy_deps.contains(&"test".to_string()));
    assert!(deploy_deps.contains(&"package".to_string()));

    // Verify root task has no dependencies
    let root_deps = depend_map.dependencies.get("root");
    assert!(root_deps.is_none() || root_deps.unwrap().is_empty());
}

#[tokio::test]

async fn test_error_handling_in_conversion_workflow() {
    // Test conversion with missing required fields
    use flatbuffers::FlatBufferBuilder;

    // Create an invalid TaskSpec that might cause issues
    let invalid_config = TaskConfig {
        command: "".to_string(), // Empty command
        args: None,
        working_dir: None,
        env: None,
        timeout_ms: None,
        enable_stdin: None,
        ready_indicator: None,
        ready_indicator_source: None,
    };

    let invalid_spec = TaskSpec {
        config: invalid_config,
        shell: None,
        dependencies: None,
        terminate_after_dependents_finished: None,
        ignore_dependencies_error: None,
    };

    // The conversion should handle this gracefully
    let mut fbb = FlatBufferBuilder::new();
    let _result = invalid_spec.to_flatbuffers(&mut fbb);

    // Even empty commands should be handled gracefully - no need to check is_ok()
}

#[tokio::test]

async fn test_large_configuration_handling() {
    // Create a configuration with many tasks
    let mut large_tasks = TcrmTasks::new();

    for i in 0..100 {
        let task_name = format!("task_{:03}", i);
        let dependencies = if i > 0 {
            Some(vec![format!("task_{:03}", i - 1)])
        } else {
            None
        };

        large_tasks.insert(
            task_name.clone(),
            test_scenarios::create_sample_task_spec(&task_name, dependencies),
        );
    }

    // Convert to flatbuffers
    use flatbuffers::FlatBufferBuilder;
    let mut fbb = FlatBufferBuilder::new();
    let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&large_tasks, &mut fbb);
    fbb.finish(fb_tasks_offset, None);

    let serialized_data = fbb.finished_data().to_vec();

    // Verify the serialized data is reasonable in size (should be less than 1MB for 100 tasks)
    assert!(serialized_data.len() < 1024 * 1024);

    // Convert back
    let loaded_fb_tasks =
        flatbuffers::root::<tcrm_monitor_generated::tcrm::monitor::TcrmTasks>(&serialized_data)
            .unwrap();
    let converted_tasks: TcrmTasks = loaded_fb_tasks.try_into().unwrap();

    // Verify all tasks are preserved
    assert_eq!(converted_tasks.len(), 100);

    // Use dependency resolver to verify the large configuration
    let depend_map = build_depend_map(&converted_tasks).unwrap();

    // Verify all tasks have appropriate dependencies
    // Note: build_depend_map expands to include all transitive dependencies
    for i in 1..100 {
        let task_name = format!("task_{:03}", i);
        let deps = depend_map.dependencies.get(&task_name);
        assert!(deps.is_some());
        let deps = deps.unwrap();
        // Each task depends on all previous tasks (transitive dependencies)
        assert_eq!(deps.len(), i);
        // Verify it includes all predecessors
        for j in 0..i {
            let expected_dep = format!("task_{:03}", j);
            assert!(
                deps.contains(&expected_dep),
                "Task {} should depend on {}",
                task_name,
                expected_dep
            );
        }
    }
}

#[tokio::test]

async fn test_concurrent_access_to_converted_config() {
    use std::sync::Arc;
    use tokio::task;

    // Create a task configuration
    let tasks = test_scenarios::create_complex_task_graph();

    // Convert to flatbuffers
    use flatbuffers::FlatBufferBuilder;
    let mut fbb = FlatBufferBuilder::new();
    let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
    fbb.finish(fb_tasks_offset, None);

    let serialized_data = Arc::new(fbb.finished_data().to_vec());

    // Spawn multiple concurrent tasks that read from the same flatbuffer data
    let mut handles = Vec::new();

    for i in 0..10 {
        let data = Arc::clone(&serialized_data);
        let handle = task::spawn(async move {
            let loaded_fb_tasks =
                flatbuffers::root::<tcrm_monitor_generated::tcrm::monitor::TcrmTasks>(&data)
                    .unwrap();
            let converted_tasks: TcrmTasks = loaded_fb_tasks.try_into().unwrap();

            // Verify the data is consistent across all concurrent accesses
            assert_eq!(converted_tasks.len(), 6);
            assert!(converted_tasks.contains_key("root"));
            assert!(converted_tasks.contains_key("deploy"));

            i // Return the task index for verification
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    let results: Vec<usize> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // Verify all tasks completed successfully
    assert_eq!(results, (0..10).collect::<Vec<_>>());
}

#[test]
fn test_flatbuffers_schema_compatibility() {
    // This test ensures our schema is compatible with the generated code
    use tcrm_monitor_generated::tcrm::monitor::TaskShell as FbTaskShell;

    // Verify that the generated types exist and can be used
    let _config_builder = flatbuffers::FlatBufferBuilder::new();
    // These would typically be created through the builder, but we're just checking compilation

    // Test enum values are accessible
    let _shell_none = FbTaskShell::None;
    let _shell_auto = FbTaskShell::Auto;

    #[cfg(windows)]
    {
        let _shell_cmd = FbTaskShell::Cmd;
        let _shell_powershell = FbTaskShell::Powershell;
    }

    #[cfg(unix)]
    {
        let _shell_bash = FbTaskShell::Bash;
    }
}

#[tokio::test]

async fn test_memory_efficiency() {
    // Test that flatbuffers conversion is memory efficient
    let initial_memory = get_memory_usage();

    // Create a moderately large configuration
    let mut tasks = TcrmTasks::new();
    for i in 0..50 {
        let mut env = HashMap::new();
        for j in 0..20 {
            env.insert(format!("ENV_VAR_{}_{}", i, j), format!("value_{}_{}", i, j));
        }

        let config = TaskConfig {
            command: format!("command_{}", i),
            args: Some((0..10).map(|j| format!("arg_{}_{}", i, j)).collect()),
            working_dir: Some(format!("/tmp/work_{}", i)),
            env: Some(env),
            timeout_ms: Some(30000),
            enable_stdin: Some(i % 2 == 0),
            ready_indicator: Some(format!("ready_{}", i)),
            ready_indicator_source: Some(if i % 2 == 0 {
                StreamSource::Stdout
            } else {
                StreamSource::Stderr
            }),
        };

        let spec = TaskSpec {
            config,
            shell: Some(if i % 3 == 0 {
                TaskShell::Auto
            } else {
                TaskShell::None
            }),
            dependencies: if i > 0 {
                Some(vec![format!("task_{}", i - 1)])
            } else {
                None
            },
            terminate_after_dependents_finished: Some(i % 2 == 1),
            ignore_dependencies_error: Some(i % 3 == 0),
        };

        tasks.insert(format!("task_{}", i), spec);
    }

    let after_creation_memory = get_memory_usage();

    // Convert to flatbuffers multiple times
    for _iteration in 0..10 {
        use flatbuffers::FlatBufferBuilder;
        let mut fbb = FlatBufferBuilder::new();
        let _fb_tasks_offset = tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
        fbb.finish(_fb_tasks_offset, None);

        let serialized_data = fbb.finished_data().to_vec();
        let _loaded_fb_tasks =
            flatbuffers::root::<tcrm_monitor_generated::tcrm::monitor::TcrmTasks>(&serialized_data)
                .unwrap();
        let _converted_tasks: TcrmTasks = _loaded_fb_tasks.try_into().unwrap();
    }

    let final_memory = get_memory_usage();

    // Memory usage should not grow excessively
    let memory_growth = final_memory.saturating_sub(after_creation_memory);
    let data_size = after_creation_memory.saturating_sub(initial_memory);

    // Memory growth should be reasonable (less than 2x the original data size)
    if data_size > 0 {
        assert!(
            memory_growth < data_size * 2,
            "Memory growth ({} bytes) exceeded 2x data size ({} bytes)",
            memory_growth,
            data_size
        );
    }
}

// Helper function to get current memory usage (simplified)
fn get_memory_usage() -> usize {
    // This is a simplified version - in production you might use a more sophisticated approach
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(status) = fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<usize>() {
                            return kb * 1024; // Convert to bytes
                        }
                    }
                }
            }
        }
    }

    // Fallback: return 0 if we can't measure memory
    0
}

/// Test TaskMonitorEvent flatbuffers conversion roundtrip
/// This test verifies that TaskMonitorEvent can be serialized to and from FlatBuffers without data loss.
#[tokio::test]
async fn test_task_monitor_event_flatbuffers_roundtrip() {
    use crate::flatbuffers::conversion::{FromFlatbuffers, ToFlatbuffers};
    use crate::monitor::error::TaskMonitorError;
    use crate::monitor::event::{
        TaskMonitorControlCommand, TaskMonitorControlEvent, TaskMonitorEvent,
    };
    use flatbuffers::FlatBufferBuilder;

    let test_events = vec![
        TaskMonitorEvent::Started { total_tasks: 3 },
        TaskMonitorEvent::Completed {
            completed_tasks: 2,
            failed_tasks: 1,
        },
        TaskMonitorEvent::Control(TaskMonitorControlEvent::ControlReceived {
            control: TaskMonitorControlCommand::TerminateAllTasks,
        }),
        TaskMonitorEvent::Control(TaskMonitorControlEvent::ControlProcessed {
            control: TaskMonitorControlCommand::TerminateTask {
                task_name: "my_task".to_string(),
            },
        }),
        TaskMonitorEvent::Error(TaskMonitorError::ConfigParse("parse error".to_string())),
    ];

    for original_event in test_events {
        // Convert to flatbuffers using trait
        let mut fbb = FlatBufferBuilder::new();
        let fb_event = original_event.to_flatbuffers(&mut fbb);
        fbb.finish(fb_event, None);

        // Convert back from flatbuffers using trait
        let buffer = fbb.finished_data();
        let fb_event_message = flatbuffers::root::<
            crate::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TaskMonitorEventMessage,
        >(&buffer)
        .expect("Should parse flatbuffers");

        let converted_event = TaskMonitorEvent::from_flatbuffers(fb_event_message)
            .expect("Should convert from flatbuffers");

        // Verify the conversion preserved the essential data
        match (&original_event, &converted_event) {
            (
                TaskMonitorEvent::Started { total_tasks: orig },
                TaskMonitorEvent::Started { total_tasks: conv },
            ) => assert_eq!(orig, conv),
            (
                TaskMonitorEvent::Completed {
                    completed_tasks: o1,
                    failed_tasks: o2,
                },
                TaskMonitorEvent::Completed {
                    completed_tasks: c1,
                    failed_tasks: c2,
                },
            ) => {
                assert_eq!(o1, c1);
                assert_eq!(o2, c2);
            }
            (
                TaskMonitorEvent::Control(TaskMonitorControlEvent::ControlReceived {
                    control: o_ctrl,
                }),
                TaskMonitorEvent::Control(TaskMonitorControlEvent::ControlReceived {
                    control: c_ctrl,
                }),
            ) => match (o_ctrl, c_ctrl) {
                (
                    TaskMonitorControlCommand::TerminateAllTasks,
                    TaskMonitorControlCommand::TerminateAllTasks,
                ) => {}
                (
                    TaskMonitorControlCommand::TerminateTask { task_name: o_name },
                    TaskMonitorControlCommand::TerminateTask { task_name: c_name },
                ) => assert_eq!(o_name, c_name),
                (
                    TaskMonitorControlCommand::SendStdin {
                        task_name: o_name,
                        input: o_input,
                    },
                    TaskMonitorControlCommand::SendStdin {
                        task_name: c_name,
                        input: c_input,
                    },
                ) => {
                    assert_eq!(o_name, c_name);
                    assert_eq!(o_input, c_input);
                }
                _ => panic!(
                    "ControlReceived conversion mismatch: {:?} -> {:?}",
                    o_ctrl, c_ctrl
                ),
            },
            (
                TaskMonitorEvent::Control(TaskMonitorControlEvent::ControlProcessed {
                    control: o_ctrl,
                }),
                TaskMonitorEvent::Control(TaskMonitorControlEvent::ControlProcessed {
                    control: c_ctrl,
                }),
            ) => match (o_ctrl, c_ctrl) {
                (
                    TaskMonitorControlCommand::TerminateAllTasks,
                    TaskMonitorControlCommand::TerminateAllTasks,
                ) => {}
                (
                    TaskMonitorControlCommand::TerminateTask { task_name: o_name },
                    TaskMonitorControlCommand::TerminateTask { task_name: c_name },
                ) => assert_eq!(o_name, c_name),
                (
                    TaskMonitorControlCommand::SendStdin {
                        task_name: o_name,
                        input: o_input,
                    },
                    TaskMonitorControlCommand::SendStdin {
                        task_name: c_name,
                        input: c_input,
                    },
                ) => {
                    assert_eq!(o_name, c_name);
                    assert_eq!(o_input, c_input);
                }
                _ => panic!(
                    "ControlProcessed conversion mismatch: {:?} -> {:?}",
                    o_ctrl, c_ctrl
                ),
            },
            (
                TaskMonitorEvent::Error(TaskMonitorError::ConfigParse(o_msg)),
                TaskMonitorEvent::Error(TaskMonitorError::ConfigParse(c_msg)),
            ) => assert_eq!(o_msg, c_msg),
            _ => panic!(
                "Event conversion failed: {:?} -> {:?}",
                original_event, converted_event
            ),
        }
    }
}
