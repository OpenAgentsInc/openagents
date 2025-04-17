import { type Connection, type WSMessage } from "agents";
import { generateId, generateText, type ToolSet, type ToolResult } from "ai";
import { AsyncLocalStorage } from "node:async_hooks";
import type { ToolContext, UIPart } from "@openagents/core";
import { getFileContentsTool } from "../../common/tools/github/getFileContents";
import { addIssueCommentTool } from "../../common/tools/github/addIssueComment";
import { tools as commonTools } from "../../common/tools";
import { solverTools } from "./tools";
import { getSolverSystemPrompt } from "./prompts";
import { model } from "../../common/config";
import type { SolverState, Issue, ImplementationStep } from "./types";
import { OpenAgent } from "../../common/types";

export const solverContext = new AsyncLocalStorage<Solver>();

/**
 * Solver Agent that handles issue resolution in OpenAgents Projects
 */
export class Solver extends OpenAgent<SolverState> {
  // Initialize state by extending the base state with solver-specific properties
  initialState: SolverState = {
    ...this.baseInitialState as any, // Cast to any to avoid strict typing issues with the spread
    // Add solver-specific initial state properties here
  };
  tools: ToolSet = {};

  /**
   * Sets the current issue being worked on
   */
  setCurrentIssue(issue: Issue) {
    this.updateState({
      currentIssue: issue
    });

    this.addAgentObservation(`Now working on issue #${issue.number}: ${issue.title}`);

    return { success: true, message: `Set current issue to #${issue.number}` };
  }

  /**
   * Updates the implementation step status
   */
  updateStepStatus(stepId: string, status: ImplementationStep['status'], notes?: string) {
    if (!this.state.implementationSteps) return false;

    const updatedSteps = this.state.implementationSteps.map(step => {
      if (step.id === stepId) {
        return {
          ...step,
          status,
          ...(status === 'in_progress' ? { started: new Date() } : {}),
          ...(status === 'completed' ? { completed: new Date() } : {}),
          notes: notes ? (step.notes ? `${step.notes}\n\n${notes}` : notes) : step.notes
        };
      }
      return step;
    });

    this.updateState({
      implementationSteps: updatedSteps,
      observations: [...(this.state.observations || []), `Step ${stepId} status changed to ${status}${notes ? " with note" : ""}`]
    });

    return true;
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

      // --- Command Handling ---
      if (parsedMessage.type === 'command' && parsedMessage.command) {
        console.log(`Processing command: ${parsedMessage.command}`);

        switch (parsedMessage.command) {
          case 'setIssue':
            if (parsedMessage.issue) {
              this.setCurrentIssue(parsedMessage.issue);
            }
            break;
          case 'setRepo':
            if (parsedMessage.owner && parsedMessage.repo) {
              this.setRepositoryContext(
                parsedMessage.owner,
                parsedMessage.repo,
                parsedMessage.branch || 'main'
              );
            }
            break;
          default:
            console.warn(`Received unknown command: ${parsedMessage.command}`);
        }

        callInfer = true; // Call infer after processing commands
      }

      // --- GitHub Token Logic ---
      // Check if it's a message containing the token
      if (parsedMessage.githubToken) {
        console.log("Processing githubToken update...");
        const githubToken = parsedMessage.githubToken;
        this.updateState({
          githubToken
        });

        // Only call infer if there's also a user message present
        if (parsedMessage.userMessage && parsedMessage.userMessage.content) {
          console.log("User message present with token, will call infer.");
          callInfer = true;
        } else {
          console.log("Token update only, not calling infer yet.");
          // Don't exit here - we might also have an issue or repo update
        }
      }

      // --- Issue Information Logic ---
      if (parsedMessage.issue) {
        console.log("Processing issue update...");
        this.setCurrentIssue(parsedMessage.issue);
        callInfer = true;
      }

      // --- Repository Context Logic ---
      if (parsedMessage.repoOwner && parsedMessage.repoName) {
        console.log("Processing repository context update...");
        this.setRepositoryContext(
          parsedMessage.repoOwner,
          parsedMessage.repoName,
          parsedMessage.branch || 'main'
        );
        callInfer = true;
      }

      // --- User Message Handling ---
      // Check if there's a user message that needs inference
      if (parsedMessage.userMessage && parsedMessage.userMessage.content) {
        console.log("User message present, will call infer.");

        // Update messages array with the new user message
        this.updateState({
          messages: [
            ...(this.state.messages || []),
            {
              ...parsedMessage.userMessage,
              id: parsedMessage.userMessage.id || generateId(),
              role: 'user',
              createdAt: parsedMessage.userMessage.createdAt || new Date()
            }
          ]
        });

        callInfer = true;
      }

      // Call infer only if flagged to do so
      if (callInfer) {
        console.log("Calling infer() based on message contents...");
        this.infer();
      } else {
        console.log("Not calling infer() - no trigger detected.");
      }

    } catch (error) {
      console.error("Error processing received message:", error);
      console.error("Error parsing message - message is not logged for security");
    }
  }

  /**
   * Main inference method that generates AI responses based on the current state
   */
  async infer() {
    return solverContext.run(this, async () => {
      // Add initial planning thought
      this.updateScratchpad("Processing request and planning response");

      // Get GitHub token from state
      const token = this.state.githubToken;

      // Get current state messages
      let messages = this.state.messages || [];

      // If there's more than 10 messages, take the first 3 and last 5
      if (messages.length > 10) {
        messages = messages.slice(0, 3).concat(messages.slice(-5));
        console.log("Truncated messages to first 3 and last 5");
      }

      // Set up tool context
      const toolContext: ToolContext = { githubToken: token };

      // Combine solver-specific tools with common tools and GitHub tools
      const tools = {
        get_file_contents: getFileContentsTool(toolContext),
        add_issue_comment: addIssueCommentTool(toolContext),
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
        tools: tools as ToolSet,
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

        this.addAgentObservation(`Generated response: ${snippet}`);
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
          this.addAgentObservation(`Used tool: ${toolCall.toolName} with args: ${JSON.stringify(toolCall.args)}`);

          // Find matching result if available
          const toolResult = result.toolResults && result.toolResults[i] as ToolResult<string, any, any>;

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
            this.addAgentObservation(`Tool result from ${toolCall.toolName}: ${resultSnippet}`);

            // Update state based on tool results (e.g., if we fetched issue details)
            if (toolCall.toolName === 'getIssueDetails' && toolResult.result) {
              try {
                // If the result looks like an issue, update the current issue
                const resultObj = toolResult.result as any;
                if (resultObj.id && resultObj.number && resultObj.title) {
                  this.setCurrentIssue(resultObj);
                }
              } catch (error) {
                console.error("[infer] Error updating issue from tool result:", error);
              }
            }
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
        this.updateScratchpad(`Processing user request: ${lastUserMessage}`);
      }

      // Finally, update state with the new message
      this.updateState({
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
