/**
 * Type definitions for Claude Code control and integration
 * @since Phase 3
 */

// Claude Code command types that can be sent remotely
export interface ClaudeCodeCommand {
  readonly type: "send_prompt" | "start_session" | "get_status" | "switch_project" | "end_session"
  readonly commandId: string
  readonly sessionId?: string
  readonly machineId: string
  readonly userId: string
  readonly timestamp: Date
  readonly data: {
    readonly prompt?: string                    // For send_prompt
    readonly projectPath?: string              // For start_session, switch_project
    readonly maxTurns?: number                 // Conversation limits
    readonly systemPrompt?: string             // Custom system prompts
    readonly options?: ClaudeCodeOptions       // Additional options
  }
}

// Claude Code execution options
export interface ClaudeCodeOptions {
  readonly maxTurns?: number
  readonly timeout?: number                    // Timeout in milliseconds
  readonly temperature?: number                // Model temperature
  readonly model?: string                      // Specific model to use
  readonly includeThinking?: boolean           // Include Claude's thinking process
}

// Response from Claude Code instances
export interface ClaudeCodeResponse {
  readonly type: "response_chunk" | "session_started" | "session_ended" | "error" | "thinking" | "tool_use"
  readonly commandId: string
  readonly sessionId: string
  readonly machineId: string
  readonly timestamp: Date
  readonly data: {
    readonly content?: string                  // Claude's response content
    readonly thinking?: string                 // Claude's reasoning process
    readonly toolUse?: ToolUseEvent           // File edits, commands run
    readonly status?: "thinking" | "complete" | "error" | "cancelled"
    readonly error?: string                    // Error message if status is error
    readonly metadata?: ResponseMetadata       // Additional response metadata
  }
}

// Tool use events from Claude Code
export interface ToolUseEvent {
  readonly toolName: string                   // "edit", "bash", "read", etc.
  readonly toolUseId: string
  readonly input: unknown                     // Tool-specific input
  readonly output?: string                    // Tool output (if completed)
  readonly isError?: boolean                  // Whether tool execution failed
  readonly timestamp: Date
}

// Response metadata
export interface ResponseMetadata {
  readonly tokenUsage?: {
    readonly inputTokens: number
    readonly outputTokens: number
    readonly totalTokens: number
  }
  readonly modelUsed?: string
  readonly processingTime?: number             // Time in milliseconds
}

// Claude Code session information
export interface ClaudeCodeSession {
  readonly sessionId: string
  readonly machineId: string
  readonly userId: string
  readonly projectPath: string
  readonly projectName?: string
  readonly status: "active" | "idle" | "ended" | "error"
  readonly claudeVersion: string
  readonly startedAt: Date
  readonly lastPromptAt?: Date
  readonly lastResponseAt?: Date
  readonly messageCount: number
  readonly totalTokens: number
}

// Machine Claude Code capabilities
export interface MachineClaudeInfo {
  readonly machineId: string
  readonly hostname: string
  readonly claudeVersion: string
  readonly sdkVersion: string
  readonly supportedFeatures: ReadonlyArray<string>  // ["file_edit", "command_exec", "git_ops"]
  readonly activeProjects: ReadonlyArray<string>
  readonly activeSessions: ReadonlyArray<ClaudeCodeSession>
  readonly lastHeartbeat: Date
  readonly status: "online" | "offline" | "busy"
}

// Remote prompt tracking
export interface RemotePrompt {
  readonly promptId: string
  readonly sessionId: string
  readonly machineId: string
  readonly userId: string
  readonly promptText: string
  readonly promptOptions?: ClaudeCodeOptions
  readonly status: "sent" | "processing" | "completed" | "failed" | "cancelled"
  readonly sentAt: Date
  readonly completedAt?: Date
  readonly responseMessageCount?: number
  readonly tokenUsage?: ResponseMetadata["tokenUsage"]
}

// Security and audit events
export interface ClaudeCodeAuditEvent {
  readonly eventId: string
  readonly machineId: string
  readonly userId: string
  readonly sessionId: string
  readonly eventType: "prompt_sent" | "session_started" | "session_ended" | "error" | "rate_limit"
  readonly severity: "low" | "medium" | "high" | "critical"
  readonly details: unknown
  readonly timestamp: Date
}

// Enhanced WebSocket message types for Claude Code control
export interface ClaudeCodeControlMessage {
  readonly type: "claude_command" | "claude_response" | "claude_status" | "claude_heartbeat"
  readonly messageId: string
  readonly machineId: string
  readonly userId: string
  readonly timestamp: Date
  readonly data: ClaudeCodeCommand | ClaudeCodeResponse | MachineClaudeInfo
}

// Service configuration
export interface ClaudeCodeServiceConfig {
  readonly enableRemoteControl: boolean
  readonly maxConcurrentSessions: number
  readonly sessionTimeoutMs: number
  readonly promptRateLimit: number            // Max prompts per minute
  readonly auditLogging: boolean
  readonly allowedProjects?: ReadonlyArray<string>  // Project path restrictions
}