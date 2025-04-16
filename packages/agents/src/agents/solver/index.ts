import { Agent, type Connection, type WSMessage } from "agents";
import { type UIMessage, generateId, generateText, generateObject, type ToolSet } from "ai";
import { AsyncLocalStorage } from "node:async_hooks";
import type { Env } from "../../types";
import type { UIPart } from "@openagents/core/src/chat/types";
import type { ToolContext } from "@openagents/core/src/tools/toolContext";
import { tools as commonTools } from "../../common/tools";
import { solverTools } from "./tools";
import { getSolverSystemPrompt } from "./prompts";
import { openrouter, model, smallModel } from "../../common/config";
import type { SolverState, Problem, SolutionStep } from "./types";

export const solverContext = new AsyncLocalStorage<Solver>();

/**
 * Chat Agent implementation that handles problem-solving interactions
 */
export class Solver extends Agent<Env, SolverState> {
  initialState: SolverState = {
    messages: [],
    githubToken: undefined,
    scratchpad: '',
    observations: []
  };
  tools: ToolSet = {};
  
  private ctx: any; // DurableObjectState type
  
  constructor(ctx: any, env: Env) {
    super(ctx, env);
    this.ctx = ctx; // Store ctx for direct storage access
    console.log("[Constructor] Solver instance created.");
  }

  /**
   * Safely updates the agent's state by merging the provided partial state
   * with the existing state. Ensures ...this.state is always included.
   * @param partialState An object containing the state properties to update.
   */
  private updateState(partialState: Partial<SolverState>) {
    this.setState({
      ...this.state,
      ...partialState,
    });
    console.log('[updateState] Updated in-memory state via this.setState.');
  }

  /**
   * Adds an observation to the agent's state
   */
  async addAgentObservation(observation: string) {
    await this.updateState({
      observations: [...(this.state.observations || []), observation]
    });
  }

  /**
   * Updates the agent's scratchpad with thought
   */
  private async updateScratchpad(thought: string) {
    const timestamp = new Date().toISOString();
    const formattedThought = `${timestamp}: ${thought}`;
    
    this.updateState({
      scratchpad: this.state.scratchpad
        ? `${this.state.scratchpad}\n- ${formattedThought}`
        : `- ${formattedThought}`
    });
  }

  /**
   * Handles incoming WebSocket messages
   */
  async onMessage(connection: Connection, message: WSMessage) {
    try {
      const parsedMessage = JSON.parse(message as string);

      // Create a safe copy for logging that redacts sensitive information
      const safeMessageForLogging = { ...parsedMessage };
      if (safeMessageForLogging.githubToken) {
        safeMessageForLogging.githubToken = "[REDACTED]";
      }

      console.log("ON MESSAGE RECEIVED:", safeMessageForLogging);

      // Flag to decide whether to call infer
      let callInfer = false;

      // GitHub Token handling
      if (parsedMessage.githubToken) {
        console.log("Processing githubToken update...");
        const githubToken = parsedMessage.githubToken;
        await this.updateState({
          githubToken
        });

        // Only call infer if there's also a user message present
        if (parsedMessage.userMessage && parsedMessage.userMessage.content) {
          console.log("User message present with token, will call infer.");
          callInfer = true;
        } else {
          console.log("Token update only, not calling infer.");
          return; // Exit if only token was updated
        }
      }

      // User Message handling
      else if (parsedMessage.userMessage && parsedMessage.userMessage.content) {
        console.log("User message present, will call infer.");
        callInfer = true;
      }

      // Unhandled message type
      else {
        console.warn("Received unhandled message structure via send():", safeMessageForLogging);
        return; // Exit for unhandled message types
      }

      // Call infer only if flagged to do so
      if (callInfer) {
        console.log("Calling infer() based on message contents...");
        this.infer();
      }

    } catch (error) {
      console.error("Error processing received message:", error);
      console.error("Error parsing message - message is not logged for security");
    }
  }

  /**
   * Main inference method that generates AI responses based on the current state
   */
  @unstable_callable({
    description: "Generate an AI response for problem-solving",
    streaming: true
  })
  async infer() {
    return solverContext.run(this, async () => {
      // Add initial planning thought
      await this.updateScratchpad("Analyzing problem and planning solution");

      // Use githubToken from state if available
      const token = this.state.githubToken;

      // Get current state messages
      let messages = this.state.messages || [];

      // If there's more than 10 messages, take the first 3 and last 5
      if (messages.length > 10) {
        messages = messages.slice(0, 3).concat(messages.slice(-5));
        console.log("Truncated messages to first 3 and last 5", messages);
      }

      // Set up tool context
      const toolContext: ToolContext = { githubToken: token };
      
      // Combine solver-specific tools with common tools
      const tools = {
        ...solverTools,
        ...commonTools
      };

      // Generate system prompt based on current state
      const systemPrompt = getSolverSystemPrompt({
        state: this.state,
        model,
        temperature: 0.7
      });

      // Generate text using AI
      const result = await generateText({
        system: systemPrompt,
        model,
        messages,
        tools,
        maxTokens: 5000,
        temperature: 0.7,
        maxSteps: 5,
      });

      // Debug logging for result structure
      console.log("[Debug] Text response exists:", !!result.text);
      console.log("[Debug] Text response length:", result.text?.length || 0);
      console.log("[Debug] Tool calls length:", result.toolCalls?.length || 0);
      console.log("[Debug] Tool results length:", result.toolResults?.length || 0);

      // Add observation for the response
      if (result.text) {
        const snippet = result.text.length > 50
          ? `${result.text.substring(0, 50)}...`
          : result.text;

        await this.addAgentObservation(`Generated response: ${snippet}`);
      }

      // Create message parts array to handle both text and tool calls
      const messageParts: UIPart[] = [];

      // Add text part if there is text content
      if (result.text) {
        messageParts.push({
          type: 'text' as const,
          text: result.text
        });
      }

      // Process tool calls and results
      if (result.toolCalls && result.toolCalls.length > 0) {
        for (let i = 0; i < result.toolCalls.length; i++) {
          const toolCall = result.toolCalls[i];
          
          // Add observation for tool usage
          await this.addAgentObservation(`Used tool: ${toolCall.toolName} with args: ${JSON.stringify(toolCall.args)}`);
          
          // Find matching result if available
          const toolResult = result.toolResults && result.toolResults[i];
          
          if (toolResult) {
            // Add the tool with result to messageParts
            messageParts.push({
              type: 'tool-invocation' as const,
              toolInvocation: {
                state: 'result' as const,
                toolCallId: toolCall.toolCallId,
                toolName: toolCall.toolName as any,
                args: toolCall.args,
                result: toolResult.result
              }
            });
            
            // Add observation for tool result
            const resultSnippet = typeof toolResult.result === 'string' && toolResult.result.length > 50
              ? `${toolResult.result.substring(0, 50)}...`
              : JSON.stringify(toolResult.result).substring(0, 50) + '...';
            await this.addAgentObservation(`Tool result from ${toolCall.toolName}: ${resultSnippet}`);
          } else {
            // If we only have the call (tool hasn't finished), push the call part
            messageParts.push({
              type: 'tool-invocation' as const,
              toolInvocation: {
                state: 'call' as const,
                toolCallId: toolCall.toolCallId,
                toolName: toolCall.toolName as any,
                args: toolCall.args
              }
            });
          }
        }
      }

      // Add a thought about the interaction to the scratchpad
      if (messages.length > 0 && messages[messages.length - 1].role === 'user') {
        const lastUserMessage = messages[messages.length - 1].content;
        await this.updateScratchpad(`Processing user request: ${lastUserMessage}`);
      }

      // Finally, update state with the new message
      await this.updateState({
        messages: [
          ...messages,
          {
            id: generateId(),
            role: 'assistant' as const,
            content: result.text || '',
            createdAt: new Date(),
            parts: messageParts
          }
        ]
      });

      return {};
    });
  }
}