// Base OpenAgent class that implements common functionality for all agents

import { Agent } from "agents";
import { generateId, generateText } from "ai";
import type { CoreMessage, ToolCallPart, ToolResultPart } from "ai";
import { createOpenRouter } from "@openrouter/ai-sdk-provider";
import type { BaseAgentState, InferProps, InferResponse } from "./types";
import { Effect, Runtime, Cause } from "effect";
import { solverTools } from "../agents/solver/tools";
import type { SolverToolName } from "../agents/solver/tools";
import { solverContext } from "../agents/solver/index";
import type { Solver } from "../agents/solver/index";
import { env } from "cloudflare:workers";

/**
 * Base OpenAgent class that implements common functionality for all agent types
 */
export class OpenAgent<T extends BaseAgentState> extends Agent<Env, T> {
  // Provide default base state that can be extended by subclasses
  // Using a method instead of a property to ensure proper typing when inherited
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

  /**
   * Safely updates the agent's state by merging the provided partial state
   * with the existing state. Ensures ...this.state is always included.
   * @param partialState An object containing the state properties to update.
   */
  protected updateState(partialState: Partial<T>) {
    this.setState({
      ...this.state,
      ...partialState,
    });
    console.log('[updateState] Updated in-memory state via this.setState.');
  }

  /**
   * Sets the GitHub token for the agent
   */
  setGithubToken(token: string) {
    this.updateState({
      githubToken: token
    } as Partial<T>);

    console.log("GitHub token updated");
    return { success: true, message: "GitHub token updated" };
  }

  /**
   * Gets the GitHub token for the agent
   */
  getGithubToken() {
    return this.state.githubToken || "";
  }

  /**
   * Sets the current repository context
   */
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

  /**
   * Sets the file currently being worked on
   */
  setCurrentFile(filePath: string) {
    this.updateState({
      workingFilePath: filePath
    } as Partial<T>);

    this.addAgentObservation(`Now working on file: ${filePath}`);

    return { success: true, message: `Current file set to ${filePath}` };
  }

  /**
   * Adds an observation to the agent's state
   */
  addAgentObservation(observation: string) {
    this.updateState({
      observations: [...(this.state.observations || []), observation]
    } as Partial<T>);

    return { success: true };
  }

  /**
   * Updates the agent's scratchpad with agent thoughts
   */
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

  /**
   * Gets the system prompt for the agent
   * This is a base implementation - agent subclasses should override to provide their specific system prompts
   */
  getSystemPrompt() {
    // Base system prompt that all agents can use
    const basePrompt = `You are an autonomous agent designed to assist with development tasks.
You have access to a repository and can help with understanding code, implementing features, and fixing issues.

Current context:
${this.state.currentRepoOwner ? `Repository: ${this.state.currentRepoOwner}/${this.state.currentRepoName}` : 'No repository set'}
${this.state.currentBranch ? `Branch: ${this.state.currentBranch}` : ''}
${this.state.workingFilePath ? `Current file: ${this.state.workingFilePath}` : ''}`;

    return basePrompt;
  }

  /**
   * Helper function to execute a tool using Effect and return a toolResult part
   */
  private async executeToolEffect(toolCall: ToolCallPart): Promise<ToolResultPart> {
    const { toolName, args, toolCallId } = toolCall;
    const tool = solverTools[toolName as SolverToolName];

    if (!tool) {
      return {
        type: 'tool-result',
        toolCallId,
        toolName,
        result: { error: `Tool '${toolName}' not found` }
      } as ToolResultPart;
    }

    console.log(`[executeToolEffect] Executing tool ${toolName} with args:`, args);

    try {
      // Simple approach to tool execution that works with any tool type
      let resultValue;
      
      if (!tool.execute || typeof tool.execute !== 'function') {
        throw new Error(`Tool '${toolName}' has no execute method`);
      }
      
      // TypeScript needs help understanding this function call
      // We use 'any' here because we're at a boundary between systems
      const execFn = tool.execute as any; 
      resultValue = await execFn(args);

      console.log(`[executeToolEffect] Tool ${toolName} executed successfully`);
      return {
        type: 'tool-result',
        toolCallId,
        toolName,
        result: resultValue
      } as ToolResultPart;
    } catch (error) {
      console.error(`[executeToolEffect] Tool ${toolName} execution failed:`, error);
      
      // Handle FiberFailure from Effect-based tools
      if (error && (error as any).cause) {
        const cause = (error as any).cause as Cause.Cause<any>;
        let errorMessage = `Tool '${toolName}' failed.`;
        
        // Analyze the Cause to provide meaningful error message
        if (Cause.isFailType(cause)) {
          const specificError = cause.error;
          // Handle different error types based on their _tag
          if (specificError && specificError._tag) {
            if (specificError._tag === "FileNotFoundError") {
              errorMessage = `File not found: ${specificError.path} in ${specificError.owner}/${specificError.repo}${specificError.branch ? ` (branch: ${specificError.branch})` : ''}`;
            } else if (specificError._tag === "GitHubApiError") {
              errorMessage = `GitHub API Error: ${specificError.status ? `(${specificError.status}) ` : ''}${specificError.message}`;
            } else if (specificError._tag === "InvalidPathError") {
              errorMessage = `Invalid path: ${specificError.message}`;
            } else if (specificError._tag === "ContentDecodingError") {
              errorMessage = `Content decoding error: ${specificError.message}`;
            } else {
              errorMessage = `Error (${specificError._tag}): ${specificError.message || JSON.stringify(specificError)}`;
            }
          }
        } else if (Cause.isDieType(cause)) {
          // Handle defects (bugs in our code)
          console.error("Tool defected:", cause.defect);
          errorMessage = "Internal error in tool execution.";
        } else if (Cause.isInterruptType(cause)) {
          errorMessage = "Tool execution was interrupted.";
        }
        
        return {
          type: 'tool-result',
          toolCallId,
          toolName,
          result: { error: errorMessage }
        } as ToolResultPart;
      }
      
      // Handle standard errors
      return {
        type: 'tool-result',
        toolCallId,
        toolName,
        result: { error: error instanceof Error ? error.message : String(error) }
      } as ToolResultPart;
    }
  }

  /**
   * Shared inference method for all agents
   * Uses Vercel AI SDK with OpenRouter to generate responses
   * @param props Inference properties including model, messages, and system prompt
   * @returns Response from the AI model
   */
  async sharedInfer(props: InferProps): Promise<InferResponse> {
    // Extract model and parameters from props
    const { 
      model = "anthropic/claude-3.5-sonnet", 
      messages: initialMessages, 
      system, 
      temperature = 0.7, 
      max_tokens = 1024, 
      top_p = 0.95 
    } = props;
    
    console.log("[sharedInfer] Starting inference with model:", model);

    try {
      // --- Prepare shared resources ---
      const openrouter = createOpenRouter({ 
        apiKey: (this.env.OPENROUTER_API_KEY as string) || process.env.OPENROUTER_API_KEY || ''
      });
      
      console.log("[sharedInfer] Created OpenRouter provider");
      
      // Maximum number of tool roundtrips to prevent infinite loops
      const maxToolRoundtrips = 5;

      // Prepare initial message list and system prompt
      let currentMessages: CoreMessage[] = [];
      
      // Format initialMessages into CoreMessage format
      if (initialMessages && initialMessages.length > 0) {
        currentMessages = initialMessages.map(msg => {
          // Convert to CoreMessage by explicit role assignment
          if (msg.role === 'system') {
            return { role: 'system', content: msg.content };
          } else if (msg.role === 'user') {
            return { role: 'user', content: msg.content };
          } else if (msg.role === 'assistant') {
            return { role: 'assistant', content: msg.content };
          } else {
            // Default to user for other roles like 'data'
            return { role: 'user', content: msg.content };
          }
        });
      }
      
      // Get system prompt and ensure it's at the start of messages
      const systemPrompt = system || this.getSystemPrompt();
      if (systemPrompt) {
        // Update or insert system message
        if (currentMessages.length > 0 && currentMessages[0].role === 'system') {
          currentMessages[0] = { role: 'system', content: systemPrompt };
        } else {
          currentMessages.unshift({ role: 'system', content: systemPrompt });
        }
      }
      
      console.log(`[sharedInfer] Prepared ${currentMessages.length} messages with system prompt`);

      // --- Tool Execution Loop ---
      let textResponse = ''; // Store the final text response
      let toolCallsResult: ToolCallPart[] | undefined;
      let toolResultsList: ToolResultPart[] = [];

      for (let i = 0; i < maxToolRoundtrips; i++) {
        console.log(`[sharedInfer] Starting LLM Call ${i + 1}`);

        // Create a type assertion function to ensure this is compatible with Solver
        const asSolver = <T extends BaseAgentState>(agent: OpenAgent<T>): unknown => agent;
        
        // Ensure agent instance is available via solverContext
        const result = await solverContext.run(asSolver(this) as Solver, async () => {
          return generateText({
            model: openrouter(model),
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

        // Store the text response from this call
        textResponse = text;
        toolCallsResult = toolCalls;
        toolResultsList = []; // Reset results for this roundtrip

        // If no tool calls, break the loop
        if (!toolCalls || toolCalls.length === 0) {
          console.log('[sharedInfer] No tool calls made by LLM. Exiting loop.');
          break;
        }

        console.log(`[sharedInfer] LLM requested ${toolCalls.length} tool calls.`);

        // Execute tool calls
        const toolExecutionPromises = toolCalls.map(toolCall => this.executeToolEffect(toolCall));
        const results = await Promise.all(toolExecutionPromises);
        toolResultsList = results;

        // Add the assistant message with tool calls and the tool results
        currentMessages.push({
          role: 'assistant',
          content: [{ type: 'text', text }, ...toolCalls]
        });
        
        currentMessages.push({
          role: 'tool',
          content: results
        });

        // Check if max roundtrips reached
        if (i === maxToolRoundtrips - 1) {
          console.warn('[sharedInfer] Maximum tool roundtrips reached.');
          textResponse = textResponse + "\n\n(Maximum tool steps reached)";
          break;
        }
      }

      console.log("[sharedInfer] Inference completed successfully");
      
      // Return a properly formatted response with the final text
      return {
        id: generateId(),
        content: textResponse,
        role: "assistant",
        timestamp: new Date().toISOString(),
        model: model
      };
    } catch (error) {
      console.error("[sharedInfer] Error during AI inference:", error);
      
      // Return a response with the error message
      return {
        id: generateId(),
        content: `Error generating response: ${error instanceof Error ? error.message : String(error)}`,
        role: "assistant",
        timestamp: new Date().toISOString(),
        model: model
      };
    }
  }
}