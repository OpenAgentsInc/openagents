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
    
    // For debugging: hardcode a token
    const debugToken = "gho_placeholder_token_for_debugging_only";
    
    // Log full environment for debugging (excluding sensitive data)
    console.log("DEBUG: Agent environment:", 
      Object.keys(env).reduce((obj, key) => {
        if (key.includes('TOKEN') || key.includes('KEY') || key.includes('SECRET')) {
          obj[key] = env[key] ? `[PRESENT: ${env[key].length} chars]` : '[NOT PRESENT]';
        } else if (key === 'apiKeys') {
          obj[key] = env[key] ? `[OBJECT: ${Object.keys(env[key]).join(', ')}]` : '[NOT PRESENT]';
        } else {
          obj[key] = typeof env[key] === 'object' ? 
            '[OBJECT]' : 
            (typeof env[key] === 'function' ? '[FUNCTION]' : String(env[key]));
        }
        return obj;
      }, {})
    );
    
    // Handle GitHub token from settings if available
    // This is passed from the API request to the agent
    if (env.apiKeys && typeof env.apiKeys === 'object' && env.apiKeys.github) {
      console.log(`GitHub token found in API keys (length: ${env.apiKeys.github.length}), setting in environment`);
      env.GITHUB_TOKEN = env.apiKeys.github;
      
      // Always set GITHUB_PERSONAL_ACCESS_TOKEN regardless of environment
      console.log('Setting GITHUB_PERSONAL_ACCESS_TOKEN for MCP access');
      env.GITHUB_PERSONAL_ACCESS_TOKEN = env.apiKeys.github;
    } else {
      console.log("No GitHub token found in API keys, checking other sources");
      
      // Check if token is available directly in the environment
      if (env.GITHUB_TOKEN) {
        console.log(`Found GitHub token directly in env.GITHUB_TOKEN (length: ${env.GITHUB_TOKEN.length})`);
        
        // Always set GITHUB_PERSONAL_ACCESS_TOKEN regardless of environment
        console.log('Setting GITHUB_PERSONAL_ACCESS_TOKEN from GITHUB_TOKEN');
        env.GITHUB_PERSONAL_ACCESS_TOKEN = env.GITHUB_TOKEN;
      } else if (typeof process !== 'undefined' && process.env && process.env.GITHUB_TOKEN) {
        console.log(`Found GitHub token in process.env.GITHUB_TOKEN, copying to agent environment`);
        env.GITHUB_TOKEN = process.env.GITHUB_TOKEN;
        
        // Always set GITHUB_PERSONAL_ACCESS_TOKEN regardless of environment
        console.log('Setting GITHUB_PERSONAL_ACCESS_TOKEN from process.env.GITHUB_TOKEN');
        env.GITHUB_PERSONAL_ACCESS_TOKEN = process.env.GITHUB_TOKEN;
      } else {
        console.warn("No GitHub token found in any source, GitHub operations may be limited");
        console.warn("Please add a GitHub token in Settings > API Keys to enable full GitHub functionality");
        
        // FOR DEBUGGING ONLY - REMOVE IN PRODUCTION
        console.log("DEBUG: Setting placeholder token for debugging");
        env.GITHUB_TOKEN = debugToken;
        env.GITHUB_PERSONAL_ACCESS_TOKEN = debugToken;
        
        // Ensure apiKeys exists and has github property
        if (!env.apiKeys) {
          env.apiKeys = {};
        }
        env.apiKeys.github = debugToken;
      }
    }

    super(state, env);

    // Log environment variables
    console.log("Environment:", {
      hasEnv: !!env,
      envKeys: env ? Object.keys(env).filter(key => !key.includes('SECRET') && !key.includes('KEY') && key !== 'GITHUB_TOKEN') : [],
      hasGithubToken: !!env?.GITHUB_TOKEN,
      githubTokenLength: env?.GITHUB_TOKEN?.length || 0,
      hasApiKeys: !!env?.apiKeys,
      apiKeyProviders: env?.apiKeys ? Object.keys(env.apiKeys) : []
    });

    // Log process environment if available (for debugging)
    if (typeof process !== 'undefined' && process.env) {
      console.log("Process environment:", {
        hasProcessEnv: true,
        processEnvKeys: Object.keys(process.env).filter(key => !key.includes('SECRET') && !key.includes('KEY') && key !== 'GITHUB_TOKEN'),
        hasGithubToken: !!process.env.GITHUB_TOKEN,
        githubTokenLength: process.env.GITHUB_TOKEN?.length || 0
      });
    }

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
 * Helper function to initialize GitHub token for MCP
 * This ensures GitHub token is available BEFORE MCP clients are created
 */
async function initializeGitHubToken(env: Env): Promise<void> {
  console.log("=== INITIALIZING GITHUB TOKEN FOR MCP ===");
  
  // For debugging: always set a token
  const debugToken = "gho_placeholder_token_for_debugging_only";
  
  console.log("DEBUG: Environment state before initialization:");
  try {
    console.log(`- env.apiKeys exists: ${!!env.apiKeys}`);
    if (env.apiKeys) {
      console.log(`- env.apiKeys is object: ${typeof env.apiKeys === 'object'}`);
      console.log(`- env.apiKeys keys: ${Object.keys(env.apiKeys).join(', ')}`);
      console.log(`- env.apiKeys.github exists: ${!!env.apiKeys.github}`);
      if (env.apiKeys.github) {
        console.log(`- env.apiKeys.github length: ${env.apiKeys.github.length}`);
      }
    }
    console.log(`- env.GITHUB_TOKEN exists: ${!!env.GITHUB_TOKEN}`);
    console.log(`- env.GITHUB_PERSONAL_ACCESS_TOKEN exists: ${!!env.GITHUB_PERSONAL_ACCESS_TOKEN}`);
  } catch (error) {
    console.error("Error logging environment state:", error);
  }
  
  if (env.apiKeys && typeof env.apiKeys === 'object' && env.apiKeys.github) {
    const githubToken = env.apiKeys.github;
    console.log(`Found GitHub token in API keys (length: ${githubToken.length})`);
    
    // Set in Env for MCP client initialization
    env.GITHUB_TOKEN = githubToken;
    
    // The key issue was MCP client needs this specific env variable
    env.GITHUB_PERSONAL_ACCESS_TOKEN = githubToken;
    
    console.log("GitHub token successfully set for MCP client initialization");
  } else {
    console.warn("No GitHub token found in API keys.");
    console.warn("Please add a GitHub token in Settings > API Keys to enable GitHub functionality.");
    
    // For debugging purposes ONLY - remove in production!
    console.log("DEBUG: Setting placeholder token for debugging");
    env.GITHUB_TOKEN = debugToken;
    env.GITHUB_PERSONAL_ACCESS_TOKEN = debugToken;
    
    // Ensure apiKeys exists and has github property
    if (!env.apiKeys) {
      env.apiKeys = {};
    }
    env.apiKeys.github = debugToken;
  }
  
  // Double check that tokens are properly set
  console.log("DEBUG: Environment state after initialization:");
  try {
    console.log(`- env.GITHUB_TOKEN exists: ${!!env.GITHUB_TOKEN}`);
    if (env.GITHUB_TOKEN) {
      console.log(`- env.GITHUB_TOKEN length: ${env.GITHUB_TOKEN.length}`);
    }
    console.log(`- env.GITHUB_PERSONAL_ACCESS_TOKEN exists: ${!!env.GITHUB_PERSONAL_ACCESS_TOKEN}`);
    if (env.GITHUB_PERSONAL_ACCESS_TOKEN) {
      console.log(`- env.GITHUB_PERSONAL_ACCESS_TOKEN length: ${env.GITHUB_PERSONAL_ACCESS_TOKEN.length}`);
    }
    console.log(`- env.apiKeys.github exists: ${!!(env.apiKeys && env.apiKeys.github)}`);
    if (env.apiKeys && env.apiKeys.github) {
      console.log(`- env.apiKeys.github length: ${env.apiKeys.github.length}`);
    }
  } catch (error) {
    console.error("Error logging environment state:", error);
  }
  
  console.log("=== GITHUB TOKEN INITIALIZATION COMPLETE ===");
}

/**
 * Worker entry point that routes incoming requests to the appropriate handler
 */
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext) {
    // Initialize GitHub token BEFORE any agent or MCP initialization
    // This ensures the token is available during MCP client creation
    await initializeGitHubToken(env);

    // Route the request to our agent or return 404 if not found
    return (
      (await routeAgentRequest(request, env)) ||
      new Response("Not found", { status: 404 })
    );
  },
} satisfies ExportedHandler<Env>;
