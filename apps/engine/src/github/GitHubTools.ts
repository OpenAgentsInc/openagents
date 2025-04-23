import type { AiToolkit } from "@effect/ai"
import { Console, Effect, Layer, Ref } from "effect"
import type { AgentState } from "./AgentStateTypes.js"
import { GitHubClient } from "./GitHub.js"
import { MemoryManager } from "./MemoryManager.js"
import { PlanManager } from "./PlanManager.js"

// GitHub tool names for reference
export const TOOL_NAMES = {
  GET_ISSUE: "GetGitHubIssue",
  LIST_ISSUES: "ListGitHubIssues",
  CREATE_COMMENT: "CreateGitHubComment",
  UPDATE_ISSUE: "UpdateGitHubIssue",
  GET_REPOSITORY: "GetGitHubRepository",
  GET_ISSUE_COMMENTS: "GetGitHubIssueComments",
  CREATE_AGENT_STATE: "CreateAgentStateForIssue",
  LOAD_AGENT_STATE: "LoadAgentState",
  SAVE_AGENT_STATE: "SaveAgentState"
}

/**
 * Context for stateful tool execution
 */
export interface StatefulToolContext {
  readonly stateRef: Ref.Ref<AgentState>
  readonly planManager: PlanManager
  readonly memoryManager: MemoryManager
}

/**
 * Tag for the StatefulToolContext 
 */
export class StatefulToolContext extends Effect.Tag("StatefulToolContext")<
  StatefulToolContext,
  {
    readonly stateRef: Ref.Ref<AgentState>
    readonly planManager: PlanManager
    readonly memoryManager: MemoryManager
  }
>() {}

// Define the GitHub tools service
export class GitHubTools extends Effect.Tag("GitHubTools")<
  GitHubTools,
  {
    tools: AiToolkit.AiToolkit<any>
    handlers: Record<string, (...args: Array<any>) => any>
  }
>() {
  static readonly fullName = "GitHubTools"
}

// Create a class for tool execution errors
export class ToolExecutionError extends Error {
  readonly _tag = "ToolExecutionError"
  readonly toolName: string
  readonly params: any

  constructor(message: string, toolName: string, params: any) {
    super(message)
    this.name = "ToolExecutionError"
    this.toolName = toolName
    this.params = params
  }
}

// Create the layer for the tools
export const GitHubToolsLayer = Layer.effect(
  GitHubTools,
  Effect.gen(function*() {
    // Get dependencies from the context
    const github = yield* GitHubClient
    
    const handlers = {
      GetGitHubIssue: (params: { owner: string; repo: string; issueNumber: number }) =>
        Effect.gen(function*() {
          const { stateRef, memoryManager, planManager } = yield* StatefulToolContext // Get context
          yield* Console.log(`🛠️ Tool called: GetGitHubIssue with params: ${JSON.stringify(params)}`)
          
          // Perform core action
          const result = yield* github.getIssue(params.owner, params.repo, params.issueNumber)
          yield* Console.log(`✅ Tool result obtained.`)
          
          // Update state via Ref using Managers AFTER success
          yield* Ref.update(stateRef, (currentState) => {
            const toolCallData = { 
              tool_name: TOOL_NAMES.GET_ISSUE, 
              parameters: params, 
              status: "success", 
              result_preview: JSON.stringify(result).substring(0, 100), 
              full_result_ref: null 
            }
            // Chain sync updates using pipe
            return Effect.runSync(
              Effect.gen(function*() {
                let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                if (currentStepId) {
                  state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                }
                // Update metrics 
                return { 
                  ...state1, 
                  metrics: { 
                    ...state1.metrics, 
                    tools_called: state1.metrics.tools_called + 1 
                  } 
                }
              })
            )
          })
          
          return result
        }).pipe(
          Effect.catchAll((error) => 
            Effect.gen(function*() {
              const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
              yield* Console.error(`❌ Tool error: ${String(error)}`)
              
              // Update state via Ref on FAILURE
              yield* Ref.update(stateRef, (currentState) => {
                const toolCallData = { 
                  tool_name: TOOL_NAMES.GET_ISSUE, 
                  parameters: params, 
                  status: "error", 
                  result_preview: String(error), 
                  full_result_ref: null 
                }
                
                return Effect.runSync(
                  Effect.gen(function*() {
                    let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                    const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                    if (currentStepId) {
                      state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                    }
                    
                    // Update error state
                    const now = new Date().toISOString()
                    return { 
                      ...state1, 
                      metrics: { 
                        ...state1.metrics, 
                        tools_called: state1.metrics.tools_called + 1 
                      },
                      error_state: {
                        ...state1.error_state,
                        last_error: {
                          timestamp: now,
                          message: `Tool error: ${String(error)}`,
                          type: "tool_error" as const,
                          details: JSON.stringify(params)
                        },
                        consecutive_error_count: state1.error_state.consecutive_error_count + 1
                      }
                    }
                  })
                )
              })
              
              // Fail the tool effect
              return yield* Effect.fail(new ToolExecutionError(String(error), TOOL_NAMES.GET_ISSUE, params))
            })
          )
        ),

      ListGitHubIssues: (params: { owner: string; repo: string; state?: "open" | "closed" | "all" }) =>
        Effect.gen(function*() {
          const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
          yield* Console.log(`🛠️ Tool called: ListGitHubIssues with params: ${JSON.stringify(params)}`)
          
          // Perform core action
          const result = yield* github.listIssues(params.owner, params.repo, params.state || "open")
          yield* Console.log(`✅ Tool result obtained.`)
          
          // Update state via Ref
          yield* Ref.update(stateRef, (currentState) => {
            const toolCallData = { 
              tool_name: TOOL_NAMES.LIST_ISSUES, 
              parameters: params, 
              status: "success", 
              result_preview: JSON.stringify(result.issues.slice(0, 3)).substring(0, 100) + (result.issues.length > 3 ? "..." : ""), 
              full_result_ref: null 
            }
            
            return Effect.runSync(
              Effect.gen(function*() {
                let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                if (currentStepId) {
                  state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                }
                return { ...state1, metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 } }
              })
            )
          })
          
          return result
        }).pipe(
          Effect.catchAll((error) =>
            Effect.gen(function*() {
              const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
              yield* Console.error(`❌ Tool error: ${String(error)}`)
              
              // Update state via Ref on FAILURE
              yield* Ref.update(stateRef, (currentState) => {
                const toolCallData = { 
                  tool_name: TOOL_NAMES.LIST_ISSUES, 
                  parameters: params, 
                  status: "error", 
                  result_preview: String(error), 
                  full_result_ref: null 
                }
                
                return Effect.runSync(
                  Effect.gen(function*() {
                    let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                    const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                    if (currentStepId) {
                      state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                    }
                    
                    // Update error state
                    const now = new Date().toISOString()
                    return { 
                      ...state1, 
                      metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 },
                      error_state: {
                        ...state1.error_state,
                        last_error: {
                          timestamp: now,
                          message: `Tool error: ${String(error)}`,
                          type: "tool_error" as const,
                          details: JSON.stringify(params)
                        },
                        consecutive_error_count: state1.error_state.consecutive_error_count + 1
                      }
                    }
                  })
                )
              })
              
              return yield* Effect.fail(new ToolExecutionError(String(error), TOOL_NAMES.LIST_ISSUES, params))
            })
          )
        ),

      CreateGitHubComment: (params: { owner: string; repo: string; issueNumber: number; body: string }) =>
        Effect.gen(function*() {
          const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
          yield* Console.log(`🛠️ Tool called: CreateGitHubComment with params: ${JSON.stringify(params)}`)
          
          // Perform core action
          const result = yield* github.createIssueComment(params.owner, params.repo, params.issueNumber, params.body)
          yield* Console.log(`✅ Tool result obtained.`)
          
          // Update state via Ref
          yield* Ref.update(stateRef, (currentState) => {
            const toolCallData = { 
              tool_name: TOOL_NAMES.CREATE_COMMENT, 
              parameters: params, 
              status: "success", 
              result_preview: JSON.stringify(result).substring(0, 100), 
              full_result_ref: null 
            }
            
            return Effect.runSync(
              Effect.gen(function*() {
                let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                if (currentStepId) {
                  state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                }
                return { ...state1, metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 } }
              })
            )
          })
          
          return result
        }).pipe(
          Effect.catchAll((error) =>
            Effect.gen(function*() {
              const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
              yield* Console.error(`❌ Tool error: ${String(error)}`)
              
              // Update state via Ref on FAILURE
              yield* Ref.update(stateRef, (currentState) => {
                const toolCallData = { 
                  tool_name: TOOL_NAMES.CREATE_COMMENT, 
                  parameters: params, 
                  status: "error", 
                  result_preview: String(error), 
                  full_result_ref: null 
                }
                
                return Effect.runSync(
                  Effect.gen(function*() {
                    let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                    const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                    if (currentStepId) {
                      state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                    }
                    
                    // Update error state
                    const now = new Date().toISOString()
                    return { 
                      ...state1, 
                      metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 },
                      error_state: {
                        ...state1.error_state,
                        last_error: {
                          timestamp: now,
                          message: `Tool error: ${String(error)}`,
                          type: "tool_error" as const,
                          details: JSON.stringify(params)
                        },
                        consecutive_error_count: state1.error_state.consecutive_error_count + 1
                      }
                    }
                  })
                )
              })
              
              return yield* Effect.fail(new ToolExecutionError(String(error), TOOL_NAMES.CREATE_COMMENT, params))
            })
          )
        ),

      UpdateGitHubIssue: (
        params: {
          owner: string
          repo: string
          issueNumber: number
          title?: string
          body?: string
          state?: "open" | "closed"
          labels?: Array<string>
          assignees?: Array<string>
        }
      ) =>
        Effect.gen(function*() {
          const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
          yield* Console.log(`🛠️ Tool called: UpdateGitHubIssue with params: ${JSON.stringify(params)}`)
          
          // Perform core action
          const { issueNumber, owner, repo, ...updates } = params
          const result = yield* github.updateIssue(owner, repo, issueNumber, updates)
          yield* Console.log(`✅ Tool result obtained.`)
          
          // Update state via Ref
          yield* Ref.update(stateRef, (currentState) => {
            const toolCallData = { 
              tool_name: TOOL_NAMES.UPDATE_ISSUE, 
              parameters: params, 
              status: "success", 
              result_preview: JSON.stringify(result).substring(0, 100), 
              full_result_ref: null 
            }
            
            return Effect.runSync(
              Effect.gen(function*() {
                let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                if (currentStepId) {
                  state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                }
                return { ...state1, metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 } }
              })
            )
          })
          
          return result
        }).pipe(
          Effect.catchAll((error) =>
            Effect.gen(function*() {
              const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
              yield* Console.error(`❌ Tool error: ${String(error)}`)
              
              // Update state via Ref on FAILURE
              yield* Ref.update(stateRef, (currentState) => {
                const toolCallData = { 
                  tool_name: TOOL_NAMES.UPDATE_ISSUE, 
                  parameters: params, 
                  status: "error", 
                  result_preview: String(error), 
                  full_result_ref: null 
                }
                
                return Effect.runSync(
                  Effect.gen(function*() {
                    let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                    const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                    if (currentStepId) {
                      state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                    }
                    
                    // Update error state
                    const now = new Date().toISOString()
                    return { 
                      ...state1, 
                      metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 },
                      error_state: {
                        ...state1.error_state,
                        last_error: {
                          timestamp: now,
                          message: `Tool error: ${String(error)}`,
                          type: "tool_error" as const,
                          details: JSON.stringify(params)
                        },
                        consecutive_error_count: state1.error_state.consecutive_error_count + 1
                      }
                    }
                  })
                )
              })
              
              return yield* Effect.fail(new ToolExecutionError(String(error), TOOL_NAMES.UPDATE_ISSUE, params))
            })
          )
        ),

      GetGitHubRepository: (params: { owner: string; repo: string }) =>
        Effect.gen(function*() {
          const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
          yield* Console.log(`🛠️ Tool called: GetGitHubRepository with params: ${JSON.stringify(params)}`)
          
          // Perform core action
          const result = yield* github.getRepository(params.owner, params.repo)
          yield* Console.log(`✅ Tool result obtained.`)
          
          // Update state via Ref
          yield* Ref.update(stateRef, (currentState) => {
            const toolCallData = { 
              tool_name: TOOL_NAMES.GET_REPOSITORY, 
              parameters: params, 
              status: "success", 
              result_preview: JSON.stringify(result).substring(0, 100), 
              full_result_ref: null 
            }
            
            return Effect.runSync(
              Effect.gen(function*() {
                let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                if (currentStepId) {
                  state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                }
                return { ...state1, metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 } }
              })
            )
          })
          
          return result
        }).pipe(
          Effect.catchAll((error) =>
            Effect.gen(function*() {
              const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
              yield* Console.error(`❌ Tool error: ${String(error)}`)
              
              // Update state via Ref on FAILURE
              yield* Ref.update(stateRef, (currentState) => {
                const toolCallData = { 
                  tool_name: TOOL_NAMES.GET_REPOSITORY, 
                  parameters: params, 
                  status: "error", 
                  result_preview: String(error), 
                  full_result_ref: null 
                }
                
                return Effect.runSync(
                  Effect.gen(function*() {
                    let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                    const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                    if (currentStepId) {
                      state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                    }
                    
                    // Update error state
                    const now = new Date().toISOString()
                    return { 
                      ...state1, 
                      metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 },
                      error_state: {
                        ...state1.error_state,
                        last_error: {
                          timestamp: now,
                          message: `Tool error: ${String(error)}`,
                          type: "tool_error" as const,
                          details: JSON.stringify(params)
                        },
                        consecutive_error_count: state1.error_state.consecutive_error_count + 1
                      }
                    }
                  })
                )
              })
              
              return yield* Effect.fail(new ToolExecutionError(String(error), TOOL_NAMES.GET_REPOSITORY, params))
            })
          )
        ),

      GetGitHubIssueComments: (params: { owner: string; repo: string; issueNumber: number }) =>
        Effect.gen(function*() {
          const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
          yield* Console.log(`🛠️ Tool called: GetGitHubIssueComments with params: ${JSON.stringify(params)}`)
          
          // Perform core action
          const result = yield* github.getIssueComments(params.owner, params.repo, params.issueNumber)
          yield* Console.log(`✅ Tool result obtained.`)
          
          // Update state via Ref
          yield* Ref.update(stateRef, (currentState) => {
            const toolCallData = { 
              tool_name: TOOL_NAMES.GET_ISSUE_COMMENTS, 
              parameters: params, 
              status: "success", 
              result_preview: JSON.stringify(result.slice(0, 2)).substring(0, 100) + (result.length > 2 ? "..." : ""), 
              full_result_ref: null 
            }
            
            return Effect.runSync(
              Effect.gen(function*() {
                let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                if (currentStepId) {
                  state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                }
                return { ...state1, metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 } }
              })
            )
          })
          
          return result
        }).pipe(
          Effect.catchAll((error) =>
            Effect.gen(function*() {
              const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
              yield* Console.error(`❌ Tool error: ${String(error)}`)
              
              // Update state via Ref on FAILURE
              yield* Ref.update(stateRef, (currentState) => {
                const toolCallData = { 
                  tool_name: TOOL_NAMES.GET_ISSUE_COMMENTS, 
                  parameters: params, 
                  status: "error", 
                  result_preview: String(error), 
                  full_result_ref: null 
                }
                
                return Effect.runSync(
                  Effect.gen(function*() {
                    let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                    const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                    if (currentStepId) {
                      state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                    }
                    
                    // Update error state
                    const now = new Date().toISOString()
                    return { 
                      ...state1, 
                      metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 },
                      error_state: {
                        ...state1.error_state,
                        last_error: {
                          timestamp: now,
                          message: `Tool error: ${String(error)}`,
                          type: "tool_error" as const,
                          details: JSON.stringify(params)
                        },
                        consecutive_error_count: state1.error_state.consecutive_error_count + 1
                      }
                    }
                  })
                )
              })
              
              return yield* Effect.fail(new ToolExecutionError(String(error), TOOL_NAMES.GET_ISSUE_COMMENTS, params))
            })
          )
        ),

      CreateAgentStateForIssue: (params: { owner: string; repo: string; issueNumber: number }) =>
        Effect.gen(function*() {
          const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
          yield* Console.log(`🛠️ Tool called: CreateAgentStateForIssue with params: ${JSON.stringify(params)}`)
          
          // Perform core action
          const result = yield* github.createAgentStateForIssue(params.owner, params.repo, params.issueNumber)
          yield* Console.log(`✅ Tool result obtained.`)
          
          // Update state via Ref
          yield* Ref.update(stateRef, (currentState) => {
            const toolCallData = { 
              tool_name: TOOL_NAMES.CREATE_AGENT_STATE, 
              parameters: params, 
              status: "success", 
              result_preview: `Created state with ID: ${result.agent_info.instance_id}`, 
              full_result_ref: null 
            }
            
            return Effect.runSync(
              Effect.gen(function*() {
                let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                if (currentStepId) {
                  state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                }
                return { ...state1, metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 } }
              })
            )
          })
          
          return result
        }).pipe(
          Effect.catchAll((error) =>
            Effect.gen(function*() {
              const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
              yield* Console.error(`❌ Tool error: ${String(error)}`)
              
              // Update state via Ref on FAILURE
              yield* Ref.update(stateRef, (currentState) => {
                const toolCallData = { 
                  tool_name: TOOL_NAMES.CREATE_AGENT_STATE, 
                  parameters: params, 
                  status: "error", 
                  result_preview: String(error), 
                  full_result_ref: null 
                }
                
                return Effect.runSync(
                  Effect.gen(function*() {
                    let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                    const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                    if (currentStepId) {
                      state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                    }
                    
                    // Update error state
                    const now = new Date().toISOString()
                    return { 
                      ...state1, 
                      metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 },
                      error_state: {
                        ...state1.error_state,
                        last_error: {
                          timestamp: now,
                          message: `Tool error: ${String(error)}`,
                          type: "tool_error" as const,
                          details: JSON.stringify(params)
                        },
                        consecutive_error_count: state1.error_state.consecutive_error_count + 1
                      }
                    }
                  })
                )
              })
              
              return yield* Effect.fail(new ToolExecutionError(String(error), TOOL_NAMES.CREATE_AGENT_STATE, params))
            })
          )
        ),

      LoadAgentState: (params: { instanceId: string }) =>
        Effect.gen(function*() {
          const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
          yield* Console.log(`🛠️ Tool called: LoadAgentState with params: ${JSON.stringify(params)}`)
          
          // Perform core action
          const result = yield* github.loadAgentState(params.instanceId)
          yield* Console.log(`✅ Tool result obtained.`)
          
          // Update state via Ref
          yield* Ref.update(stateRef, (currentState) => {
            const toolCallData = { 
              tool_name: TOOL_NAMES.LOAD_AGENT_STATE, 
              parameters: params, 
              status: "success", 
              result_preview: `Loaded state with ID: ${result.agent_info.instance_id}`, 
              full_result_ref: null 
            }
            
            return Effect.runSync(
              Effect.gen(function*() {
                let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                if (currentStepId) {
                  state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                }
                return { ...state1, metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 } }
              })
            )
          })
          
          return result
        }).pipe(
          Effect.catchAll((error) =>
            Effect.gen(function*() {
              const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
              yield* Console.error(`❌ Tool error: ${String(error)}`)
              
              // Update state via Ref on FAILURE
              yield* Ref.update(stateRef, (currentState) => {
                const toolCallData = { 
                  tool_name: TOOL_NAMES.LOAD_AGENT_STATE, 
                  parameters: params, 
                  status: "error", 
                  result_preview: String(error), 
                  full_result_ref: null 
                }
                
                return Effect.runSync(
                  Effect.gen(function*() {
                    let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                    const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                    if (currentStepId) {
                      state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                    }
                    
                    // Update error state
                    const now = new Date().toISOString()
                    return { 
                      ...state1, 
                      metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 },
                      error_state: {
                        ...state1.error_state,
                        last_error: {
                          timestamp: now,
                          message: `Tool error: ${String(error)}`,
                          type: "tool_error" as const,
                          details: JSON.stringify(params)
                        },
                        consecutive_error_count: state1.error_state.consecutive_error_count + 1
                      }
                    }
                  })
                )
              })
              
              return yield* Effect.fail(new ToolExecutionError(String(error), TOOL_NAMES.LOAD_AGENT_STATE, params))
            })
          )
        ),

      SaveAgentState: (params: { state: AgentState }) =>
        Effect.gen(function*() {
          const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
          yield* Console.log(`🛠️ Tool called: SaveAgentState`)
          yield* Console.log(`📝 Parameters: [state object]`)
          
          // Perform core action
          const result = yield* github.saveAgentState(params.state)
          yield* Console.log(`✅ Tool result obtained.`)
          
          // Update state via Ref
          yield* Ref.update(stateRef, (currentState) => {
            const toolCallData = { 
              tool_name: TOOL_NAMES.SAVE_AGENT_STATE, 
              parameters: { instance_id: params.state.agent_info.instance_id }, 
              status: "success", 
              result_preview: `Saved state with ID: ${result.agent_info.instance_id}`, 
              full_result_ref: null 
            }
            
            return Effect.runSync(
              Effect.gen(function*() {
                let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                if (currentStepId) {
                  state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                }
                return { ...state1, metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 } }
              })
            )
          })
          
          return result
        }).pipe(
          Effect.catchAll((error) =>
            Effect.gen(function*() {
              const { stateRef, memoryManager, planManager } = yield* StatefulToolContext
              yield* Console.error(`❌ Tool error: ${String(error)}`)
              
              // Update state via Ref on FAILURE
              yield* Ref.update(stateRef, (currentState) => {
                const toolCallData = { 
                  tool_name: TOOL_NAMES.SAVE_AGENT_STATE, 
                  parameters: { instance_id: params.state.agent_info.instance_id }, 
                  status: "error", 
                  result_preview: String(error), 
                  full_result_ref: null 
                }
                
                return Effect.runSync(
                  Effect.gen(function*() {
                    let state1 = yield* memoryManager.addToolInvocationLogEntry(currentState, toolCallData)
                    const currentStepId = state1.plan[state1.current_task.current_step_index]?.id
                    if (currentStepId) {
                      state1 = yield* planManager.addToolCallToStep(state1, currentStepId, toolCallData)
                    }
                    
                    // Update error state
                    const now = new Date().toISOString()
                    return { 
                      ...state1, 
                      metrics: { ...state1.metrics, tools_called: state1.metrics.tools_called + 1 },
                      error_state: {
                        ...state1.error_state,
                        last_error: {
                          timestamp: now,
                          message: `Tool error: ${String(error)}`,
                          type: "tool_error" as const,
                          details: "Failed to save agent state"
                        },
                        consecutive_error_count: state1.error_state.consecutive_error_count + 1
                      }
                    }
                  })
                )
              })
              
              return yield* Effect.fail(new ToolExecutionError(String(error), TOOL_NAMES.SAVE_AGENT_STATE, { instance_id: params.state.agent_info.instance_id }))
            })
          )
        )
    }

    // Define schemas for the tools
    const toolSchemas = {
      [TOOL_NAMES.GET_ISSUE]: {
        type: "function",
        function: {
          name: TOOL_NAMES.GET_ISSUE,
          description: "Fetch a GitHub issue by its number",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" },
              issueNumber: { type: "integer", description: "The issue number" }
            },
            required: ["owner", "repo", "issueNumber"]
          }
        }
      },
      [TOOL_NAMES.LIST_ISSUES]: {
        type: "function",
        function: {
          name: TOOL_NAMES.LIST_ISSUES,
          description: "List GitHub issues in a repository",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" },
              state: { type: "string", enum: ["open", "closed", "all"], description: "Filter issues by state" }
            },
            required: ["owner", "repo"]
          }
        }
      },
      [TOOL_NAMES.CREATE_COMMENT]: {
        type: "function",
        function: {
          name: TOOL_NAMES.CREATE_COMMENT,
          description: "Create a comment on a GitHub issue",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" },
              issueNumber: { type: "integer", description: "The issue number" },
              body: { type: "string", description: "The comment text" }
            },
            required: ["owner", "repo", "issueNumber", "body"]
          }
        }
      },
      [TOOL_NAMES.UPDATE_ISSUE]: {
        type: "function",
        function: {
          name: TOOL_NAMES.UPDATE_ISSUE,
          description: "Update a GitHub issue",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" },
              issueNumber: { type: "integer", description: "The issue number" },
              title: { type: "string", description: "The new title for the issue" },
              body: { type: "string", description: "The new body for the issue" },
              state: { type: "string", enum: ["open", "closed"], description: "The new state for the issue" },
              labels: { type: "array", items: { type: "string" }, description: "Labels to set on the issue" },
              assignees: { type: "array", items: { type: "string" }, description: "Users to assign to the issue" }
            },
            required: ["owner", "repo", "issueNumber"]
          }
        }
      },
      [TOOL_NAMES.GET_REPOSITORY]: {
        type: "function",
        function: {
          name: TOOL_NAMES.GET_REPOSITORY,
          description: "Get information about a GitHub repository",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" }
            },
            required: ["owner", "repo"]
          }
        }
      },
      [TOOL_NAMES.GET_ISSUE_COMMENTS]: {
        type: "function",
        function: {
          name: TOOL_NAMES.GET_ISSUE_COMMENTS,
          description: "Get comments on a GitHub issue",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" },
              issueNumber: { type: "integer", description: "The issue number" }
            },
            required: ["owner", "repo", "issueNumber"]
          }
        }
      },
      [TOOL_NAMES.CREATE_AGENT_STATE]: {
        type: "function",
        function: {
          name: TOOL_NAMES.CREATE_AGENT_STATE,
          description: "Create an agent state for a GitHub issue",
          parameters: {
            type: "object",
            properties: {
              owner: { type: "string", description: "The owner of the repository" },
              repo: { type: "string", description: "The name of the repository" },
              issueNumber: { type: "integer", description: "The issue number" }
            },
            required: ["owner", "repo", "issueNumber"]
          }
        }
      },
      [TOOL_NAMES.LOAD_AGENT_STATE]: {
        type: "function",
        function: {
          name: TOOL_NAMES.LOAD_AGENT_STATE,
          description: "Load an agent state by instance ID",
          parameters: {
            type: "object",
            properties: {
              instanceId: { type: "string", description: "The instance ID of the state to load" }
            },
            required: ["instanceId"]
          }
        }
      },
      [TOOL_NAMES.SAVE_AGENT_STATE]: {
        type: "function",
        function: {
          name: TOOL_NAMES.SAVE_AGENT_STATE,
          description: "Save an agent state",
          parameters: {
            type: "object",
            properties: {
              state: { type: "object", description: "The agent state to save" }
            },
            required: ["state"]
          }
        }
      }
    }

    // Create the toolkit object with schema definitions
    const toolkit = {
      tools: toolSchemas
    }

    return {
      tools: toolkit as unknown as AiToolkit.AiToolkit<any>,
      handlers
    }
  })
).pipe(Layer.provide(GitHubClient.Default))

// Default implementation
export const GitHubToolsDefault = GitHubToolsLayer