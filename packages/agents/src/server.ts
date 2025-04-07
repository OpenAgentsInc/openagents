import { routeAgentRequest, type Schedule } from "agents";

import { unstable_getSchedulePrompt } from "agents/schedule";

import { AIChatAgent } from "agents/ai-chat-agent";
import {
  createDataStreamResponse,
  generateId,
  streamText,
  type StreamTextOnFinishCallback,
} from "ai";
import { processToolCalls } from "./utils";
import { tools, executions } from "./tools";
import { AsyncLocalStorage } from "node:async_hooks";
// import { createWorkersAI } from 'workers-ai-provider';
import { createGoogleGenerativeAI } from '@ai-sdk/google';
import { env } from "cloudflare:workers";
import { OpenAIAgentPlugin } from "./plugins/github-plugin";
import type { AgentPlugin } from "./plugins/plugin-interface";

// const workersai = createWorkersAI({ binding: env.AI });
// const model = workersai("@cf/meta/llama-3.3-70b-instruct-fp8-fast");

const google = createGoogleGenerativeAI({
  apiKey: env.GOOGLE_API_KEY,
});

const model = google("gemini-2.5-pro-exp-03-25");

// we use ALS to expose the agent context to the tools
export const agentContext = new AsyncLocalStorage<Coder>();

/**
 * Chat Agent implementation that handles real-time AI chat interactions
 */
export class Coder extends AIChatAgent<Env> {
  private plugins: AgentPlugin[] = [];
  private combinedTools: Record<string, any>;

  constructor(state: DurableObjectState, env: Env) {
    console.log("=== CODER AGENT CONSTRUCTOR CALLED ===");
    super(state, env);
    
    // Log environment variables
    console.log("Environment:", {
      hasEnv: !!env,
      envKeys: env ? Object.keys(env) : [],
      hasGithubToken: !!env?.GITHUB_TOKEN
    });
    
    // Initialize with the base tools
    this.combinedTools = { ...tools };
    console.log(`Base tools loaded: ${Object.keys(tools).length} tools`);
    console.log("Base tools:", Object.keys(tools));
    
    try {
      // Add the GitHub plugin
      console.log("Creating GitHub plugin...");
      const githubPlugin = new OpenAIAgentPlugin();
      console.log("GitHub plugin created successfully");
      
      this.plugins.push(githubPlugin);
      console.log("GitHub plugin added to plugins list");
      console.log(`Total plugins: ${this.plugins.length}`);
    } catch (pluginError) {
      console.error("ERROR creating GitHub plugin:", pluginError);
      if (pluginError.stack) {
        console.error("Error stack:", pluginError.stack);
      }
    }
    
    // Store GitHub token in environment if available
    if (env.GITHUB_TOKEN) {
      console.log("GitHub token found in environment");
    } else {
      console.log("No GitHub token found in environment, public API access only");
    }
    
    // Initialize plugins
    console.log("Starting plugin initialization...");
    this.initializePlugins()
      .then(() => {
        console.log("Agent plugins initialized successfully");
      })
      .catch(err => {
        console.error("Failed to initialize agent plugins:", err);
        if (err.stack) {
          console.error("Error stack:", err.stack);
        }
      });
    
    console.log("=== CODER AGENT CONSTRUCTOR COMPLETED ===");
  }

  private async initializePlugins(): Promise<void> {
    console.log("=== INITIALIZING AGENT PLUGINS ===");
    console.log(`Found ${this.plugins.length} plugins to initialize`);
    
    try {
      // Initialize each plugin
      for (const plugin of this.plugins) {
        console.log(`Initializing plugin: ${plugin.name}...`);
        
        try {
          await plugin.initialize(this);
          console.log(`Plugin ${plugin.name} initialized successfully`);
          
          // Get tools from the plugin
          const pluginTools = plugin.getTools();
          const toolCount = Object.keys(pluginTools).length;
          console.log(`Plugin ${plugin.name} provided ${toolCount} tools`);
          
          if (toolCount > 0) {
            console.log(`Tools from ${plugin.name}:`, Object.keys(pluginTools));
            
            // Add tools to the combined tools
            this.combinedTools = { ...this.combinedTools, ...pluginTools };
            console.log(`Added ${toolCount} tools from ${plugin.name} to combined tools`);
          } else {
            console.warn(`Plugin ${plugin.name} did not provide any tools`);
          }
        } catch (pluginError) {
          console.error(`ERROR initializing plugin ${plugin.name}:`, pluginError);
          if (pluginError.stack) {
            console.error("Error stack:", pluginError.stack);
          }
        }
      }
      
      const totalTools = Object.keys(this.combinedTools).length;
      console.log(`=== PLUGIN INITIALIZATION COMPLETE ===`);
      console.log(`Total tools available: ${totalTools}`);
      console.log("Available tools:", Object.keys(this.combinedTools));
    } catch (error) {
      console.error("CRITICAL ERROR initializing plugins:", error);
      if (error.stack) {
        console.error("Error stack:", error.stack);
      }
    }
  }

  /**
   * Handles incoming chat messages and manages the response stream
   * @param onFinish - Callback function executed when streaming completes
   */

  // biome-ignore lint/complexity/noBannedTypes: <explanation>
  async onChatMessage(onFinish: StreamTextOnFinishCallback<{}>) {
    // Create a streaming response that handles both text and tool outputs
    return agentContext.run(this, async () => {
      const dataStreamResponse = createDataStreamResponse({
        execute: async (dataStream) => {
          // Process any pending tool calls from previous messages
          // This handles human-in-the-loop confirmations for tools
          const processedMessages = await processToolCalls({
            messages: this.messages,
            dataStream,
            tools: this.combinedTools,
            executions,
          });

          // Stream the AI response using GPT-4
          const result = streamText({
            model,
            system: `You are a helpful assistant that can do various tasks...

${unstable_getSchedulePrompt({ date: new Date() })}

You have access to GitHub tools that let you interact with GitHub repositories through the Model Context Protocol (MCP):

REPOSITORY OPERATIONS:
- githubGetFile: Get the contents of a file from a repository
- githubPushFiles: Push multiple files to a repository in a single commit
- githubCreateRepository: Create a new GitHub repository
- githubCreateBranch: Create a new branch in a repository

ISSUE OPERATIONS:
- githubListIssues: List issues in a repository with filtering options
- githubCreateIssue: Create a new issue in a repository
- githubGetIssue: Get details about a specific issue
- githubUpdateIssue: Update an existing issue (title, body, state)

PULL REQUEST OPERATIONS:
- githubListPullRequests: List pull requests in a repository
- githubCreatePullRequest: Create a new pull request
- githubGetPullRequest: Get details about a specific pull request

CODE OPERATIONS:
- githubSearchCode: Search for code across GitHub repositories
- githubListCommits: List commits in a repository

If the user asks about GitHub or needs to work with GitHub repositories, use the appropriate GitHub tools.
If the user asks to schedule a task, use the schedule tool to schedule the task.
`,
            messages: processedMessages,
            tools: this.combinedTools,
            onFinish,
            onError: (error) => {
              console.error("Error while streaming:", error);
            },
            maxSteps: 10,
          });

          // Merge the AI response stream with tool execution outputs
          result.mergeIntoDataStream(dataStream);
        },
      });

      return dataStreamResponse;
    });
  }
  async executeTask(description: string, task: Schedule<string>) {
    await this.saveMessages([
      ...this.messages,
      {
        id: generateId(),
        role: "user",
        content: `Running scheduled task: ${description}`,
        createdAt: new Date(),
      },
    ]);
  }
}

/**
 * Worker entry point that routes incoming requests to the appropriate handler
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    // Route the request to our agent or return 404 if not found
    return (
      (await routeAgentRequest(request, env)) ||
      new Response("Not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;
