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

  setGithubToken(token: string) {
    this.updateState({
      githubToken: token
    } as Partial<T>);

    console.log("GitHub token updated");
    return { success: true, message: "GitHub token updated" };
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

    console.log(`[executeToolEffect] Executing tool '${toolName}' with args:`, args);

    try {
      let resultValue: unknown;

      // We no longer need special handling for Effect-based tools
      // since effectTool wraps them in Promises
      console.log(`[executeToolEffect] Executing tool '${toolName}'...`);
      
      // Call the tool's execute function with the expected arguments format
      const execFn = tool.execute as (a: any, o: any) => Promise<any>;
      resultValue = await Promise.resolve(execFn(args, {}));
      console.log(`[executeToolEffect] Tool '${toolName}' completed successfully.`);
      // Note: Our effectTool factory automatically wraps Effect-based tools in Promises,
      // so we no longer need special handling here for Effect-based vs standard tools

      // --- Return success ToolResultPart ---
      return {
        type: 'tool-result',
        toolCallId,
        toolName,
        result: resultValue // Vercel SDK expects the actual result here
      } as ToolResultPart;

    } catch (error) {
      // --- Handle ALL errors (from runPromise or standard execute) ---
      console.error(`[executeToolEffect] Tool '${toolName}' execution failed:`, error);
      let errorMessage = `Tool '${toolName}' failed.`;

      // Check if it's a FiberFailure from Effect.runPromise
      if (error && (error as any)[Symbol.toStringTag] === 'FiberFailure') {
          const cause = (error as any).cause as Cause.Cause<any>;
          console.log(`[executeToolEffect] Analyzing Effect failure Cause for tool '${toolName}'...`);
          // Analyze Cause (same logic as before)
          if (Cause.isFailType(cause)) {
            const specificError = cause.error as any; // Cast to any to check _tag
            if (specificError && specificError._tag) {
              // Map specific tagged errors to user-friendly messages
              if (specificError._tag === "FileNotFoundError") { /* ... */ errorMessage = `File not found...`; }
              else if (specificError._tag === "GitHubApiError") { /* ... */ errorMessage = `GitHub API Error...`; }
              // ... other tagged errors
              else { errorMessage = `Error (${specificError._tag}): ${specificError.message || JSON.stringify(specificError)}`; }
            } else {
              // Handle untagged Fail errors
               errorMessage = `Tool '${toolName}' failed with: ${JSON.stringify(specificError)}`;
            }
          } else if (Cause.isDieType(cause)) {
            console.error(`Tool '${toolName}' defected:`, Cause.pretty(cause)); // Log pretty cause for defects
            errorMessage = `An internal error occurred while executing tool '${toolName}'.`;
          } else if (Cause.isInterruptType(cause)) {
            console.warn(`Tool '${toolName}' was interrupted.`);
            errorMessage = `Tool '${toolName}' execution was interrupted.`;
          } else {
             console.error(`Tool '${toolName}' failed with unknown cause:`, Cause.pretty(cause));
             errorMessage = `Tool '${toolName}' failed with an unknown error.`;
          }
      } else {
        // Standard JavaScript error from non-Effect tools or other issues
        errorMessage = `Tool '${toolName}' failed: ${error instanceof Error ? error.message : String(error)}`;
      }

      // --- Return error ToolResultPart ---
      return {
        type: 'tool-result',
        toolCallId,
        toolName,
        result: { error: errorMessage } // Vercel SDK convention for errors
      } as ToolResultPart;
    }
  }

  /**
   * Shared inference method using Vercel AI SDK and OpenRouter.
   * Handles multi-turn tool calls.
   */
  async sharedInfer(props: InferProps): Promise<InferResponse> {
    // ... (Parameter extraction and OpenRouter setup remain the same) ...
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

        textResponse = text; // Update text response from this round

        if (!toolCalls || toolCalls.length === 0) {
          console.log('[sharedInfer] No tool calls made by LLM. Exiting loop.');
          break; // Exit loop if no tools called
        }

        console.log(`[sharedInfer] LLM requested ${toolCalls.length} tool calls.`);

        // Execute all tool calls concurrently
        const toolExecutionPromises = toolCalls.map(toolCall => this.executeToolEffect(toolCall));
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
        model: model
      };

    } catch (error) {
      // ... (Error handling remains the same) ...
       console.error("[sharedInfer] Error during AI inference:", error);
      return {
        id: generateId(),
        content: `Error generating response: ${error instanceof Error ? error.message : String(error)}`,
        role: "assistant",
        timestamp: new Date().toISOString(),
        model: model
      };
    }
  } // End sharedInfer
} // End OpenAgent class