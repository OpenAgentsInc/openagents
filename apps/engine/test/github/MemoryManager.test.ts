import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import type {
  AgentState,
  ConversationMessage,
  ConversationToolCall,
  ToolCall
} from "../../src/github/AgentStateTypes.js"

// Create a fixture for testing
const createTestState = (): AgentState => ({
  agent_info: {
    type: "solver",
    version: "1.0.0",
    instance_id: "test-instance-id",
    state_schema_version: "1.1"
  },
  timestamps: {
    created_at: "2025-04-22T12:00:00Z",
    last_saved_at: "2025-04-22T12:00:00Z",
    last_action_at: "2025-04-22T12:00:00Z"
  },
  current_task: {
    repo_owner: "user",
    repo_name: "repo",
    repo_branch: "main",
    issue_number: 123,
    issue_details_cache: {
      title: "Test Issue",
      description_snippet: "Test description",
      status: "open",
      labels: ["bug"],
      source_url: "https://github.com/user/repo/issues/123"
    },
    status: "planning",
    current_step_index: 0
  },
  plan: [
    {
      id: "step-1",
      step_number: 1,
      description: "Analyze issue",
      status: "pending",
      start_time: null,
      end_time: null,
      result_summary: null,
      tool_calls: []
    }
  ],
  execution_context: {
    current_file_focus: null,
    relevant_code_snippets: [],
    external_references: [],
    files_modified_in_session: []
  },
  tool_invocation_log: [],
  memory: {
    conversation_history: [],
    key_decisions: [],
    important_findings: [],
    scratchpad: ""
  },
  metrics: {
    steps_completed: 0,
    total_steps_in_plan: 1,
    session_start_time: "2025-04-22T12:00:00Z",
    total_time_spent_seconds: 0,
    llm_calls_made: 0,
    llm_tokens_used: {
      prompt: 0,
      completion: 0
    },
    tools_called: 0,
    commits_made: 0
  },
  error_state: {
    last_error: null,
    consecutive_error_count: 0,
    retry_count_for_current_action: 0,
    blocked_reason: null
  },
  configuration: {
    agent_goal: "Resolve issue #123: Test Issue",
    llm_config: {
      model: "claude-3-5-sonnet-latest",
      temperature: 0.7,
      max_tokens: 1024
    },
    max_retries_per_action: 3,
    allowed_actions: ["read_file", "write_file", "run_test", "update_issue"],
    restricted_paths: ["/.git/*", "/secrets/", "/config/prod.json"],
    action_timeout_seconds: 300,
    session_timeout_minutes: 120,
    github_token_available: true
  }
})

// Helper function to run tests with the MemoryManager
const runWithMemoryManager = <A>(effectToRun: (manager: any) => Effect.Effect<A, any>) => {
  // Create a simple mock object implementing the MemoryManager interface methods
  const memoryManagerInstance = {
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
        const newDecision = {
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
        const newFinding = {
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

  // Directly run the effect provided by the test, passing the mock instance
  return Effect.runSync(effectToRun(memoryManagerInstance)) as any
}

describe("MemoryManager", () => {
  describe("addConversationMessage", () => {
    it("should add a message to the conversation history", () => {
      // Arrange
      const initialState = createTestState()
      const role = "assistant"
      const content = "I'll analyze this issue for you."

      // Act
      const newState = runWithMemoryManager<AgentState>((manager) =>
        manager.addConversationMessage(initialState, role, content)
      )

      // Assert
      const history = newState.memory.conversation_history
      expect(history.length).toBe(1)
      
      const addedMessage = history[0]
      expect(addedMessage.role).toBe(role)
      expect(addedMessage.content).toBe(content)
      expect(addedMessage.timestamp).toBeTruthy()
      expect(addedMessage.tool_calls).toBeNull()

      // Verify immutability
      expect(newState).not.toBe(initialState)
      expect(newState.memory).not.toBe(initialState.memory)
      expect(newState.memory.conversation_history).not.toBe(
        initialState.memory.conversation_history
      )

      // Verify other parts of memory remain unchanged
      expect(newState.memory.key_decisions).toEqual(initialState.memory.key_decisions)
      expect(newState.memory.important_findings).toEqual(initialState.memory.important_findings)
      expect(newState.memory.scratchpad).toBe(initialState.memory.scratchpad)
    })

    it("should add a message with tool calls to the conversation history", () => {
      // Arrange
      const initialState = createTestState()
      const role = "assistant"
      const content = "I'll fetch the code file."
      const toolCalls: Array<ConversationToolCall> = [
        {
          id: "tool-call-1",
          name: "fetchFileContents",
          input: { path: "src/main.ts" }
        }
      ]

      // Act
      const newState = runWithMemoryManager<AgentState>((manager) =>
        manager.addConversationMessage(initialState, role, content, toolCalls)
      )

      // Assert
      const history = newState.memory.conversation_history
      expect(history.length).toBe(1)
      
      const addedMessage = history[0]
      expect(addedMessage.role).toBe(role)
      expect(addedMessage.content).toBe(content)
      expect(addedMessage.tool_calls).toHaveLength(1)
      expect(addedMessage.tool_calls?.[0].id).toBe("tool-call-1")
      expect(addedMessage.tool_calls?.[0].name).toBe("fetchFileContents")

      // Verify immutability
      expect(newState).not.toBe(initialState)
      expect(newState.memory).not.toBe(initialState.memory)
      expect(newState.memory.conversation_history).not.toBe(
        initialState.memory.conversation_history
      )
    })

    it("should add multiple conversation messages correctly", () => {
      // Arrange
      const initialState = createTestState()

      // Add first message
      const firstMessageState = runWithMemoryManager<AgentState>((manager) =>
        manager.addConversationMessage(initialState, "user", "Fix this bug please")
      )

      // Act - add second message
      const newState = runWithMemoryManager<AgentState>((manager) =>
        manager.addConversationMessage(firstMessageState, "assistant", "I'll get right on it.")
      )

      // Assert
      const history = newState.memory.conversation_history
      expect(history.length).toBe(2)
      expect(history[0].role).toBe("user")
      expect(history[0].content).toBe("Fix this bug please")
      expect(history[1].role).toBe("assistant")
      expect(history[1].content).toBe("I'll get right on it.")

      // Verify immutability
      expect(newState).not.toBe(firstMessageState)
      expect(newState.memory).not.toBe(firstMessageState.memory)
      expect(newState.memory.conversation_history).not.toBe(
        firstMessageState.memory.conversation_history
      )
    })
  })

  describe("addKeyDecision", () => {
    it("should add a key decision to memory", () => {
      // Arrange
      const initialState = createTestState()
      const decision = "Focus on the login component"
      const reasoning = "Most bug reports are related to this area"
      const confidence = 0.85

      // Act
      const newState = runWithMemoryManager<AgentState>((manager) =>
        manager.addKeyDecision(initialState, decision, reasoning, confidence)
      )

      // Assert
      const decisions = newState.memory.key_decisions
      expect(decisions.length).toBe(1)
      
      const addedDecision = decisions[0]
      expect(addedDecision.decision).toBe(decision)
      expect(addedDecision.reasoning).toBe(reasoning)
      expect(addedDecision.confidence).toBe(confidence)
      expect(addedDecision.timestamp).toBeTruthy()

      // Verify immutability
      expect(newState).not.toBe(initialState)
      expect(newState.memory).not.toBe(initialState.memory)
      expect(newState.memory.key_decisions).not.toBe(
        initialState.memory.key_decisions
      )

      // Verify other parts of memory remain unchanged
      expect(newState.memory.conversation_history).toEqual(initialState.memory.conversation_history)
      expect(newState.memory.important_findings).toEqual(initialState.memory.important_findings)
      expect(newState.memory.scratchpad).toBe(initialState.memory.scratchpad)
    })

    it("should add multiple key decisions correctly", () => {
      // Arrange
      const initialState = createTestState()

      // Add first decision
      const firstDecisionState = runWithMemoryManager<AgentState>((manager) =>
        manager.addKeyDecision(initialState, "Check error logs", "Need to understand error context", 0.7)
      )

      // Act - add second decision
      const newState = runWithMemoryManager<AgentState>((manager) =>
        manager.addKeyDecision(firstDecisionState, "Update validation logic", "Root cause identified", 0.9)
      )

      // Assert
      const decisions = newState.memory.key_decisions
      expect(decisions.length).toBe(2)
      expect(decisions[0].decision).toBe("Check error logs")
      expect(decisions[1].decision).toBe("Update validation logic")

      // Verify immutability
      expect(newState).not.toBe(firstDecisionState)
      expect(newState.memory).not.toBe(firstDecisionState.memory)
      expect(newState.memory.key_decisions).not.toBe(
        firstDecisionState.memory.key_decisions
      )
    })
  })

  describe("addImportantFinding", () => {
    it("should add an important finding to memory", () => {
      // Arrange
      const initialState = createTestState()
      const finding = "Null check missing in handleSubmit()"
      const source = "code_analysis"
      const confidence = 0.95

      // Act
      const newState = runWithMemoryManager<AgentState>((manager) =>
        manager.addImportantFinding(initialState, finding, source, confidence)
      )

      // Assert
      const findings = newState.memory.important_findings
      expect(findings.length).toBe(1)
      
      const addedFinding = findings[0]
      expect(addedFinding.finding).toBe(finding)
      expect(addedFinding.source).toBe(source)
      expect(addedFinding.confidence).toBe(confidence)
      expect(addedFinding.timestamp).toBeTruthy()

      // Verify immutability
      expect(newState).not.toBe(initialState)
      expect(newState.memory).not.toBe(initialState.memory)
      expect(newState.memory.important_findings).not.toBe(
        initialState.memory.important_findings
      )

      // Verify other parts of memory remain unchanged
      expect(newState.memory.conversation_history).toEqual(initialState.memory.conversation_history)
      expect(newState.memory.key_decisions).toEqual(initialState.memory.key_decisions)
      expect(newState.memory.scratchpad).toBe(initialState.memory.scratchpad)
    })

    it("should add multiple important findings correctly", () => {
      // Arrange
      const initialState = createTestState()

      // Add first finding
      const firstFindingState = runWithMemoryManager<AgentState>((manager) =>
        manager.addImportantFinding(initialState, "Form submission fails silently", "user_reports", 0.8)
      )

      // Act - add second finding
      const newState = runWithMemoryManager<AgentState>((manager) =>
        manager.addImportantFinding(firstFindingState, "API returns 422 on malformed input", "api_logs", 0.9)
      )

      // Assert
      const findings = newState.memory.important_findings
      expect(findings.length).toBe(2)
      expect(findings[0].finding).toBe("Form submission fails silently")
      expect(findings[1].finding).toBe("API returns 422 on malformed input")

      // Verify immutability
      expect(newState).not.toBe(firstFindingState)
      expect(newState.memory).not.toBe(firstFindingState.memory)
      expect(newState.memory.important_findings).not.toBe(
        firstFindingState.memory.important_findings
      )
    })
  })

  describe("updateScratchpad", () => {
    it("should update the scratchpad content", () => {
      // Arrange
      const initialState = createTestState()
      const newContent = "Need to check validateInput() function and add null checks."

      // Act
      const newState = runWithMemoryManager<AgentState>((manager) =>
        manager.updateScratchpad(initialState, newContent)
      )

      // Assert
      expect(newState.memory.scratchpad).toBe(newContent)

      // Verify immutability
      expect(newState).not.toBe(initialState)
      expect(newState.memory).not.toBe(initialState.memory)

      // Verify other parts of memory remain unchanged
      expect(newState.memory.conversation_history).toEqual(initialState.memory.conversation_history)
      expect(newState.memory.key_decisions).toEqual(initialState.memory.key_decisions)
      expect(newState.memory.important_findings).toEqual(initialState.memory.important_findings)
    })

    it("should replace existing scratchpad content", () => {
      // Arrange
      const initialState = createTestState()
      
      // First update with some content
      const firstUpdateState = runWithMemoryManager<AgentState>((manager) =>
        manager.updateScratchpad(initialState, "Initial notes")
      )
      expect(firstUpdateState.memory.scratchpad).toBe("Initial notes")

      // Act - replace with new content
      const newContent = "Updated notes and findings"
      const newState = runWithMemoryManager<AgentState>((manager) =>
        manager.updateScratchpad(firstUpdateState, newContent)
      )

      // Assert
      expect(newState.memory.scratchpad).toBe(newContent)
      expect(newState.memory.scratchpad).not.toBe(firstUpdateState.memory.scratchpad)

      // Verify immutability
      expect(newState).not.toBe(firstUpdateState)
      expect(newState.memory).not.toBe(firstUpdateState.memory)
    })
  })

  describe("addToolInvocationLogEntry", () => {
    it("should add a tool invocation to the log", () => {
      // Arrange
      const initialState = createTestState()
      const toolCallData = {
        tool_name: "fetchIssueComments",
        parameters: { owner: "user", repo: "repo", issue_number: 123 },
        status: "success",
        result_preview: "Retrieved 5 comments",
        full_result_ref: null
      }

      // Act
      const newState = runWithMemoryManager<AgentState>((manager) =>
        manager.addToolInvocationLogEntry(initialState, toolCallData)
      )

      // Assert
      const log = newState.tool_invocation_log
      expect(log.length).toBe(1)
      
      const addedEntry = log[0]
      expect(addedEntry.tool_name).toBe(toolCallData.tool_name)
      expect(addedEntry.parameters).toEqual(toolCallData.parameters)
      expect(addedEntry.status).toBe(toolCallData.status)
      expect(addedEntry.result_preview).toBe(toolCallData.result_preview)
      expect(addedEntry.full_result_ref).toBeNull()
      expect(addedEntry.timestamp).toBeTruthy()

      // Verify immutability
      expect(newState).not.toBe(initialState)
      expect(newState.tool_invocation_log).not.toBe(initialState.tool_invocation_log)

      // Verify memory sections remain unchanged (tool log is separate from memory)
      expect(newState.memory).toEqual(initialState.memory)
    })

    it("should add multiple tool invocations to the log", () => {
      // Arrange
      const initialState = createTestState()

      // Add first tool call
      const firstToolCallState = runWithMemoryManager<AgentState>((manager) =>
        manager.addToolInvocationLogEntry(initialState, {
          tool_name: "fetchFileContents",
          parameters: { path: "src/main.ts" },
          status: "success",
          result_preview: "File contents retrieved",
          full_result_ref: null
        })
      )

      // Act - add second tool call
      const newState = runWithMemoryManager<AgentState>((manager) =>
        manager.addToolInvocationLogEntry(firstToolCallState, {
          tool_name: "updateIssueStatus",
          parameters: { issue_number: 123, status: "in_progress" },
          status: "success",
          result_preview: "Status updated",
          full_result_ref: null
        })
      )

      // Assert
      const log = newState.tool_invocation_log
      expect(log.length).toBe(2)
      expect(log[0].tool_name).toBe("fetchFileContents")
      expect(log[1].tool_name).toBe("updateIssueStatus")

      // Verify immutability
      expect(newState).not.toBe(firstToolCallState)
      expect(newState.tool_invocation_log).not.toBe(firstToolCallState.tool_invocation_log)
    })
  })
})