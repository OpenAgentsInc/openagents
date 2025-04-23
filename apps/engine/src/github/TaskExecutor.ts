import { Completions } from "@effect/ai"
// Import for the AnthropicClient.layerConfig definition
import { AnthropicClient } from "@effect/ai-anthropic"
import { Config, Console, Effect, Layer, Ref, Stream } from "effect"
import type { AgentState } from "./AgentStateTypes.js"
import { GitHubClient } from "./GitHub.js"
import { GitHubTools, StatefulToolContext, ToolExecutionError } from "./GitHubTools.js"
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
 * Define Anthropic Layer configuration
 * Using AnthropicClient imported at the top
 */
export const AnthropicLayer = AnthropicClient.layerConfig({
  apiKey: Config.secret("ANTHROPIC_API_KEY")
})

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
    const githubTools = yield* GitHubTools
    const completions = yield* Completions.Completions

    return {
      executeNextStep: (currentState: AgentState): Effect.Effect<AgentState, Error, any> =>
        Effect.gen(function*() {
          yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Starting execution`)

          // 1. Get current step
          yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Getting current step`)
          let currentStep
          try {
            currentStep = yield* planManager.getCurrentStep(currentState)
            yield* Console.log(
              `DEBUG: TaskExecutor.executeNextStep - Successfully got current step: ${currentStep.step_number}: ${currentStep.description}`
            )
          } catch (err) {
            yield* Console.error(
              `DEBUG ERROR: TaskExecutor.executeNextStep - Failed to get current step: ${
                err instanceof Error ? err.stack : String(err)
              }`
            )
            throw err
          }

          // 2. Update status to in_progress
          yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Updating step status to in_progress`)
          let workingState
          try {
            workingState = yield* planManager.updateStepStatus(currentState, currentStep.id, "in_progress")
            yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Successfully updated step status to in_progress`)
          } catch (err) {
            yield* Console.error(
              `DEBUG ERROR: TaskExecutor.executeNextStep - Failed to update step status: ${
                err instanceof Error ? err.stack : String(err)
              }`
            )
            throw err
          }

          // 3. Save state with updated status
          yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Saving initial state with in_progress status`)
          try {
            workingState = yield* githubClient.saveAgentState(workingState)
            yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Successfully saved initial state`)
          } catch (err) {
            yield* Console.error(
              `DEBUG ERROR: TaskExecutor.executeNextStep - Failed to save initial state: ${
                err instanceof Error ? err.stack : String(err)
              }`
            )
            throw err
          }

          // 4. Create a Ref to hold the state during AI/tool executions
          yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Creating stateRef for tool context`)
          let stateRef
          try {
            stateRef = yield* Ref.make(workingState)
            yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Successfully created stateRef`)
          } catch (err) {
            yield* Console.error(
              `DEBUG ERROR: TaskExecutor.executeNextStep - Failed to create stateRef: ${
                err instanceof Error ? err.stack : String(err)
              }`
            )
            throw err
          }

          // 5. Create StatefulToolContext for the tool handlers
          yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Creating StatefulToolContext`)
          const toolContextService = StatefulToolContext.of({
            stateRef,
            planManager: planManager as unknown as PlanManager,
            memoryManager: memoryManager as unknown as MemoryManager
          })
          const toolContextLayer = Layer.succeed(StatefulToolContext, toolContextService)
          yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Successfully created StatefulToolContext layer`)

          try {
            // 6. Construct prompt from state
            yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Constructing AI prompt`)
            const prompt = constructPromptFromState(workingState, currentStep)
            yield* Console.log(
              `DEBUG: TaskExecutor.executeNextStep - Successfully constructed prompt with length: ${prompt.length}`
            )
            yield* Console.log("🧠 Prompting AI to execute step...")

            // 7. Get tools/handlers from githubTools
            yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Getting tools and handlers from githubTools`)
            const { handlers, tools } = githubTools
            yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Successfully got tools and handlers`)

            // 8. Prepare the AI toolkitStream
            yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - About to call completions.toolkitStream`)
            yield* Console.log(
              `DEBUG: TaskExecutor.executeNextStep - Using model: ${workingState.configuration.llm_config.model}`
            )
            yield* Console.log(
              `DEBUG: TaskExecutor.executeNextStep - Using temperature: ${workingState.configuration.llm_config.temperature}`
            )
            yield* Console.log(
              `DEBUG: TaskExecutor.executeNextStep - Using maxTokens: ${workingState.configuration.llm_config.max_tokens}`
            )

            let aiResponseStream

            // 9. Set up buffers for response processing
            let responseBuffer = ""
            const toolOutputs: Array<any> = []
            let finalResponse = ""

            // DEBUGGING: Set this to true to use a simple completion instead of toolkitStream
            // This is useful for diagnosing if the issue is specifically with toolkitStream
            const useSimpleCompletionForDebugging = false

            try {
              if (useSimpleCompletionForDebugging) {
                // Simple completion approach for debugging
                yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Using simple completion for debugging`)

                // Create a simulated response directly
                const simpleResponse = "Hello from the AI service"
                yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Simple debug response: ${simpleResponse}`)

                // Set the response buffer with our static text
                responseBuffer = simpleResponse

                // Create a simple stream with just one item
                const textPart = {
                  _tag: "Text" as const,
                  content: simpleResponse
                }

                const streamItem = {
                  response: {
                    parts: [textPart]
                  },
                  value: { _tag: "None" as const }
                }

                aiResponseStream = Stream.make(streamItem)
                yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Created mock aiResponseStream for debugging`)
              } else {
                // Normal approach with toolkitStream
                // Use `as any` to work around library typings issues
                aiResponseStream = (completions.toolkitStream as any)({
                  model: workingState.configuration.llm_config.model,
                  messages: [{ role: "user", content: prompt }],
                  tools: { toolkit: tools, handlers } as any,
                  temperature: workingState.configuration.llm_config.temperature,
                  maxTokens: workingState.configuration.llm_config.max_tokens
                })
                yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Successfully created aiResponseStream`)
              }
            } catch (err) {
              yield* Console.error(
                `DEBUG ERROR: TaskExecutor.executeNextStep - Failed to create aiResponseStream: ${
                  err instanceof Error ? err.stack : String(err)
                }`
              )
              throw err
            }
            yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Initialized response buffers`)

            // 10. Process the AI stream
            yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - About to process AI stream with Stream.tap`)
            try {
              yield* aiResponseStream.pipe(
                Stream.tap((chunk) =>
                  Effect.gen(function*() {
                    yield* Console.log(
                      `DEBUG: TaskExecutor.executeNextStep - Processing stream chunk: ${
                        JSON.stringify(chunk).substring(0, 100)
                      }...`
                    )
                    const currentState = yield* Ref.get(stateRef)
                    let nextState = currentState

                    // Handle Text Deltas
                    if ((chunk as any).response && (chunk as any).response.parts) {
                      for (const part of (chunk as any).response.parts) {
                        if (part._tag === "Text" && part.content) {
                          responseBuffer += part.content
                          yield* Console.log(
                            `DEBUG: TaskExecutor.executeNextStep - Received text part: "${
                              part.content.substring(0, 50)
                            }..."`
                          )
                          // Could broadcast text deltas via SSE here
                        } else if (part._tag === "ToolCall") {
                          // Log tool usage before execution
                          yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - AI requesting tool: ${part.name}`)
                          // AI framework handles calling the handler now
                        }
                      }
                    }

                    // Handle Tool Results (after handler runs and updates state via Ref)
                    if ((chunk as any).value?._tag === "Some") {
                      const toolResult = (chunk as any).value.value
                      yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Tool ${toolResult.name} executed.`)

                      // State should have been updated inside the handler via Ref
                      // Add the result to conversation history
                      const toolResultMessage = {
                        role: "tool" as const,
                        content: JSON.stringify(toolResult.result),
                        tool_call_id: toolResult.id
                      }

                      // Store tool outputs for potential follow-up prompts
                      toolOutputs.push(toolResult)

                      // Add the tool result message to conversation history
                      yield* Console.log(
                        `DEBUG: TaskExecutor.executeNextStep - Adding tool result to conversation history`
                      )
                      try {
                        nextState = yield* memoryManager.addConversationMessage(
                          nextState,
                          toolResultMessage.role,
                          toolResultMessage.content,
                          [{ id: toolResultMessage.tool_call_id, name: toolResult.name, input: toolResult.input }]
                        )
                        yield* Console.log(
                          `DEBUG: TaskExecutor.executeNextStep - Successfully added tool result to conversation history`
                        )
                      } catch (err) {
                        yield* Console.error(
                          `DEBUG ERROR: TaskExecutor.executeNextStep - Failed to add tool result to conversation: ${
                            err instanceof Error ? err.stack : String(err)
                          }`
                        )
                      }
                    }

                    // Update Ref if state changed within the chunk processing
                    if (nextState !== currentState) {
                      yield* Console.log(
                        `DEBUG: TaskExecutor.executeNextStep - Updating stateRef from stream processor`
                      )
                      yield* Ref.set(stateRef, nextState)
                      yield* Console.log(
                        `DEBUG: TaskExecutor.executeNextStep - Successfully updated stateRef from stream processor`
                      )
                    }
                  })
                ),
                // Provide the StatefulToolContext Layer to the stream processing
                (stream: any) => {
                  console.log(`DEBUG: TaskExecutor.executeNextStep - Providing StatefulToolContext Layer to stream`)
                  return Effect.provide(stream as unknown as Effect.Effect<any, any, any>, toolContextLayer)
                },
                Stream.runDrain
              ).pipe(
                Effect.tap(() =>
                  Effect.sync(() => {
                    console.log(`DEBUG: TaskExecutor.executeNextStep - Stream.runDrain completed successfully`)
                  })
                ),
                Effect.catchAll((aiError) => {
                  // Handle errors specifically from the AI/Tool stream
                  return Effect.gen(function*() {
                    const errorMsg = aiError instanceof Error ? aiError.message : String(aiError)
                    const stackTrace = aiError instanceof Error ? aiError.stack : "No stack trace available"
                    yield* Console.error(`DEBUG ERROR: AI/Tool Stream Error: ${errorMsg}`)
                    yield* Console.error(`DEBUG ERROR: AI/Tool Stream Error Stack: ${stackTrace}`)

                    // Update stateRef with AI error details
                    const now = new Date().toISOString()
                    yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - Updating stateRef with AI error details`)
                    yield* Ref.update(stateRef, (s) => ({
                      ...s,
                      error_state: {
                        ...s.error_state,
                        last_error: {
                          timestamp: now,
                          message: `AI Error: ${errorMsg}`,
                          type: "internal" as const,
                          details: stackTrace || ""
                        },
                        consecutive_error_count: s.error_state.consecutive_error_count + 1
                      },
                      current_task: { ...s.current_task, status: "error" }
                    }))
                    yield* Console.log(
                      `DEBUG: TaskExecutor.executeNextStep - Successfully updated stateRef with AI error details`
                    )

                    // Re-throw the error
                    return Effect.fail(aiError instanceof Error ? aiError : new Error(String(aiError)))
                  })
                })
              )
              yield* Console.log(`DEBUG: TaskExecutor.executeNextStep - AI stream processing completed`)
            } catch (err) {
              const errorMsg = err instanceof Error ? err.message : String(err)
              const stackTrace = err instanceof Error ? err.stack : "No stack trace available"
              yield* Console.error(
                `DEBUG ERROR: TaskExecutor.executeNextStep - Error processing AI stream: ${errorMsg}`
              )
              yield* Console.error(`DEBUG ERROR: TaskExecutor.executeNextStep - Stack trace: ${stackTrace}`)
              throw err
            }

            // 11. After stream processing, get final state and add final assistant message
            workingState = yield* Ref.get(stateRef)
            finalResponse = responseBuffer.trim()

            if (finalResponse) {
              workingState = yield* memoryManager.addConversationMessage(workingState, "assistant", finalResponse)
            }

            // 12. Update Step Status based on final response
            const isStepComplete = responseBuffer.includes("STEP COMPLETED")
            const stepFailed = responseBuffer.includes("STEP FAILED") ||
              (workingState.error_state.last_error &&
                workingState.error_state.last_error.timestamp > currentState.timestamps.last_action_at)

            if (stepFailed) {
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

              // Update error state if not already set by a tool handler
              if (!workingState.error_state.last_error) {
                const now = new Date().toISOString()
                workingState = {
                  ...workingState,
                  error_state: {
                    ...workingState.error_state,
                    last_error: {
                      timestamp: now,
                      message: failureMessage,
                      type: "internal" as const,
                      details: responseBuffer
                    },
                    consecutive_error_count: workingState.error_state.consecutive_error_count + 1
                  }
                }
              }
            } else if (isStepComplete) {
              yield* Console.log(`Step ${currentStep.step_number} completed successfully.`)

              // Update status to completed with a result summary
              const resultSummary = responseBuffer.substring(responseBuffer.indexOf("STEP COMPLETED"))

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
              // If no explicit markers, check for errors before assuming completion
              if (workingState.error_state.last_error) {
                yield* Console.log(`Step ${currentStep.step_number} failed (error detected).`)

                workingState = yield* planManager.updateStepStatus(
                  workingState,
                  currentStep.id,
                  "error",
                  `Failed: ${workingState.error_state.last_error.message.slice(0, 200)}`
                )
              } else {
                // Assume completion if no errors or explicit markers
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
            }

            // 13. Update metrics
            workingState = {
              ...workingState,
              metrics: {
                ...workingState.metrics,
                llm_calls_made: workingState.metrics.llm_calls_made + 1,
                llm_tokens_used: {
                  prompt: Math.floor(workingState.metrics.llm_tokens_used.prompt + prompt.length / 4), // Rough estimate
                  completion: Math.floor(workingState.metrics.llm_tokens_used.completion + responseBuffer.length / 4)
                }
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

            // Update error state if not already set by a tool handler
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

          // 14. Update timestamps
          workingState = {
            ...workingState,
            timestamps: {
              ...workingState.timestamps,
              last_action_at: new Date().toISOString(),
              last_saved_at: new Date().toISOString()
            }
          }

          // 15. Save the final state
          yield* githubClient.saveAgentState(workingState)
          yield* Console.log(`Agent state saved for instance ${workingState.agent_info.instance_id}`)

          // 16. Return the final state
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
