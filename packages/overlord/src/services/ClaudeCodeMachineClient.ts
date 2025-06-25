/**
 * Machine-side client for Claude Code WebSocket server
 * Connects local Claude Code instances to the remote control server
 */
import { Context, Data, Effect, Layer, Queue, Ref, Schedule, Stream } from "effect"
import { WebSocket } from "ws"
import * as fs from "node:fs/promises"
import * as os from "node:os"
import * as path from "node:path"
import { spawn } from "node:child_process"
import type {
  ClaudeCodeCommand,
  ClaudeCodeResponse,
  ClaudeCodeSession,
  MachineClaudeInfo
} from "../types/ClaudeCodeTypes.js"

// Machine WebSocket message types
type MachineMessage = 
  | {
      type: "register"
      machineId: string
      machineInfo: {
        hostname: string
        claudeVersion: string
        sdkVersion: string
        supportedFeatures: string[]
        activeProjects: string[]
      }
    }
  | {
      type: "heartbeat"
      machineId: string
      sessions: Array<{
        sessionId: string
        status: string
        messageCount: number
      }>
    }
  | {
      type: "response"
      machineId: string
      response: ClaudeCodeResponse
    }
  | {
      type: "error"
      machineId: string
      error: string
      sessionId?: string
    }

// Errors
export class MachineClientError extends Data.TaggedError("MachineClientError")<{
  message: string
  cause?: unknown
}> {}

export class ClaudeCodeNotFoundError extends Data.TaggedError("ClaudeCodeNotFoundError")<{
  message: string
}> {}

export class ClaudeCodeExecutionError extends Data.TaggedError("ClaudeCodeExecutionError")<{
  message: string
  sessionId?: string
  cause?: unknown
}> {}

// Configuration
export interface MachineClientConfig {
  readonly serverUrl: string
  readonly machineId: string
  readonly apiKey: string
  readonly claudeCodePath?: string // Path to Claude Code executable
  readonly heartbeatInterval?: number // Default: 30000 (30 seconds)
  readonly reconnectDelay?: number // Default: 5000 (5 seconds)
  readonly maxReconnectAttempts?: number // Default: 10
}

export const MachineClientConfig = Context.GenericTag<MachineClientConfig>(
  "@openagentsinc/overlord/MachineClientConfig"
)

// Service interface
export interface ClaudeCodeMachineClient {
  readonly connect: () => Effect.Effect<void, MachineClientError | ClaudeCodeNotFoundError>
  readonly disconnect: () => Effect.Effect<void>
  readonly getStatus: () => Effect.Effect<{
    connected: boolean
    machineInfo: MachineClaudeInfo
    activeSessions: ReadonlyArray<ClaudeCodeSession>
  }>
  readonly executeCommand: (command: ClaudeCodeCommand) => Effect.Effect<void, ClaudeCodeExecutionError>
}

export const ClaudeCodeMachineClient = Context.GenericTag<ClaudeCodeMachineClient>(
  "@openagentsinc/overlord/ClaudeCodeMachineClient"
)

// Implementation
export const ClaudeCodeMachineClientLive = Layer.effect(
  ClaudeCodeMachineClient,
  Effect.gen(function*() {
    const config = yield* MachineClientConfig
    
    // State
    const ws = yield* Ref.make<WebSocket | null>(null)
    const connected = yield* Ref.make(false)
    const reconnectAttempts = yield* Ref.make(0)
    const activeSessions = yield* Ref.make(new Map<string, ClaudeCodeSession>())
    const commandQueue = yield* Queue.unbounded<ClaudeCodeCommand>()
    // Response queue not used yet, will be needed for real Claude Code integration
    
    // Claude Code process management
    const claudeProcesses = yield* Ref.make(new Map<string, any>()) // sessionId -> ChildProcess
    
    // Detect Claude Code installation
    const detectClaudeCode = () => Effect.gen(function*() {
      // Check provided path first
      const providedPath = config.claudeCodePath
      if (providedPath) {
        try {
          yield* Effect.tryPromise(() => fs.access(providedPath))
          return providedPath
        } catch {
          // Continue to auto-detection
        }
      }
      
      // Common installation paths
      const possiblePaths = [
        "/usr/local/bin/claude",
        "/opt/claude/claude",
        path.join(os.homedir(), ".local/bin/claude"),
        "claude" // In PATH
      ]
      
      for (const claudePath of possiblePaths) {
        try {
          // Try to execute with --version
          yield* Effect.tryPromise(() => 
            new Promise((resolve, reject) => {
              const child = spawn(claudePath, ["--version"], { 
                timeout: 5000,
                stdio: ["ignore", "pipe", "pipe"]
              })
              
              child.on("error", reject)
              child.on("exit", (code) => {
                if (code === 0) resolve(true)
                else reject(new Error(`Exit code ${code}`))
              })
            })
          )
          
          return claudePath
        } catch {
          // Try next path
        }
      }
      
      yield* Effect.fail(new ClaudeCodeNotFoundError({
        message: "Claude Code executable not found. Please install Claude Code or specify the path."
      }))
    })
    
    // Get machine information
    const getMachineInfo = () => Effect.gen(function*() {
      const claudePath = yield* detectClaudeCode()
      
      // Get Claude version
      const claudeVersion = yield* Effect.tryPromise({
        try: () => new Promise<string>((resolve) => {
          const child = spawn(claudePath, ["--version"], { stdio: ["ignore", "pipe", "pipe"] })
          let output = ""
          
          child.stdout?.on("data", (data) => { output += data.toString() })
          child.on("exit", () => { 
            resolve(output.trim() || "unknown") 
          })
          
          setTimeout(() => {
            child.kill()
            resolve("unknown")
          }, 5000)
        }),
        catch: () => new MachineClientError({ message: "Failed to get Claude version" })
      })
      
      const info: MachineClaudeInfo = {
        machineId: config.machineId,
        hostname: os.hostname(),
        claudeVersion,
        sdkVersion: "1.0.0", // TODO: Get from actual SDK
        supportedFeatures: ["file_edit", "command_exec", "git_ops", "web_fetch"],
        activeProjects: [], // Will be populated from active sessions
        activeSessions: [],
        lastHeartbeat: new Date(),
        status: "online"
      }
      
      return info
    })
    
    // Send message to server
    const sendMessage = (message: MachineMessage) => Effect.gen(function*() {
      const socket = yield* Ref.get(ws)
      if (!socket || socket.readyState !== WebSocket.OPEN) {
        yield* Effect.fail(new MachineClientError({ message: "WebSocket not connected" }))
      } else {
        socket.send(JSON.stringify(message))
      }
    })
    
    // Start Claude Code session
    const startClaudeSession = (command: ClaudeCodeCommand) => Effect.gen(function*() {
      if (command.type !== "start_session") {
        yield* Effect.fail(new ClaudeCodeExecutionError({ 
          message: "Invalid command type for session start" 
        }))
        return
      }
      
      const claudePath = yield* detectClaudeCode()
      const projectPath = command.data.projectPath
      
      if (!projectPath) {
        yield* Effect.fail(new ClaudeCodeExecutionError({ 
          message: "Project path required for session start" 
        }))
        return
      }
      
      // Verify project path exists
      yield* Effect.tryPromise({
        try: () => fs.access(projectPath),
        catch: () => new ClaudeCodeExecutionError({ 
          message: `Project path not found: ${projectPath}` 
        })
      })
      
      // Create Claude Code session
      const session: ClaudeCodeSession = {
        sessionId: command.sessionId || `session_${Date.now()}`,
        machineId: config.machineId,
        userId: command.userId,
        projectPath,
        projectName: path.basename(projectPath),
        status: "active",
        claudeVersion: yield* getMachineInfo().pipe(Effect.map(info => info.claudeVersion)),
        startedAt: new Date(),
        messageCount: 0,
        totalTokens: 0
      }
      
      // Update active sessions
      yield* Ref.update(activeSessions, (sessions) => {
        const newSessions = new Map(sessions)
        newSessions.set(session.sessionId, session)
        return newSessions
      })
      
      // Send response
      const response: ClaudeCodeResponse = {
        type: "session_started",
        commandId: command.commandId,
        sessionId: session.sessionId,
        machineId: config.machineId,
        timestamp: new Date(),
        data: {
          status: "complete",
          content: `Session started: ${session.sessionId} for project ${session.projectPath}`
        }
      }
      
      yield* sendMessage({
        type: "response",
        machineId: config.machineId,
        response
      })
    })
    
    // Send prompt to Claude Code
    const sendPromptToClaudeCode = (command: ClaudeCodeCommand) => Effect.gen(function*() {
      if (command.type !== "send_prompt") {
        yield* Effect.fail(new ClaudeCodeExecutionError({ 
          message: "Invalid command type for prompt" 
        }))
        return
      }
      
      const sessionId = command.sessionId
      if (!sessionId) {
        yield* Effect.fail(new ClaudeCodeExecutionError({ 
          message: "Session ID required for prompt" 
        }))
        return
      }
      
      const sessions = yield* Ref.get(activeSessions)
      const session = sessions.get(sessionId)
      
      if (!session) {
        yield* Effect.fail(new ClaudeCodeExecutionError({ 
          message: `Session not found: ${sessionId}`,
          sessionId 
        }))
        return
      }
      
      const prompt = command.data.prompt
      if (!prompt) {
        yield* Effect.fail(new ClaudeCodeExecutionError({ 
          message: "Prompt text required",
          sessionId 
        }))
        return
      }
      
      // TODO: Integrate real Claude Code SDK here
      // For now, simulate the response
      yield* Effect.log(`Sending prompt to Claude Code session ${sessionId}: ${prompt}`)
      
      // Simulate thinking
      yield* sendMessage({
        type: "response",
        machineId: config.machineId,
        response: {
          type: "thinking",
          commandId: command.commandId,
          sessionId,
          machineId: config.machineId,
          timestamp: new Date(),
          data: {
            thinking: "Analyzing the request and considering the best approach..."
          }
        }
      })
      
      // Simulate response chunks
      const responseText = `I understand you want help with: "${prompt}". 
      
Here's what I would do:
1. First, I'd analyze the current project structure
2. Then implement the requested functionality
3. Finally, test the implementation

This is a simulated response. Real Claude Code integration is pending.`
      
      const words = responseText.split(" ")
      for (let i = 0; i < words.length; i += 5) {
        const chunk = words.slice(i, i + 5).join(" ")
        
        yield* sendMessage({
          type: "response",
          machineId: config.machineId,
          response: {
            type: "response_chunk",
            commandId: command.commandId,
            sessionId,
            machineId: config.machineId,
            timestamp: new Date(),
            data: {
              content: chunk + " ",
              status: i + 5 >= words.length ? "complete" : "thinking"
            }
          }
        })
        
        // Small delay between chunks
        yield* Effect.sleep("50 millis")
      }
      
      // Update session stats
      yield* Ref.update(activeSessions, (sessions) => {
        const newSessions = new Map(sessions)
        const session = newSessions.get(sessionId)
        if (session) {
          newSessions.set(sessionId, {
            ...session,
            messageCount: session.messageCount + 2, // Prompt + response
            totalTokens: session.totalTokens + 100, // Simulated
            lastPromptAt: new Date(),
            lastResponseAt: new Date()
          })
        }
        return newSessions
      })
    })
    
    // End Claude Code session
    const endClaudeSession = (command: ClaudeCodeCommand) => Effect.gen(function*() {
      if (command.type !== "end_session") {
        yield* Effect.fail(new ClaudeCodeExecutionError({ 
          message: "Invalid command type for session end" 
        }))
        return
      }
      
      const sessionId = command.sessionId
      if (!sessionId) {
        yield* Effect.fail(new ClaudeCodeExecutionError({ 
          message: "Session ID required" 
        }))
        return
      }
      
      // Remove from active sessions
      yield* Ref.update(activeSessions, (sessions) => {
        const newSessions = new Map(sessions)
        newSessions.delete(sessionId)
        return newSessions
      })
      
      // Kill any running Claude process
      const processes = yield* Ref.get(claudeProcesses)
      const process = processes.get(sessionId)
      if (process) {
        process.kill()
        yield* Ref.update(claudeProcesses, (procs) => {
          const newProcs = new Map(procs)
          newProcs.delete(sessionId)
          return newProcs
        })
      }
      
      // Send response
      yield* sendMessage({
        type: "response",
        machineId: config.machineId,
        response: {
          type: "session_ended",
          commandId: command.commandId,
          sessionId,
          machineId: config.machineId,
          timestamp: new Date(),
          data: {
            status: "complete"
          }
        }
      })
    })
    
    // Execute command
    const executeCommand = (command: ClaudeCodeCommand) => Effect.gen(function*() {
      yield* Effect.log(`Executing command: ${command.type} (${command.commandId})`)
      
      switch (command.type) {
        case "start_session":
          yield* startClaudeSession(command)
          break
          
        case "send_prompt":
          yield* sendPromptToClaudeCode(command)
          break
          
        case "end_session":
          yield* endClaudeSession(command)
          break
          
        case "get_status":
          // Send current status
          const sessions = yield* Ref.get(activeSessions)
          yield* sendMessage({
            type: "response",
            machineId: config.machineId,
            response: {
              type: "session_started", // Reuse type
              commandId: command.commandId,
              sessionId: "status",
              machineId: config.machineId,
              timestamp: new Date(),
              data: {
                status: "complete",
                content: JSON.stringify({
                  sessions: Array.from(sessions.values())
                }, null, 2)
              }
            }
          })
          break
          
        case "switch_project":
          // TODO: Implement project switching
          yield* Effect.fail(new ClaudeCodeExecutionError({ 
            message: "Project switching not yet implemented" 
          }))
          break
          
        default:
          yield* Effect.fail(new ClaudeCodeExecutionError({ 
            message: `Unknown command type: ${command.type}` 
          }))
      }
    }).pipe(
      Effect.catchAll((error) => Effect.gen(function*() {
        // Send error response
        const errorMessage: MachineMessage = {
          type: "error",
          machineId: config.machineId,
          error: error.message
        }
        if (command.sessionId) {
          errorMessage.sessionId = command.sessionId
        }
        yield* sendMessage(errorMessage)
        
        yield* Effect.fail(error)
      }))
    )
    
    // WebSocket connection
    const connect = () => Effect.gen(function*() {
      const machineInfo = yield* getMachineInfo()
      
      const wsUrl = `${config.serverUrl}/machine`
      const headers = {
        Authorization: `Bearer machine:${config.machineId}:${config.apiKey}`
      }
      
      yield* Effect.log(`Connecting to Claude Code server: ${wsUrl}`)
      
      const socket = new WebSocket(wsUrl, { headers })
      
      socket.on("open", () => {
        Effect.runSync(Effect.gen(function*() {
          yield* Ref.set(ws, socket)
          yield* Ref.set(connected, true)
          yield* Ref.set(reconnectAttempts, 0)
          
          yield* Effect.log("Connected to Claude Code server")
          
          // Send registration
          yield* sendMessage({
            type: "register",
            machineId: config.machineId,
            machineInfo: {
              hostname: machineInfo.hostname,
              claudeVersion: machineInfo.claudeVersion,
              sdkVersion: machineInfo.sdkVersion,
              supportedFeatures: [...machineInfo.supportedFeatures],
              activeProjects: [...machineInfo.activeProjects]
            }
          })
        }))
      })
      
      socket.on("message", (data) => {
        Effect.runSync(Effect.gen(function*() {
          const message = JSON.parse(data.toString())
          
          if (message.type === "command") {
            // Queue command for processing
            yield* Queue.offer(commandQueue, message.command)
          }
        }).pipe(
          Effect.catchAll((error) => 
            Effect.log(`Error processing server message: ${error}`)
          )
        ))
      })
      
      socket.on("error", (error) => {
        Effect.runSync(Effect.log(`WebSocket error: ${error.message}`))
      })
      
      socket.on("close", () => {
        Effect.runSync(Effect.gen(function*() {
          yield* Ref.set(ws, null)
          yield* Ref.set(connected, false)
          
          yield* Effect.log("Disconnected from Claude Code server")
          
          // Attempt reconnection
          const attempts = yield* Ref.get(reconnectAttempts)
          if (attempts < (config.maxReconnectAttempts || 10)) {
            yield* Ref.update(reconnectAttempts, n => n + 1)
            yield* Effect.log(`Reconnecting... (attempt ${attempts + 1})`)
            yield* Effect.sleep(`${config.reconnectDelay || 5000} millis`)
            yield* connect()
          } else {
            yield* Effect.fail(new MachineClientError({
              message: "Max reconnection attempts reached"
            }))
          }
        }))
      })
      
      yield* Ref.set(ws, socket)
      
      // Start heartbeat
      yield* Effect.repeat(
        Effect.gen(function*() {
          const isConnected = yield* Ref.get(connected)
          if (!isConnected) return
          
          const sessions = yield* Ref.get(activeSessions)
          yield* sendMessage({
            type: "heartbeat",
            machineId: config.machineId,
            sessions: Array.from(sessions.values()).map(s => ({
              sessionId: s.sessionId,
              status: s.status,
              messageCount: s.messageCount
            }))
          })
        }),
        Schedule.fixed(`${config.heartbeatInterval || 30000} millis`)
      ).pipe(Effect.fork)
      
      // Start command processor
      yield* Stream.fromQueue(commandQueue).pipe(
        Stream.mapEffect((command) => executeCommand(command)),
        Stream.runDrain
      ).pipe(Effect.fork)
    })
    
    // Disconnect
    const disconnect = () => Effect.gen(function*() {
      const socket = yield* Ref.get(ws)
      if (socket) {
        socket.close()
        yield* Ref.set(ws, null)
        yield* Ref.set(connected, false)
      }
    })
    
    // Get status
    const getStatus = () => Effect.gen(function*() {
      const isConnected = yield* Ref.get(connected)
      const sessions = yield* Ref.get(activeSessions)
      const machineInfo = yield* getMachineInfo()
      
      return {
        connected: isConnected,
        machineInfo: {
          ...machineInfo,
          activeSessions: Array.from(sessions.values())
        },
        activeSessions: Array.from(sessions.values())
      }
    })
    
    return {
      connect,
      disconnect,
      getStatus,
      executeCommand
    }
  })
)