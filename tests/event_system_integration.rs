use std::collections::HashMap;
use tcrm_monitor::monitor::config::{TaskShell, TaskSpec};
use tcrm_monitor::monitor::error::SendStdinErrorReason;
use tcrm_monitor::monitor::event::{TaskMonitorControlType, TaskMonitorEvent};
use tcrm_monitor::monitor::executor::direct::TaskMonitorControl;
use tcrm_monitor::monitor::tasks::TaskMonitor;
use tcrm_task::tasks::config::TaskConfig;
use tokio::sync::mpsc;

/// Test the TaskMonitorEvent system
#[tokio::test]
async fn test_task_monitor_event_system() {
    // Create a simple task configuration
    let mut tasks = HashMap::new();

    let echo_task = TaskSpec {
        config: TaskConfig {
            command: if cfg!(windows) {
                "powershell".to_string()
            } else {
                "echo".to_string()
            },
            args: if cfg!(windows) {
                Some(vec![
                    "-Command".to_string(),
                    "echo 'Task started'; Start-Sleep 1; echo 'Task completed'".to_string(),
                ])
            } else {
                Some(vec!["Task output".to_string()])
            },
            working_dir: None,
            env: None,
            timeout_ms: Some(10000),
            enable_stdin: Some(true),
            ready_indicator: None,
            ready_indicator_source: None,
        },
        shell: Some(TaskShell::Auto),
        dependencies: Some(vec![]),
        terminate_after_dependents_finished: Some(false),
        ignore_dependencies_error: Some(false),
    };

    tasks.insert("echo_task".to_string(), echo_task);

    // Create TaskMonitor
    let mut task_monitor = TaskMonitor::new(tasks).unwrap();

    // Create channels for events and control
    let (event_tx, mut event_rx) = mpsc::channel::<TaskMonitorEvent>(100);
    let (control_tx, control_rx) = mpsc::channel::<TaskMonitorControl>(10);

    // Start the monitor with event tracking
    let monitor_handle = tokio::spawn(async move {
        task_monitor
            .execute_all_direct_with_control(Some(event_tx), control_rx)
            .await;
    });

    // Collect events
    let mut received_events = Vec::new();
    let mut execution_started = false;
    let mut execution_completed = false;
    let mut stdin_events = 0;
    let mut control_events = 0;

    // Monitor events for a few seconds
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(3));
    tokio::pin!(timeout);

    // Send some control messages to test the event system
    tokio::spawn(async move {
        // Wait a bit for execution to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Send stdin to the task
        let _ = control_tx
            .send(TaskMonitorControl::SendStdin {
                task_name: "echo_task".to_string(),
                input: "test input\n".to_string(),
            })
            .await;

        // Wait a bit more
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Test sending stdin to non-existent task (should generate error event)
        let _ = control_tx
            .send(TaskMonitorControl::SendStdin {
                task_name: "non_existent_task".to_string(),
                input: "test input\n".to_string(),
            })
            .await;
    });

    loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Some(event) => {
                        println!("Received event: {:?}", event);

                        match &event {
                            TaskMonitorEvent::ExecutionStarted { total_tasks } => {
                                assert_eq!(*total_tasks, 1);
                                execution_started = true;
                            }
                            TaskMonitorEvent::ExecutionCompleted { completed_tasks, failed_tasks } => {
                                execution_completed = true;
                                assert_eq!(*completed_tasks, 1);
                                assert_eq!(*failed_tasks, 0);
                            }
                            TaskMonitorEvent::StdinSent { task_name, input_length } => {
                                assert_eq!(task_name, "echo_task");
                                assert_eq!(*input_length, 11); // "test input\n"
                                stdin_events += 1;
                            }
                            TaskMonitorEvent::StdinError { task_name, error } => {
                                assert_eq!(task_name, "non_existent_task");
                                // We expect some kind of stdin error for the non-existent task
                                match error {
                                    SendStdinErrorReason::TaskNotFound(_) |
                                    SendStdinErrorReason::StdinNotEnabled(_) |
                                    SendStdinErrorReason::TaskNotReady(_) |
                                    SendStdinErrorReason::ChannelClosed(_) => {
                                        stdin_events += 1;
                                    }
                                }
                            }
                            TaskMonitorEvent::ControlReceived { control_type } => {
                                match control_type {
                                    TaskMonitorControlType::SendStdin => control_events += 1,
                                    _ => {}
                                }
                            }
                            TaskMonitorEvent::ControlProcessed { control_type } => {
                                match control_type {
                                    TaskMonitorControlType::SendStdin => control_events += 1,
                                    _ => {}
                                }
                            }
                            TaskMonitorEvent::ControlError { control_type, error: _ } => {
                                match control_type {
                                    TaskMonitorControlType::SendStdin => control_events += 1,
                                    _ => {}
                                }
                            }
                            _ => {}
                        }

                        received_events.push(event);

                        // Exit when we have enough events and execution is complete
                        if execution_completed && stdin_events >= 1 && control_events >= 2 {
                            break;
                        }
                    }
                    None => break,
                }
            }
            _ = &mut timeout => {
                println!("Test timeout reached");
                break;
            }
        }
    }

    // Wait for monitor to complete
    let _ = monitor_handle.await;

    // Verify we received the expected events
    assert!(
        execution_started,
        "Should have received ExecutionStarted event"
    );
    assert!(
        execution_completed,
        "Should have received ExecutionCompleted event"
    );
    assert!(
        stdin_events >= 1,
        "Should have received at least 1 stdin event"
    );
    assert!(
        control_events >= 1,
        "Should have received at least 1 control event"
    );

    println!("Test completed successfully!");
    println!("Total events received: {}", received_events.len());
    println!("Stdin events: {}", stdin_events);
    println!("Control events: {}", control_events);
}
