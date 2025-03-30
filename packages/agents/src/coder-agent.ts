import { AIChatAgent } from "agents/ai-chat-agent";
import { unstable_callable } from "agents";
import {
  createDataStreamResponse,
  generateId,
  streamText,
  type StreamTextOnFinishCallback,
} from "ai";
import { AsyncLocalStorage } from "node:async_hooks";
import { processToolCalls } from "./utils";
import { coderTools, coderExecutions } from "./coder-tools";
import { createOpenRouter } from "@openrouter/ai-sdk-provider";

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
  projectContext: {
    repoOwner?: string;
    repoName?: string;
    branch?: string;
    path?: string;
  } = {};

  /**
   * Set the project context for this agent
   * This method is marked as callable by clients through RPC
   */
  @unstable_callable({
    description: "Set the repository context for the coding agent"
  })
  async setProjectContext(context: {
    repoOwner?: string;
    repoName?: string;
    branch?: string;
    path?: string;
  }) {
    try {
      console.log("📁 Setting project context:", JSON.stringify(context));
      this.projectContext = { ...this.projectContext, ...context };
      console.log("🔄 Updated project context:", JSON.stringify(this.projectContext));
      
      // Return the updated context to confirm success
      return { 
        success: true, 
        context: this.projectContext 
      };
    } catch (error) {
      console.error("❌ Error setting project context:", error);
      return {
        success: false,
        error: String(error)
      };
    }
  }

  /**
   * Get the current project context
   * This method is marked as callable by clients through RPC
   */
  @unstable_callable({
    description: "Get the current repository context"
  })
  async getProjectContext() {
    try {
      console.log("🔍 Getting project context:", JSON.stringify(this.projectContext));
      return { ...this.projectContext };
    } catch (error) {
      console.error("❌ Error getting project context:", error);
      return { error: String(error) };
    }
  }
  
  /**
   * Get messages for this agent
   * This method is marked as callable by clients through RPC
   */
  @unstable_callable({
    description: "Get the message history for this agent"
  })
  async getMessages() {
    try {
      console.log(`📋 Getting ${this.messages.length} messages from agent`);
      // Ensure we get the latest messages from storage
      const messages = this.sql`select * from cf_ai_chat_agent_messages order by created_at asc`;
      console.log(`📊 Found ${messages?.length || 0} messages in storage`);
      
      if (!messages || messages.length === 0) {
        console.log(`📝 No messages found in storage, returning in-memory messages`);
        return this.messages;
      }
      
      // Parse the messages from storage
      const parsedMessages = messages.map(row => {
        try {
          return JSON.parse(String(row.message));
        } catch (e) {
          console.error(`❌ Error parsing message: ${e}`);
          return null;
        }
      }).filter(Boolean);
      
      console.log(`🔄 Returning ${parsedMessages.length} messages from storage`);
      return parsedMessages;
    } catch (error) {
      console.error("❌ Error getting messages:", error);
      // Return empty array instead of error to avoid breaking the client
      return [];
    }
  }

  /**
   * Handles incoming chat messages for coding-related requests
   * @param onFinish - Callback function executed when streaming completes
   */
  async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    console.log(`📝 CoderAgent.onChatMessage called with ${this.messages.length} messages`);

    // Check for required OpenRouter API key
    if (!process.env.OPENROUTER_API_KEY) {
      console.error("🚨 CRITICAL ERROR: OPENROUTER_API_KEY environment variable is not set!");
      // Fallback to a dummy model - this won't work for real AI responses but allows connection
      const model = {
        invoke: async () => { 
          return { text: "⚠️ This agent requires an OpenRouter API key to be configured. Please contact the administrator." };
        }
      };
      return coderAgentContext.run(this, async () => {
        return new Response("Agent is misconfigured. Please set OPENROUTER_API_KEY in environment variables.", {
          status: 200, // Don't use 500 as it breaks WebSocket connections
          headers: { "Content-Type": "text/plain" }
        });
      });
    }
    
    // With API key available, create the OpenRouter instance
    try {
      const openrouter = createOpenRouter({
        apiKey: process.env.OPENROUTER_API_KEY,
      });
      const model = openrouter("openai/gpt-4o-mini");

      // Create a streaming response that handles both text and tool outputs
      return coderAgentContext.run(this, async () => {
        // Log current messages for debugging
        console.log(`📊 Current messages in CoderAgent:`, JSON.stringify(this.messages.map(m => ({
          id: m.id,
          role: m.role,
          contentLength: m.content?.length || 0
        }))));
        
        const dataStreamResponse = createDataStreamResponse({
          execute: async (dataStream) => {
            // Process any pending tool calls from previous messages
            const processedMessages = await processToolCalls({
              messages: this.messages,
              dataStream,
              tools: coderTools,
              executions: coderExecutions,
            });

            console.log(`🔄 Processing ${processedMessages.length} messages`);

            // Create a wrapper for onFinish that will save messages
            const saveMessagesOnFinish: StreamTextOnFinishCallback<{}> = async (completion) => {
              try {
                console.log(`✅ AI response complete, saving conversation state`);
                
                // Let the original callback process as normal
                if (onFinish) {
                  await onFinish(completion);
                }
                
                // No need to save messages here - AIChatAgent base class will handle this
                // through its own messaging mechanism
                console.log(`💾 Messages will be persisted by the AIChatAgent base class`);
              } catch (error) {
                console.error("❌ Error in saveMessagesOnFinish:", error);
              }
            };

            // Stream the AI response using Claude or GPT
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
              onFinish: saveMessagesOnFinish, 
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
    } catch (error) {
      console.error("🚨 Error creating OpenRouter client:", error);
      return new Response(`Error initializing AI model: ${error instanceof Error ? error.message : String(error)}`, { 
        status: 200, // Don't use 500 as it breaks WebSocket connections
        headers: { "Content-Type": "text/plain" }
      });
    }
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
