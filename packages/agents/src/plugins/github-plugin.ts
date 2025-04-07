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
      // Repository-related tools
      githubListRepos: tool({
        description: "List repositories for a GitHub user or organization",
        parameters: z.object({
          owner: z.string().describe("The GitHub username or organization name"),
          limit: z.number().optional().describe("Maximum number of repositories to return")
        }),
        execute: async ({ owner, limit = 10 }) => {
          try {
            return await this.mockGitHubAPI('github_list_repos', { owner, limit });
          } catch (error) {
            console.error("Error listing GitHub repos:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubGetRepo: tool({
        description: "Get details about a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name")
        }),
        execute: async ({ owner, repo }) => {
          try {
            return await this.mockGitHubAPI('github_get_repo', { owner, repo });
          } catch (error) {
            console.error("Error getting GitHub repo:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubCreateRepo: tool({
        description: "Create a new GitHub repository",
        parameters: z.object({
          name: z.string().describe("The name of the repository"),
          description: z.string().optional().describe("Repository description"),
          private: z.boolean().optional().describe("Whether the repository should be private")
        }),
        execute: async ({ name, description, private: isPrivate }) => {
          try {
            return await this.mockGitHubAPI('github_create_repo', { name, description, private: isPrivate });
          } catch (error) {
            console.error("Error creating GitHub repo:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubListBranches: tool({
        description: "List branches in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name")
        }),
        execute: async ({ owner, repo }) => {
          try {
            return await this.mockGitHubAPI('github_list_branches', { owner, repo });
          } catch (error) {
            console.error("Error listing branches:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Issue-related tools
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
            return await this.mockGitHubAPI('github_list_issues', { owner, repo, state, limit });
          } catch (error) {
            console.error("Error listing GitHub issues:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubGetIssue: tool({
        description: "Get details about a specific GitHub issue",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          issue_number: z.number().describe("The issue number")
        }),
        execute: async ({ owner, repo, issue_number }) => {
          try {
            return await this.mockGitHubAPI('github_get_issue', { owner, repo, issue_number });
          } catch (error) {
            console.error("Error getting GitHub issue:", error);
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
          body: z.string().describe("Issue description/body")
        }),
        execute: async ({ owner, repo, title, body }) => {
          try {
            return await this.mockGitHubAPI('github_create_issue', { owner, repo, title, body });
          } catch (error) {
            console.error("Error creating GitHub issue:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubUpdateIssue: tool({
        description: "Update an existing issue in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          issue_number: z.number().describe("The issue number"),
          title: z.string().optional().describe("New issue title"),
          body: z.string().optional().describe("New issue description/body"),
          state: z.enum(['open', 'closed']).optional().describe("New issue state")
        }),
        execute: async ({ owner, repo, issue_number, title, body, state }) => {
          try {
            return await this.mockGitHubAPI('github_update_issue', { 
              owner, repo, issue_number, title, body, state 
            });
          } catch (error) {
            console.error("Error updating GitHub issue:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Pull request tools
      githubListPullRequests: tool({
        description: "List pull requests in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          state: z.enum(['open', 'closed', 'all']).optional().describe("PR state (open, closed, all)"),
          limit: z.number().optional().describe("Maximum number of PRs to return")
        }),
        execute: async ({ owner, repo, state = 'open', limit = 10 }) => {
          try {
            return await this.mockGitHubAPI('github_list_pull_requests', { owner, repo, state, limit });
          } catch (error) {
            console.error("Error listing GitHub PRs:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubCreatePullRequest: tool({
        description: "Create a new pull request in a GitHub repository",
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
            return await this.mockGitHubAPI('github_create_pull_request', { 
              owner, repo, title, body, head, base 
            });
          } catch (error) {
            console.error("Error creating GitHub PR:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      // Content tools
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
            return await this.mockGitHubAPI('github_get_file_contents', { owner, repo, path, ref });
          } catch (error) {
            console.error("Error getting file contents:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubListCommits: tool({
        description: "List commits in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          path: z.string().optional().describe("Filter commits by file path"),
          limit: z.number().optional().describe("Maximum number of commits to return")
        }),
        execute: async ({ owner, repo, path, limit = 10 }) => {
          try {
            return await this.mockGitHubAPI('github_list_commits', { owner, repo, path, limit });
          } catch (error) {
            console.error("Error listing GitHub commits:", error);
            return `Error: ${error instanceof Error ? error.message : String(error)}`;
          }
        }
      }),

      githubGetCommit: tool({
        description: "Get details of a specific commit in a GitHub repository",
        parameters: z.object({
          owner: z.string().describe("The repository owner (username or org)"),
          repo: z.string().describe("The repository name"),
          ref: z.string().describe("Commit SHA")
        }),
        execute: async ({ owner, repo, ref }) => {
          try {
            return await this.mockGitHubAPI('github_get_commit', { owner, repo, ref });
          } catch (error) {
            console.error("Error getting GitHub commit:", error);
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
   * Mock GitHub API responses
   * In a real implementation, this would call the GitHub API
   */
  private async mockGitHubAPI(toolName: string, params: any): Promise<string> {
    console.log(`[GitHub Plugin] Mock API call to ${toolName} with parameters:`, params);
    
    // Mock responses for each tool
    switch (toolName) {
      case 'github_list_repos':
        return JSON.stringify([
          { name: 'repo1', description: 'First repository', stars: 10 },
          { name: 'repo2', description: 'Second repository', stars: 20 }
        ]);
        
      case 'github_get_repo':
        return JSON.stringify({
          name: params.repo,
          owner: params.owner,
          description: 'Repository details',
          stars: 42,
          forks: 13,
          issues: 5
        });
        
      case 'github_create_repo':
        return JSON.stringify({
          name: params.name,
          description: params.description || '',
          private: params.private || false,
          created_at: new Date().toISOString(),
          url: `https://github.com/user/${params.name}`
        });
        
      case 'github_list_branches':
        return JSON.stringify([
          { name: 'main', protected: true, commit: { sha: 'abc123' } },
          { name: 'develop', protected: false, commit: { sha: 'def456' } }
        ]);
        
      case 'github_list_issues':
        return JSON.stringify([
          { number: 1, title: 'First issue', state: 'open' },
          { number: 2, title: 'Second issue', state: 'closed' }
        ]);
        
      case 'github_get_issue':
        return JSON.stringify({
          number: params.issue_number,
          title: 'Issue title',
          body: 'Issue description',
          state: 'open',
          created_at: new Date().toISOString(),
          comments: 3
        });
        
      case 'github_create_issue':
        return JSON.stringify({
          number: 3,
          title: params.title,
          body: params.body,
          created_at: new Date().toISOString()
        });
        
      case 'github_update_issue':
        return JSON.stringify({
          number: params.issue_number,
          title: params.title || 'Updated issue title',
          body: params.body || 'Updated issue description',
          state: params.state || 'open',
          updated_at: new Date().toISOString()
        });
        
      case 'github_list_pull_requests':
        return JSON.stringify([
          { number: 1, title: 'First PR', state: 'open', user: { login: 'user1' } },
          { number: 2, title: 'Second PR', state: 'closed', user: { login: 'user2' } }
        ]);
        
      case 'github_create_pull_request':
        return JSON.stringify({
          number: 3,
          title: params.title,
          body: params.body,
          head: params.head,
          base: params.base,
          created_at: new Date().toISOString(),
          url: `https://github.com/${params.owner}/${params.repo}/pull/3`
        });
        
      case 'github_get_file_contents':
        return `// Sample file content for ${params.path}\nconsole.log("Hello world!");`;
        
      case 'github_list_commits':
        return JSON.stringify([
          { 
            sha: 'abc123', 
            commit: { 
              message: 'First commit',
              author: { name: 'User1', date: new Date().toISOString() }
            }
          },
          { 
            sha: 'def456', 
            commit: { 
              message: 'Second commit', 
              author: { name: 'User2', date: new Date().toISOString() }
            }
          }
        ]);
        
      case 'github_get_commit':
        return JSON.stringify({
          sha: params.ref,
          commit: {
            message: 'Commit message',
            author: { name: 'User', date: new Date().toISOString() }
          },
          files: [
            { filename: 'file1.js', additions: 10, deletions: 5, changes: 15 },
            { filename: 'file2.js', additions: 7, deletions: 3, changes: 10 }
          ]
        });
        
      default:
        throw new Error(`GitHub API mock not implemented for tool: ${toolName}`);
    }
  }
}