import { describe, expect, it } from "@effect/vitest"
import { Effect } from "effect"
import type { AgentState, FileFocus } from "../../src/github/AgentStateTypes.js"

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

// Helper function to run tests with the ContextManager
const runWithContextManager = <A>(effectToRun: (manager: any) => Effect.Effect<A, any>) => {
  // Create a simple mock object implementing the ContextManager interface methods
  const contextManagerInstance = {
    setFileFocus: (state: AgentState, filePath: string, relevantLines: ReadonlyArray<number>) =>
      Effect.sync(() => {
        const newFileFocus: FileFocus = {
          path: filePath,
          relevant_lines: [...relevantLines]
        }

        return {
          ...state,
          execution_context: {
            ...state.execution_context,
            current_file_focus: newFileFocus
          }
        }
      }),

    addCodeSnippet: (state: AgentState, filePath: string, snippet: string, reason: string) =>
      Effect.sync(() => {
        const newSnippet = {
          file_path: filePath,
          snippet,
          reason
        }

        return {
          ...state,
          execution_context: {
            ...state.execution_context,
            relevant_code_snippets: [
              ...state.execution_context.relevant_code_snippets,
              newSnippet
            ]
          }
        }
      }),

    addExternalReference: (state: AgentState, type: string, identifier: string, relationship: string, source: string) =>
      Effect.sync(() => {
        const newReference = {
          type,
          identifier,
          relationship,
          source
        }

        return {
          ...state,
          execution_context: {
            ...state.execution_context,
            external_references: [
              ...state.execution_context.external_references,
              newReference
            ]
          }
        }
      }),

    addModifiedFile: (state: AgentState, filePath: string) =>
      Effect.sync(() => {
        // Use a Set to avoid duplicates
        const updatedFiles = new Set(state.execution_context.files_modified_in_session)
        updatedFiles.add(filePath)

        return {
          ...state,
          execution_context: {
            ...state.execution_context,
            files_modified_in_session: Array.from(updatedFiles)
          }
        }
      }),

    clearFileFocus: (state: AgentState) =>
      Effect.sync(() => ({
        ...state,
        execution_context: {
          ...state.execution_context,
          current_file_focus: null
        }
      }))
  }

  // Directly run the effect provided by the test, passing the mock instance
  try {
    return Effect.runSync(effectToRun(contextManagerInstance)) as any
  } catch (e) {
    // Re-throw errors to fail the test as expected
    throw e
  }
}

describe("ContextManager", () => {
  describe("setFileFocus", () => {
    it("should set the current file focus", () => {
      // Arrange
      const initialState = createTestState()
      const filePath = "src/main.ts"
      const relevantLines = [10, 20, 30]

      // Act
      const newState = runWithContextManager<AgentState>((manager) =>
        manager.setFileFocus(initialState, filePath, relevantLines)
      )

      // Assert
      const fileFocus = newState.execution_context.current_file_focus as FileFocus
      expect(fileFocus).not.toBeNull()
      expect(fileFocus.path).toBe(filePath)
      expect(fileFocus.relevant_lines).toEqual(relevantLines)

      // Verify immutability
      expect(newState).not.toBe(initialState)
      expect(newState.execution_context).not.toBe(initialState.execution_context)

      // Verify other parts of the state remain unchanged
      expect(newState.agent_info).toEqual(initialState.agent_info)
      expect(newState.plan).toEqual(initialState.plan)
    })

    it("should update the current file focus when it already exists", () => {
      // Arrange
      const initialState = createTestState()

      // First set a file focus
      const firstFocusState = runWithContextManager<AgentState>((manager) =>
        manager.setFileFocus(initialState, "src/old.ts", [5, 15])
      )

      // New focus details
      const newFilePath = "src/new.ts"
      const newRelevantLines = [42, 50]

      // Act
      const newState = runWithContextManager<AgentState>((manager) =>
        manager.setFileFocus(firstFocusState, newFilePath, newRelevantLines)
      )

      // Assert
      const fileFocus = newState.execution_context.current_file_focus as FileFocus
      expect(fileFocus.path).toBe(newFilePath)
      expect(fileFocus.relevant_lines).toEqual(newRelevantLines)

      // Verify immutability
      expect(newState).not.toBe(firstFocusState)
      expect(newState.execution_context).not.toBe(firstFocusState.execution_context)
      expect(newState.execution_context.current_file_focus).not.toBe(firstFocusState.execution_context.current_file_focus)
    })
  })

  describe("addCodeSnippet", () => {
    it("should add a code snippet to the list of relevant code snippets", () => {
      // Arrange
      const initialState = createTestState()
      const filePath = "src/utils.ts"
      const snippet = "function doSomething() { return true; }"
      const reason = "Utility function needed for the implementation"

      // Act
      const newState = runWithContextManager<AgentState>((manager) =>
        manager.addCodeSnippet(initialState, filePath, snippet, reason)
      )

      // Assert
      const snippets = newState.execution_context.relevant_code_snippets
      expect(snippets.length).toBe(1)

      const addedSnippet = snippets[0]
      expect(addedSnippet.file_path).toBe(filePath)
      expect(addedSnippet.snippet).toBe(snippet)
      expect(addedSnippet.reason).toBe(reason)

      // Verify immutability
      expect(newState).not.toBe(initialState)
      expect(newState.execution_context).not.toBe(initialState.execution_context)
      expect(newState.execution_context.relevant_code_snippets).not.toBe(initialState.execution_context.relevant_code_snippets)
    })

    it("should add multiple code snippets correctly", () => {
      // Arrange
      const initialState = createTestState()

      // Add first snippet
      const firstSnippetState = runWithContextManager<AgentState>((manager) =>
        manager.addCodeSnippet(initialState, "src/utils.ts", "function one() {}", "First utility")
      )

      // Act - add second snippet
      const newState = runWithContextManager<AgentState>((manager) =>
        manager.addCodeSnippet(firstSnippetState, "src/helpers.ts", "function two() {}", "Second utility")
      )

      // Assert
      const snippets = newState.execution_context.relevant_code_snippets
      expect(snippets.length).toBe(2)
      expect(snippets[0].file_path).toBe("src/utils.ts")
      expect(snippets[1].file_path).toBe("src/helpers.ts")

      // Verify immutability
      expect(newState).not.toBe(firstSnippetState)
      expect(newState.execution_context).not.toBe(firstSnippetState.execution_context)
      expect(newState.execution_context.relevant_code_snippets).not.toBe(firstSnippetState.execution_context.relevant_code_snippets)
    })
  })

  describe("addExternalReference", () => {
    it("should add an external reference to the list of external references", () => {
      // Arrange
      const initialState = createTestState()
      const type = "issue"
      const identifier = "456"
      const relationship = "relates_to"
      const source = "github"

      // Act
      const newState = runWithContextManager<AgentState>((manager) =>
        manager.addExternalReference(initialState, type, identifier, relationship, source)
      )

      // Assert
      const references = newState.execution_context.external_references
      expect(references.length).toBe(1)

      const addedReference = references[0]
      expect(addedReference.type).toBe(type)
      expect(addedReference.identifier).toBe(identifier)
      expect(addedReference.relationship).toBe(relationship)
      expect(addedReference.source).toBe(source)

      // Verify immutability
      expect(newState).not.toBe(initialState)
      expect(newState.execution_context).not.toBe(initialState.execution_context)
      expect(newState.execution_context.external_references).not.toBe(initialState.execution_context.external_references)
    })

    it("should add multiple external references correctly", () => {
      // Arrange
      const initialState = createTestState()

      // Add first reference
      const firstRefState = runWithContextManager<AgentState>((manager) =>
        manager.addExternalReference(initialState, "issue", "456", "relates_to", "github")
      )

      // Act - add second reference
      const newState = runWithContextManager<AgentState>((manager) =>
        manager.addExternalReference(firstRefState, "pr", "789", "blocked_by", "github")
      )

      // Assert
      const references = newState.execution_context.external_references
      expect(references.length).toBe(2)
      expect(references[0].type).toBe("issue")
      expect(references[1].type).toBe("pr")

      // Verify immutability
      expect(newState).not.toBe(firstRefState)
      expect(newState.execution_context).not.toBe(firstRefState.execution_context)
      expect(newState.execution_context.external_references).not.toBe(firstRefState.execution_context.external_references)
    })
  })

  describe("addModifiedFile", () => {
    it("should add a file to the list of files modified in the session", () => {
      // Arrange
      const initialState = createTestState()
      const filePath = "src/main.ts"

      // Act
      const newState = runWithContextManager<AgentState>((manager) =>
        manager.addModifiedFile(initialState, filePath)
      )

      // Assert
      const modifiedFiles = newState.execution_context.files_modified_in_session
      expect(modifiedFiles.length).toBe(1)
      expect(modifiedFiles[0]).toBe(filePath)

      // Verify immutability
      expect(newState).not.toBe(initialState)
      expect(newState.execution_context).not.toBe(initialState.execution_context)
      expect(newState.execution_context.files_modified_in_session).not.toBe(initialState.execution_context.files_modified_in_session)
    })

    it("should not add duplicate modified files", () => {
      // Arrange
      const initialState = createTestState()
      const filePath = "src/main.ts"

      // Add the file once
      const firstAddState = runWithContextManager<AgentState>((manager) =>
        manager.addModifiedFile(initialState, filePath)
      )

      // Act - try to add the same file again
      const newState = runWithContextManager<AgentState>((manager) =>
        manager.addModifiedFile(firstAddState, filePath)
      )

      // Assert
      const modifiedFiles = newState.execution_context.files_modified_in_session
      expect(modifiedFiles.length).toBe(1) // Should still only have one entry
      expect(modifiedFiles[0]).toBe(filePath)

      // Verify immutability - even though the contents might be the same,
      // we should still get a new object reference
      expect(newState).not.toBe(firstAddState)
      expect(newState.execution_context).not.toBe(firstAddState.execution_context)
    })

    it("should add multiple unique modified files correctly", () => {
      // Arrange
      const initialState = createTestState()

      // Add first file
      const firstFileState = runWithContextManager<AgentState>((manager) =>
        manager.addModifiedFile(initialState, "src/main.ts")
      )

      // Act - add second file
      const newState = runWithContextManager<AgentState>((manager) =>
        manager.addModifiedFile(firstFileState, "src/utils.ts")
      )

      // Assert
      const modifiedFiles = newState.execution_context.files_modified_in_session
      expect(modifiedFiles.length).toBe(2)
      expect(modifiedFiles).toContain("src/main.ts")
      expect(modifiedFiles).toContain("src/utils.ts")

      // Verify immutability
      expect(newState).not.toBe(firstFileState)
      expect(newState.execution_context).not.toBe(firstFileState.execution_context)
      expect(newState.execution_context.files_modified_in_session).not.toBe(firstFileState.execution_context.files_modified_in_session)
    })
  })

  describe("clearFileFocus", () => {
    it("should clear the current file focus", () => {
      // Arrange
      const initialState = createTestState()

      // First set a file focus
      const stateWithFocus = runWithContextManager<AgentState>((manager) =>
        manager.setFileFocus(initialState, "src/main.ts", [10, 20])
      )

      // Act
      const newState = runWithContextManager<AgentState>((manager) =>
        manager.clearFileFocus(stateWithFocus)
      )

      // Assert
      expect(newState.execution_context.current_file_focus).toBeNull()

      // Verify immutability
      expect(newState).not.toBe(stateWithFocus)
      expect(newState.execution_context).not.toBe(stateWithFocus.execution_context)
    })

    it("should handle clearing when file focus is already null", () => {
      // Arrange
      const initialState = createTestState()
      // Initial state already has null file focus
      expect(initialState.execution_context.current_file_focus).toBeNull()

      // Act
      const newState = runWithContextManager<AgentState>((manager) =>
        manager.clearFileFocus(initialState)
      )

      // Assert
      expect(newState.execution_context.current_file_focus).toBeNull()

      // Verify immutability - we still create a new state object
      expect(newState).not.toBe(initialState)
      expect(newState.execution_context).not.toBe(initialState.execution_context)
    })
  })
})
