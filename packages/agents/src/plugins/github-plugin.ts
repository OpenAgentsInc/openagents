/**
 * GitHub plugin for the Coder agent
 * Provides GitHub tools that can be used by the agent
 */
import type { AgentPlugin } from './plugin-interface';
import { AIChatAgent } from 'agents/ai-chat-agent';
import { tool } from 'ai';
import { z } from 'zod';

export class OpenAIAgentPlugin implements AgentPlugin {
  name = 'github';
  private agent: AIChatAgent<any> | null = null;
  private readonly gitHubTools: Record<string, any> = {};

  constructor() {
    // Define the GitHub tools that will be exposed to the agent
    this.gitHubTools = {
      // List repositories
      githubListRepos: tool({
        description: "List repositories for a GitHub user or organization",
        parameters: z.object({
          owner: z.string().describe("The GitHub username or organization name"),
          limit: z.number().optional().describe("Maximum number of repositories to return")
        }),
        execute: async ({ owner, limit = 10 }) => {
          try {
            return await this.callMCPTool('github_list_repos', { owner, limit });
          } catch (error) {
            console.error("Error listing GitHub repos:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Get repository details
      githubGetRepo: tool({
        description: "Get details about a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name")
        }),
        execute: async ({ owner, repo }) => {
          try {
            return await this.callMCPTool('github_get_repo', { owner, repo });
          } catch (error) {
            console.error("Error getting GitHub repo:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // List issues
      githubListIssues: tool({
        description: "List issues in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          state: z.enum(['open', 'closed', 'all']).optional().describe("Issue state (open, closed, all)"),
          limit: z.number().optional().describe("Maximum number of issues to return")
        }),
        execute: async ({ owner, repo, state = 'open', limit = 10 }) => {
          try {
            return await this.callMCPTool('github_list_issues', { owner, repo, state, limit });
          } catch (error) {
            console.error("Error listing GitHub issues:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Create issue
      githubCreateIssue: tool({
        description: "Create a new issue in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          title: z.string().describe("Issue title"),
          body: z.string().describe("Issue description/body")
        }),
        execute: async ({ owner, repo, title, body }) => {
          try {
            return await this.callMCPTool('github_create_issue', { owner, repo, title, body });
          } catch (error) {
            console.error("Error creating GitHub issue:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Get file contents
      githubGetFileContents: tool({
        description: "Get contents of a file from a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          path: z.string().describe("Path to the file in the repository"),
          ref: z.string().optional().describe("Branch, tag, or commit SHA")
        }),
        execute: async ({ owner, repo, path, ref }) => {
          try {
            return await this.callMCPTool('github_get_file_contents', { owner, repo, path, ref });
          } catch (error) {
            console.error("Error getting file contents:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),
    };
  }

  async initialize(agent: AIChatAgent<any>): Promise<void> {
    this.agent = agent;
    console.log("GitHub plugin initialized");
  }

  getTools(): Record<string, any> {
    return this.gitHubTools;
  }

  /**
   * Call an MCP tool by making a fetch request to the MCP API
   * This is a proxy method that bridges the Worker environment to the MCP service
   */
  private async callMCPTool(toolName: string, params: any): Promise<string> {
    if (!this.agent) {
      throw new Error("GitHub plugin not properly initialized");
    }

    try {
      // Make request to MCP bridge API
      // In production, this would be the actual URL of your MCP bridge API
      const apiUrl = 'https://agents.openagents.com/mcp/execute';
      
      console.log(`Calling MCP tool ${toolName} with parameters:`, params);
      
      const response = await fetch(apiUrl, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          tool: toolName,
          parameters: params
        })
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`MCP tool execution failed: ${errorText}`);
      }

      const result = await response.json();
      // Handle the result with proper type checking
      if (result && typeof result === 'object' && 'text' in result && typeof result.text === 'string') {
        return result.text;
      } else {
        return JSON.stringify(result);
      }
    } catch (error) {
      console.error(`Error executing MCP tool ${toolName}:`, error);
      throw error;
    }
  }
}