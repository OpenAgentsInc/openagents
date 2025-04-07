import { routeAgentRequest, type Connection, type ConnectionContext, type Schedule, type WSMessage } from "agents";

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

// Define types for incoming message
interface IncomingMessage {
  type: string;
  init: {
    method: string;
    body?: string;
  };
}

// Create a context for tools to access the GitHub token
const toolContext = new AsyncLocalStorage<{
  githubToken?: string;
  tools: Record<string, any>;
}>();

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
    
    // Initialize with the base tools
    this.combinedTools = { ...tools };
    console.log(`Base tools loaded: ${Object.keys(tools).length} tools`);
    console.log("Base tools:", Object.keys(tools));

    try {
      // Add the GitHub plugin
      console.log("Creating GitHub plugin (MCP implementation)...");
      const githubPlugin = new OpenAIAgentPlugin();
      console.log("GitHub plugin created successfully");

      this.plugins.push(githubPlugin);
      console.log("GitHub plugin added to plugins list");
      console.log(`Total plugins: ${this.plugins.length}`);
    } catch (pluginError) {
      const error = pluginError as Error;
      console.error("ERROR creating GitHub plugin:", error.message || String(pluginError));
      if (error.stack) {
        console.error("Error stack:", error.stack);
      }
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
          const error = pluginError as Error;
          console.error(`ERROR initializing plugin ${plugin.name}:`, error.message || String(pluginError));
          if (error.stack) {
            console.error("Error stack:", error.stack);
          }
        }
      }

      const totalTools = Object.keys(this.combinedTools).length;
      console.log(`=== PLUGIN INITIALIZATION COMPLETE ===`);
      console.log(`Total tools available: ${totalTools}`);
      console.log("Available tools:", Object.keys(this.combinedTools));
    } catch (err) {
      const error = err as Error;
      console.error("CRITICAL ERROR initializing plugins:", error.message || String(err));
      if (error.stack) {
        console.error("Error stack:", error.stack);
      }
    }
  }
  /**
   * Override onMessage to extract GitHub token from request body
   */
  override async onMessage(connection: Connection, message: WSMessage) {
    console.log("onMessage called with message type:", typeof message);
    
    if (typeof message === "string") {
      console.log("Parsing message as JSON");
      let data: IncomingMessage;
      try {
        data = JSON.parse(message) as IncomingMessage;
        console.log("Message parsed successfully:", data.type);
        
        if (data.type === "cf_agent_use_chat_request" && data.init.method === "POST") {
          console.log("Processing chat request with body");
          
          try {
            // Parse the request body
            const { body } = data.init;
            if (!body) {
              console.log("No body found in request");
            } else {
              const requestData = JSON.parse(body as string);
              console.log("Request data parsed from body:", Object.keys(requestData));
              
              // Extract token from different possible locations
              const githubToken = 
                requestData.githubToken || 
                (requestData.data?.apiKeys?.github) || 
                (requestData.apiKeys?.github);
              
              if (githubToken) {
                console.log(`Found GitHub token in request body (length: ${githubToken.length})`);
                
                // Set token in environment for the agent
                const env = (this as any).env as Env;
                if (env) {
                  console.log("Setting token in agent environment");
                  if (!env.apiKeys) env.apiKeys = {};
                  env.apiKeys.github = githubToken;
                  env.GITHUB_TOKEN = githubToken;
                  env.GITHUB_PERSONAL_ACCESS_TOKEN = githubToken;
                }
                
                // Set up tool context with GitHub token
                const context = {
                  githubToken,
                  tools,
                };
                
                // Run the rest of the message handling with the tool context
                console.log("Running message handler with GitHub token in context");
                return toolContext.run(context, async () => {
                  return super.onMessage(connection, message);
                });
              } else {
                console.log("No GitHub token found in request body");
              }
            }
          } catch (error) {
            console.error("Error parsing request body:", error);
          }
        }
      } catch (error) {
        console.error("Error parsing message JSON:", error);
      }
    }
    
    console.log("Falling back to default message handler");
    return super.onMessage(connection, message);
  }

  onRequest(request: Request): Promise<Response> {
    console.log("onRequest", request.url, request.method);
    console.log("onRequest headers:", [...request.headers.keys()].join(', '));
    console.log("onRequest x-api-key:", request.headers.get('x-api-key'));
    console.log("onRequest x-github-token:", request.headers.get('x-github-token'));
    return super.onRequest(request);
  }

  /**
   * Handle chat messages with the GitHub token context
   */
  async onChatMessage(onFinish: StreamTextOnFinishCallback<any>) {
    console.log("onChatMessage called");
    
    // Get context with GitHub token if available
    const context = toolContext.getStore();
    if (context?.githubToken) {
      console.log(`Using GitHub token from context (length: ${context.githubToken.length})`);
    } else {
      console.log("No GitHub token available in context");
    }
    
    // Create a streaming response that handles both text and tool outputs
    return agentContext.run(this, async () => {
      console.log("Running with agent context");
      
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

          // Stream the AI response using the configured model
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

TASK SCHEDULING:
- scheduleTask: Schedule a task to be executed at a later time
- listScheduledTasks: List all currently scheduled tasks with their details
- deleteScheduledTask: Delete a scheduled task (note: only one task can be scheduled at a time)

If the user asks about GitHub or needs to work with GitHub repositories, use the appropriate GitHub tools.
If the user asks to schedule a task, use the scheduleTask tool.
If the user asks to list scheduled tasks, use the listScheduledTasks tool.
If the user asks to delete a scheduled task, use the deleteScheduledTask tool.
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
  async fetch(request: Request, env: Env) {
    // Extract GitHub token from headers BEFORE routing the request
    console.log("FETCH HANDLER: Checking request headers");
    console.log(`FETCH HANDLER: Request URL: ${request.url}`);
    console.log(`FETCH HANDLER: Request method: ${request.method}`);

    // Log all headers for debugging
    const headerNames: string[] = [];
    request.headers.forEach((_, key) => headerNames.push(key));
    console.log(`FETCH HANDLER: Headers found: ${headerNames.join(', ')}`);

    // Extract GitHub token from specific headers
    const apiKey = request.headers.get('x-api-key');
    console.log(`FETCH HANDLER: x-api-key header: ${apiKey ? `found (length: ${apiKey.length})` : 'not found'}`);

    const githubToken = request.headers.get('x-github-token');
    console.log(`FETCH HANDLER: x-github-token header: ${githubToken ? `found (length: ${githubToken.length})` : 'not found'}`);

    // Initialize apiKeys in env if a token was found
    if (githubToken && githubToken.trim() !== '') {
      console.log("FETCH HANDLER: Using GitHub token from x-github-token header");
      if (!env.apiKeys) env.apiKeys = {};
      env.apiKeys.github = githubToken;
      env.GITHUB_TOKEN = githubToken;
      env.GITHUB_PERSONAL_ACCESS_TOKEN = githubToken;
    } else if (apiKey && apiKey.trim() !== '') {
      console.log("FETCH HANDLER: Using GitHub token from x-api-key header");
      if (!env.apiKeys) env.apiKeys = {};
      env.apiKeys.github = apiKey;
      env.GITHUB_TOKEN = apiKey;
      env.GITHUB_PERSONAL_ACCESS_TOKEN = apiKey;
    }

    // Route the request to our agent or return 404 if not found
    return (
      (await routeAgentRequest(request, env, {
        // Run logic before a WebSocket client connects
        onBeforeConnect: (request) => {
          console.log("FETCH HANDLER: onBeforeConnect");
          console.log(" onBeforeConnect request", request);
          console.log(" onBeforeConnect request.headers", request.headers);
          console.log(" onBeforeConnect request.headers.get('x-api-key')" + request.headers.get('x-api-key'));
          console.log(" onBeforeConnect request.headers.get('x-github-token')" + request.headers.get('x-github-token'));
          // Your code/auth code here
          // You can return a Response here - e.g. a HTTP 403 Not Authorized -
          // which will stop further request processing and will NOT invoke the
          // Agent.
          // return Response.json({"error": "not authorized"}, { status: 403 })
        },
        // Run logic before a HTTP client clients
        onBeforeRequest: (request) => {
          // Your code/auth code here
          // Returning nothing will result in the call to the Agent continuing
          console.log("FETCH HANDLER: onBeforeRequest");
          console.log(" onBeforeRequest request", request);
          console.log(" onBeforeRequest request.headers", request.headers);
          console.log(" onBeforeRequest request.headers.get('x-api-key')" + request.headers.get('x-api-key'));
          console.log(" onBeforeRequest request.headers.get('x-github-token')" + request.headers.get('x-github-token'));
        },
      })) ||
      new Response("Not found", { status: 404 })
    );
  },
};
