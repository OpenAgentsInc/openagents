-- Create agent tables for agent manager service

-- Agent table for storing agent definitions
CREATE TABLE agents (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    config JSONB NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Agent instances table for running instances
CREATE TABLE agent_instances (
    id UUID PRIMARY KEY,
    agent_id UUID NOT NULL REFERENCES agents(id),
    status TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    ended_at TIMESTAMP,
    CONSTRAINT valid_status CHECK (status IN ('Starting', 'Running', 'Paused', 'Stopping', 'Stopped', 'Error'))
);

-- Agent state table for persistent state
CREATE TABLE agent_states (
    instance_id UUID REFERENCES agent_instances(id),
    state_key TEXT NOT NULL,
    state_value JSONB NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, state_key)
);

-- Plans table for agent execution plans
CREATE TABLE plans (
    id UUID PRIMARY KEY,
    agent_id UUID NOT NULL REFERENCES agents(id),
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL,
    task_ids UUID[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    ended_at TIMESTAMP,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT valid_status CHECK (status IN ('Created', 'InProgress', 'Completed', 'Failed', 'Cancelled'))
);

-- Tasks table for individual agent tasks
CREATE TABLE tasks (
    id UUID PRIMARY KEY,
    plan_id UUID NOT NULL REFERENCES plans(id),
    instance_id UUID NOT NULL REFERENCES agent_instances(id),
    task_type TEXT NOT NULL,
    status TEXT NOT NULL,
    priority SMALLINT NOT NULL DEFAULT 1,
    input JSONB NOT NULL DEFAULT '{}'::jsonb,
    output JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    started_at TIMESTAMP,
    ended_at TIMESTAMP,
    error TEXT,
    CONSTRAINT valid_status CHECK (status IN ('Pending', 'Scheduled', 'Running', 'Completed', 'Failed', 'Cancelled')),
    CONSTRAINT valid_priority CHECK (priority BETWEEN 0 AND 255)
);

-- Metrics table for resource monitoring
CREATE TABLE agent_metrics (
    instance_id UUID NOT NULL REFERENCES agent_instances(id),
    timestamp TIMESTAMP NOT NULL DEFAULT NOW(),
    memory_usage BIGINT NOT NULL,
    cpu_usage FLOAT NOT NULL,
    task_count INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    uptime INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (instance_id, timestamp)
);

-- Indexes for performance
CREATE INDEX idx_agent_instances_agent_id ON agent_instances(agent_id);
CREATE INDEX idx_agent_instances_status ON agent_instances(status);
CREATE INDEX idx_agent_states_instance_id ON agent_states(instance_id);
CREATE INDEX idx_plans_agent_id ON plans(agent_id);
CREATE INDEX idx_tasks_plan_id ON tasks(plan_id);
CREATE INDEX idx_tasks_instance_id ON tasks(instance_id);
CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_agent_metrics_instance_id_timestamp ON agent_metrics(instance_id, timestamp);
