import { AIChatAgent } from "agents/ai-chat-agent";
import {
  createDataStreamResponse,
  generateId,
  streamText,
  type StreamTextOnFinishCallback,
} from "ai";
import { AsyncLocalStorage } from "node:async_hooks";
import { openai } from "@ai-sdk/openai";
import { processToolCalls } from "./utils";
import { coderTools, coderExecutions } from "./coder-tools";

// We use AsyncLocalStorage to expose the agent context to the tools
export const coderAgentContext = new AsyncLocalStorage<CoderAgent>();

/**
 * Specialized agent for coding assistance and development tasks
 *
 * This agent extends the base AIChatAgent to provide coding-specific functionality:
 * - Code generation and analysis
 * - Repository management
 * - File operations
 * - Command execution
 */
export class CoderAgent extends AIChatAgent<Env> {
  // Track the current repository/project context
  private projectContext: {
    repoOwner?: string;
    repoName?: string;
    branch?: string;
    path?: string;
  } = {};

  /**
   * Set the project context for this agent
   */
  setProjectContext(context: {
    repoOwner?: string;
    repoName?: string;
    branch?: string;
    path?: string;
  }) {
    this.projectContext = { ...this.projectContext, ...context };
    console.log("📁 Updated project context:", this.projectContext);
  }

  /**
   * Get the current project context
   */
  getProjectContext() {
    return { ...this.projectContext };
  }

  /**
   * Handles incoming chat messages for coding-related requests
   * @param onFinish - Callback function executed when streaming completes
   */
  async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    // Create a streaming response that handles both text and tool outputs
    return coderAgentContext.run(this, async () => {
      const dataStreamResponse = createDataStreamResponse({
        execute: async (dataStream) => {
          // Process any pending tool calls from previous messages
          const processedMessages = await processToolCalls({
            messages: this.messages,
            dataStream,
            tools: coderTools,
            executions: coderExecutions,
          });

          // Stream the AI response using Claude or GPT
          const model = openai("gpt-4o-2024-11-20"); // TODO: Make configurable or use Claude

          const result = streamText({
            model,
            system: `You are Claude, a helpful AI coding assistant specialized in software development.

You can help with:
- Writing, refactoring, and debugging code
- Explaining code concepts and implementation details
- Guiding software architecture decisions
- Searching through repositories and analyzing codebases
- Creating, reviewing, and managing pull requests
- Running shell commands to help with development tasks

You have access to specialized tools for coding tasks. When appropriate, use these tools to assist the user more effectively.

${this.projectContext.repoOwner && this.projectContext.repoName ?
                `Current project context: ${this.projectContext.repoOwner}/${this.projectContext.repoName}` +
                (this.projectContext.branch ? ` (branch: ${this.projectContext.branch})` : '') :
                'No specific project context set. You can help with general coding questions or set a repository context.'
              }

Always provide thoughtful, well-explained responses for coding tasks. If writing code, include clear comments and follow best practices.`,
            messages: processedMessages,
            tools: coderTools,
            onFinish,
            onError: (error) => {
              console.error("Error while streaming:", error);
            },
            maxSteps: 15, // More steps for complex coding tasks
          });

          // Merge the AI response stream with tool execution outputs
          result.mergeIntoDataStream(dataStream);
        },
      });

      return dataStreamResponse;
    });
  }

  /**
   * Execute a scheduled task for this agent
   */
  async executeScheduledTask(taskId: string, payload: any) {
    console.log(`Executing scheduled task ${taskId} with payload:`, payload);
    await this.saveMessages([
      ...this.messages,
      {
        id: generateId(),
        role: "assistant",
        content: `I've completed the scheduled task: ${payload.description || taskId}`,
        createdAt: new Date(),
      },
    ]);
    return { status: "completed", taskId };
  }
}
