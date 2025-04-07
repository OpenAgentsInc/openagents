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
          branch: z.string().optional().describe("Branch name, defaults to main/master")
        }),
        execute: async ({ owner, repo, path, branch }) => {
          try {
            console.log(`Getting file contents for ${owner}/${repo}/${path}`);
            // Get GitHub token if available
            const token = this.agent?.env?.GITHUB_TOKEN;
            // Use direct GitHub API
            return await directGitHubTools.getFileContents(owner, repo, path, branch, token);
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
          })).describe("Array of files to push")
        }),
        execute: async ({ owner, repo, branch, message, files }) => {
          try {
            return await this.callMCPTool('push_files', { owner, repo, branch, message, files });
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
          private: z.boolean().optional().describe("Whether the repository should be private")
        }),
        execute: async ({ name, description, private: isPrivate }) => {
          try {
            return await this.callMCPTool('create_repository', { name, description, private: isPrivate });
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
          from_branch: z.string().describe("The source branch to create from")
        }),
        execute: async ({ owner, repo, branch, from_branch }) => {
          try {
            return await this.callMCPTool('create_branch', { owner, repo, branch, from_branch });
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
          direction: z.enum(['asc', 'desc']).optional().describe("Sort direction")
        }),
        execute: async ({ owner, repo, state, sort, direction }) => {
          try {
            console.log(`Listing issues for ${owner}/${repo}`);
            // Get GitHub token if available
            const token = this.agent?.env?.GITHUB_TOKEN;
            // Use direct GitHub API
            return await directGitHubTools.listIssues(owner, repo, state, sort, direction, token);
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
          labels: z.array(z.string()).optional().describe("Labels to apply to the issue")
        }),
        execute: async ({ owner, repo, title, body, labels }) => {
          try {
            console.log(`Creating issue in ${owner}/${repo}: ${title}`);
            // Get GitHub token if available
            const token = this.agent?.env?.GITHUB_TOKEN;
            // Use direct GitHub API
            return await directGitHubTools.createIssue(owner, repo, title, body, labels, token);
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
          issue_number: z.number().describe("The issue number")
        }),
        execute: async ({ owner, repo, issue_number }) => {
          try {
            console.log(`Getting issue ${issue_number} from ${owner}/${repo}`);
            // Get GitHub token if available
            const token = this.agent?.env?.GITHUB_TOKEN;
            // Use direct GitHub API
            return await directGitHubTools.getIssue(owner, repo, issue_number, token);
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
          state: z.enum(['open', 'closed']).optional().describe("New issue state")
        }),
        execute: async ({ owner, repo, issue_number, title, body, state }) => {
          try {
            return await this.callMCPTool('update_issue', { owner, repo, issue_number, title, body, state });
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
          direction: z.enum(['asc', 'desc']).optional().describe("Sort direction")
        }),
        execute: async ({ owner, repo, state, sort, direction }) => {
          try {
            console.log(`Listing pull requests for ${owner}/${repo}`);
            // Get GitHub token if available
            const token = this.agent?.env?.GITHUB_TOKEN;
            // Use direct GitHub API
            return await directGitHubTools.listPullRequests(owner, repo, state, sort, direction, token);
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
          base: z.string().describe("The branch you want to merge into")
        }),
        execute: async ({ owner, repo, title, body, head, base }) => {
          try {
            return await this.callMCPTool('create_pull_request', { owner, repo, title, body, head, base });
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
          pull_number: z.number().describe("The pull request number")
        }),
        execute: async ({ owner, repo, pull_number }) => {
          try {
            return await this.callMCPTool('get_pull_request', { owner, repo, pull_number });
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
          repo: z.string().optional().describe("Filter by repository name")
        }),
        execute: async ({ query, language, user, repo }) => {
          try {
            const searchQuery = `${query}${language ? ` language:${language}` : ''}${user ? ` user:${user}` : ''}${repo ? ` repo:${repo}` : ''}`;
            return await this.callMCPTool('search_code', { q: searchQuery });
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
          perPage: z.number().optional().describe("Items per page")
        }),
        execute: async ({ owner, repo, sha, page, perPage }) => {
          try {
            console.log(`Listing commits for ${owner}/${repo}`);
            // Get GitHub token if available
            const token = this.agent?.env?.GITHUB_TOKEN;
            // Use direct GitHub API
            return await directGitHubTools.listCommits(owner, repo, sha, page, perPage, token);
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
}