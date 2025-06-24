-- Overlord Database Schema for PlanetScale
-- Phase 1: Foundation tables

-- Claude Code sessions tracking
CREATE TABLE IF NOT EXISTS claude_sessions (
  id VARCHAR(255) PRIMARY KEY,           -- session UUID from JSONL filename
  user_id VARCHAR(255) NOT NULL,         -- user account identifier
  project_path TEXT NOT NULL,            -- original project path
  project_name VARCHAR(255),             -- friendly project name
  status ENUM('active', 'inactive', 'archived') DEFAULT 'active',
  started_at TIMESTAMP NOT NULL,
  last_activity TIMESTAMP NOT NULL,
  message_count INT DEFAULT 0,
  total_cost DECIMAL(10,6) DEFAULT 0,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  
  INDEX idx_user_id (user_id),
  INDEX idx_project_path (project_path(255)),
  INDEX idx_status (status),
  INDEX idx_last_activity (last_activity)
);

-- Individual conversation messages
CREATE TABLE IF NOT EXISTS claude_messages (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  session_id VARCHAR(255) NOT NULL,     -- references claude_sessions.id
  entry_uuid VARCHAR(255) NOT NULL,     -- from JSONL entry
  role ENUM('user', 'assistant', 'system') NOT NULL,
  content LONGTEXT NOT NULL,
  model VARCHAR(100),
  token_usage JSON,                     -- {input: N, output: N, total: N}
  cost DECIMAL(8,6),
  timestamp TIMESTAMP NOT NULL,
  metadata JSON,                        -- tool calls, thinking, etc.
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  
  FOREIGN KEY (session_id) REFERENCES claude_sessions(id) ON DELETE CASCADE,
  INDEX idx_session_id (session_id),
  INDEX idx_timestamp (timestamp),
  INDEX idx_role (role)
);

-- User machines running Overlord
CREATE TABLE IF NOT EXISTS user_machines (
  id VARCHAR(255) PRIMARY KEY,          -- machine identifier
  user_id VARCHAR(255) NOT NULL,
  machine_name VARCHAR(255) NOT NULL,
  hostname VARCHAR(255),
  platform VARCHAR(50),                 -- darwin, linux, windows
  overlord_version VARCHAR(50),
  last_ping TIMESTAMP NOT NULL,
  status ENUM('online', 'offline') DEFAULT 'offline',
  capabilities JSON,                    -- what commands are available
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  
  INDEX idx_user_id (user_id),
  INDEX idx_status (status),
  INDEX idx_last_ping (last_ping)
);

-- Remote command execution log (for future phases)
CREATE TABLE IF NOT EXISTS remote_commands (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  machine_id VARCHAR(255) NOT NULL,
  user_id VARCHAR(255) NOT NULL,
  command_type VARCHAR(100) NOT NULL,   -- bash, file_read, file_write, etc.
  command_data JSON NOT NULL,           -- command details
  status ENUM('pending', 'running', 'completed', 'failed') DEFAULT 'pending',
  result JSON,                          -- command output/results
  error_message TEXT,
  started_at TIMESTAMP,
  completed_at TIMESTAMP,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  
  FOREIGN KEY (machine_id) REFERENCES user_machines(id) ON DELETE CASCADE,
  INDEX idx_machine_id (machine_id),
  INDEX idx_user_id (user_id),
  INDEX idx_status (status),
  INDEX idx_command_type (command_type)
);