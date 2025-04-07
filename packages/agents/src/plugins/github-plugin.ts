/**
 * GitHub plugin for the Coder agent - MCP GitHub API implementation
 * This version uses MCP tools for all GitHub operations
 */
import type { AgentPlugin } from './plugin-interface';
import { AIChatAgent } from 'agents/ai-chat-agent';
import { tool } from 'ai';
import { z } from 'zod';

export class OpenAIAgentPlugin implements AgentPlugin {
  name = 'github';
  private agent: AIChatAgent<any> | null = null;
  private readonly gitHubTools: Record<string, any> = {};
  
  /**
   * Helper method to safely access agent environment
   * This avoids the protected env access issue
   */
  private getAgentEnv(): Record<string, any> | undefined {
    if (!this.agent) return undefined;
    
    // Use a trick to access the protected env
    // This is safe because we're not modifying anything, just reading
    return (this.agent as any).env;
  }
  
  /**
   * Gets the GitHub token from the agent environment or parameters
   * Prioritizes parameter token over environment token for flexibility
   */
  private getGitHubToken(paramToken?: string): string | undefined {
    console.log("GitHub token search beginning...");
    
    // First check if a token was passed directly to the method
    if (paramToken && paramToken.trim() !== '') {
      console.log(`Using GitHub token provided directly in the parameter (length: ${paramToken.length})`);
      return paramToken;
    } else {
      console.log("No GitHub token provided as parameter");
    }
    
    // Check agent environment - this is the ONLY source we should use (from user API Keys)
    if (!this.agent) {
      console.warn("Agent not initialized, cannot access environment for GitHub token");
    } else {
      try {
        // Use getEnv() helper to safely access the protected env property
        const env = this.getAgentEnv();
        
        if (!env) {
          console.warn("Agent environment not available, cannot access GitHub token");
        } else {
          // Get token from agent environment
          const envToken = env.GITHUB_TOKEN;
          
          if (envToken) {
            console.log(`Using GitHub token from agent environment (length: ${envToken.length})`);
            return envToken;
          } else {
            console.warn("No GitHub token found in agent environment");
          }
          
          // Log all environment keys for debugging (without exposing secrets)
          try {
            console.log("Available environment keys:", Object.keys(env)
              .filter(key => !key.includes('SECRET') && !key.includes('KEY') && key !== 'GITHUB_TOKEN'));
          } catch (e) {
            console.warn("Could not list environment keys");
          }
        }
      } catch (error) {
        console.warn("Error accessing agent environment:", error instanceof Error ? error.message : String(error));
      }
    }
    
    console.warn("⚠️ No GitHub token found in agent environment. GitHub API access will be limited to public repositories only.");
    console.warn("Please add a GitHub token in the Settings > API Keys page to enable full GitHub functionality.");
    return undefined;
  }

  constructor() {
    console.log("=== INITIALIZING GITHUB PLUGIN (CONSTRUCTOR) ===");
    console.log("Using MCP GitHub API implementation");

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
            // Use MCP tool
            console.log(`Using MCP tool for getting file contents from ${owner}/${repo}`);
            console.log(`MCP call with GitHub token: ${githubToken ? 'token present' : 'NO TOKEN FOUND'}`);
            if (!githubToken) {
              console.warn('⚠️ GitHub API calls without a token will be limited to public repositories and may fail.');
              console.warn('Please add a GitHub token in Settings > API Keys to enable full GitHub functionality.');
            }
            return await this.callMCPTool('get_file_contents', { 
              owner, 
              repo, 
              path,
              branch,
              token: githubToken 
            });
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
            // Use MCP tool
            console.log(`Using MCP tool for listing issues in ${owner}/${repo}`);
            return await this.callMCPTool('list_issues', { 
              owner, 
              repo, 
              state,
              sort,
              direction,
              token: githubToken
            });
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
            // Use MCP tool
            console.log(`Using MCP tool for creating issue in ${owner}/${repo}`);
            return await this.callMCPTool('create_issue', { 
              owner, 
              repo, 
              title, 
              body, 
              labels,
              token: githubToken
            });
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
            // Use MCP tool
            console.log(`Using MCP tool for getting issue in ${owner}/${repo}`);
            return await this.callMCPTool('get_issue', { 
              owner, 
              repo, 
              issue_number,
              token: githubToken
            });
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
            // Use MCP tool
            console.log(`Using MCP tool for listing pull requests in ${owner}/${repo}`);
            return await this.callMCPTool('list_pull_requests', { 
              owner, 
              repo, 
              state,
              sort,
              direction,
              token: githubToken
            });
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
            // Use MCP tool
            console.log(`Using MCP tool for listing commits in ${owner}/${repo}`);
            return await this.callMCPTool('list_commits', { 
              owner, 
              repo, 
              sha,
              page,
              per_page: perPage,
              token: githubToken
            });
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
    
    // Use our helper to safely access the environment
    const env = this.getAgentEnv();
    
    // Log environment information
    const hasEnv = !!env;
    const hasGithubToken = !!(env && env.GITHUB_TOKEN);
    const tokenLength = hasGithubToken ? env.GITHUB_TOKEN.length : 0;
    
    console.log("Agent environment:", {
      hasEnv,
      hasGithubToken,
      tokenLength,
      envKeys: hasEnv ? Object.keys(env).filter(k => !k.includes('SECRET') && !k.includes('KEY')) : []
    });
    
    if (hasGithubToken) {
      console.log(`GitHub token found in agent environment (length: ${tokenLength})`);
    } else {
      console.warn("No GitHub token found in agent environment. GitHub API calls will have limited functionality.");
    }
    
    console.log("GitHub plugin initialization complete - using MCP GitHub tools");
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
      // Log the MCP tool call
      console.log(`Calling MCP tool: ${toolName}`);
      console.log(`MCP tool parameters:`, JSON.stringify({
        ...params,
        token: params.token ? `[TOKEN LENGTH: ${params.token.length}]` : 'None'
      }));
      
      if (!params.token) {
        console.warn('⚠️ No GitHub token provided for MCP call. This operation may fail if accessing private repositories.');
        console.warn('⚠️ Please add a GitHub token in Settings > API Keys to enable full GitHub functionality.');
      }
      
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

      // Log response status
      console.log(`MCP tool ${toolName} response status: ${response.status}`);

      // Handle specific error cases
      if (!response.ok) {
        const errorText = await response.text();
        const status = response.status;
        
        console.error(`MCP tool ${toolName} error (${status}):`, errorText);
        
        // Try to parse the error response as JSON
        let errorDetails = errorText;
        try {
          const errorJson = JSON.parse(errorText);
          if (errorJson.error) {
            errorDetails = errorJson.error;
          } else if (errorJson.message) {
            errorDetails = errorJson.message;
          }
        } catch (e) {
          // Not JSON, use as is
        }
        
        // Handle authentication errors specially
        if (status === 401 || status === 403) {
          return JSON.stringify({
            error: `Authentication error: GitHub API returned ${status}. Please add a GitHub token in Settings > API Keys page with appropriate permissions. Details: ${errorDetails}`
          });
        }
        
        // Handle rate limiting
        if (status === 429) {
          return JSON.stringify({
            error: `Rate limit exceeded: GitHub API rate limit reached. Please try again later or provide a GitHub token with higher rate limits.`
          });
        }
        
        // Handle not found errors (usually wrong repo name)
        if (status === 404) {
          return JSON.stringify({
            error: `Resource not found (404): The repository or resource couldn't be found. Please check that the repository exists and is spelled correctly.`
          });
        }
        
        // Handle Cloudflare connection timeout errors (522/524)
        if (status === 522 || status === 524) {
          // Check if this might be due to trying to access a private repo without a token
          const noTokenMessage = !params.token ? 
            ` This may occur when trying to access a private repository without authentication. If ${params.owner}/${params.repo} is a private repository, please add a GitHub token in Settings > API Keys.` : '';
            
          return JSON.stringify({
            error: `Connection timeout (${status}): GitHub API request timed out. ${noTokenMessage}`
          });
        }
        
        // Handle other Cloudflare errors
        if (status >= 500 && status < 600) {
          return JSON.stringify({
            error: `Server error (${status}): There was an issue connecting to the GitHub service.`
          });
        }
        
        // Generic error for other cases
        return JSON.stringify({
          error: `MCP tool execution failed (${status}): ${errorDetails}`
        });
      }

      // Process successful response
      const result = await response.json();
      console.log(`MCP tool ${toolName} successful response:`, 
        typeof result === 'object' ? 
          (result.text ? result.text.substring(0, 100) + "..." : JSON.stringify(result).substring(0, 100) + "...") 
          : result);
      
      return result.text || JSON.stringify(result);
    } catch (error) {
      console.error(`Error executing MCP tool ${toolName}:`, error);
      
      // Return a user-friendly error message as JSON without retry logic
      const errorMessage = error instanceof Error ? error.message : String(error);
      const noTokenMessage = !params.token && params.owner && params.repo ? 
        ` If ${params.owner}/${params.repo} is a private repository, please add a GitHub token in Settings > API Keys.` : '';
      
      return JSON.stringify({
        error: `Error calling GitHub service: ${errorMessage}.${noTokenMessage}`
      });
    }
  }
}