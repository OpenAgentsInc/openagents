-- Overlord Database Schema for PlanetScale
-- Enhanced to handle all Claude Code conversation data types

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

-- Enhanced messages table to handle all Claude Code message types
CREATE TABLE IF NOT EXISTS claude_messages (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  session_id VARCHAR(255) NOT NULL,     -- references claude_sessions.id
  entry_uuid VARCHAR(255) NOT NULL,     -- from JSONL entry
  entry_type ENUM('user', 'assistant', 'summary', 'tool_use', 'tool_result') NOT NULL,
  role ENUM('user', 'assistant', 'system') NULL,  -- NULL for tool/summary entries
  
  -- Content fields (used based on entry_type)
  content LONGTEXT NULL,                -- main text content for user/assistant
  thinking LONGTEXT NULL,               -- assistant thinking content (before response)
  summary LONGTEXT NULL,                -- summary text for summary entries
  
  -- Metadata fields
  model VARCHAR(100) NULL,              -- AI model used (e.g., claude-3-5-sonnet)
  token_usage JSON NULL,                -- {input_tokens: N, output_tokens: N, total_tokens: N}
  cost DECIMAL(8,6) NULL,               -- estimated cost in USD
  timestamp TIMESTAMP NOT NULL,         -- when this entry was created
  turn_count INT NULL,                  -- for summary entries
  
  -- Tool-specific fields
  tool_name VARCHAR(255) NULL,          -- for tool_use entries (e.g., "Read", "Bash")
  tool_input JSON NULL,                 -- for tool_use entries (parameters)
  tool_use_id VARCHAR(255) NULL,        -- for tool_result entries (links to tool_use)
  tool_output LONGTEXT NULL,            -- for tool_result entries (command output)
  tool_is_error BOOLEAN DEFAULT FALSE,  -- for tool_result entries
  
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  
  FOREIGN KEY (session_id) REFERENCES claude_sessions(id) ON DELETE CASCADE,
  INDEX idx_session_id (session_id),
  INDEX idx_timestamp (timestamp),
  INDEX idx_entry_type (entry_type),
  INDEX idx_tool_use_id (tool_use_id),
  INDEX idx_entry_uuid (entry_uuid)
);

-- Separate table for images in user messages
CREATE TABLE IF NOT EXISTS claude_message_images (
  id BIGINT PRIMARY KEY AUTO_INCREMENT,
  message_id BIGINT NOT NULL,           -- references claude_messages.id
  image_data MEDIUMTEXT NOT NULL,       -- base64 encoded image data
  mime_type VARCHAR(50),                -- image/png, image/jpeg, etc.
  position INT NOT NULL,                -- order of images in message (0-based)
  
  FOREIGN KEY (message_id) REFERENCES claude_messages(id) ON DELETE CASCADE,
  INDEX idx_message_id (message_id)
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

-- View to simplify querying conversations with all their data
CREATE VIEW IF NOT EXISTS claude_conversation_view AS
SELECT 
  m.id,
  m.session_id,
  m.entry_uuid,
  m.entry_type,
  m.timestamp,
  CASE 
    WHEN m.entry_type = 'user' THEN m.content
    WHEN m.entry_type = 'assistant' THEN m.content
    WHEN m.entry_type = 'summary' THEN m.summary
    WHEN m.entry_type = 'tool_use' THEN CONCAT('🔧 ', m.tool_name, ': ', CAST(m.tool_input AS CHAR))
    WHEN m.entry_type = 'tool_result' THEN SUBSTRING(m.tool_output, 1, 500)
  END AS display_content,
  m.thinking,
  m.model,
  m.token_usage,
  m.cost,
  m.tool_name,
  m.tool_use_id,
  m.tool_is_error,
  COUNT(i.id) AS image_count
FROM claude_messages m
LEFT JOIN claude_message_images i ON m.id = i.message_id
GROUP BY m.id
ORDER BY m.session_id, m.timestamp;

-- Migration helper: Update existing messages to new schema format
-- This would be run if we had existing data in the old format
-- UPDATE claude_messages SET entry_type = 'user' WHERE role = 'user';
-- UPDATE claude_messages SET entry_type = 'assistant' WHERE role = 'assistant';