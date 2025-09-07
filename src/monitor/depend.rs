use std::collections::{HashMap, HashSet};

use crate::monitor::{config::TcrmTasks, error::TaskMonitorError};

#[derive(Debug)]
pub struct TaskMonitorDependMap {
    pub dependencies: HashMap<String, Vec<String>>,
    pub dependents: HashMap<String, Vec<String>>,
}
pub fn build_depend_map(tasks: &TcrmTasks) -> Result<TaskMonitorDependMap, TaskMonitorError> {
    let mut dependencies = HashMap::new();
    let mut dependents = HashMap::new();
    // Collect and expand dependencies
    for (name, config) in tasks {
        let direct_deps = config.dependencies.clone().unwrap_or_default();

        // Expand to all transitive dependencies
        let mut all_deps = HashSet::new();
        let mut stack = direct_deps;

        while let Some(dep) = stack.pop() {
            if !tasks.contains_key(&dep) {
                return Err(TaskMonitorError::DependencyNotFound {
                    dep: dep.clone(),
                    task: name.clone(),
                });
            }
            if all_deps.insert(dep.clone()) {
                let config = match tasks.get(&dep) {
                    Some(c) => c,
                    None => {
                        return Err(TaskMonitorError::ConfigParse(format!(
                            "Unable to get task: {}",
                            dep
                        )));
                    }
                };
                if let Some(dep_cfg) = &config.dependencies {
                    stack.extend(dep_cfg.clone());
                }
            }
        }

        // Insert full dependency list
        let all_deps_vec = all_deps.into_iter().collect::<Vec<_>>();
        if all_deps_vec.is_empty() {
            continue;
        }
        dependencies.insert(name.clone(), all_deps_vec.clone());
        // Build reverse dependency map
        for dep in &all_deps_vec {
            dependents
                .entry(dep.clone())
                .or_insert_with(Vec::new)
                .push(name.clone());
        }
    }
    Ok(TaskMonitorDependMap {
        dependencies,
        dependents,
    })
}

/// Check circular dependencies in tasks, tasks with no dependencies will not check
pub fn check_circular_dependencies(
    dependencies: &HashMap<String, Vec<String>>,
) -> Result<(), TaskMonitorError> {
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();

    for task_name in dependencies.keys() {
        if !visited.contains(task_name.as_str()) {
            if has_cycle(&dependencies, task_name, &mut visited, &mut rec_stack) {
                return Err(TaskMonitorError::CircularDependency(task_name.clone()));
            }
        }
    }
    Ok(())
}
fn has_cycle<'a>(
    dependencies: &'a HashMap<String, Vec<String>>,
    task_name: &'a str,
    visited: &mut HashSet<&'a str>,
    rec_stack: &mut HashSet<&'a str>,
) -> bool {
    visited.insert(task_name);
    rec_stack.insert(task_name);

    if let Some(deps) = dependencies.get(task_name) {
        for dep in deps {
            if !visited.contains(dep.as_str()) {
                if has_cycle(dependencies, dep, visited, rec_stack) {
                    return true;
                }
            } else if rec_stack.contains(dep.as_str()) {
                return true;
            }
        }
    }

    rec_stack.remove(task_name);
    false
}
