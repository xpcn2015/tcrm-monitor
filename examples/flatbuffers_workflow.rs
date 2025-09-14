/// Advanced example demonstrating flatbuffers serialization and deserialization
/// with direct byte reading for efficient storage and transmission
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tcrm_monitor::{
    flatbuffers::conversion::ToFlatbuffers,
    monitor::config::{TaskShell, TaskSpec, TcrmTasks},
};
use tcrm_task::tasks::config::{StreamSource, TaskConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 FlatBuffers Workflow Example");
    println!("==========================================");

    // Create tasks configuration
    let tasks = create_example_tasks();

    println!("\n📝 Created {} tasks", tasks.len());

    // Demonstrate serialization and deserialization from bytes
    println!("\n🔄 Converting to FlatBuffers format...");
    let serialized_data = serialize_tasks(&tasks)?;
    println!("✅ Serialized to {} bytes", serialized_data.len());

    println!("\n🔄 Loading from FlatBuffers format...");
    let loaded_tasks = deserialize_tasks(&serialized_data)?;
    println!("✅ Loaded {} tasks", loaded_tasks.len());
    verify_data_integrity(&tasks, &loaded_tasks)?;

    // Demonstrate saving and loading from file
    let file_path = "example_tasks.fb";
    fs::write(file_path, &serialized_data)?;
    println!("💾 Saved to {}", file_path);

    println!("\n📂 Loading from file...");
    let file_data = fs::read(file_path)?;
    let file_loaded_tasks = deserialize_tasks(&file_data)?;
    println!("✅ Loaded {} tasks from file", file_loaded_tasks.len());
    verify_data_integrity(&tasks, &file_loaded_tasks)?;

    // Performance comparison
    benchmark_performance(&tasks)?;

    // Cleanup
    if Path::new(file_path).exists() {
        fs::remove_file(file_path)?;
        println!("🗑️  Cleaned up {}", file_path);
    }

    println!("\n🎉 FlatBuffers workflow example completed successfully!");
    Ok(())
}

fn create_example_tasks() -> TcrmTasks {
    let mut tasks = TcrmTasks::new();

    // Database setup task
    let db_env = {
        let mut env = HashMap::new();
        env.insert("DB_HOST".to_string(), "localhost".to_string());
        env.insert("DB_PORT".to_string(), "5432".to_string());
        env.insert("DB_NAME".to_string(), "myapp".to_string());
        env.insert("DB_USER".to_string(), "admin".to_string());
        env.insert("DB_PASSWORD".to_string(), "secure123".to_string());
        env
    };

    let db_config = TaskConfig {
        command: "docker".to_string(),
        args: Some(vec![
            "run".to_string(),
            "--name".to_string(),
            "postgres-db".to_string(),
            "-e".to_string(),
            "POSTGRES_DB=myapp".to_string(),
            "-e".to_string(),
            "POSTGRES_USER=admin".to_string(),
            "-e".to_string(),
            "POSTGRES_PASSWORD=secure123".to_string(),
            "-p".to_string(),
            "5432:5432".to_string(),
            "-d".to_string(),
            "postgres:13".to_string(),
        ]),
        working_dir: Some("/opt/database".to_string()),
        env: Some(db_env),
        timeout_ms: Some(60000),
        enable_stdin: Some(false),
        ready_indicator: Some("database system is ready to accept connections".to_string()),
        ready_indicator_source: Some(StreamSource::Stderr),
    };

    let db_spec = TaskSpec {
        config: db_config,
        shell: Some(TaskShell::Auto),
        dependencies: None,
        terminate_after_dependents_finished: Some(false),
        ignore_dependencies_error: Some(false),
    };

    tasks.insert("setup_database".to_string(), db_spec);

    // Backend API task
    let api_config = TaskConfig {
        command: "cargo".to_string(),
        args: Some(vec!["run".to_string(), "--release".to_string()]),
        working_dir: Some("/app/backend".to_string()),
        env: Some({
            let mut env = HashMap::new();
            env.insert("RUST_LOG".to_string(), "info".to_string());
            env.insert("SERVER_PORT".to_string(), "8080".to_string());
            env.insert(
                "DATABASE_URL".to_string(),
                "postgres://admin:secure123@localhost:5432/myapp".to_string(),
            );
            env
        }),
        timeout_ms: Some(120000),
        enable_stdin: Some(true),
        ready_indicator: Some("Server listening on".to_string()),
        ready_indicator_source: Some(StreamSource::Stdout),
    };

    let api_spec = TaskSpec {
        config: api_config,
        shell: Some(TaskShell::Auto),
        dependencies: Some(vec!["setup_database".to_string()]),
        terminate_after_dependents_finished: Some(true),
        ignore_dependencies_error: Some(false),
    };

    tasks.insert("start_api".to_string(), api_spec);

    // Frontend development server
    let frontend_config = TaskConfig {
        command: if cfg!(windows) { "npm.cmd" } else { "npm" }.to_string(),
        args: Some(vec!["run".to_string(), "dev".to_string()]),
        working_dir: Some("/app/frontend".to_string()),
        env: Some({
            let mut env = HashMap::new();
            env.insert("NODE_ENV".to_string(), "development".to_string());
            env.insert(
                "REACT_APP_API_URL".to_string(),
                "http://localhost:8080".to_string(),
            );
            env.insert("PORT".to_string(), "3000".to_string());
            env
        }),
        timeout_ms: Some(90000),
        enable_stdin: Some(false),
        ready_indicator: Some("webpack compiled".to_string()),
        ready_indicator_source: Some(StreamSource::Stdout),
    };

    let frontend_spec = TaskSpec {
        config: frontend_config,
        shell: Some(TaskShell::Auto),
        dependencies: Some(vec!["start_api".to_string()]),
        terminate_after_dependents_finished: Some(true),
        ignore_dependencies_error: Some(false),
    };

    tasks.insert("start_frontend".to_string(), frontend_spec);

    // Test runner task
    let test_config = TaskConfig {
        command: "cargo".to_string(),
        args: Some(vec![
            "test".to_string(),
            "--".to_string(),
            "--nocapture".to_string(),
        ]),
        working_dir: Some("/app/backend".to_string()),
        env: Some({
            let mut env = HashMap::new();
            env.insert("RUST_LOG".to_string(), "debug".to_string());
            env.insert(
                "TEST_DATABASE_URL".to_string(),
                "postgres://admin:secure123@localhost:5432/myapp_test".to_string(),
            );
            env
        }),
        timeout_ms: Some(300000), // 5 minutes for tests
        enable_stdin: Some(false),
        ready_indicator: None,
        ready_indicator_source: None,
    };

    let test_spec = TaskSpec {
        config: test_config,
        shell: Some(TaskShell::Auto),
        dependencies: Some(vec!["setup_database".to_string()]),
        terminate_after_dependents_finished: Some(false),
        ignore_dependencies_error: Some(true), // Continue even if tests fail
    };

    tasks.insert("run_tests".to_string(), test_spec);

    // Monitoring task
    let monitor_config = TaskConfig {
        command: "docker".to_string(),
        args: Some(vec![
            "run".to_string(),
            "--name".to_string(),
            "prometheus".to_string(),
            "-p".to_string(),
            "9090:9090".to_string(),
            "-v".to_string(),
            "/opt/prometheus:/etc/prometheus".to_string(),
            "-d".to_string(),
            "prom/prometheus".to_string(),
        ]),
        working_dir: Some("/opt/monitoring".to_string()),
        env: None,
        timeout_ms: Some(30000),
        enable_stdin: Some(false),
        ready_indicator: Some("Server is ready to receive web requests".to_string()),
        ready_indicator_source: Some(StreamSource::Stdout),
    };

    let monitor_spec = TaskSpec {
        config: monitor_config,
        shell: Some(TaskShell::Auto),
        dependencies: Some(vec!["start_api".to_string(), "start_frontend".to_string()]),
        terminate_after_dependents_finished: Some(false),
        ignore_dependencies_error: Some(false),
    };

    tasks.insert("start_monitoring".to_string(), monitor_spec);

    tasks
}

fn serialize_tasks(tasks: &TcrmTasks) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use flatbuffers::FlatBufferBuilder;

    let mut fbb = FlatBufferBuilder::new();
    let fb_tasks_offset = tasks.to_flatbuffers(&mut fbb)?;
    fbb.finish(fb_tasks_offset, None);

    Ok(fbb.finished_data().to_vec())
}

fn deserialize_tasks(data: &[u8]) -> Result<TcrmTasks, Box<dyn std::error::Error>> {
    let fb_tasks = flatbuffers::root::<
        tcrm_monitor::flatbuffers::tcrm_monitor_generated::tcrm::monitor::TcrmTasks,
    >(data)?;
    let tasks: TcrmTasks = fb_tasks.try_into()?;
    Ok(tasks)
}

fn verify_data_integrity(
    original: &TcrmTasks,
    converted: &TcrmTasks,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 Verifying data integrity...");

    if original.len() != converted.len() {
        return Err(format!(
            "Task count mismatch: {} vs {}",
            original.len(),
            converted.len()
        )
        .into());
    }

    for (name, original_spec) in original {
        let converted_spec = converted
            .get(name)
            .ok_or_else(|| format!("Missing task: {}", name))?;

        // Verify core fields
        if original_spec.config.command != converted_spec.config.command {
            return Err(format!(
                "Command mismatch for {}: {} vs {}",
                name, original_spec.config.command, converted_spec.config.command
            )
            .into());
        }

        if original_spec.config.timeout_ms != converted_spec.config.timeout_ms {
            return Err(format!(
                "Timeout mismatch for {}: {:?} vs {:?}",
                name, original_spec.config.timeout_ms, converted_spec.config.timeout_ms
            )
            .into());
        }

        // Verify environment variables
        match (&original_spec.config.env, &converted_spec.config.env) {
            (Some(orig_env), Some(conv_env)) => {
                if orig_env.len() != conv_env.len() {
                    return Err(format!(
                        "Environment variable count mismatch for {}: {} vs {}",
                        name,
                        orig_env.len(),
                        conv_env.len()
                    )
                    .into());
                }
                for (key, value) in orig_env {
                    if conv_env.get(key) != Some(value) {
                        return Err(format!(
                            "Environment variable mismatch for {} key {}: {:?} vs {:?}",
                            name,
                            key,
                            Some(value),
                            conv_env.get(key)
                        )
                        .into());
                    }
                }
            }
            (None, None) => {}
            _ => return Err(format!("Environment variable presence mismatch for {}", name).into()),
        }

        println!("  ✅ Task '{}' integrity verified", name);
    }

    println!("✅ All data integrity checks passed");
    Ok(())
}

fn benchmark_performance(tasks: &TcrmTasks) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n⚡ Running performance benchmarks...");

    let iterations = 100;

    // Benchmark serialization
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _serialized = serialize_tasks(tasks)?;
    }
    let serialization_time = start.elapsed();

    // Benchmark deserialization
    let serialized_data = serialize_tasks(tasks)?;
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _deserialized = deserialize_tasks(&serialized_data)?;
    }
    let deserialization_time = start.elapsed();

    println!("📊 Performance Results ({} iterations):", iterations);
    println!(
        "  Serialization:   {:?} total, {:.2}ms per operation",
        serialization_time,
        serialization_time.as_secs_f64() * 1000.0 / iterations as f64
    );
    println!(
        "  Deserialization: {:?} total, {:.2}ms per operation",
        deserialization_time,
        deserialization_time.as_secs_f64() * 1000.0 / iterations as f64
    );
    println!("  Data size:       {} bytes", serialized_data.len());
    println!(
        "  Compression:     {:.1}% of original (estimated)",
        serialized_data.len() as f64 / estimate_json_size(tasks) as f64 * 100.0
    );

    Ok(())
}

fn estimate_json_size(tasks: &TcrmTasks) -> usize {
    // Rough estimation of JSON size for comparison
    let mut size = 0;
    for (name, spec) in tasks {
        size += name.len() + 20; // Task name + JSON overhead
        size += spec.config.command.len() + 20;
        if let Some(args) = &spec.config.args {
            size += args.iter().map(|s| s.len() + 5).sum::<usize>();
        }
        if let Some(env) = &spec.config.env {
            size += env
                .iter()
                .map(|(k, v)| k.len() + v.len() + 10)
                .sum::<usize>();
        }
        if let Some(deps) = &spec.dependencies {
            size += deps.iter().map(|s| s.len() + 5).sum::<usize>();
        }
        size += 100; // Other fields and JSON formatting
    }
    size
}
