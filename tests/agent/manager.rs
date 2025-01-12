use openagents::agents::agent::{Agent, AgentInstance, Plan, Task, InstanceStatus, PlanStatus, TaskStatus};
use uuid::Uuid;
use serde_json::json;
use chrono::Utc;

// Mock structures for testing
pub struct MockAgentManager {
    agents: Vec<Agent>,
    instances: Vec<AgentInstance>,
    plans: Vec<Plan>,
    tasks: Vec<Task>,
}

impl MockAgentManager {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            instances: Vec::new(),
            plans: Vec::new(),
            tasks: Vec::new(),
        }
    }

    pub fn create_agent(&mut self, name: &str, description: &str, config: serde_json::Value) -> Agent {
        let agent = Agent {
            id: Uuid::new_v4(),
            name: name.into(),
            description: description.into(),
            pubkey: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            enabled: true,
            config,
            created_at: Utc::now().timestamp(),
        };
        self.agents.push(agent.clone());
        agent
    }

    pub fn create_instance(&mut self, agent_id: Uuid) -> AgentInstance {
        let instance = AgentInstance {
            id: Uuid::new_v4(),
            agent_id,
            status: InstanceStatus::Starting,
            created_at: Utc::now().timestamp(),
            ended_at: None,
        };
        self.instances.push(instance.clone());
        instance
    }

    pub fn create_plan(&mut self, agent_id: Uuid, name: &str) -> Plan {
        let plan = Plan {
            id: Uuid::new_v4(),
            agent_id,
            name: name.into(),
            description: "Test plan".into(),
            status: PlanStatus::Created,
            task_ids: Vec::new(),
            created_at: Utc::now().timestamp(),
            ended_at: None,
            metadata: json!({}),
        };
        self.plans.push(plan.clone());
        plan
    }

    pub fn create_task(&mut self, plan_id: Uuid, instance_id: Uuid, task_type: &str) -> Task {
        let task = Task {
            id: Uuid::new_v4(),
            plan_id,
            instance_id,
            task_type: task_type.into(),
            status: TaskStatus::Pending,
            priority: 1,
            input: json!({}),
            output: None,
            created_at: Utc::now().timestamp(),
            started_at: None,
            ended_at: None,
            error: None,
        };
        self.tasks.push(task.clone());
        task
    }

    pub fn update_instance_status(&mut self, instance_id: Uuid, status: InstanceStatus) -> bool {
        if let Some(instance) = self.instances.iter_mut().find(|i| i.id == instance_id) {
            instance.status = status;
            true
        } else {
            false
        }
    }

    pub fn update_task_status(&mut self, task_id: Uuid, status: TaskStatus) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = status;
            if matches!(status, TaskStatus::Running) {
                task.started_at = Some(Utc::now().timestamp());
            }
            if matches!(status, TaskStatus::Completed | TaskStatus::Failed) {
                task.ended_at = Some(Utc::now().timestamp());
            }
            true
        } else {
            false
        }
    }
}

#[test]
fn test_agent_manager_creation() {
    let mut manager = MockAgentManager::new();
    
    // Create an agent
    let agent = manager.create_agent(
        "Test Agent",
        "A test agent",
        json!({
            "version": "1.0.0",
            "memory_limit": 512
        })
    );
    
    assert_eq!(manager.agents.len(), 1);
    assert_eq!(manager.agents[0].name, "Test Agent");
}

#[test]
fn test_instance_lifecycle_management() {
    let mut manager = MockAgentManager::new();
    
    // Create agent and instance
    let agent = manager.create_agent("Lifecycle Test", "Testing lifecycle", json!({}));
    let instance = manager.create_instance(agent.id);
    
    // Test initial state
    assert!(matches!(instance.status, InstanceStatus::Starting));
    
    // Test status transitions
    assert!(manager.update_instance_status(instance.id, InstanceStatus::Running));
    assert!(manager.update_instance_status(instance.id, InstanceStatus::Paused));
    assert!(manager.update_instance_status(instance.id, InstanceStatus::Stopping));
    assert!(manager.update_instance_status(instance.id, InstanceStatus::Stopped));
}

#[test]
fn test_plan_and_task_management() {
    let mut manager = MockAgentManager::new();
    
    // Setup agent and instance
    let agent = manager.create_agent("Task Test", "Testing tasks", json!({}));
    let instance = manager.create_instance(agent.id);
    
    // Create plan
    let plan = manager.create_plan(agent.id, "Test Plan");
    assert_eq!(plan.name, "Test Plan");
    
    // Create tasks
    let task1 = manager.create_task(plan.id, instance.id, "task_type_1");
    let task2 = manager.create_task(plan.id, instance.id, "task_type_2");
    
    assert_eq!(manager.tasks.len(), 2);
    
    // Test task status updates
    assert!(manager.update_task_status(task1.id, TaskStatus::Running));
    assert!(manager.update_task_status(task1.id, TaskStatus::Completed));
    assert!(manager.update_task_status(task2.id, TaskStatus::Failed));
}

#[test]
fn test_concurrent_tasks() {
    let mut manager = MockAgentManager::new();
    
    // Setup
    let agent = manager.create_agent("Concurrent Test", "Testing concurrent tasks", json!({}));
    let instance = manager.create_instance(agent.id);
    let plan = manager.create_plan(agent.id, "Concurrent Plan");
    
    // Create multiple tasks
    let tasks: Vec<Task> = (0..5)
        .map(|i| manager.create_task(plan.id, instance.id, &format!("task_{}", i)))
        .collect();
    
    // Simulate concurrent execution
    for (i, task) in tasks.iter().enumerate() {
        assert!(manager.update_task_status(task.id, TaskStatus::Running));
        if i % 2 == 0 {
            assert!(manager.update_task_status(task.id, TaskStatus::Completed));
        } else {
            assert!(manager.update_task_status(task.id, TaskStatus::Failed));
        }
    }
    
    // Verify final states
    let completed_count = manager.tasks
        .iter()
        .filter(|t| matches!(t.status, TaskStatus::Completed))
        .count();
    let failed_count = manager.tasks
        .iter()
        .filter(|t| matches!(t.status, TaskStatus::Failed))
        .count();
    
    assert_eq!(completed_count, 3);
    assert_eq!(failed_count, 2);
}