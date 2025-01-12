use chrono::Utc;
use openagents::agents::agent::{
    Agent, AgentInstance, InstanceStatus, Plan, PlanStatus, Task, TaskStatus,
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

// Mock structures for testing
pub struct MockAgentManager {
    agents: Vec<Agent>,
    instances: Vec<AgentInstance>,
    plans: Vec<Plan>,
    tasks: Vec<Task>,
    instance_states: HashMap<Uuid, serde_json::Value>,
    instance_counts: HashMap<Uuid, usize>, // Track instance count per agent
}

impl MockAgentManager {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            instances: Vec::new(),
            plans: Vec::new(),
            tasks: Vec::new(),
            instance_states: HashMap::new(),
            instance_counts: HashMap::new(),
        }
    }

    pub fn create_agent(
        &mut self,
        name: &str,
        description: &str,
        config: serde_json::Value,
    ) -> Agent {
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
        self.instance_counts.insert(agent.id, 0);
        agent
    }

    pub fn create_instance(&mut self, agent_id: Uuid) -> AgentInstance {
        // Check resource limits
        if let Some(agent) = self.agents.iter().find(|a| a.id == agent_id) {
            let current_count = self.instance_counts.get(&agent_id).unwrap_or(&0);
            
            if let Some(max_instances) = agent.config.get("max_instances").and_then(|v| v.as_u64()) {
                if *current_count >= max_instances as usize {
                    return AgentInstance {
                        id: Uuid::new_v4(),
                        agent_id,
                        status: InstanceStatus::Error,
                        created_at: Utc::now().timestamp(),
                        ended_at: Some(Utc::now().timestamp()),
                    };
                }
            }
            
            let instance = AgentInstance {
                id: Uuid::new_v4(),
                agent_id,
                status: InstanceStatus::Starting,
                created_at: Utc::now().timestamp(),
                ended_at: None,
            };
            
            self.instances.push(instance.clone());
            *self.instance_counts.entry(agent_id).or_insert(0) += 1;
            
            // Initialize instance state
            if let Some(initial_state) = agent.config.get("initial_state") {
                self.instance_states.insert(instance.id, initial_state.clone());
            }
            
            instance
        } else {
            AgentInstance {
                id: Uuid::new_v4(),
                agent_id,
                status: InstanceStatus::Error,
                created_at: Utc::now().timestamp(),
                ended_at: Some(Utc::now().timestamp()),
            }
        }
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
        // First check if instance exists
        let instance_exists = self.instances.iter().any(|i| i.id == instance_id);
        if !instance_exists {
            return false;
        }

        // Handle error recovery preparation
        let recovery_info = if matches!(status, InstanceStatus::Error) {
            self.tasks.iter()
                .find(|t| t.instance_id == instance_id)
                .map(|task| task.plan_id)
        } else {
            None
        };

        // If we need recovery, handle it first
        if let Some(plan_id) = recovery_info {
            let recovery_task = self.create_task(plan_id, instance_id, "recovery_task");
            let task_id = recovery_task.id;
            
            // Update task statuses
            self.update_task_status(task_id, TaskStatus::Running);
            self.update_task_status(task_id, TaskStatus::Completed);
            
            // Finally update the instance status to Running
            if let Some(instance) = self.instances.iter_mut().find(|i| i.id == instance_id) {
                instance.status = InstanceStatus::Running;
            }
            return true;
        }

        // If no recovery needed, just update the status
        if let Some(instance) = self.instances.iter_mut().find(|i| i.id == instance_id) {
            instance.status = status;
            return true;
        }

        false
    }

    pub fn update_task_status(&mut self, task_id: Uuid, status: TaskStatus) -> bool {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            task.status = status.clone();
            if matches!(status, TaskStatus::Running) {
                task.started_at = Some(Utc::now().timestamp());
            }
            if matches!(status, TaskStatus::Completed | TaskStatus::Failed) {
                task.ended_at = Some(Utc::now().timestamp());
            }
            
            // Update instance status based on task status
            if matches!(status, TaskStatus::Failed) {
                if let Some(instance) = self.instances.iter_mut().find(|i| i.id == task.instance_id) {
                    instance.status = InstanceStatus::Error;
                }
            }
            true
        } else {
            false
        }
    }

    pub fn set_instance_state(&mut self, instance_id: Uuid, state: serde_json::Value) -> bool {
        if self.instances.iter().any(|i| i.id == instance_id) {
            self.instance_states.insert(instance_id, state);
            true
        } else {
            false
        }
    }
    
    pub fn get_instance_state(&self, instance_id: Uuid) -> Option<serde_json::Value> {
        self.instance_states.get(&instance_id).cloned()
    }
    
    pub fn update_instance_state(
        &mut self,
        instance_id: Uuid,
        key: &str,
        value: serde_json::Value,
    ) -> bool {
        if let Some(state) = self.instance_states.get_mut(&instance_id) {
            if let Some(obj) = state.as_object_mut() {
                obj.insert(key.to_string(), value);
                true
            } else {
                let mut new_state = serde_json::Map::new();
                new_state.insert(key.to_string(), value);
                *state = serde_json::Value::Object(new_state);
                true
            }
        } else {
            let mut new_state = serde_json::Map::new();
            new_state.insert(key.to_string(), value);
            self.instance_states.insert(instance_id, serde_json::Value::Object(new_state));
            true
        }
    }
    
    pub fn check_version_compatibility(&self, agent: &Agent, platform_version: &str) -> bool {
        let config = agent.config.as_object().unwrap();
        let min_version = config["min_platform_version"].as_str().unwrap();
        let max_version = config["max_platform_version"].as_str().unwrap();
        
        platform_version >= min_version && platform_version <= max_version
    }
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_lifecycle() {
        let mut manager = MockAgentManager::new();
        
        // Create agent
        let config = json!({
            "max_instances": 2,
            "min_platform_version": "1.0",
            "max_platform_version": "2.0",
            "initial_state": {"status": "ready"}
        });
        
        let agent = manager.create_agent("test_agent", "Test Agent", config);
        
        // Create instance
        let instance = manager.create_instance(agent.id);
        assert!(matches!(instance.status, InstanceStatus::Starting));
        
        // Update instance status
        assert!(manager.update_instance_status(instance.id, InstanceStatus::Running));
        
        // Create plan and task
        let plan = manager.create_plan(agent.id, "test_plan");
        let task = manager.create_task(plan.id, instance.id, "test_task");
        
        // Update task status
        assert!(manager.update_task_status(task.id, TaskStatus::Running));
        assert!(manager.update_task_status(task.id, TaskStatus::Completed));
        
        // Test instance state management
        assert!(manager.set_instance_state(instance.id, json!({"key": "value"})));
        assert_eq!(
            manager.get_instance_state(instance.id),
            Some(json!({"key": "value"}))
        );
        assert!(manager.update_instance_state(instance.id, "new_key", json!("new_value")));
        
        // Test version compatibility
        assert!(manager.check_version_compatibility(&agent, "1.5"));
        assert!(!manager.check_version_compatibility(&agent, "0.9"));
    }

    #[test]
    fn test_instance_limits() {
        let mut manager = MockAgentManager::new();
        
        // Create agent with max 1 instance
        let config = json!({
            "max_instances": 1,
            "min_platform_version": "1.0",
            "max_platform_version": "2.0"
        });
        
        let agent = manager.create_agent("test_agent", "Test Agent", config);
        
        // First instance should succeed
        let instance1 = manager.create_instance(agent.id);
        assert!(matches!(instance1.status, InstanceStatus::Starting));
        
        // Second instance should fail
        let instance2 = manager.create_instance(agent.id);
        assert!(matches!(instance2.status, InstanceStatus::Error));
        assert!(instance2.ended_at.is_some());
    }

    #[test]
    fn test_error_recovery() {
        let mut manager = MockAgentManager::new();
        
        let config = json!({
            "max_instances": 2,
            "min_platform_version": "1.0",
            "max_platform_version": "2.0"
        });
        
        let agent = manager.create_agent("test_agent", "Test Agent", config);
        let instance = manager.create_instance(agent.id);
        let plan = manager.create_plan(agent.id, "test_plan");
        let task = manager.create_task(plan.id, instance.id, "test_task");
        
        // Simulate error and recovery
        assert!(manager.update_task_status(task.id, TaskStatus::Failed));
        assert!(manager.update_instance_status(instance.id, InstanceStatus::Error));
        
        // Verify instance recovered to Running state
        if let Some(recovered_instance) = manager.instances.iter().find(|i| i.id == instance.id) {
            assert!(matches!(recovered_instance.status, InstanceStatus::Running));
        }
    }

    #[test]
    fn test_instance_state_management() {
        let mut manager = MockAgentManager::new();
        
        let config = json!({
            "max_instances": 1,
            "initial_state": {"counter": 0}
        });
        
        let agent = manager.create_agent("test_agent", "Test Agent", config);
        let instance = manager.create_instance(agent.id);
        
        // Test initial state
        assert_eq!(
            manager.get_instance_state(instance.id),
            Some(json!({"counter": 0}))
        );
        
        // Test state updates
        assert!(manager.update_instance_state(instance.id, "counter", json!(1)));
        assert!(manager.update_instance_state(instance.id, "status", json!("ready")));
        
        // Verify final state
        let final_state = manager.get_instance_state(instance.id).unwrap();
        assert_eq!(final_state["counter"], json!(1));
        assert_eq!(final_state["status"], json!("ready"));
        
        // Test non-existent instance
        assert!(!manager.update_instance_state(Uuid::new_v4(), "test", json!(true)));
    }

    #[test]
    fn test_task_status_transitions() {
        let mut manager = MockAgentManager::new();
        
        let config = json!({"max_instances": 1});
        let agent = manager.create_agent("test_agent", "Test Agent", config);
        let instance = manager.create_instance(agent.id);
        let plan = manager.create_plan(agent.id, "test_plan");
        let task = manager.create_task(plan.id, instance.id, "test_task");
        
        // Test valid transitions
        assert!(manager.update_task_status(task.id, TaskStatus::Scheduled));
        assert!(manager.update_task_status(task.id, TaskStatus::Running));
        assert!(manager.update_task_status(task.id, TaskStatus::Completed));
        
        // Test non-existent task
        assert!(!manager.update_task_status(Uuid::new_v4(), TaskStatus::Running));
        
        // Verify timestamps
        if let Some(completed_task) = manager.tasks.iter().find(|t| t.id == task.id) {
            assert!(completed_task.started_at.is_some());
            assert!(completed_task.ended_at.is_some());
        }
    }
}
