import { Effect, Layer } from "effect"
import type { AgentState, CodeSnippet, ExternalReference, FileFocus } from "./AgentStateTypes.js"

/**
 * Service for managing the execution context within the agent state
 */
export interface ContextManager {
  /**
   * Sets the current file focus
   * @param state Current agent state
   * @param filePath Path to the file to focus on
   * @param relevantLines Array of line numbers that are relevant
   * @returns Updated agent state with the new file focus
   */
  readonly setFileFocus: (
    state: AgentState,
    filePath: string,
    relevantLines: ReadonlyArray<number>
  ) => Effect.Effect<AgentState>

  /**
   * Adds a code snippet to the list of relevant code snippets
   * @param state Current agent state
   * @param filePath Path to the file containing the snippet
   * @param snippet The code snippet text
   * @param reason The reason this snippet is relevant
   * @returns Updated agent state with the code snippet added
   */
  readonly addCodeSnippet: (
    state: AgentState,
    filePath: string,
    snippet: string,
    reason: string
  ) => Effect.Effect<AgentState>

  /**
   * Adds an external reference (like an issue or PR) to the execution context
   * @param state Current agent state
   * @param type Type of reference (e.g., "issue", "pr")
   * @param identifier Identifier of the reference (e.g., "123")
   * @param relationship How this reference relates to the current task (e.g., "relates_to", "blocked_by")
   * @param source Where this reference is from (e.g., "github")
   * @returns Updated agent state with the external reference added
   */
  readonly addExternalReference: (
    state: AgentState,
    type: string,
    identifier: string,
    relationship: string,
    source: string
  ) => Effect.Effect<AgentState>

  /**
   * Adds a file to the list of files modified in the current session
   * @param state Current agent state
   * @param filePath Path to the file that was modified
   * @returns Updated agent state with the modified file added
   */
  readonly addModifiedFile: (
    state: AgentState,
    filePath: string
  ) => Effect.Effect<AgentState>

  /**
   * Clears the current file focus
   * @param state Current agent state
   * @returns Updated agent state with the file focus cleared
   */
  readonly clearFileFocus: (
    state: AgentState
  ) => Effect.Effect<AgentState>
}

/**
 * Effect Tag for the ContextManager service
 */
// eslint-disable-next-line @typescript-eslint/no-unsafe-declaration-merging
export class ContextManager extends Effect.Tag("ContextManager")<
  ContextManager,
  {
    setFileFocus: (
      state: AgentState,
      filePath: string,
      relevantLines: ReadonlyArray<number>
    ) => Effect.Effect<AgentState>
    
    addCodeSnippet: (
      state: AgentState,
      filePath: string,
      snippet: string,
      reason: string
    ) => Effect.Effect<AgentState>
    
    addExternalReference: (
      state: AgentState,
      type: string,
      identifier: string,
      relationship: string,
      source: string
    ) => Effect.Effect<AgentState>
    
    addModifiedFile: (
      state: AgentState,
      filePath: string
    ) => Effect.Effect<AgentState>
    
    clearFileFocus: (
      state: AgentState
    ) => Effect.Effect<AgentState>
  }
>() {}

/**
 * Implementation of the ContextManager service
 */
const contextManagerImpl = {
  setFileFocus: (
    state: AgentState,
    filePath: string,
    relevantLines: ReadonlyArray<number>
  ) => Effect.sync(() => {
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

  addCodeSnippet: (
    state: AgentState,
    filePath: string,
    snippet: string,
    reason: string
  ) => Effect.sync(() => {
    const newSnippet: CodeSnippet = {
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

  addExternalReference: (
    state: AgentState,
    type: string,
    identifier: string,
    relationship: string,
    source: string
  ) => Effect.sync(() => {
    const newReference: ExternalReference = {
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

  addModifiedFile: (
    state: AgentState,
    filePath: string
  ) => Effect.sync(() => {
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

  clearFileFocus: (
    state: AgentState
  ) => Effect.sync(() => ({
    ...state,
    execution_context: {
      ...state.execution_context,
      current_file_focus: null
    }
  }))
}

/**
 * Layer that provides the ContextManager implementation
 */
export const ContextManagerLayer = Layer.succeed(
  ContextManager,
  ContextManager.of(contextManagerImpl)
)

// Alias for consistency
export const ContextManagerLive = ContextManagerLayer