use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::collections::HashMap;
use std::hint::black_box;
use std::time::Duration;
use tcrm_monitor::monitor::config::{TaskShell, TaskSpec, TcrmTasks};
use tcrm_monitor::monitor::tasks::TaskMonitor;
use tcrm_task::tasks::config::TaskConfig;
use tokio::time::timeout;

fn create_simple_task_config(command: &str) -> TaskConfig {
    TaskConfig {
        command: command.to_string(),
        args: None,
        working_dir: None,
        env: None,
        timeout_ms: Some(5000),
        enable_stdin: Some(false),
        ready_indicator: None,
        ready_indicator_source: None,
    }
}

fn create_task_spec(command: &str, dependencies: Option<Vec<String>>) -> TaskSpec {
    TaskSpec {
        config: create_simple_task_config(command),
        shell: Some(TaskShell::Auto),
        dependencies,
        terminate_after_dependents_finished: Some(false),
        ignore_dependencies_error: Some(false),
    }
}

fn create_echo_tasks(count: usize) -> TcrmTasks {
    let mut tasks = TcrmTasks::new();

    for i in 0..count {
        let spec = create_task_spec(&format!("echo 'Task {}'", i), None);
        tasks.insert(format!("echo_task_{}", i), spec);
    }

    tasks
}

fn create_chained_echo_tasks(count: usize) -> TcrmTasks {
    let mut tasks = TcrmTasks::new();

    for i in 0..count {
        let dependencies = if i == 0 {
            None
        } else {
            Some(vec![format!("echo_task_{}", i - 1)])
        };

        let spec = create_task_spec(&format!("echo 'Task {}'", i), dependencies);
        tasks.insert(format!("echo_task_{}", i), spec);
    }

    tasks
}

fn create_cpu_intensive_tasks(count: usize, intensity: usize) -> TcrmTasks {
    let mut tasks = TcrmTasks::new();

    // generates numbers from 1 to N and multiplies each by itself.
    for i in 0..count {
        let command = if cfg!(windows) {
            format!(
                "powershell -c '1..{} | ForEach-Object {{ $_ * $_ }}'",
                intensity * 1000
            )
        } else {
            format!(
                "seq 1 {} | xargs -I{{}} expr {{}} \\* {{}}",
                intensity * 1000
            )
        };
        let spec = TaskSpec::new(TaskConfig::new(command));
        tasks.insert(format!("cpu_task_{}", i), spec);
    }

    tasks
}

fn create_io_intensive_tasks(count: usize) -> TcrmTasks {
    let mut tasks = TcrmTasks::new();

    // Lists files in /usr or C:\ and selects the first 100 entries
    for i in 0..count {
        let command = if cfg!(windows) {
            format!(
                "powershell -c 'Get-ChildItem C:\\ -Recurse -ErrorAction SilentlyContinue | Select-Object -First 100'"
            )
        } else {
            format!("find /usr -type f 2>/dev/null | head -100")
        };

        let spec = TaskSpec::new(TaskConfig::new(command));
        tasks.insert(format!("io_task_{}", i), spec);
    }

    tasks
}

async fn run_simple_execution_benchmark(
    tasks: &TcrmTasks,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut executor = match TaskMonitor::new(tasks.clone()) {
        Ok(monitor) => monitor,
        Err(e) => {
            return Err(Box::new(e));
        }
    };

    // Use a timeout to prevent hanging in case of issues
    timeout(Duration::from_secs(30), async {
        executor.execute_all_direct(None).await
    })
    .await?;

    Ok(())
}
fn bench_task_monitor_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("TaskMonitor Creation");

    let task_counts = vec![1, 5, 10, 25, 50];

    for count in task_counts {
        group.throughput(Throughput::Elements(count as u64));
        let tasks = create_echo_tasks(count);

        group.bench_with_input(
            BenchmarkId::new("monitor_creation", count),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    let monitor = TaskMonitor::new(black_box(tasks.clone()));
                    black_box(monitor)
                });
            },
        );
    }

    group.finish();
}

fn bench_execute_all_direct_with_tx(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("Task Execution With Event Channel");
    let task_counts = vec![1, 5, 10, 25, 50];
    for count in task_counts {
        group.throughput(Throughput::Elements(count as u64));
        let tasks = create_echo_tasks(count);
        group.bench_with_input(
            BenchmarkId::new("execute_with_events", count),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    let (event_tx, _event_rx) = tokio::sync::mpsc::channel(1024);
                    match TaskMonitor::new(black_box(tasks.clone())) {
                        Ok(mut monitor) => {
                            rt.block_on(monitor.execute_all_direct(Some(event_tx)));
                        }
                        Err(e) => {
                            panic!("TaskMonitor setup failed: {}", e);
                        }
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_execute_all_direct_without_tx(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("Task Execution Without Event Channel");
    let task_counts = vec![1, 5, 10, 25, 50];
    for count in task_counts {
        group.throughput(Throughput::Elements(count as u64));
        let tasks = create_echo_tasks(count);
        group.bench_with_input(
            BenchmarkId::new("execute_without_events", count),
            &tasks,
            |b, tasks| {
                b.iter(|| match TaskMonitor::new(black_box(tasks.clone())) {
                    Ok(mut monitor) => {
                        rt.block_on(monitor.execute_all_direct(None));
                    }
                    Err(e) => {
                        panic!("TaskMonitor setup failed: {}", e);
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_task_setup_overhead(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("Task Setup Overhead");

    let task_counts = vec![1, 5, 10, 25, 50, 100];

    for count in task_counts {
        group.throughput(Throughput::Elements(count as u64));

        // Benchmark just the setup phase without execution
        group.bench_with_input(
            BenchmarkId::new("setup_only", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let tasks = create_echo_tasks(black_box(count));
                    let monitor = TaskMonitor::new(black_box(tasks));
                    black_box(monitor)
                });
            },
        );

        // Benchmark setup + initialization but stop before command execution
        let tasks = create_echo_tasks(count);
        group.bench_with_input(
            BenchmarkId::new("setup_and_init", count),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    rt.block_on(async {
                        match TaskMonitor::new(black_box(tasks.clone())) {
                            Ok(monitor) => {
                                // Just measure the initialization overhead,
                                // not actual task execution
                                let task_count = monitor.tasks.len();
                                let dependency_count = monitor.dependencies.len();
                                black_box((task_count, dependency_count))
                            }
                            Err(e) => {
                                panic!("TaskMonitor setup failed: {}", e);
                            }
                        }
                    })
                });
            },
        );
    }

    group.finish();
}

fn bench_dependency_resolution_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("Dependency Resolution Performance");

    let chain_lengths = vec![2, 5, 10, 25, 50, 100];

    for length in chain_lengths {
        group.throughput(Throughput::Elements(length as u64));

        group.bench_with_input(
            BenchmarkId::new("linear_chain", length),
            &length,
            |b, &length| {
                b.iter(|| {
                    let tasks = create_chained_echo_tasks(black_box(length));
                    let monitor = TaskMonitor::new(black_box(tasks));
                    black_box(monitor)
                });
            },
        );

        // Benchmark wide dependency graphs (many tasks depending on one)
        group.bench_with_input(
            BenchmarkId::new("wide_dependencies", length),
            &length,
            |b, &length| {
                b.iter(|| {
                    let mut tasks = TcrmTasks::new();

                    // Root task
                    let root_spec = create_task_spec("echo 'root'", None);
                    tasks.insert("root".to_string(), root_spec);

                    // Many tasks depending on root
                    for i in 0..length {
                        let spec = create_task_spec(
                            &format!("echo 'dependent_{}'", i),
                            Some(vec!["root".to_string()]),
                        );
                        tasks.insert(format!("dependent_{}", i), spec);
                    }

                    let monitor = TaskMonitor::new(black_box(tasks));
                    black_box(monitor)
                });
            },
        );
    }

    group.finish();
}

fn bench_cpu_intensive_task_execution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("CPU Intensive Task Execution");
    group.sample_size(10);

    let task_counts = vec![1, 3, 5];
    let intensity = 10;

    for count in task_counts {
        group.throughput(Throughput::Elements(count as u64));
        let tasks = create_cpu_intensive_tasks(count, intensity);

        group.bench_with_input(
            BenchmarkId::new("cpu_intensive_tasks", count),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    let result = rt.block_on(run_simple_execution_benchmark(black_box(tasks)));
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

fn bench_io_intensive_task_execution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("IO Intensive Task Execution");
    group.sample_size(10);

    let task_counts = vec![1, 3, 5];

    for count in task_counts {
        group.throughput(Throughput::Elements(count as u64));
        let tasks = create_io_intensive_tasks(count);

        group.bench_with_input(
            BenchmarkId::new("io_intensive_tasks", count),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    let result = rt.block_on(run_simple_execution_benchmark(black_box(tasks)));
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

fn bench_simple_task_execution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("Simple Task Execution");
    group.sample_size(10); // Reduce sample size for execution benchmarks

    let task_counts = vec![1, 3, 5];

    for count in task_counts {
        group.throughput(Throughput::Elements(count as u64));
        let tasks = create_echo_tasks(count);

        group.bench_with_input(
            BenchmarkId::new("independent_echo_tasks", count),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    let result = rt.block_on(run_simple_execution_benchmark(black_box(tasks)));
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

fn bench_chained_task_execution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("Chained Task Execution");
    group.sample_size(10); // Reduce sample size for execution benchmarks

    let task_counts = vec![2, 3, 5];

    for count in task_counts {
        group.throughput(Throughput::Elements(count as u64));
        let tasks = create_chained_echo_tasks(count);

        group.bench_with_input(
            BenchmarkId::new("sequential_echo_tasks", count),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    let result = rt.block_on(run_simple_execution_benchmark(black_box(tasks)));
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

fn bench_concurrent_task_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Concurrent Task Creation");

    let task_counts = vec![5, 10, 25];

    for count in task_counts {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("parallel_creation", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let tasks = create_echo_tasks(black_box(count));
                    let monitors: Vec<_> = (0..count)
                        .map(|i| {
                            let single_task = {
                                let mut single = TcrmTasks::new();
                                single.insert(
                                    format!("task_{}", i),
                                    tasks.get(&format!("echo_task_{}", i)).unwrap().clone(),
                                );
                                single
                            };
                            TaskMonitor::new(single_task)
                        })
                        .collect();
                    black_box(monitors)
                });
            },
        );
    }

    group.finish();
}

fn bench_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory Allocation");

    let task_counts = vec![10, 50, 100, 500];

    for count in task_counts {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(
            BenchmarkId::new("task_allocation", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let tasks = create_echo_tasks(black_box(count));
                    let monitors: Vec<_> = tasks
                        .iter()
                        .map(|(name, spec)| {
                            let mut single_task = TcrmTasks::new();
                            single_task.insert(name.clone(), spec.clone());
                            TaskMonitor::new(single_task)
                        })
                        .collect();
                    black_box(monitors)
                });
            },
        );
    }

    group.finish();
}

fn bench_task_configuration_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("Task Configuration Parsing");

    let complexities = vec![0, 5, 10, 25, 50];

    for complexity in complexities {
        group.throughput(Throughput::Elements(1));

        group.bench_with_input(
            BenchmarkId::new("config_parsing", complexity),
            &complexity,
            |b, &complexity| {
                b.iter(|| {
                    let mut env = HashMap::new();
                    for i in 0..complexity {
                        env.insert(format!("VAR_{}", i), format!("value_{}", i));
                    }

                    let args = if complexity > 0 {
                        Some((0..complexity).map(|i| format!("arg_{}", i)).collect())
                    } else {
                        None
                    };

                    let config = TaskConfig {
                        command: "test_command".to_string(),
                        args,
                        working_dir: Some("/test/dir".to_string()),
                        env: Some(env),
                        timeout_ms: Some(5000),
                        enable_stdin: Some(true),
                        ready_indicator: Some("ready".to_string()),
                        ready_indicator_source: Some(
                            tcrm_task::tasks::config::StreamSource::Stdout,
                        ),
                    };

                    let spec = TaskSpec {
                        config,
                        shell: Some(TaskShell::Auto),
                        dependencies: None,
                        terminate_after_dependents_finished: Some(false),
                        ignore_dependencies_error: Some(false),
                    };

                    black_box(spec)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_task_monitor_creation,
    bench_task_setup_overhead,
    bench_dependency_resolution_performance,
    bench_execute_all_direct_without_tx,
    bench_execute_all_direct_with_tx,
    bench_simple_task_execution,
    bench_chained_task_execution,
    bench_cpu_intensive_task_execution,
    bench_io_intensive_task_execution,
    bench_concurrent_task_creation,
    bench_memory_allocation,
    bench_task_configuration_parsing
);
criterion_main!(benches);
