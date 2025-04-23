import { Console, Effect, Layer, Ref, Stream } from "effect"
import type { AgentState } from "./AgentStateTypes.js"
import { GitHubClient } from "./GitHub.js"
import { GitHubTools, ToolExecutionError } from "./GitHubTools.js"
import { MemoryManager } from "./MemoryManager.js"
import { PlanManager } from "./PlanManager.js"

/**
 * Service for executing tasks and managing agent state during execution
 */
// eslint-disable-next-line @typescript-eslint/no-unsafe-declaration-merging
export interface TaskExecutor {
  /**
   * Execute the next step in the agent's plan
   * @param currentState Current agent state
   * @returns Updated agent state after step execution
   */
  readonly executeNextStep: (currentState: AgentState) => Effect.Effect<AgentState, Error, any>
}

/**
 * Effect Tag for the TaskExecutor service
 */
// eslint-disable-next-line @typescript-eslint/no-unsafe-declaration-merging
export class TaskExecutor extends Effect.Tag("TaskExecutor")<
  TaskExecutor,
  {
    executeNextStep: (currentState: AgentState) => Effect.Effect<AgentState, Error, any>
  }
>() {}

// Helper to construct a prompt from state context
const constructPromptFromState = (
  state: AgentState,
  currentStep: { description: string; step_number: number }
): string => {
  // Get recent conversation history (up to 10 messages)
  const recentMessages = state.memory.conversation_history
    .slice(-10)
    .map((msg) => `${msg.role.toUpperCase()}: ${msg.content}`)
    .join("\n\n")

  // Get key decisions and findings
  const decisions = state.memory.key_decisions
    .map((d) => `- Decision: ${d.decision} (Confidence: ${d.confidence})\n  Reasoning: ${d.reasoning}`)
    .join("\n")

  const findings = state.memory.important_findings
    .map((f) => `- Finding: ${f.finding} (Source: ${f.source}, Confidence: ${f.confidence})`)
    .join("\n")

  // Format the current file focus
  let fileFocus = "None"
  if (state.execution_context.current_file_focus) {
    fileFocus = `File: ${state.execution_context.current_file_focus.path}, Lines: ${
      state.execution_context.current_file_focus.relevant_lines.join(", ")
    }`
  }

  // Gather relevant code snippets
  const codeSnippets = state.execution_context.relevant_code_snippets
    .slice(-5)
    .map((s) => `--- ${s.file_path} ---\nReason: ${s.reason}\n\`\`\`\n${s.snippet}\n\`\`\``)
    .join("\n\n")

  // Gather external references
  const references = state.execution_context.external_references
    .map((r) => `- ${r.type} ${r.identifier} (${r.relationship})`)
    .join("\n")

  // Compile the prompt
  return `
You are an autonomous AI agent executing a task on GitHub issue. Your goal is to help solve problems and complete tasks within the OpenAgents repository.

CURRENT GOAL: ${state.configuration.agent_goal || "Process GitHub issues and implement solutions"}

CURRENT STEP: Step ${currentStep.step_number} - ${currentStep.description}

CURRENT FILE FOCUS: ${fileFocus}

${recentMessages ? `RECENT CONVERSATION:\n${recentMessages}\n\n` : ""}
${decisions ? `KEY DECISIONS:\n${decisions}\n\n` : ""}
${findings ? `IMPORTANT FINDINGS:\n${findings}\n\n` : ""}
${codeSnippets ? `RELEVANT CODE SNIPPETS:\n${codeSnippets}\n\n` : ""}
${references ? `EXTERNAL REFERENCES:\n${references}\n\n` : ""}
${state.memory.scratchpad ? `SCRATCHPAD:\n${state.memory.scratchpad}\n\n` : ""}

INSTRUCTIONS:
1. Execute the current step: ${currentStep.description}
2. Use the tools available to you when needed
3. Make sure to record important findings and decisions
4. When the step is complete, clearly indicate "STEP COMPLETED" with a brief summary

If you encounter an error or problem that you cannot solve, indicate "STEP FAILED" with the reason.

Proceed with the current step now.
`.trim()
}

/**
 * Layer that provides the TaskExecutor implementation
 */
export const TaskExecutorLayer = Layer.effect(
  TaskExecutor,
  Effect.gen(function*(_) {
    // Get dependencies from the context
    const planManager = yield* PlanManager
    const memoryManager = yield* MemoryManager
    const githubClient = yield* GitHubClient
    // Only for initialization, not used in the actual functions yet
    yield* GitHubTools

    // Mock simplified AI completion since we don't have actual integration yet
    // This will be replaced with the real AI integration in a future phase
    const simulateAIWithTools = (input: string) =>
      Effect.gen(function*() {
        // Log that we're simulating AI
        yield* Console.log("🤖 Simulating AI with tools...")
        yield* Console.log("📝 Prompt (summarized):", input.substring(0, 100) + "...")

        // Create a simulated response
        const response = `I've analyzed the current step and completed it successfully. 

STEP COMPLETED: The step has been executed and all required tasks were performed.`

        // Create simulated text part for the response
        const textPart = {
          _tag: "Text" as const,
          content: response
        }

        // Simulate a Stream of responses
        return Stream.fromIterable([{
          response: {
            parts: [textPart]
          },
          value: { _tag: "None" as const }
        }])
      })

    return {
      executeNextStep: (currentState: AgentState): Effect.Effect<AgentState, Error, any> =>
        Effect.gen(function*() {
          // 1. Get current step
          const currentStep = yield* planManager.getCurrentStep(currentState)
          yield* Console.log(`Executing step ${currentStep.step_number}: ${currentStep.description}`)

          // 2. Update status to in_progress
          let workingState = yield* planManager.updateStepStatus(currentState, currentStep.id, "in_progress")

          // 3. Save state with updated status
          workingState = yield* githubClient.saveAgentState(workingState)

          // 4. Create a Ref to hold the state during AI/tool executions
          const stateRef = yield* Ref.make(workingState)

          try {
            // 5. Construct prompt from state
            const prompt = constructPromptFromState(workingState, currentStep)
            yield* Console.log("🧠 Prompting AI to execute step...")

            // 6. Call simulated AI with tools
            const aiResponseStream = yield* simulateAIWithTools(prompt)

            // 7. Buffer for collecting text response
            let responseBuffer = ""
            let isStepComplete = false
            let stepSuccessful = true

            // 8. Process the stream
            yield* aiResponseStream.pipe(
              Stream.tap((chunk) =>
                Effect.gen(function*() {
                  // For each chunk, check if it contains text, tool call, or tool result
                  if (chunk.response && chunk.response.parts) {
                    // Handle text parts
                    for (const part of chunk.response.parts) {
                      if (part._tag === "Text" && part.content) {
                        responseBuffer += part.content

                        // Check for step completion markers in the text
                        if (part.content.includes("STEP COMPLETED")) {
                          isStepComplete = true
                          stepSuccessful = true
                        } else if (part.content.includes("STEP FAILED")) {
                          isStepComplete = true
                          stepSuccessful = false
                        }
                      }
                      // Real tool calls would be handled here in the full implementation
                    }
                  }

                  // Tool results would be handled here in the full implementation
                  if (chunk.value && chunk.value._tag === "None") {
                    // In a real implementation, "Some" would indicate a tool result
                    yield* Console.log("🔧 No tool result in this chunk.")
                  }
                })
              ),
              Stream.runDrain
            )

            // 9. Add the remaining response to conversation history
            if (responseBuffer.trim().length > 0) {
              workingState = yield* Ref.get(stateRef)
              workingState = yield* memoryManager.addConversationMessage(
                workingState,
                "assistant",
                responseBuffer.trim()
              )
              yield* Ref.set(stateRef, workingState)
            }

            // 10. Get final state
            workingState = yield* Ref.get(stateRef)

            // 11. Update step status based on completion
            if (isStepComplete) {
              if (stepSuccessful) {
                yield* Console.log(`Step ${currentStep.step_number} completed successfully.`)
                // Update status to completed with a result summary
                const resultSummary = responseBuffer.includes("STEP COMPLETED")
                  ? responseBuffer.substring(responseBuffer.indexOf("STEP COMPLETED"))
                  : "Step completed successfully."

                workingState = yield* planManager.updateStepStatus(
                  workingState,
                  currentStep.id,
                  "completed",
                  resultSummary.slice(0, 200) // Limit summary length
                )

                // Advance step index
                workingState = {
                  ...workingState,
                  current_task: {
                    ...workingState.current_task,
                    current_step_index: workingState.current_task.current_step_index + 1
                  }
                }
              } else {
                yield* Console.log(`Step ${currentStep.step_number} failed.`)
                // Extract failure message if possible
                const failureMessage = responseBuffer.includes("STEP FAILED")
                  ? responseBuffer.substring(responseBuffer.indexOf("STEP FAILED"))
                  : "Step failed during execution."

                workingState = yield* planManager.updateStepStatus(
                  workingState,
                  currentStep.id,
                  "error",
                  failureMessage.slice(0, 200) // Limit summary length
                )

                // Update error state
                const now = new Date().toISOString()
                workingState = {
                  ...workingState,
                  error_state: {
                    ...workingState.error_state,
                    last_error: {
                      timestamp: now,
                      message: failureMessage,
                      type: "internal",
                      details: responseBuffer
                    },
                    consecutive_error_count: workingState.error_state.consecutive_error_count + 1
                  }
                }
              }
            } else {
              // If no completion marker found, assume completion
              yield* Console.log(`Step ${currentStep.step_number} assumed completed (no explicit marker).`)
              workingState = yield* planManager.updateStepStatus(
                workingState,
                currentStep.id,
                "completed",
                "Step assumed completed (no explicit marker)."
              )

              // Advance step index
              workingState = {
                ...workingState,
                current_task: {
                  ...workingState.current_task,
                  current_step_index: workingState.current_task.current_step_index + 1
                }
              }
            }

            // 12. Update LLM metrics
            workingState = {
              ...workingState,
              metrics: {
                ...workingState.metrics,
                llm_calls_made: workingState.metrics.llm_calls_made + 1,
                // For TypeScript, treat llm_tokens_used as number but ensure compatibility with the type
                llm_tokens_used: workingState.metrics.llm_tokens_used as unknown as number +
                  500 as unknown as typeof workingState.metrics.llm_tokens_used
              }
            }
          } catch (error) {
            // Handle errors during execution
            const err = error instanceof Error ? error : new Error(String(error))
            yield* Console.error(`Error executing step ${currentStep.step_number}: ${err.message}`)

            // Determine error type
            let errorType: "api_error" | "tool_error" | "internal" = "internal"
            if (error instanceof ToolExecutionError) {
              errorType = "tool_error"
            }

            // Update step status to error
            workingState = yield* planManager.updateStepStatus(
              workingState,
              currentStep.id,
              "error",
              `Failed: ${err.message}`
            )

            // Update error state
            const now = new Date().toISOString()
            workingState = {
              ...workingState,
              error_state: {
                ...workingState.error_state,
                last_error: {
                  timestamp: now,
                  message: err.message,
                  type: errorType,
                  details: err.stack ?? ""
                },
                consecutive_error_count: workingState.error_state.consecutive_error_count + 1
              }
            }
          }

          // 13. Save the final state
          yield* githubClient.saveAgentState(workingState)
          yield* Console.log(`Agent state saved for instance ${workingState.agent_info.instance_id}`)

          // 14. Return the final state
          return workingState
        }).pipe(
          // Ensure the error type is always Error
          Effect.catchAll((error) => Effect.fail(error instanceof Error ? error : new Error(String(error)))),
          // orDie ensures Type<AgentState, never, any> is converted to Type<AgentState, Error, never>
          Effect.orDie
        )
    }
  })
)

// Default implementation
export const TaskExecutorDefault = TaskExecutorLayer
