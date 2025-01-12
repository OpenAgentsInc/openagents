#[path = "agent/manager.rs"]
mod manager;
#[path = "agent/nostr.rs"]
mod nostr;

use openagents::agents::agent::{Agent, AgentInstance, Plan, Task, InstanceStatus, PlanStatus, TaskStatus};
use uuid::Uuid;
use serde_json::json;
use chrono::Utc;

#[test]
fn test_agent_creation() {
    let agent = Agent {
        id: Uuid::new_v4(),
        name: "Test Agent".into(),
        description: "A test agent".into(),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        enabled: true,
        config: json!({
            "version": "1.0.0",
            "memory_limit": 512,
            "cpu_limit": 1000,
            "capabilities": ["read", "write"]
        }),
        created_at: Utc::now().timestamp(),
    };

    assert_eq!(agent.name, "Test Agent");
    assert!(agent.enabled);
    assert_eq!(agent.pubkey.len(), 64);
}

#[test]
fn test_agent_instance_lifecycle() {
    let agent_id = Uuid::new_v4();
    let instance = AgentInstance {
        id: Uuid::new_v4(),
        agent_id,
        status: InstanceStatus::Starting,
        created_at: Utc::now().timestamp(),
        ended_at: None,
    };

    assert_eq!(instance.agent_id, agent_id);
    assert!(matches!(instance.status, InstanceStatus::Starting));
    assert!(instance.ended_at.is_none());
}

#[test]
fn test_plan_creation_and_tasks() {
    let agent_id = Uuid::new_v4();
    let plan_id = Uuid::new_v4();
    
    let plan = Plan {
        id: plan_id,
        agent_id,
        name: "Test Plan".into(),
        description: "A test plan".into(),
        status: PlanStatus::Created,
        task_ids: vec![],
        created_at: Utc::now().timestamp(),
        ended_at: None,
        metadata: json!({
            "priority": "high",
            "category": "test"
        }),
    };

    let task = Task {
        id: Uuid::new_v4(),
        plan_id: plan.id,
        instance_id: Uuid::new_v4(),
        task_type: "test_task".into(),
        status: TaskStatus::Pending,
        priority: 1,
        input: json!({"test": "input"}),
        output: None,
        created_at: Utc::now().timestamp(),
        started_at: None,
        ended_at: None,
        error: None,
    };

    assert_eq!(plan.name, "Test Plan");
    assert!(matches!(plan.status, PlanStatus::Created));
    assert_eq!(task.plan_id, plan.id);
    assert!(matches!(task.status, TaskStatus::Pending));
}

#[test]
fn test_task_state_transitions() {
    let task = Task {
        id: Uuid::new_v4(),
        plan_id: Uuid::new_v4(),
        instance_id: Uuid::new_v4(),
        task_type: "state_test".into(),
        status: TaskStatus::Pending,
        priority: 1,
        input: json!({}),
        output: None,
        created_at: Utc::now().timestamp(),
        started_at: None,
        ended_at: None,
        error: None,
    };

    // Test initial state
    assert!(matches!(task.status, TaskStatus::Pending));
    assert!(task.started_at.is_none());
    assert!(task.ended_at.is_none());
    assert!(task.error.is_none());

    // Clone task first to avoid ownership issues
    let task1 = task.clone();
    let task2 = task.clone();
    
    // Create a task that simulates completion
    let completed_task = Task {
        status: TaskStatus::Completed,
        started_at: Some(Utc::now().timestamp()),
        ended_at: Some(Utc::now().timestamp()),
        output: Some(json!({"result": "success"})),
        ..task1
    };

    assert!(matches!(completed_task.status, TaskStatus::Completed));
    assert!(completed_task.started_at.is_some());
    assert!(completed_task.ended_at.is_some());
    assert!(completed_task.output.is_some());

    // Create a task that simulates failure
    let failed_task = Task {
        status: TaskStatus::Failed,
        started_at: Some(Utc::now().timestamp()),
        ended_at: Some(Utc::now().timestamp()),
        error: Some("Test error".into()),
        ..task2
    };

    assert!(matches!(failed_task.status, TaskStatus::Failed));
    assert!(failed_task.error.is_some());
}

#[test]
fn test_agent_config_validation() {
    let agent = Agent {
        id: Uuid::new_v4(),
        name: "Config Test Agent".into(),
        description: "Testing config validation".into(),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        enabled: true,
        config: json!({
            "version": "1.0.0",
            "memory_limit": 512,
            "cpu_limit": 1000,
            "capabilities": ["read", "write"],
            "min_platform_version": "0.1.0",
            "dependencies": [],
            "required_integrations": ["nostr"]
        }),
        created_at: Utc::now().timestamp(),
    };

    let config = agent.config.as_object().unwrap();
    
    // Test required fields
    assert!(config.contains_key("version"));
    assert!(config.contains_key("memory_limit"));
    assert!(config.contains_key("cpu_limit"));
    
    // Test capabilities array
    let capabilities = config["capabilities"].as_array().unwrap();
    assert!(capabilities.contains(&json!("read")));
    assert!(capabilities.contains(&json!("write")));
    
    // Test version format
    let version = config["version"].as_str().unwrap();
    assert!(version.split('.').count() == 3);
}
