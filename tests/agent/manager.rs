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

// Keep all existing tests unchanged
