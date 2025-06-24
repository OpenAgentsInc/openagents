# Overlord: Claude Code Sync Architecture Analysis

## Executive Summary

This document analyzes the architecture for **Overlord**, a long-running CLI service that will bridge local Claude Code instances with the OpenAgents.com web interface. Named after the StarCraft Zerg unit that provides oversight and coordination, Overlord will enable centralized management of multiple Claude Code agents through a web-based dashboard.

## Problem Statement

Currently, Claude Code operates as a local-only CLI tool, storing conversation data in JSONL files on the user's filesystem. Users cannot:

1. **Centrally manage multiple projects** - Each Claude Code instance is isolated
2. **Access conversations remotely** - Data is locked to the local machine
3. **Collaborate or share** - No way to share conversations or get help from others
4. **Analyze patterns** - No aggregated view of usage, costs, or productivity metrics
5. **Control local operations remotely** - Cannot execute commands or manage projects from a web interface

## Vision: Claude Code as a Service

Overlord transforms Claude Code from an isolated desktop tool into a networked agent that can be:

- **Monitored**: Real-time status, active sessions, resource usage
- **Controlled**: Start/stop sessions, execute commands, manage projects
- **Synchronized**: All conversation data available in cloud database
- **Analyzed**: Usage patterns, cost optimization, productivity insights
- **Collaborated**: Share sessions, get help, manage team access

## Architecture Overview

```
┌─────────────────┐     WebSocket     ┌──────────────────┐     HTTP/DB     ┌─────────────────┐
│   Local User    │ ◄─────────────── │     Overlord     │ ◄──────────────► │ OpenAgents.com  │
│  (Claude Code)  │                  │   CLI Service    │                  │  Web Interface  │
└─────────────────┘                  └──────────────────┘                  └─────────────────┘
         │                                      │                                     │
         ▼                                      ▼                                     ▼
┌─────────────────┐                  ┌──────────────────┐                  ┌─────────────────┐
│ ~/.claude/      │                  │   File Watcher   │                  │   PlanetScale   │
│ projects/       │                  │   JSONL Parser   │                  │    Database     │
│ *.jsonl files   │                  │   Sync Manager   │                  │   (Enhanced)    │
└─────────────────┘                  └──────────────────┘                  └─────────────────┘
```

## Core Components Analysis

### 1. Local File System Integration

**Current Claude Code Storage:**
- Primary: `~/.claude/projects/`
- Secondary: `~/.config/claude/projects/`
- Structure: `{project-path-hashed}/{session-uuid}.jsonl`

**Overlord File Watching:**
```typescript
// Real-time monitoring of JSONL files
interface FileWatcher {
  watchPaths: string[]              // Multiple Claude config paths
  onFileCreated: (path: string) => void    // New session started
  onFileModified: (path: string) => void   // Session messages added
  onFileDeleted: (path: string) => void    // Session cleanup
}
```

**JSONL Parsing Pipeline:**
```typescript
interface ConversationEntry {
  type: "user" | "assistant" | "summary"
  uuid: string
  timestamp: string
  sessionId: string
  projectPath: string
  message?: ClaudeMessage
  metadata?: {
    model: string
    usage: TokenUsage
    cost: number
  }
}
```

### 2. Database Schema Extensions

**New Tables Needed:**

```sql
-- Claude Code sessions tracking
CREATE TABLE claude_sessions (
  id VARCHAR(255) PRIMARY KEY,           -- session UUID from JSONL filename
  user_id VARCHAR(255) NOT NULL,        -- user account identifier
  project_path TEXT NOT NULL,           -- original project path
  project_name VARCHAR(255),            -- friendly project name
  status ENUM('active', 'inactive', 'archived') DEFAULT 'active',
  started_at TIMESTAMP NOT NULL,
  last_activity TIMESTAMP NOT NULL,
  message_count INT DEFAULT 0,
  total_cost DECIMAL(10,6) DEFAULT 0,
  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  
  INDEX idx_user_id (user_id),
  INDEX idx_project_path (project_path),
  INDEX idx_status (status),
  INDEX idx_last_activity (last_activity)
);

-- Individual conversation messages
CREATE TABLE claude_messages (
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
CREATE TABLE user_machines (
  id VARCHAR(255) PRIMARY KEY,          -- machine identifier
  user_id VARCHAR(255) NOT NULL,
  machine_name VARCHAR(255) NOT NULL,
  hostname VARCHAR(255),
  platform VARCHAR(50),                -- darwin, linux, windows
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

-- Remote command execution log
CREATE TABLE remote_commands (
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
```

### 3. WebSocket Communication Protocol

**Client → Server Messages:**
```typescript
interface OverlordMessage {
  type: "heartbeat" | "file_change" | "command_result" | "session_update"
  machineId: string
  timestamp: string
  data: HeartbeatData | FileChangeData | CommandResultData | SessionUpdateData
}

interface FileChangeData {
  action: "created" | "modified" | "deleted"
  filePath: string
  sessionId: string
  projectPath: string
  newEntries?: ConversationEntry[]  // for created/modified
}

interface CommandResultData {
  commandId: string
  status: "completed" | "failed"
  result?: any
  error?: string
  output?: string
}
```

**Server → Client Messages:**
```typescript
interface ServerMessage {
  type: "execute_command" | "sync_request" | "config_update"
  commandId: string
  timestamp: string
  data: ExecuteCommandData | SyncRequestData | ConfigUpdateData
}

interface ExecuteCommandData {
  command: "bash" | "file_read" | "file_write" | "claude_prompt"
  args: {
    script?: string              // for bash
    path?: string               // for file operations
    content?: string            // for file_write
    prompt?: string             // for claude_prompt
    sessionId?: string          // for claude_prompt
  }
}
```

### 4. Overlord CLI Service Architecture

**Built on Previous CLI Package:**
The deleted `packages/cli` provides the perfect foundation with:
- Effect-based architecture with proper error handling
- Command structure using `@effect/cli`
- AI integration with `@openagentsinc/ai`
- Container management patterns

**Enhanced Overlord Structure:**
```typescript
// packages/overlord/src/Overlord.ts
interface OverlordService {
  // Core daemon functionality
  startDaemon(): Effect.Effect<void, OverlordError>
  stopDaemon(): Effect.Effect<void, OverlordError>
  
  // File system monitoring
  watchClaudeFiles(): Effect.Effect<Stream<FileChangeEvent>, OverlordError>
  
  // WebSocket management
  connectToServer(): Effect.Effect<WebSocketConnection, OverlordError>
  sendMessage(message: OverlordMessage): Effect.Effect<void, OverlordError>
  
  // Command execution
  executeCommand(command: ExecuteCommandData): Effect.Effect<CommandResult, OverlordError>
  
  // Sync management
  syncSession(sessionId: string): Effect.Effect<void, OverlordError>
  syncAllSessions(): Effect.Effect<SyncSummary, OverlordError>
}
```

**Command Structure:**
```bash
# Primary daemon command
overlord daemon --user-id=user123 --api-key=xxx

# Manual sync commands
overlord sync --session=abc-123
overlord sync --all
overlord sync --project="/path/to/project"

# Status and control
overlord status
overlord stop
overlord restart

# Configuration
overlord config --set api-endpoint=https://openagents.com/api
overlord config --set sync-interval=30s
```

## Integration Points

### 1. Web Interface Enhancements

**New Dashboard Sections:**

**Machine Management:**
- Connected machines list with status indicators
- Machine capabilities and system info
- Last seen timestamps and version info

**Session Browser:**
- All Claude Code sessions across all machines
- Filter by project, date, machine, status
- Search across conversation content
- Cost and usage analytics

**Remote Control Panel:**
- Execute bash commands on remote machines
- Browse and edit files
- Start/stop Claude Code sessions
- Real-time command output streaming

**Analytics Dashboard:**
- Usage patterns across projects and machines
- Cost breakdown by model and session
- Productivity metrics and insights
- Token usage optimization recommendations

### 2. Real-time Features

**Live Session Monitoring:**
```typescript
// WebSocket streams for real-time updates
interface LiveSessionUpdate {
  sessionId: string
  messageCount: number
  lastMessage: {
    role: string
    timestamp: string
    preview: string  // first 100 chars
  }
  currentCost: number
  tokenUsage: TokenUsage
}
```

**Command Execution Streaming:**
```typescript
// Real-time command output
interface CommandOutput {
  commandId: string
  type: "stdout" | "stderr" | "exit"
  data: string
  timestamp: string
}
```

### 3. Security Considerations

**Authentication:**
- User API keys for machine authentication
- JWT tokens for web interface sessions
- Machine-specific certificates for enhanced security

**Command Authorization:**
```typescript
interface CommandPermissions {
  allowBashExecution: boolean
  allowFileWrite: boolean
  allowedDirectories: string[]
  forbiddenCommands: string[]
  maxExecutionTime: number
}
```

**Data Privacy:**
- All conversation data encrypted in transit
- Local file content never transmitted unless explicitly requested
- User control over what data gets synced

## Technical Implementation Strategy

### Phase 1: Foundation (Hours 1-16)
1. **Restore CLI Package**: Recover and enhance the deleted `packages/cli`
2. **Basic File Watching**: Implement JSONL file monitoring
3. **Database Schema**: Create new tables for Claude Code data
4. **WebSocket Infrastructure**: Basic client-server communication

### Phase 2: Core Sync (Hours 17-32)
1. **JSONL Parsing Pipeline**: Convert local files to database records
2. **Bidirectional Sync**: Handle file changes and database updates
3. **Session Management**: Track active sessions and metadata
4. **Basic Web UI**: Display synced conversations

### Phase 3: Remote Control (Hours 33-48)
1. **Command Execution**: Bash, file operations, Claude prompts
2. **Real-time Streaming**: Live command output and session updates
3. **Security Framework**: Permissions and authentication
4. **Advanced Web UI**: Remote control panel

### Phase 4: Analytics & Polish (Hours 49-64)
1. **Usage Analytics**: Cost tracking, productivity metrics
2. **Advanced Features**: Collaboration, sharing, team management
3. **Performance Optimization**: Efficient sync, caching
4. **Documentation**: User guides, API docs, deployment guides

## Example User Workflows

### 1. Developer Productivity Dashboard
```
User works on 3 different projects with Claude Code locally:
- /Users/dev/project-a (web app)
- /Users/dev/project-b (API service) 
- /Users/dev/project-c (data analysis)

Overlord syncs all sessions to OpenAgents.com where user can:
- See aggregate cost across all projects: $47.23 this month
- Find that conversation where Claude helped debug the React component
- Notice that project-c uses expensive models - optimize for o1-mini
- Share the interesting database optimization conversation with teammate
```

### 2. Remote Development Support
```
User is traveling but needs to check something on development machine:

From web interface:
1. Connect to home machine (overlord daemon running)
2. Browse ~/code/project/src/components/
3. Edit Button.tsx directly in web interface
4. Run "npm test" and see output in real-time
5. Start new Claude Code session to debug failing test
6. All conversation data immediately available in web dashboard
```

### 3. Team Collaboration
```
Senior developer helping junior developer:

1. Junior shares session link: openagents.com/sessions/abc-123
2. Senior can see entire Claude conversation history
3. Senior uses remote control to check local environment
4. Both can contribute to same conversation thread
5. Knowledge base builds up in shared organizational dashboard
```

## StarCraft Thematic Integration

**Overlord Unit Parallels:**
- **Provides oversight**: Monitors multiple Claude Code "units"
- **Enables coordination**: Connects isolated instances
- **Transport capability**: Moves data between local and cloud
- **Detection and reconnaissance**: Watches for changes and activity
- **Supply management**: Tracks resource usage and optimization

**Command Naming:**
```bash
overlord spawn          # Start new daemon
overlord detect          # Discover Claude installations  
overlord transport       # Manual data sync
overlord evolve          # Update to new version
overlord burrow          # Background daemon mode
overlord unburrow        # Bring daemon to foreground
```

## Risk Analysis and Mitigation

### Technical Risks

**File System Race Conditions:**
- Risk: Multiple processes writing to same JSONL files
- Mitigation: File locking, atomic writes, conflict resolution

**WebSocket Connection Reliability:**
- Risk: Network interruptions causing data loss
- Mitigation: Message queuing, retry mechanisms, offline mode

**Database Synchronization:**
- Risk: Local and remote data getting out of sync
- Mitigation: Checksum validation, conflict resolution, manual merge tools

### Security Risks

**Remote Code Execution:**
- Risk: Web interface executing arbitrary commands
- Mitigation: Sandboxing, permission system, command whitelist

**Data Exposure:**
- Risk: Sensitive conversations leaked
- Mitigation: Encryption, access controls, audit logging

**Authentication Bypass:**
- Risk: Unauthorized access to user's machine
- Mitigation: Strong API keys, machine certificates, session timeouts

### Operational Risks

**Resource Consumption:**
- Risk: Overlord daemon consuming too much CPU/memory
- Mitigation: Efficient file watching, rate limiting, resource monitoring

**Database Growth:**
- Risk: Conversation data growing unbounded
- Mitigation: Data retention policies, compression, archiving

**User Experience:**
- Risk: Complex setup discouraging adoption
- Mitigation: One-command installation, clear documentation, sensible defaults

## Success Metrics

### Technical Metrics
- **Sync Latency**: < 5 seconds from local file change to web UI update
- **Uptime**: > 99.5% availability for daemon and web services
- **Performance**: Handle 1000+ concurrent sessions per user
- **Data Integrity**: Zero data loss during sync operations

### User Experience Metrics
- **Setup Time**: < 5 minutes from download to first sync
- **Response Time**: < 200ms for web UI interactions
- **Reliability**: < 0.1% failed command executions
- **Discoverability**: Users find key features within first session

### Business Metrics
- **Adoption Rate**: % of Claude Code users who install Overlord
- **Engagement**: Average sessions per day per user
- **Retention**: % of users still active after 30 days
- **Value Demonstration**: Measured productivity improvements

## Conclusion

Overlord represents a fundamental evolution of Claude Code from an isolated desktop tool to a networked, manageable, and collaborative development platform. By bridging local Claude Code instances with a centralized web interface, we enable:

1. **Unified Management**: Single dashboard for all Claude Code activity
2. **Remote Accessibility**: Access conversations and control from anywhere
3. **Team Collaboration**: Share knowledge and assist team members
4. **Data Intelligence**: Analytics and optimization insights
5. **Enhanced Productivity**: Streamlined workflows and reduced context switching

The architecture leverages existing OpenAgents infrastructure (Effect framework, PlanetScale database, WebSocket services) while building on the proven patterns from the deleted CLI package. This approach minimizes technical risk while maximizing user value.

The StarCraft "Overlord" theme provides both technical inspiration (oversight, coordination, transport) and user-friendly metaphors that make the system approachable and memorable.

**Next Steps:**
1. Review and approve this architectural analysis
2. Restore and enhance the CLI package foundation
3. Begin Phase 1 implementation with file watching and basic sync
4. Establish feedback loops with early adopters
5. Iterate toward full remote control and analytics capabilities

This system will transform Claude Code from a powerful local tool into the foundation for a collaborative AI development platform.