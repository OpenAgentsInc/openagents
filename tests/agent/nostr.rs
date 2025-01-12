use openagents::nostr::event::Event;
use openagents::agents::agent::{Agent, AgentInstance, InstanceStatus};
use uuid::Uuid;
use serde_json::json;

fn create_agent_status_event(agent: &Agent, instance: &AgentInstance) -> Event {
    Event {
        id: "a6b6c6d6e6f6".into(),
        pubkey: agent.pubkey.clone(),
        created_at: chrono::Utc::now().timestamp(),
        kind: 30001,
        tags: vec![
            vec!["d".into(), "agent_status".into()],
            vec!["p".into(), agent.pubkey.clone()],
        ],
        content: json!({
            "agent_id": agent.id.to_string(),
            "instance_id": instance.id.to_string(),
            "status": format!("{:?}", instance.status),
            "name": agent.name,
            "config": agent.config
        }).to_string(),
        sig: "0123456789abcdef".into(),
        tagidx: None,
    }
}

fn create_agent_task_event(agent: &Agent, task_type: &str, progress: u8) -> Event {
    Event {
        id: "b7c7d7e7f7g7".into(),
        pubkey: agent.pubkey.clone(),
        created_at: chrono::Utc::now().timestamp(),
        kind: 1001,
        tags: vec![
            vec!["p".into(), agent.pubkey.clone()],
            vec!["t".into(), "task_update".into()],
            vec!["r".into(), task_type.into()],
        ],
        content: json!({
            "task": task_type,
            "progress": progress
        }).to_string(),
        sig: "0123456789abcdef".into(),
        tagidx: None,
    }
}

fn create_agent_control_event(agent: &Agent, command: &str) -> Event {
    Event {
        id: "c8d8e8f8g8h8".into(),
        pubkey: agent.pubkey.clone(),
        created_at: chrono::Utc::now().timestamp(),
        kind: 20001,
        tags: vec![
            vec!["p".into(), agent.pubkey.clone()],
            vec!["c".into(), "control".into()],
        ],
        content: json!({
            "command": command,
            "reason": "user_requested"
        }).to_string(),
        sig: "0123456789abcdef".into(),
        tagidx: None,
    }
}

#[test]
fn test_agent_status_event() {
    let agent = Agent {
        id: Uuid::new_v4(),
        name: "Test Agent".into(),
        description: "A test agent".into(),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        enabled: true,
        config: json!({
            "version": "1.0.0",
            "memory_limit": 512
        }),
        created_at: chrono::Utc::now().timestamp(),
    };

    let instance = AgentInstance {
        id: Uuid::new_v4(),
        agent_id: agent.id,
        status: InstanceStatus::Running,
        created_at: chrono::Utc::now().timestamp(),
        ended_at: None,
    };

    let event = create_agent_status_event(&agent, &instance);

    assert_eq!(event.kind, 30001);
    assert!(event.tags.iter().any(|t| t[0] == "d" && t[1] == "agent_status"));
    
    let content: serde_json::Value = serde_json::from_str(&event.content).unwrap();
    assert_eq!(content["agent_id"].as_str().unwrap(), agent.id.to_string());
    assert_eq!(content["status"].as_str().unwrap(), "Running");
}

#[test]
fn test_agent_task_event() {
    let agent = Agent {
        id: Uuid::new_v4(),
        name: "Task Test Agent".into(),
        description: "Testing task events".into(),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        enabled: true,
        config: json!({}),
        created_at: chrono::Utc::now().timestamp(),
    };

    let event = create_agent_task_event(&agent, "analyze_data", 75);

    assert_eq!(event.kind, 1001);
    assert!(event.tags.iter().any(|t| t[0] == "t" && t[1] == "task_update"));
    
    let content: serde_json::Value = serde_json::from_str(&event.content).unwrap();
    assert_eq!(content["task"].as_str().unwrap(), "analyze_data");
    assert_eq!(content["progress"].as_u64().unwrap(), 75);
}

#[test]
fn test_agent_control_event() {
    let agent = Agent {
        id: Uuid::new_v4(),
        name: "Control Test Agent".into(),
        description: "Testing control events".into(),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        enabled: true,
        config: json!({}),
        created_at: chrono::Utc::now().timestamp(),
    };

    let event = create_agent_control_event(&agent, "pause");

    assert_eq!(event.kind, 20001);
    assert!(event.tags.iter().any(|t| t[0] == "c" && t[1] == "control"));
    
    let content: serde_json::Value = serde_json::from_str(&event.content).unwrap();
    assert_eq!(content["command"].as_str().unwrap(), "pause");
    assert_eq!(content["reason"].as_str().unwrap(), "user_requested");
}

#[test]
fn test_event_tag_indexing() {
    let agent = Agent {
        id: Uuid::new_v4(),
        name: "Index Test Agent".into(),
        description: "Testing event tag indexing".into(),
        pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        enabled: true,
        config: json!({}),
        created_at: chrono::Utc::now().timestamp(),
    };

    let instance = AgentInstance {
        id: Uuid::new_v4(),
        agent_id: agent.id,
        status: InstanceStatus::Running,
        created_at: chrono::Utc::now().timestamp(),
        ended_at: None,
    };

    let mut event = create_agent_status_event(&agent, &instance);
    event.build_index();

    use std::collections::HashSet;
    let mut check = HashSet::new();
    check.insert("agent_status".into());
    
    assert!(event.generic_tag_val_intersect('d', &check));
}