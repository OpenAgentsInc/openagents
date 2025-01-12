use openagents::agents::{AgentManager, InstanceStatus};
use serde_json::json;
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

async fn setup_test_db() -> PgPool {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgPool::connect(&database_url).await.unwrap()
}

#[tokio::test]
async fn test_agent_validation_errors() {
    let pool = setup_test_db().await;
    let manager = AgentManager::new(pool);

    // Test invalid pubkey
    let result = manager
        .create_agent(
            "Invalid Agent",
            "Testing invalid pubkey",
            "too_short",
            json!({}),
        )
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid pubkey length"));

    // Test invalid config - memory limit
    let result = manager
        .create_agent(
            "Invalid Config",
            "Testing invalid memory limit",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            json!({
                "memory_limit": 0,
            }),
        )
        .await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("memory_limit must be between 1 and 4096"));

    // Test invalid config - cpu limit
    let result = manager
        .create_agent(
            "Invalid Config",
            "Testing invalid cpu limit",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            json!({
                "cpu_limit": -1.0,
            }),
        )
        .await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("cpu_limit must be between 0 and 400"));
}

#[tokio::test]
async fn test_instance_state_persistence() {
    let pool = setup_test_db().await;
    let manager = AgentManager::new(pool);

    // Create agent with initial state
    let agent = manager
        .create_agent(
            "State Test",
            "Testing state persistence",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            json!({
                "initial_state": {
                    "counter": 0,
                    "status": "initialized",
                    "nested": {
                        "key": "value"
                    }
                }
            }),
        )
        .await
        .unwrap();

    // Create instance
    let instance = manager.create_instance(agent.id).await.unwrap();

    // Verify initial state
    let state = manager.get_instance_state(instance.id).await.unwrap().unwrap();
    assert_eq!(state["counter"], 0);
    assert_eq!(state["status"], "initialized");
    assert_eq!(state["nested"]["key"], "value");

    // Update state
    manager
        .set_instance_state(
            instance.id,
            json!({
                "counter": 1,
                "status": "running",
                "new_key": true
            }),
        )
        .await
        .unwrap();

    // Verify updated state
    let state = manager.get_instance_state(instance.id).await.unwrap().unwrap();
    assert_eq!(state["counter"], 1);
    assert_eq!(state["status"], "running");
    assert_eq!(state["new_key"], true);
    assert!(!state.as_object().unwrap().contains_key("nested"));

    // Test non-existent instance
    let result = manager
        .set_instance_state(Uuid::new_v4(), json!({"test": true}))
        .await
        .unwrap();
    assert!(!result);
}

#[tokio::test]
async fn test_resource_monitoring_and_limits() {
    let pool = setup_test_db().await;
    let manager = AgentManager::new(pool);

    // Create agent with strict limits
    let agent = manager
        .create_agent(
            "Resource Test",
            "Testing resource monitoring",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            json!({
                "memory_limit": 256,
                "cpu_limit": 50.0
            }),
        )
        .await
        .unwrap();

    let instance = manager.create_instance(agent.id).await.unwrap();

    // Test within limits
    manager
        .update_instance_metrics(
            instance.id,
            json!({
                "memory_usage": 128,
                "cpu_usage": 25.0,
                "task_count": 1,
                "error_count": 0,
                "uptime": 300
            }),
        )
        .await
        .unwrap();

    assert!(manager.check_resource_limits(instance.id).await.unwrap());

    // Test exceeding memory limit
    manager
        .update_instance_metrics(
            instance.id,
            json!({
                "memory_usage": 512,
                "cpu_usage": 25.0,
                "task_count": 1,
                "error_count": 0,
                "uptime": 360
            }),
        )
        .await
        .unwrap();

    assert!(!manager.check_resource_limits(instance.id).await.unwrap());

    // Test exceeding CPU limit
    manager
        .update_instance_metrics(
            instance.id,
            json!({
                "memory_usage": 128,
                "cpu_usage": 75.0,
                "task_count": 1,
                "error_count": 0,
                "uptime": 420
            }),
        )
        .await
        .unwrap();

    assert!(!manager.check_resource_limits(instance.id).await.unwrap());
}

#[tokio::test]
async fn test_cache_consistency() {
    let pool = setup_test_db().await;
    let manager = AgentManager::new(pool);

    // Create agent and instance
    let agent = manager
        .create_agent(
            "Cache Test",
            "Testing cache consistency",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            json!({}),
        )
        .await
        .unwrap();

    let instance = manager.create_instance(agent.id).await.unwrap();

    // Update status and verify cache
    manager
        .update_instance_status(instance.id, InstanceStatus::Running)
        .await
        .unwrap();

    // Update metrics and verify cache
    let metrics = json!({
        "memory_usage": 256,
        "cpu_usage": 50.0,
        "task_count": 1,
        "error_count": 0,
        "uptime": 300
    });

    manager
        .update_instance_metrics(instance.id, metrics.clone())
        .await
        .unwrap();

    // Update state and verify cache
    let state = json!({
        "counter": 1,
        "status": "running"
    });

    manager
        .set_instance_state(instance.id, state.clone())
        .await
        .unwrap();

    // Verify instance exists and is running by checking state
    let cached_state = manager.get_instance_state(instance.id).await.unwrap().unwrap();
    assert_eq!(cached_state, state);

    // Verify metrics indirectly through resource limits check
    assert!(manager.check_resource_limits(instance.id).await.unwrap());
}

#[tokio::test]
async fn test_edge_cases() {
    let pool = setup_test_db().await;
    let manager = AgentManager::new(pool);

    // Test non-existent agent
    let result = manager.create_instance(Uuid::new_v4()).await;
    assert!(result.is_err());

    // Test disabled agent
    let agent = manager
        .create_agent(
            "Disabled Agent",
            "Testing disabled agent",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            json!({}),
        )
        .await
        .unwrap();

    // Disable agent in database
    sqlx::query!(
        "UPDATE agents SET enabled = false WHERE id = $1",
        agent.id
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = manager.create_instance(agent.id).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Agent is disabled"));

    // Test empty state
    let agent = manager
        .create_agent(
            "Empty State",
            "Testing empty state",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            json!({}),
        )
        .await
        .unwrap();

    let instance = manager.create_instance(agent.id).await.unwrap();
    let state = manager.get_instance_state(instance.id).await.unwrap();
    assert!(state.is_none());

    // Test invalid metrics
    let result = manager
        .update_instance_metrics(
            instance.id,
            json!({
                "memory_usage": "invalid",
                "cpu_usage": "invalid",
                "task_count": "invalid",
                "error_count": "invalid",
                "uptime": "invalid"
            }),
        )
        .await;
    assert!(result.is_ok()); // Should handle invalid values with defaults
}
