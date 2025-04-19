// Base OpenAgent class that implements common functionality for all agents

import { Agent } from "agents";
import { generateId, generateText } from "ai";
import type { CoreMessage, ToolCallPart, ToolResultPart } from "ai"; // Correct type-only import
import { createOpenRouter } from "@openrouter/ai-sdk-provider";
import type { BaseAgentState, InferProps, InferResponse } from "./types";
import { Effect, Runtime, Cause } from "effect"; // Correct imports
import { solverTools } from "../agents/solver/tools";
import type { SolverToolName } from "../agents/solver/tools"; // Correct type-only import
import { solverContext } from "../agents/solver/index";
import type { Solver } from "../agents/solver/index"; // Correct type-only import
import { env } from "cloudflare:workers";

/**
 * Base OpenAgent class that implements common functionality for all agent types
 */
export class OpenAgent<T extends BaseAgentState> extends Agent<Env, T> {
  // ... (getBaseInitialState, constructor, updateState, token methods, context methods remain the same) ...

  protected getBaseInitialState(): BaseAgentState {
    return {
      messages: [],
      githubToken: undefined,
      currentRepoOwner: undefined,
      currentRepoName: undefined,
      currentBranch: undefined,
      scratchpad: '',
      observations: [],
      workingFilePath: undefined
    };
  }

  constructor(ctx: any, env: Env) {
    super(ctx, env);
  }

  protected updateState(partialState: Partial<T>) {
    this.setState({
      ...this.state,
      ...partialState,
    });
    console.log('[updateState] Updated in-memory state via this.setState.');
  }

  /**
   * Sets the GitHub token in the agent's state and ensures it's properly persisted
   * This method has been hardened to ensure token persistence
   * 
   * IMPORTANT: This must be defined as a regular method, not an async method,
   * because async methods can't be called over the RPC interface.
   */
  setGithubToken(token: string) {
    if (!token) {
      console.error("setGithubToken: Empty or null token was provided!");
      return { success: false, message: "GitHub token cannot be empty" };
    }
    
    console.log(`setGithubToken: Setting GitHub token (length: ${token.length})`);
    
    // First, update the state with the new token
    this.updateState({
      githubToken: token
    } as Partial<T>);

    // Force a state persistence - but using syncState since we can't use async
    this.setState({
      ...this.state,
      githubToken: token
    });
    
    // Double verify the token was properly set
    if (this.state.githubToken !== token) {
      console.error("setGithubToken: Token was not properly set in state after direct setState!");
      return { success: false, message: "Failed to update GitHub token in state" };
    }
    
    // Add an observation to record this action
    try {
      this.addAgentObservation(`GitHub token updated (length: ${token.length})`);
    } catch (e) {
      console.error("Failed to add observation:", e);
    }

    console.log("GitHub token updated successfully and persisted");
    return { success: true, message: "GitHub token updated", tokenLength: token.length };
  }

  getGithubToken() {
    return this.state.githubToken || "";
  }

  setRepositoryContext(owner: string, repo: string, branch: string = 'main') {
    console.log(`[setRepositoryContext] Setting context to ${owner}/${repo} on branch ${branch}`);

    this.updateState({
      currentRepoOwner: owner,
      currentRepoName: repo,
      currentBranch: branch
    } as Partial<T>);

    this.addAgentObservation(`Repository context set to ${owner}/${repo}:${branch}`);

    return { success: true, message: `Context set to ${owner}/${repo}:${branch}` };
  }

  setCurrentFile(filePath: string) {
    this.updateState({
      workingFilePath: filePath
    } as Partial<T>);

    this.addAgentObservation(`Now working on file: ${filePath}`);

    return { success: true, message: `Current file set to ${filePath}` };
  }

  addAgentObservation(observation: string) {
    this.updateState({
      observations: [...(this.state.observations || []), observation]
    } as Partial<T>);

    return { success: true };
  }

  protected updateScratchpad(thought: string) {
    const timestamp = new Date().toISOString();
    const formattedThought = `${timestamp}: ${thought}`;

    this.updateState({
      scratchpad: this.state.scratchpad
        ? `${this.state.scratchpad}\n- ${formattedThought}`
        : `- ${formattedThought}`
    } as Partial<T>);

    return { success: true };
  }

  getSystemPrompt() {
    // ... (implementation remains the same) ...
    const basePrompt = `You are an autonomous agent designed to assist with development tasks.
You have access to a repository and can help with understanding code, implementing features, and fixing issues.

Current context:
${this.state.currentRepoOwner ? `Repository: ${this.state.currentRepoOwner}/${this.state.currentRepoName}` : 'No repository set'}
${this.state.currentBranch ? `Branch: ${this.state.currentBranch}` : ''}
${this.state.workingFilePath ? `Current file: ${this.state.workingFilePath}` : ''}`;
    return basePrompt;
  }


  /**
   * Helper function to execute a tool, handling both Effect-based and standard tools.
   */
  private async executeToolEffect(toolCall: ToolCallPart): Promise<ToolResultPart> {
    const { toolName, args, toolCallId } = toolCall;
    const tool = solverTools[toolName as SolverToolName];

    // --- Check if tool exists ---
    if (!tool || typeof tool.execute !== 'function') {
      console.error(`[executeToolEffect] Tool '${toolName}' not found or has no execute method.`);
      return {
        type: 'tool-result', // Correct type for Vercel AI SDK
        toolCallId,
        toolName,
        result: { error: `Tool '${toolName}' not found or is not executable.` }
      } as ToolResultPart; // Type assertion for clarity
    }

    // Add GitHub token to args for GitHub-related tools if not already present
    let enhancedArgs = { ...args };
    if ((toolName === 'fetchFileContents' || toolName === 'getFileContents') && 
        !enhancedArgs.token && this.state.githubToken) {
      console.log(`[executeToolEffect] Adding GitHub token to tool args (length: ${this.state.githubToken.length})`);
      enhancedArgs.token = this.state.githubToken;
    }

    console.log(`[executeToolEffect] Executing tool '${toolName}' with args:`, enhancedArgs);

    try {
      // Execute the tool with standard Promise-based approach
      console.log(`[executeToolEffect] Executing tool '${toolName}'...`);

      // Call the tool's execute function with the expected arguments format
      const execFn = tool.execute as (a: any, o: any) => Promise<any>;
      const resultValue = await Promise.resolve(execFn(enhancedArgs, {}));

      console.log(`[executeToolEffect] Tool '${toolName}' completed successfully.`);

      // Return success ToolResultPart
      return {
        type: 'tool-result',
        toolCallId,
        toolName,
        result: resultValue // Vercel SDK expects the actual result here
      } as ToolResultPart;

    } catch (error) {
      // Enhanced error handling with detailed logging
      console.error(`[executeToolEffect] Tool '${toolName}' execution failed. Raw error:`, error);
      
      // Check if this is an enriched error from our effectTool
      const isEnrichedError = error instanceof Error && 
                              (error as any).effectError !== undefined;
      console.log(`[executeToolEffect] Is enriched error? ${isEnrichedError}`);
      
      // Access the GitHub token for debugging (to confirm if it's really missing)
      console.log(`[executeToolEffect] Agent state check:`, { 
        hasToken: !!this.state.githubToken,
        tokenLength: this.state.githubToken ? this.state.githubToken.length : 0,
        repoContext: `${this.state.currentRepoOwner || 'none'}/${this.state.currentRepoName || 'none'}`
      });
      
      // Create a user-friendly error message
      let errorMessage = "Tool execution failed.";
      
      if (error instanceof Error) {
        // Use the error message directly
        errorMessage = error.message;
        
        // Special handling for GitHub token errors (detected by message content)
        if (error.message.includes("GitHub token")) {
          errorMessage = `GitHub token is missing. Please make sure you've connected your GitHub account.`;
          
          // Try to set a diagnostic observation
          try {
            this.addAgentObservation("Error detected: GitHub token is missing or invalid");
          } catch (e) {
            console.error("Failed to add observation:", e);
          }
        }
      } else {
        // Fallback for non-Error objects
        errorMessage = `Tool '${toolName}' failed: ${String(error)}`;
      }
      
      console.log(`[executeToolEffect] Formatted error message: ${errorMessage}`);
      
      // Enhanced logging for enriched errors
      if (isEnrichedError) {
        const enrichedError = error as any;
        console.log("[executeToolEffect] Enriched error details:", {
          causeType: enrichedError.causeType || 'unknown',
          effectError: enrichedError.effectError || {},
          hasOriginalError: !!enrichedError.originalError,
          errorMessage: errorMessage
        });
      }

      // Return error ToolResultPart with detailed information if available
      return {
        type: 'tool-result',
        toolCallId,
        toolName,
        result: {
          error: errorMessage,
          // Add detailed error information if available (from effectTool)
          ...(isEnrichedError && {
            errorDetails: (error as any).effectError,
            errorType: (error as any).causeType
          })
        }
      } as ToolResultPart;
    }
  }

  /**
   * Shared inference method using Vercel AI SDK and OpenRouter.
   * Handles multi-turn tool calls.
   */
  async sharedInfer(props: InferProps): Promise<InferResponse> {
    // Extract parameters from props, but explicitly set OpenRouter model
    const {
      model: requestedModel = "anthropic/claude-3.5-sonnet",
      messages: initialMessages,
      system,
      temperature = 0.7,
      max_tokens = 1024,
      top_p = 0.95,
      githubToken // Extract GitHub token if provided
    } = props;

    // Define the OpenRouter model identifier explicitly to avoid issues with CF AI models
    const openRouterModel = "anthropic/claude-3.5-sonnet";

    console.log("[sharedInfer] Requested model:", requestedModel);
    console.log("[sharedInfer] Using OpenRouter model:", openRouterModel);

    try {
      // If a token was provided as a parameter, apply it directly to the state
      if (githubToken && typeof githubToken === 'string') {
        console.log(`[sharedInfer] Using GitHub token from parameters (length: ${githubToken.length})`);
        try {
          // Apply token directly to state instead of calling setGithubToken
          this.updateState({
            githubToken: githubToken
          } as Partial<T>);
          console.log("[sharedInfer] ✓ Token applied to state");
        } catch (tokenError) {
          console.error("[sharedInfer] ✗ Failed to apply token to state:", tokenError);
        }
      }
      
      // Check GitHub token status before attempting any operations
      const hasValidToken = !!this.state.githubToken && this.state.githubToken.length > 0;
      console.log("[sharedInfer] GitHub token status:", {
        hasToken: hasValidToken,
        tokenLength: this.state.githubToken ? this.state.githubToken.length : 0, 
        repoContext: `${this.state.currentRepoOwner || 'none'}/${this.state.currentRepoName || 'none'}`,
        tokenProvidedInParams: !!githubToken
      });
      
      // If we might need to use GitHub-related tools, warn about missing token
      if (!hasValidToken) {
        console.warn("[sharedInfer] No valid GitHub token found. GitHub-related tools will fail.");
        this.addAgentObservation("Warning: No GitHub token available. GitHub operations will fail.");
      }
      
      const openrouter = createOpenRouter({
        apiKey: (this.env.OPENROUTER_API_KEY as string) || process.env.OPENROUTER_API_KEY || ''
      });
      console.log("[sharedInfer] Created OpenRouter provider");

      const maxToolRoundtrips = 5;
      let currentMessages: CoreMessage[] = [];

      // ... (Message formatting and system prompt handling remain the same) ...
      if (initialMessages && initialMessages.length > 0) {
        currentMessages = initialMessages.map(msg => {
          if (msg.role === 'system') {
            return { role: 'system', content: msg.content };
          } else if (msg.role === 'user') {
            return { role: 'user', content: msg.content };
          } else if (msg.role === 'assistant') {
            return { role: 'assistant', content: msg.content };
          } else {
            return { role: 'user', content: msg.content };
          }
        });
      }

      const systemPrompt = system || this.getSystemPrompt();
      if (systemPrompt) {
        if (currentMessages.length > 0 && currentMessages[0].role === 'system') {
          currentMessages[0] = { role: 'system', content: systemPrompt };
        } else {
          currentMessages.unshift({ role: 'system', content: systemPrompt });
        }
      }
      console.log(`[sharedInfer] Prepared ${currentMessages.length} messages with system prompt`);


      let textResponse = '';
      let toolCallsResult: ToolCallPart[] | undefined;

      // --- Tool Execution Loop ---
      for (let i = 0; i < maxToolRoundtrips; i++) {
        console.log(`[sharedInfer] Starting LLM Call ${i + 1}`);

        const asSolver = <T extends BaseAgentState>(agent: OpenAgent<T>): unknown => agent;

        // Ensure AsyncLocalStorage context is set for tool execution
        const result = await solverContext.run(asSolver(this) as Solver, async () => {
          return generateText({
            model: openrouter(openRouterModel), // Use the explicitly defined OpenRouter model
            messages: currentMessages,
            tools: solverTools,
            toolChoice: 'auto',
            temperature,
            maxTokens: max_tokens,
            topP: top_p
          });
        });

        const { text, toolCalls, finishReason, usage } = result;
        console.log(`[sharedInfer] LLM Call ${i + 1} finished. Reason: ${finishReason}`);
        console.log(`[sharedInfer] LLM Usage: Prompt ${usage?.promptTokens}, Completion ${usage?.completionTokens}`);

        textResponse = text; // Update text response from this round

        if (!toolCalls || toolCalls.length === 0) {
          console.log('[sharedInfer] No tool calls made by LLM. Exiting loop.');
          break; // Exit loop if no tools called
        }

        console.log(`[sharedInfer] LLM requested ${toolCalls.length} tool calls.`);

        // Execute all tool calls concurrently, injecting GitHub token when appropriate
        const toolExecutionPromises = toolCalls.map(toolCall => {
          // Check if this is a GitHub-related tool call that might need a token
          if (toolCall.toolName === 'fetchFileContents' || toolCall.toolName === 'getFileContents') {
            // Clone the tool call and add token from params or state if not already present
            const enhancedToolCall = { ...toolCall };
            
            // Use explicit token from params, or fallback to state
            const tokenToUse = githubToken || this.state.githubToken;
            
            if (tokenToUse && !enhancedToolCall.args.token) {
              enhancedToolCall.args = { 
                ...enhancedToolCall.args,
                token: tokenToUse
              };
              console.log(`[sharedInfer] Adding token to ${toolCall.toolName} call (token length: ${tokenToUse.length})`);
            }
            return this.executeToolEffect(enhancedToolCall);
          } else {
            // Use the original tool call for other tools
            return this.executeToolEffect(toolCall);
          }
        });
        
        const toolResultsList = await Promise.all(toolExecutionPromises);

        // Add assistant message with tool requests and the tool results message
        currentMessages.push({
          role: 'assistant',
          content: [{ type: 'text', text }, ...toolCalls]
        });
        currentMessages.push({
          role: 'tool',
          content: toolResultsList // Use the results from executeToolEffect
        });

        if (i === maxToolRoundtrips - 1) {
          console.warn('[sharedInfer] Maximum tool roundtrips reached.');
          textResponse = textResponse + "\n\n(Maximum tool steps reached)";
          break;
        }
      } // End of loop

      console.log("[sharedInfer] Inference completed successfully");
      // Return the final response
      return {
        id: generateId(),
        content: textResponse, // Final text after potential tool use
        role: "assistant",
        timestamp: new Date().toISOString(),
        model: openRouterModel // Use the explicitly defined OpenRouter model
      };

    } catch (error) {
      // Enhanced error logging to help debug OpenRouter issues
      console.error("[sharedInfer] Error during AI inference:", error);

      // Log more details about the error when available
      if (error instanceof Error) {
        if ((error as any).cause) {
          console.error("[sharedInfer] Error cause:", (error as any).cause);
        }
        if ((error as any).data) {
          console.error("[sharedInfer] Error data:", (error as any).data);
        }
        if ((error as any).response) {
          try {
            console.error("[sharedInfer] Error response:", JSON.stringify((error as any).response));
          } catch (e) {
            console.error("[sharedInfer] Error response (non-stringifiable):", (error as any).response);
          }
        }
      }
      return {
        id: generateId(),
        content: `Error generating response: ${error instanceof Error ? error.message : String(error)}`,
        role: "assistant",
        timestamp: new Date().toISOString(),
        model: openRouterModel // Use the explicitly defined OpenRouter model
      };
    }
  } // End sharedInfer
} // End OpenAgent class
