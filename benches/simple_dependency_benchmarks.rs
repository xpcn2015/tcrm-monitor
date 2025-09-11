use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use tcrm_monitor::monitor::config::{TaskShell, TaskSpec, TcrmTasks};
use tcrm_monitor::monitor::depend::build_depend_map;
use tcrm_task::tasks::config::TaskConfig;

fn create_linear_dependency_chain(size: usize) -> TcrmTasks {
    let mut tasks = TcrmTasks::new();

    for i in 0..size {
        let task_name = format!("task_{}", i);
        let dependencies = if i > 0 {
            Some(vec![format!("task_{}", i - 1)])
        } else {
            None
        };

        let config = TaskConfig {
            command: format!("cmd_{}", i),
            args: None,
            working_dir: None,
            env: None,
            timeout_ms: Some(30000),
            enable_stdin: Some(false),
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

    tasks
}

fn create_complex_graph(size: usize) -> TcrmTasks {
    let mut tasks = TcrmTasks::new();

    for i in 0..size {
        let task_name = format!("task_{}", i);
        let mut dependencies = Vec::new();

        // Create some complex dependency patterns
        if i > 0 {
            dependencies.push(format!("task_{}", i - 1));
        }
        if i > 2 {
            dependencies.push(format!("task_{}", i - 3));
        }
        if i > 5 && i % 3 == 0 {
            dependencies.push(format!("task_{}", i / 2));
        }

        let config = TaskConfig {
            command: format!("cmd_{}", i),
            args: None,
            working_dir: None,
            env: None,
            timeout_ms: Some(30000),
            enable_stdin: Some(false),
            ready_indicator: None,
            ready_indicator_source: None,
        };

        let spec = TaskSpec {
            config,
            shell: Some(TaskShell::Auto),
            dependencies: if dependencies.is_empty() {
                None
            } else {
                Some(dependencies)
            },
            terminate_after_dependents_finished: Some(false),
            ignore_dependencies_error: Some(false),
        };

        tasks.insert(task_name, spec);
    }

    tasks
}

fn bench_build_depend_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("build_depend_map");

    for size in [10, 50, 100, 500] {
        group.throughput(Throughput::Elements(size as u64));

        // Linear chain benchmark
        group.bench_with_input(BenchmarkId::new("linear_chain", size), &size, |b, &size| {
            let tasks = create_linear_dependency_chain(size);
            b.iter(|| {
                let result = build_depend_map(black_box(&tasks));
                black_box(result)
            })
        });

        // Complex graph benchmark
        group.bench_with_input(
            BenchmarkId::new("complex_graph", size),
            &size,
            |b, &size| {
                let tasks = create_complex_graph(size);
                b.iter(|| {
                    let result = build_depend_map(black_box(&tasks));
                    black_box(result)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_build_depend_map);
criterion_main!(benches);
