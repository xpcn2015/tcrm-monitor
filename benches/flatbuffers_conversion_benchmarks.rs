use criterion::{ criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use flatbuffers::FlatBufferBuilder;
use std::collections::HashMap;
use std::hint::black_box;
use tcrm_monitor::{flatbuffers::conversion::ToFlatbuffers, monitor::config::{TaskShell, TaskSpec, TcrmTasks}};
use tcrm_task::tasks::config::TaskConfig;

fn create_sample_task_config(complexity: usize) -> TaskConfig {
    let mut env = HashMap::new();
    for i in 0..complexity {
        env.insert(format!("VAR_{}", i), format!("value_{}", i));
    }

    let args = if complexity > 0 {
        Some((0..complexity).map(|i| format!("arg_{}", i)).collect())
    } else {
        None
    };

    TaskConfig {
        command: "test_command".to_string(),
        args,
        working_dir: Some("/test/dir".to_string()),
        env: Some(env),
        timeout_ms: Some(5000),
        enable_stdin: Some(true),
        ready_indicator: Some("ready".to_string()),
        ready_indicator_source: Some(tcrm_task::tasks::config::StreamSource::Stdout),
    }
}

fn create_sample_task_spec(complexity: usize) -> TaskSpec {
    let config = create_sample_task_config(complexity);
    let dependencies = if complexity > 0 {
        Some((0..complexity).map(|i| format!("dep_{}", i)).collect())
    } else {
        None
    };

    TaskSpec {
        config,
        shell: Some(TaskShell::Auto),
        dependencies,
        terminate_after_dependents_finished: Some(true),
        ignore_dependencies_error: Some(false),
    }
}

fn create_sample_tasks(num_tasks: usize, complexity: usize) -> TcrmTasks {
    let mut tasks = TcrmTasks::new();
    for i in 0..num_tasks {
        let spec = create_sample_task_spec(complexity);
        tasks.insert(format!("task_{}", i), spec);
    }
    tasks
}

fn bench_taskshell_conversions(c: &mut Criterion) {
    let mut group = c.benchmark_group("TaskShell Conversions");
    
    let shells = vec![
        TaskShell::None,
        TaskShell::Auto,
        #[cfg(windows)]
        TaskShell::Cmd,
        #[cfg(windows)]
        TaskShell::Powershell,
        #[cfg(unix)]
        TaskShell::Bash,
    ];

    for shell in shells {
        group.bench_with_input(
            BenchmarkId::new("to_flatbuffers", format!("{:?}", shell)),
            &shell,
            |b, shell| {
                b.iter(|| {
                    let fb_shell: tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TaskShell = 
                        black_box(shell.clone()).into();
                    black_box(fb_shell)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("from_flatbuffers", format!("{:?}", shell)),
            &shell,
            |b, shell| {
                let fb_shell: tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TaskShell = 
                    shell.clone().into();
                b.iter(|| {
                    let converted: Result<TaskShell, _> = black_box(fb_shell).try_into();
                    black_box(converted)
                });
            },
        );
    }
    
    group.finish();
}

fn bench_taskspec_conversions(c: &mut Criterion) {
    let mut group = c.benchmark_group("TaskSpec Conversions");
    group.throughput(Throughput::Elements(1));

    let complexities = vec![0, 1, 5, 10, 25, 50];

    for complexity in complexities {
        let spec = create_sample_task_spec(complexity);

        group.bench_with_input(
            BenchmarkId::new("to_flatbuffers", complexity),
            &spec,
            |b, spec| {
                b.iter(|| {
                    let mut fbb = FlatBufferBuilder::new();
                    let result = black_box(spec).to_flatbuffers(&mut fbb);
                    black_box(result)
                });
            },
        );

        // Pre-convert for from_flatbuffers benchmark
        let mut fbb = FlatBufferBuilder::new();
        let fb_spec_offset = spec.to_flatbuffers(&mut fbb);
        fbb.finish(fb_spec_offset, None);
        let buf = fbb.finished_data();
        let fb_spec = flatbuffers::root::<
            tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TaskSpec
        >(buf).unwrap();

        group.bench_with_input(
            BenchmarkId::new("from_flatbuffers", complexity),
            &fb_spec,
            |b, fb_spec| {
                b.iter(|| {
                    let result = TaskSpec::try_from(black_box(*fb_spec));
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

fn bench_tasks_conversions(c: &mut Criterion) {
    let mut group = c.benchmark_group("TcrmTasks Conversions");
    
    let task_counts = vec![1, 5, 10, 25, 50, 100];
    let complexity = 5; // Fixed complexity for tasks

    for num_tasks in task_counts {
        group.throughput(Throughput::Elements(num_tasks as u64));
        let tasks = create_sample_tasks(num_tasks, complexity);

        group.bench_with_input(
            BenchmarkId::new("to_flatbuffers", num_tasks),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    let mut fbb = FlatBufferBuilder::new();
                    let result = tcrm_monitor::flatbuffers::conversion::tcrm_tasks_to_flatbuffers(black_box(tasks), &mut fbb);
                    black_box(result)
                });
            },
        );

        // Pre-convert for from_flatbuffers benchmark
        let mut fbb = FlatBufferBuilder::new();
        let fb_tasks_offset = tcrm_monitor::flatbuffers::conversion::tcrm_tasks_to_flatbuffers(&tasks, &mut fbb);
        fbb.finish(fb_tasks_offset, None);
        let buf = fbb.finished_data();
        let fb_tasks = flatbuffers::root::<
            tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TcrmTasks
        >(buf).unwrap();

        group.bench_with_input(
            BenchmarkId::new("from_flatbuffers", num_tasks),
            &fb_tasks,
            |b, fb_tasks| {
                b.iter(|| {
                    let result = TcrmTasks::try_from(black_box(*fb_tasks));
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

fn bench_roundtrip_conversions(c: &mut Criterion) {
    let mut group = c.benchmark_group("Roundtrip Conversions");
    
    let task_counts = vec![1, 5, 10, 25];
    let complexity = 5;

    for num_tasks in task_counts {
        group.throughput(Throughput::Elements(num_tasks as u64));
        let tasks = create_sample_tasks(num_tasks, complexity);

        group.bench_with_input(
            BenchmarkId::new("full_roundtrip", num_tasks),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    // To flatbuffers
                    let mut fbb = FlatBufferBuilder::new();
                    let fb_offset = tcrm_monitor::flatbuffers::conversion::tcrm_tasks_to_flatbuffers(black_box(tasks), &mut fbb);
                    fbb.finish(fb_offset, None);
                    // Parse back
                    let buf = fbb.finished_data();
                    let fb_tasks = flatbuffers::root::<
                        tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TcrmTasks
                    >(buf).unwrap();
                    // From flatbuffers
                    let result = TcrmTasks::try_from(fb_tasks);
                    black_box(result)
                });
            },
        );
    }

    group.finish();
}

fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory Usage");
    
    let task_counts = vec![10, 50, 100, 500];
    
    for num_tasks in task_counts {
        let tasks = create_sample_tasks(num_tasks, 10);
        
        group.bench_with_input(
            BenchmarkId::new("serialization_size", num_tasks),
            &tasks,
            |b, tasks| {
                b.iter(|| {
                    let mut fbb = FlatBufferBuilder::new();
                    let fb_offset = tcrm_monitor::flatbuffers::conversion::tcrm_tasks_to_flatbuffers(black_box(tasks), &mut fbb);
                    fbb.finish(fb_offset, None);
                    let buf = fbb.finished_data();
                    black_box(buf.len()) // Measure serialized size
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_taskshell_conversions,
    bench_taskspec_conversions,
    bench_tasks_conversions,
    bench_roundtrip_conversions,
    bench_memory_usage
);
criterion_main!(benches);

