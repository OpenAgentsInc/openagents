/**
 * GitHub plugin for the Coder agent - Direct GitHub API implementation
 * This version uses the GitHub API directly instead of going through MCP
 */
import type { AgentPlugin } from './plugin-interface';
import { AIChatAgent } from 'agents/ai-chat-agent';
import { tool } from 'ai';
import { z } from 'zod';
import { directGitHubTools } from './direct-github-tools';

export class OpenAIAgentPlugin implements AgentPlugin {
  name = 'github';
  private agent: AIChatAgent<any> | null = null;
  private readonly gitHubTools: Record<string, any> = {};
  
  /**
   * Gets the GitHub token from the agent environment or parameters
   * Prioritizes parameter token over environment token for flexibility
   */
  private getGitHubToken(paramToken?: string): string | undefined {
    // First check if a token was passed directly to the method
    if (paramToken && paramToken.trim() !== '') {
      console.log("Using GitHub token provided directly in the parameter");
      return paramToken;
    }
    
    // If no parameter token, try to get from agent environment
    const envToken = this.agent?.env?.GITHUB_TOKEN;
    
    if (envToken) {
      console.log("Using GitHub token from agent environment");
      return envToken;
    }
    
    console.warn("No GitHub token found in parameters or agent environment. GitHub API access will be limited.");
    return undefined;
  }

  constructor() {
    console.log("=== INITIALIZING GITHUB PLUGIN (CONSTRUCTOR) ===");
    console.log("Using direct GitHub API implementation instead of MCP");

    // Define the GitHub tools that will be exposed to the agent
    this.gitHubTools = {
      // Repository operations
      githubGetFile: tool({
        description: "Get the contents of a file from a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          path: z.string().describe("Path to the file in the repository"),
          branch: z.string().optional().describe("Branch name, defaults to main/master"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ owner, repo, path, branch, token }) => {
          try {
            console.log(`Getting file contents for ${owner}/${repo}/${path}`);
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Use direct GitHub API
            return await directGitHubTools.getFileContents(owner, repo, path, branch, githubToken);
          } catch (error) {
            console.error("Error getting file contents:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubPushFiles: tool({
        description: "Push multiple files to a GitHub repository in a single commit",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          branch: z.string().describe("The branch to push to"),
          message: z.string().describe("Commit message"),
          files: z.array(z.object({
            path: z.string().describe("File path in the repository"),
            content: z.string().describe("Content to write to the file")
          })).describe("Array of files to push"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ owner, repo, branch, message, files, token }) => {
          try {
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Pass token to MCP tool
            return await this.callMCPTool('push_files', { owner, repo, branch, message, files, token: githubToken });
          } catch (error) {
            console.error("Error pushing files:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubCreateRepository: tool({
        description: "Create a new GitHub repository",
        parameters: z.object({
          name: z.string().describe("The name of the repository"),
          description: z.string().optional().describe("Repository description"),
          private: z.boolean().optional().describe("Whether the repository should be private"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ name, description, private: isPrivate, token }) => {
          try {
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Pass token to MCP tool
            return await this.callMCPTool('create_repository', { 
              name, 
              description, 
              private: isPrivate,
              token: githubToken
            });
          } catch (error) {
            console.error("Error creating repository:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubCreateBranch: tool({
        description: "Create a new branch in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          branch: z.string().describe("The name of the new branch"),
          from_branch: z.string().describe("The source branch to create from"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ owner, repo, branch, from_branch, token }) => {
          try {
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Pass token to MCP tool
            return await this.callMCPTool('create_branch', { 
              owner, 
              repo, 
              branch, 
              from_branch,
              token: githubToken
            });
          } catch (error) {
            console.error("Error creating branch:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Issue operations
      githubListIssues: tool({
        description: "List issues in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          state: z.enum(['open', 'closed', 'all']).optional().describe("Issue state (open, closed, all)"),
          sort: z.enum(['created', 'updated', 'comments']).optional().describe("Sort field"),
          direction: z.enum(['asc', 'desc']).optional().describe("Sort direction"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ owner, repo, state, sort, direction, token }) => {
          try {
            console.log(`Listing issues for ${owner}/${repo}`);
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Use direct GitHub API
            return await directGitHubTools.listIssues(owner, repo, state, sort, direction, githubToken);
          } catch (error) {
            console.error("Error listing issues:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubCreateIssue: tool({
        description: "Create a new issue in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          title: z.string().describe("Issue title"),
          body: z.string().describe("Issue description/body"),
          labels: z.array(z.string()).optional().describe("Labels to apply to the issue"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ owner, repo, title, body, labels, token }) => {
          try {
            console.log(`Creating issue in ${owner}/${repo}: ${title}`);
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Use direct GitHub API
            return await directGitHubTools.createIssue(owner, repo, title, body, labels, githubToken);
          } catch (error) {
            console.error("Error creating issue:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubGetIssue: tool({
        description: "Get details of a specific issue",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          issue_number: z.number().describe("The issue number"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ owner, repo, issue_number, token }) => {
          try {
            console.log(`Getting issue ${issue_number} from ${owner}/${repo}`);
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Use direct GitHub API
            return await directGitHubTools.getIssue(owner, repo, issue_number, githubToken);
          } catch (error) {
            console.error("Error getting issue:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubUpdateIssue: tool({
        description: "Update an existing issue",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          issue_number: z.number().describe("The issue number"),
          title: z.string().optional().describe("New issue title"),
          body: z.string().optional().describe("New issue description"),
          state: z.enum(['open', 'closed']).optional().describe("New issue state"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ owner, repo, issue_number, title, body, state, token }) => {
          try {
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Pass token to MCP tool
            return await this.callMCPTool('update_issue', { 
              owner, 
              repo, 
              issue_number, 
              title, 
              body, 
              state,
              token: githubToken
            });
          } catch (error) {
            console.error("Error updating issue:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Pull Request operations
      githubListPullRequests: tool({
        description: "List pull requests in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          state: z.enum(['open', 'closed', 'all']).optional().describe("PR state (open, closed, all)"),
          sort: z.enum(['created', 'updated', 'popularity', 'long-running']).optional().describe("Sort field"),
          direction: z.enum(['asc', 'desc']).optional().describe("Sort direction"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ owner, repo, state, sort, direction, token }) => {
          try {
            console.log(`Listing pull requests for ${owner}/${repo}`);
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Use direct GitHub API
            return await directGitHubTools.listPullRequests(owner, repo, state, sort, direction, githubToken);
          } catch (error) {
            console.error("Error listing pull requests:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubCreatePullRequest: tool({
        description: "Create a new pull request",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          title: z.string().describe("PR title"),
          body: z.string().describe("PR description"),
          head: z.string().describe("The branch containing your changes"),
          base: z.string().describe("The branch you want to merge into"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ owner, repo, title, body, head, base, token }) => {
          try {
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Pass token to MCP tool
            return await this.callMCPTool('create_pull_request', { 
              owner, 
              repo, 
              title, 
              body, 
              head, 
              base,
              token: githubToken
            });
          } catch (error) {
            console.error("Error creating pull request:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubGetPullRequest: tool({
        description: "Get details of a specific pull request",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          pull_number: z.number().describe("The pull request number"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ owner, repo, pull_number, token }) => {
          try {
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Pass token to MCP tool
            return await this.callMCPTool('get_pull_request', { 
              owner, 
              repo, 
              pull_number,
              token: githubToken 
            });
          } catch (error) {
            console.error("Error getting pull request:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Search operations
      githubSearchCode: tool({
        description: "Search for code across GitHub repositories",
        parameters: z.object({
          query: z.string().describe("Search query"),
          language: z.string().optional().describe("Filter by language"),
          user: z.string().optional().describe("Filter by user/organization"),
          repo: z.string().optional().describe("Filter by repository name"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ query, language, user, repo, token }) => {
          try {
            const searchQuery = `${query}${language ? ` language:${language}` : ''}${user ? ` user:${user}` : ''}${repo ? ` repo:${repo}` : ''}`;
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Pass token to MCP tool
            return await this.callMCPTool('search_code', { 
              q: searchQuery,
              token: githubToken
            });
          } catch (error) {
            console.error("Error searching code:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Commit operations
      githubListCommits: tool({
        description: "List commits in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          sha: z.string().optional().describe("Branch or commit SHA"),
          page: z.number().optional().describe("Page number"),
          perPage: z.number().optional().describe("Items per page"),
          token: z.string().optional().describe("GitHub personal access token (optional)")
        }),
        execute: async ({ owner, repo, sha, page, perPage, token }) => {
          try {
            console.log(`Listing commits for ${owner}/${repo}`);
            // Get GitHub token using our centralized method
            const githubToken = this.getGitHubToken(token);
            // Use direct GitHub API
            return await directGitHubTools.listCommits(owner, repo, sha, page, perPage, githubToken);
          } catch (error) {
            console.error("Error listing commits:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      })
    };
  }

  async initialize(agent: AIChatAgent<any>): Promise<void> {
    console.log("=== GITHUB PLUGIN INITIALIZE METHOD CALLED ===");
    this.agent = agent;
    
    // Log environment information
    console.log("Agent environment:", {
      hasEnv: !!agent.env,
      hasGithubToken: !!(agent.env && agent.env.GITHUB_TOKEN),
    });
    
    console.log("GitHub plugin initialization complete - using direct GitHub API");
  }

  getTools(): Record<string, any> {
    console.log("Getting GitHub tools - returning", Object.keys(this.gitHubTools).length, "tools");
    return this.gitHubTools;
  }
  
  /**
   * Call an MCP tool by making a fetch request to the MCP API
   * This is a proxy method that bridges the Worker environment to the MCP service
   * @param toolName Name of the MCP tool to call
   * @param params Parameters to pass to the tool, including optional GitHub token
   * @returns The result from the MCP tool
   */
  private async callMCPTool(toolName: string, params: any): Promise<string> {
    if (!this.agent) {
      throw new Error("GitHub plugin not properly initialized");
    }

    try {
      // Make request to MCP bridge API
      const response = await fetch(`https://api.openagents.com/mcp/execute`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          tool: toolName,
          parameters: params
        })
      });

      // Handle specific error cases
      if (!response.ok) {
        const errorText = await response.text();
        const status = response.status;
        
        // Handle authentication errors specially
        if (status === 401 || status === 403) {
          return `Authentication error: GitHub API returned ${status}. Please provide a valid GitHub token with appropriate permissions.`;
        }
        
        // Handle rate limiting
        if (status === 429) {
          return `Rate limit exceeded: GitHub API rate limit reached. Please try again later or provide a GitHub token with higher rate limits.`;
        }
        
        // Generic error for other cases
        throw new Error(`MCP tool execution failed (${status}): ${errorText}`);
      }

      const result = await response.json();
      return result.text || JSON.stringify(result);
    } catch (error) {
      console.error(`Error executing MCP tool ${toolName}:`, error);
      // Return a user-friendly error message
      return `Error: ${error instanceof Error ? error.message : String(error)}`;
    }
  }
}