import { Effect, Layer } from "effect"
import type {
  AgentState,
  ConversationMessage,
  ConversationToolCall,
  ImportantFinding,
  KeyDecision,
  ToolCall
} from "./AgentStateTypes.js"

/**
 * Service for managing the memory section and tool invocation log within the agent state
 */
// eslint-disable-next-line @typescript-eslint/no-unsafe-declaration-merging
export interface MemoryManager {
  /**
   * Adds a conversation message to the memory's conversation history
   * @param state Current agent state
   * @param role The role of the message (e.g., "user", "assistant", "tool")
   * @param content The message content
   * @param toolCalls Optional array of tool calls for this message
   * @returns Updated agent state with the new conversation message
   */
  readonly addConversationMessage: (
    state: AgentState,
    role: ConversationMessage["role"],
    content: string,
    toolCalls?: ReadonlyArray<ConversationToolCall> | null
  ) => Effect.Effect<AgentState>

  /**
   * Adds a key decision to the memory's key decisions
   * @param state Current agent state
   * @param decision The decision text
   * @param reasoning The reasoning behind the decision
   * @param confidence Confidence level (0-1)
   * @returns Updated agent state with the new key decision
   */
  readonly addKeyDecision: (
    state: AgentState,
    decision: string,
    reasoning: string,
    confidence: number
  ) => Effect.Effect<AgentState>

  /**
   * Adds an important finding to the memory's important findings
   * @param state Current agent state
   * @param finding The finding text
   * @param source The source of the finding (e.g., "code_analysis")
   * @param confidence Confidence level (0-1)
   * @returns Updated agent state with the new important finding
   */
  readonly addImportantFinding: (
    state: AgentState,
    finding: string,
    source: string,
    confidence: number
  ) => Effect.Effect<AgentState>

  /**
   * Updates the scratchpad content in memory
   * @param state Current agent state
   * @param newContent The new scratchpad content (replaces existing content)
   * @returns Updated agent state with the updated scratchpad
   */
  readonly updateScratchpad: (
    state: AgentState,
    newContent: string
  ) => Effect.Effect<AgentState>

  /**
   * Adds an entry to the top-level tool invocation log
   * @param state Current agent state
   * @param toolCallData Tool call data (timestamp will be automatically added)
   * @returns Updated agent state with the new tool invocation log entry
   */
  readonly addToolInvocationLogEntry: (
    state: AgentState,
    toolCallData: Omit<ToolCall, "timestamp">
  ) => Effect.Effect<AgentState>
}

/**
 * Effect Tag for the MemoryManager service
 */
// eslint-disable-next-line @typescript-eslint/no-unsafe-declaration-merging
export class MemoryManager extends Effect.Tag("MemoryManager")<
  MemoryManager,
  {
    addConversationMessage: (
      state: AgentState,
      role: ConversationMessage["role"],
      content: string,
      toolCalls?: ReadonlyArray<ConversationToolCall> | null
    ) => Effect.Effect<AgentState>

    addKeyDecision: (
      state: AgentState,
      decision: string,
      reasoning: string,
      confidence: number
    ) => Effect.Effect<AgentState>

    addImportantFinding: (
      state: AgentState,
      finding: string,
      source: string,
      confidence: number
    ) => Effect.Effect<AgentState>

    updateScratchpad: (
      state: AgentState,
      newContent: string
    ) => Effect.Effect<AgentState>

    addToolInvocationLogEntry: (
      state: AgentState,
      toolCallData: Omit<ToolCall, "timestamp">
    ) => Effect.Effect<AgentState>
  }
>() {}

/**
 * Implementation of the MemoryManager service
 */
const memoryManagerImpl = {
  addConversationMessage: (
    state: AgentState,
    role: ConversationMessage["role"],
    content: string,
    toolCalls: ReadonlyArray<ConversationToolCall> | null = null
  ) =>
    Effect.sync(() => {
      const newMessage: ConversationMessage = {
        role,
        content,
        timestamp: new Date().toISOString(),
        tool_calls: toolCalls ? [...toolCalls] : null
      }

      return {
        ...state,
        memory: {
          ...state.memory,
          conversation_history: [...state.memory.conversation_history, newMessage]
        }
      }
    }),

  addKeyDecision: (
    state: AgentState,
    decision: string,
    reasoning: string,
    confidence: number
  ) =>
    Effect.sync(() => {
      const newDecision: KeyDecision = {
        timestamp: new Date().toISOString(),
        decision,
        reasoning,
        confidence
      }

      return {
        ...state,
        memory: {
          ...state.memory,
          key_decisions: [...state.memory.key_decisions, newDecision]
        }
      }
    }),

  addImportantFinding: (
    state: AgentState,
    finding: string,
    source: string,
    confidence: number
  ) =>
    Effect.sync(() => {
      const newFinding: ImportantFinding = {
        timestamp: new Date().toISOString(),
        finding,
        source,
        confidence
      }

      return {
        ...state,
        memory: {
          ...state.memory,
          important_findings: [...state.memory.important_findings, newFinding]
        }
      }
    }),

  updateScratchpad: (
    state: AgentState,
    newContent: string
  ) =>
    Effect.sync(() => ({
      ...state,
      memory: {
        ...state.memory,
        scratchpad: newContent
      }
    })),

  addToolInvocationLogEntry: (
    state: AgentState,
    toolCallData: Omit<ToolCall, "timestamp">
  ) =>
    Effect.sync(() => {
      const newLogEntry: ToolCall = {
        timestamp: new Date().toISOString(),
        tool_name: toolCallData.tool_name,
        parameters: toolCallData.parameters,
        status: toolCallData.status,
        result_preview: toolCallData.result_preview,
        full_result_ref: toolCallData.full_result_ref
      }

      return {
        ...state,
        tool_invocation_log: [...state.tool_invocation_log, newLogEntry]
      }
    })
}

/**
 * Layer that provides the MemoryManager implementation
 */
export const MemoryManagerLayer = Layer.succeed(
  MemoryManager,
  MemoryManager.of(memoryManagerImpl)
)

// Alias for consistency
export const MemoryManagerLive = MemoryManagerLayer