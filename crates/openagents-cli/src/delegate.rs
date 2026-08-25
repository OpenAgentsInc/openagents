//! Child agent delegation, headless execution, and parallel fan-out

use crate::cli::CoderArgs;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildWorkerTask {
    pub id: usize,
    pub prompt: String,
    pub lane: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildWorkerResult {
    pub id: usize,
    pub success: bool,
    pub output: String,
    pub duration_ms: u128,
}

pub struct DelegationSupervisor {
    pub count: usize,
    pub lane: String,
}

impl DelegationSupervisor {
    pub fn new(count: usize, lane: &str) -> Self {
        Self {
            count,
            lane: lane.to_string(),
        }
    }

    pub async fn dispatch(&self, prompt: &str) -> Vec<ChildWorkerResult> {
        let mut handles = Vec::new();
        for id in 1..=self.count {
            let task = ChildWorkerTask {
                id,
                prompt: prompt.to_string(),
                lane: self.lane.clone(),
            };
            handles.push(tokio::spawn(async move {
                Self::execute_worker(task).await
            }));
        }

        let mut results = Vec::new();
        for handle in join_all(handles).await {
            if let Ok(res) = handle {
                results.push(res);
            }
        }
        results
    }

    async fn execute_worker(task: ChildWorkerTask) -> ChildWorkerResult {
        let start = Instant::now();
        // Simulate child worker execution across isolated sandboxes
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        ChildWorkerResult {
            id: task.id,
            success: true,
            output: format!("Worker #{} completed task on lane {}: prompt received ({})", task.id, task.lane, task.prompt.len()),
            duration_ms: start.elapsed().as_millis(),
        }
    }
}

pub async fn run_delegation(args: CoderArgs) -> Result<(), Box<dyn std::error::Error>> {
    let count = args.count.max(1);
    let lane = args.lane.unwrap_or_else(|| "ox-alpha".to_string());
    let prompt = args.prompt.unwrap_or_else(|| "Execute background sweep".to_string());

    println!("Starting parallel delegation across {} child workers on lane {}...", count, lane);
    let supervisor = DelegationSupervisor::new(count, &lane);
    let results = supervisor.dispatch(&prompt).await;

    for res in &results {
        println!("Child {}: status={}, duration={}ms, output={}", res.id, if res.success { "ok" } else { "err" }, res.duration_ms, res.output);
    }
    println!("Delegation fan-out complete. {}/{} children succeeded.", results.iter().filter(|r| r.success).count(), results.len());
    Ok(())
}
