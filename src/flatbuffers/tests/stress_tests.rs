#[cfg(test)]
use pretty_assertions::assert_eq;
use rstest::*;
use std::collections::HashMap;

use tcrm_task::tasks::config::{StreamSource, TaskConfig};

use crate::{
    flatbuffers::{conversion::tcrm_tasks_to_flatbuffers, tcrm_monitor_generated},
    monitor::config::{TaskShell, TaskSpec, TcrmTasks},
};

/// Stress tests for tcrm-monitor flatbuffers conversion
/// These tests push the limits of the conversion system

#[rstest]
#[case::small(10)]
#[case::medium(100)]
#[case::large(1000)]
fn test_conversion_scalability(#[case] task_count: usize) {
    let mut tasks = TcrmTasks::new();

    // Create a large number of tasks with complex configurations
    for i in 0..task_count {
        let mut env = HashMap::new();
        for j in 0..50 {
            env.insert(format!("VAR_{}_{}", i, j), format!("value_{}_{}", i, j));
        }

        let config = TaskConfig {
            command: format!("command_{}", i),
            args: Some((0..20).map(|j| format!("arg_{}_{}", i, j)).collect()),
            working_dir: Some(format!("/tmp/workspace_{}", i)),
            env: Some(env),
            timeout_ms: Some(30000 + i as u64),
            enable_stdin: Some(i % 2 == 0),
            ready_indicator: Some(format!("ready_indicator_{}", i)),
            ready_indicator_source: Some(if i % 2 == 0 {
                StreamSource::Stdout
            } else {
                StreamSource::Stderr
            }),
        };

        let spec = TaskSpec {
            config,
            shell: Some(match i % 4 {
                0 => TaskShell::Auto,
                1 => TaskShell::None,
                #[cfg(windows)]
                2 => TaskShell::Cmd,
                #[cfg(windows)]
                3 => TaskShell::Powershell,
                #[cfg(unix)]
                2 => TaskShell::Bash,
                #[cfg(unix)]
                3 => TaskShell::Auto, // Fallback for unix
                _ => TaskShell::Auto,
            }),
            dependencies: if i > 0 {
                Some(vec![format!("task_{}", i - 1)])
            } else {
                None
            },
            terminate_after_dependents_finished: Some(i % 3 == 0),
            ignore_dependencies_error: Some(i % 5 == 0),
        };

        tasks.insert(format!("task_{}", i), spec);
    }

    // Convert to flatbuffers
    use flatbuffers::FlatBufferBuilder;
    let mut fbb = FlatBufferBuilder::new();
    let start_time = std::time::Instant::now();
    let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
    fbb.finish(fb_tasks_offset, None);
    let conversion_time = start_time.elapsed();

    let serialized_data = fbb.finished_data().to_vec();

    // Verify serialization size is reasonable (less than 1MB per 100 tasks)
    let size_limit = (task_count / 100 + 1) * 1024 * 1024;
    assert!(
        serialized_data.len() < size_limit,
        "Serialized size {} exceeds limit {} for {} tasks",
        serialized_data.len(),
        size_limit,
        task_count
    );

    // Convert back
    let parse_start = std::time::Instant::now();
    let loaded_fb_tasks =
        flatbuffers::root::<tcrm_monitor_generated::tcrm::monitor::TcrmTasks>(&serialized_data)
            .unwrap();
    let converted_tasks: TcrmTasks = loaded_fb_tasks.try_into().unwrap();
    let parse_time = parse_start.elapsed();

    // Verify all tasks are preserved
    assert_eq!(converted_tasks.len(), task_count);

    // Performance assertions (conversion should be fast)
    assert!(
        conversion_time.as_millis() < task_count as u128 * 10,
        "Conversion took too long: {}ms for {} tasks",
        conversion_time.as_millis(),
        task_count
    );
    assert!(
        parse_time.as_millis() < task_count as u128 * 5,
        "Parsing took too long: {}ms for {} tasks",
        parse_time.as_millis(),
        task_count
    );

    // Verify a sample of tasks for correctness
    let sample_indices = if task_count > 10 {
        vec![
            0,
            task_count / 4,
            task_count / 2,
            task_count * 3 / 4,
            task_count - 1,
        ]
    } else {
        (0..task_count).collect()
    };

    for &i in &sample_indices {
        let task_name = format!("task_{}", i);
        let original_task = tasks.get(&task_name).unwrap();
        let converted_task = converted_tasks.get(&task_name).unwrap();

        assert_eq!(converted_task.config.command, original_task.config.command);
        assert_eq!(
            converted_task.config.args.as_ref().unwrap().len(),
            original_task.config.args.as_ref().unwrap().len()
        );
        assert_eq!(
            converted_task.config.env.as_ref().unwrap().len(),
            original_task.config.env.as_ref().unwrap().len()
        );
    }
}

#[rstest]
#[case::ascii("hello_world")]
#[case::unicode("测试_🚀_مرحبا")]
#[case::special_chars("task-with.special@chars#")]
#[case::long_name(&"a".repeat(100))]
fn test_task_name_handling(#[case] task_name: &str) {
    let mut tasks = TcrmTasks::new();

    let config = TaskConfig {
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
        config,
        shell: Some(TaskShell::Auto),
        dependencies: None,
        terminate_after_dependents_finished: None,
        ignore_dependencies_error: None,
    };

    tasks.insert(task_name.to_string(), spec);

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

    assert_eq!(converted_tasks.len(), 1);
    assert!(converted_tasks.contains_key(task_name));
}

#[test]
fn test_deeply_nested_dependencies() {
    let mut tasks = TcrmTasks::new();
    let depth = 50;

    // Create a chain of dependencies
    for i in 0..depth {
        let task_name = format!("task_{}", i);
        let dependencies = if i > 0 {
            Some(vec![format!("task_{}", i - 1)])
        } else {
            None
        };

        let config = TaskConfig {
            command: format!("step_{}", i),
            args: None,
            working_dir: None,
            env: None,
            timeout_ms: Some((i + 1) as u64 * 1000),
            enable_stdin: None,
            ready_indicator: None,
            ready_indicator_source: None,
        };

        let spec = TaskSpec {
            config,
            shell: Some(TaskShell::Auto),
            dependencies,
            terminate_after_dependents_finished: Some(false),
            ignore_dependencies_error: Some(false),
        };

        tasks.insert(task_name, spec);
    }

    // Convert to flatbuffers
    use flatbuffers::FlatBufferBuilder;
    let mut fbb = FlatBufferBuilder::new();
    let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
    fbb.finish(fb_tasks_offset, None);

    let serialized_data = fbb.finished_data().to_vec();
    let loaded_fb_tasks =
        flatbuffers::root::<tcrm_monitor_generated::tcrm::monitor::TcrmTasks>(&serialized_data)
            .unwrap();
    let converted_tasks: TcrmTasks = loaded_fb_tasks.try_into().unwrap();

    // Verify all tasks and dependencies are preserved
    assert_eq!(converted_tasks.len(), depth);

    for i in 0..depth {
        let task_name = format!("task_{}", i);
        let task = converted_tasks.get(&task_name).unwrap();

        if i > 0 {
            let deps = task.dependencies.as_ref().unwrap();
            assert_eq!(deps.len(), 1);
            assert_eq!(deps[0], format!("task_{}", i - 1));
        } else {
            assert!(task.dependencies.is_none() || task.dependencies.as_ref().unwrap().is_empty());
        }
    }
}

#[test]
fn test_complex_dependency_graph() {
    let mut tasks = TcrmTasks::new();

    // Create a complex dependency graph (diamond pattern)
    //     root
    //    /    \
    //   A      B
    //   |      |
    //   C      D
    //    \    /
    //     end

    let configs = [
        ("root", vec![]),
        ("A", vec!["root"]),
        ("B", vec!["root"]),
        ("C", vec!["A"]),
        ("D", vec!["B"]),
        ("end", vec!["C", "D"]),
    ];

    for (name, deps) in configs {
        let config = TaskConfig {
            command: format!("cmd_{}", name),
            args: Some(vec![name.to_string()]),
            working_dir: None,
            env: None,
            timeout_ms: Some(5000),
            enable_stdin: None,
            ready_indicator: None,
            ready_indicator_source: None,
        };

        let spec = TaskSpec {
            config,
            shell: Some(TaskShell::Auto),
            dependencies: if deps.is_empty() {
                None
            } else {
                Some(deps.into_iter().map(String::from).collect())
            },
            terminate_after_dependents_finished: Some(false),
            ignore_dependencies_error: Some(false),
        };

        tasks.insert(name.to_string(), spec);
    }

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

    // Verify the dependency structure is preserved
    assert_eq!(converted_tasks.len(), 6);

    let end_task = converted_tasks.get("end").unwrap();
    let end_deps = end_task.dependencies.as_ref().unwrap();
    assert_eq!(end_deps.len(), 2);
    assert!(end_deps.contains(&"C".to_string()));
    assert!(end_deps.contains(&"D".to_string()));

    let root_task = converted_tasks.get("root").unwrap();
    assert!(
        root_task.dependencies.is_none() || root_task.dependencies.as_ref().unwrap().is_empty()
    );
}

#[test]
fn test_maximum_environment_variables() {
    let mut tasks = TcrmTasks::new();

    // Create a task with maximum reasonable environment variables
    let mut env = HashMap::new();
    for i in 0..1000 {
        env.insert(
            format!("LARGE_ENV_VAR_WITH_LONG_NAME_{:04}", i),
            format!("This is a very long environment variable value that contains lots of text and data for variable number {:04}. It should test the limits of what can be stored in a flatbuffer while remaining reasonable for real-world usage.", i)
        );
    }

    let config = TaskConfig {
        command: "env".to_string(),
        args: Some(vec!["--help".to_string()]),
        working_dir: Some(
            "/very/long/path/to/working/directory/that/tests/path/length/limits".to_string(),
        ),
        env: Some(env),
        timeout_ms: Some(u64::MAX), // Test maximum timeout
        enable_stdin: Some(true),
        ready_indicator: Some(
            "Ready with very long indicator text that might appear in logs".to_string(),
        ),
        ready_indicator_source: Some(StreamSource::Stderr),
    };

    let spec = TaskSpec {
        config,
        shell: Some(TaskShell::Auto),
        dependencies: None,
        terminate_after_dependents_finished: Some(true),
        ignore_dependencies_error: Some(false),
    };

    tasks.insert("max_env_task".to_string(), spec);

    // Convert to flatbuffers
    use flatbuffers::FlatBufferBuilder;
    let mut fbb = FlatBufferBuilder::new();
    let conversion_start = std::time::Instant::now();
    let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
    fbb.finish(fb_tasks_offset, None);
    let conversion_time = conversion_start.elapsed();

    let serialized_data = fbb.finished_data().to_vec();

    // Verify serialization completed in reasonable time
    assert!(
        conversion_time.as_millis() < 1000,
        "Conversion took too long: {}ms",
        conversion_time.as_millis()
    );

    // Parse back
    let parse_start = std::time::Instant::now();
    let loaded_fb_tasks =
        flatbuffers::root::<tcrm_monitor_generated::tcrm::monitor::TcrmTasks>(&serialized_data)
            .unwrap();
    let converted_tasks: TcrmTasks = loaded_fb_tasks.try_into().unwrap();
    let parse_time = parse_start.elapsed();

    assert!(
        parse_time.as_millis() < 1000,
        "Parsing took too long: {}ms",
        parse_time.as_millis()
    );

    // Verify all environment variables are preserved
    let task = converted_tasks.get("max_env_task").unwrap();
    let converted_env = task.config.env.as_ref().unwrap();
    assert_eq!(converted_env.len(), 1000);

    // Check a few specific environment variables
    assert!(converted_env.contains_key("LARGE_ENV_VAR_WITH_LONG_NAME_0000"));
    assert!(converted_env.contains_key("LARGE_ENV_VAR_WITH_LONG_NAME_0999"));

    // Verify timeout value is preserved correctly
    assert_eq!(task.config.timeout_ms, Some(u64::MAX));
}

#[test]
fn test_concurrent_conversions() {
    use std::sync::Arc;
    use std::thread;

    let original_tasks = Arc::new({
        let mut tasks = TcrmTasks::new();
        for i in 0..20 {
            let config = TaskConfig {
                command: format!("task_{}", i),
                args: Some(vec![format!("arg_{}", i)]),
                working_dir: None,
                env: None,
                timeout_ms: Some(5000),
                enable_stdin: None,
                ready_indicator: None,
                ready_indicator_source: None,
            };

            let spec = TaskSpec {
                config,
                shell: Some(TaskShell::Auto),
                dependencies: None,
                terminate_after_dependents_finished: None,
                ignore_dependencies_error: None,
            };

            tasks.insert(format!("task_{}", i), spec);
        }
        tasks
    });

    // Spawn multiple threads doing conversions simultaneously
    let handles: Vec<_> = (0..10)
        .map(|thread_id| {
            let tasks = Arc::clone(&original_tasks);
            thread::spawn(move || {
                use flatbuffers::FlatBufferBuilder;

                for _iteration in 0..5 {
                    let mut fbb = FlatBufferBuilder::new();
                    let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
                    fbb.finish(fb_tasks_offset, None);

                    let serialized_data = fbb.finished_data().to_vec();
                    let loaded_fb_tasks = flatbuffers::root::<
                        tcrm_monitor_generated::tcrm::monitor::TcrmTasks,
                    >(&serialized_data)
                    .unwrap();
                    let converted_tasks: TcrmTasks = loaded_fb_tasks.try_into().unwrap();

                    assert_eq!(converted_tasks.len(), 20);
                    assert!(converted_tasks.contains_key("task_0"));
                    assert!(converted_tasks.contains_key("task_19"));
                }

                thread_id // Return thread ID for verification
            })
        })
        .collect();

    // Wait for all threads to complete
    let results: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results, (0..10).collect::<Vec<_>>());
}

#[test]
fn test_error_resilience() {
    // Test that the conversion handles edge cases gracefully
    let mut tasks = TcrmTasks::new();

    // Task with minimal configuration
    let minimal_config = TaskConfig {
        command: "".to_string(), // Empty command
        args: None,
        working_dir: None,
        env: None,
        timeout_ms: None,
        enable_stdin: None,
        ready_indicator: None,
        ready_indicator_source: None,
    };

    let minimal_spec = TaskSpec {
        config: minimal_config,
        shell: None,
        dependencies: None,
        terminate_after_dependents_finished: None,
        ignore_dependencies_error: None,
    };

    tasks.insert("minimal".to_string(), minimal_spec);

    // Task with empty collections
    let empty_collections_config = TaskConfig {
        command: "test".to_string(),
        args: Some(vec![]), // Empty args
        working_dir: None,
        env: Some(HashMap::new()), // Empty env
        timeout_ms: Some(0),       // Zero timeout
        enable_stdin: None,
        ready_indicator: Some("".to_string()), // Empty indicator
        ready_indicator_source: None,
    };

    let empty_collections_spec = TaskSpec {
        config: empty_collections_config,
        shell: Some(TaskShell::Auto),
        dependencies: Some(vec![]), // Empty dependencies
        terminate_after_dependents_finished: None,
        ignore_dependencies_error: None,
    };

    tasks.insert("empty_collections".to_string(), empty_collections_spec);

    // Convert and verify it handles edge cases gracefully
    use flatbuffers::FlatBufferBuilder;
    let mut fbb = FlatBufferBuilder::new();
    let fb_tasks_offset = tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
    fbb.finish(fb_tasks_offset, None);

    let serialized_data = fbb.finished_data().to_vec();
    let loaded_fb_tasks =
        flatbuffers::root::<tcrm_monitor_generated::tcrm::monitor::TcrmTasks>(&serialized_data)
            .unwrap();
    let converted_tasks: TcrmTasks = loaded_fb_tasks.try_into().unwrap();

    assert_eq!(converted_tasks.len(), 2);

    let minimal_task = converted_tasks.get("minimal").unwrap();
    assert_eq!(minimal_task.config.command, "");

    let empty_task = converted_tasks.get("empty_collections").unwrap();
    assert_eq!(empty_task.config.args.as_ref().unwrap().len(), 0);
    assert_eq!(empty_task.config.env.as_ref().unwrap().len(), 0);
    assert_eq!(empty_task.dependencies.as_ref().unwrap().len(), 0);
}
