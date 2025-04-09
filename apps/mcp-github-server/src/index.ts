import { McpAgent } from "agents/mcp";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import * as files from "./operations/files.js";
import * as repository from "./operations/repository.js";
import * as issues from "./operations/issues.js";
import * as pulls from "./operations/pulls.js";
import * as search from "./operations/search.js";
import * as commits from "./operations/commits.js";
import * as branches from "./operations/branches.js";
import { z } from "zod";
import { githubRequest } from "./common/utils.js";
import { GitHubError } from "./common/errors.js";

type ToolContext = {
  token?: string;
};

type RequestOptions = {
  method?: string;
  body?: unknown;
  headers?: Record<string, string>;
  token?: string;
};

// Wrap the githubRequest function to include the token from context
const withToken = (token?: string) => {
  return (url: string, options: RequestOptions = {}) => {
    return githubRequest(url, { ...options, token });
  };
};

// Declare the global githubRequest to allow overriding
declare global {
  var githubRequest: (url: string, options?: RequestOptions) => Promise<unknown>;
}

export class MyMCP extends McpAgent {
  server = new McpServer({
    name: "OpenAgents GitHub MCP",
    version: "0.0.1",
  });

  async init() {
    const tools = [
      {
        name: "create_or_update_file",
        description: "Create or update a single file in a GitHub repository",
        schema: files.CreateOrUpdateFileSchema,
        handler: async (params: z.infer<typeof files.CreateOrUpdateFileSchema>) => {
          const { owner, repo, path, content, message, branch, sha } = params;
          return files.createOrUpdateFile(owner, repo, path, content, message, branch, sha);
        },
      },
      {
        name: "get_file_contents",
        description: "Get the contents of a file or directory from a GitHub repository",
        schema: files.GetFileContentsSchema,
        handler: async (params: z.infer<typeof files.GetFileContentsSchema>) => {
          const { owner, repo, path, branch } = params;
          return files.getFileContents(owner, repo, path, branch);
        },
      },
      {
        name: "push_files",
        description: "Push multiple files to a GitHub repository in a single commit",
        schema: files.PushFilesSchema,
        handler: async (params: z.infer<typeof files.PushFilesSchema>) => {
          const { owner, repo, branch, files: filesList, message } = params;
          return files.pushFiles(owner, repo, branch, filesList, message);
        },
      },
      {
        name: "search_repositories",
        description: "Search for GitHub repositories",
        schema: repository.SearchRepositoriesSchema,
        handler: repository.searchRepositories,
      },
      {
        name: "create_repository",
        description: "Create a new GitHub repository in your account",
        schema: repository.CreateRepositoryOptionsSchema,
        handler: repository.createRepository,
      },
      {
        name: "fork_repository",
        description: "Fork a GitHub repository to your account or specified organization",
        schema: repository.ForkRepositorySchema,
        handler: async (params: z.infer<typeof repository.ForkRepositorySchema>) => {
          const { owner, repo, organization } = params;
          return repository.forkRepository(owner, repo, organization);
        },
      },
      {
        name: "create_issue",
        description: "Create a new issue in a GitHub repository",
        schema: issues.CreateIssueSchema,
        handler: async (params: z.infer<typeof issues.CreateIssueSchema>) => {
          const { owner, repo, ...options } = params;
          return issues.createIssue(owner, repo, options);
        },
      },
      {
        name: "list_issues",
        description: "List issues in a GitHub repository with filtering options",
        schema: issues.ListIssuesOptionsSchema,
        handler: async (params: z.infer<typeof issues.ListIssuesOptionsSchema>) => {
          const { owner, repo, ...options } = params;
          return issues.listIssues(owner, repo, options);
        },
      },
      {
        name: "update_issue",
        description: "Update an existing issue in a GitHub repository",
        schema: issues.UpdateIssueOptionsSchema,
        handler: async (params: z.infer<typeof issues.UpdateIssueOptionsSchema>) => {
          const { owner, repo, issue_number, ...options } = params;
          return issues.updateIssue(owner, repo, issue_number, options);
        },
      },
      {
        name: "add_issue_comment",
        description: "Add a comment to an existing issue",
        schema: issues.IssueCommentSchema,
        handler: async (params: z.infer<typeof issues.IssueCommentSchema>) => {
          const { owner, repo, issue_number, body } = params;
          return issues.addIssueComment(owner, repo, issue_number, body);
        },
      },
      {
        name: "get_issue",
        description: "Get details of a specific issue in a GitHub repository",
        schema: issues.GetIssueSchema,
        handler: async (params: z.infer<typeof issues.GetIssueSchema>) => {
          const { owner, repo, issue_number } = params;
          return issues.getIssue(owner, repo, issue_number);
        },
      },
      {
        name: "create_pull_request",
        description: "Create a new pull request in a GitHub repository",
        schema: pulls.CreatePullRequestSchema,
        handler: pulls.createPullRequest,
      },
      {
        name: "get_pull_request",
        description: "Get details of a specific pull request",
        schema: pulls.GetPullRequestSchema,
        handler: async (params: z.infer<typeof pulls.GetPullRequestSchema>) => {
          const { owner, repo, pull_number } = params;
          return pulls.getPullRequest(owner, repo, pull_number);
        },
      },
      {
        name: "list_pull_requests",
        description: "List and filter repository pull requests",
        schema: pulls.ListPullRequestsSchema,
        handler: async (params: z.infer<typeof pulls.ListPullRequestsSchema>) => {
          const { owner, repo, ...options } = params;
          return pulls.listPullRequests(owner, repo, options);
        },
      },
      {
        name: "create_pull_request_review",
        description: "Create a review on a pull request",
        schema: pulls.CreatePullRequestReviewSchema,
        handler: async (params: z.infer<typeof pulls.CreatePullRequestReviewSchema>) => {
          const { owner, repo, pull_number, ...options } = params;
          return pulls.createPullRequestReview(owner, repo, pull_number, options);
        },
      },
      {
        name: "merge_pull_request",
        description: "Merge a pull request",
        schema: pulls.MergePullRequestSchema,
        handler: async (params: z.infer<typeof pulls.MergePullRequestSchema>) => {
          const { owner, repo, pull_number, ...options } = params;
          return pulls.mergePullRequest(owner, repo, pull_number, options);
        },
      },
      {
        name: "get_pull_request_files",
        description: "Get the list of files changed in a pull request",
        schema: pulls.GetPullRequestFilesSchema,
        handler: async (params: z.infer<typeof pulls.GetPullRequestFilesSchema>) => {
          const { owner, repo, pull_number } = params;
          return pulls.getPullRequestFiles(owner, repo, pull_number);
        },
      },
      {
        name: "get_pull_request_status",
        description: "Get the combined status of all status checks for a pull request",
        schema: pulls.GetPullRequestStatusSchema,
        handler: async (params: z.infer<typeof pulls.GetPullRequestStatusSchema>) => {
          const { owner, repo, pull_number } = params;
          return pulls.getPullRequestStatus(owner, repo, pull_number);
        },
      },
      {
        name: "update_pull_request_branch",
        description: "Update a pull request branch with the latest changes from the base branch",
        schema: pulls.UpdatePullRequestBranchSchema,
        handler: async (params: z.infer<typeof pulls.UpdatePullRequestBranchSchema>) => {
          const { owner, repo, pull_number, expected_head_sha } = params;
          return pulls.updatePullRequestBranch(owner, repo, pull_number, expected_head_sha);
        },
      },
      {
        name: "get_pull_request_comments",
        description: "Get the review comments on a pull request",
        schema: pulls.GetPullRequestCommentsSchema,
        handler: async (params: z.infer<typeof pulls.GetPullRequestCommentsSchema>) => {
          const { owner, repo, pull_number } = params;
          return pulls.getPullRequestComments(owner, repo, pull_number);
        },
      },
      {
        name: "get_pull_request_reviews",
        description: "Get the reviews on a pull request",
        schema: pulls.GetPullRequestReviewsSchema,
        handler: async (params: z.infer<typeof pulls.GetPullRequestReviewsSchema>) => {
          const { owner, repo, pull_number } = params;
          return pulls.getPullRequestReviews(owner, repo, pull_number);
        },
      },
      {
        name: "search_code",
        description: "Search for code across GitHub repositories",
        schema: search.SearchCodeSchema,
        handler: search.searchCode,
      },
      {
        name: "search_issues",
        description: "Search for issues and pull requests across GitHub repositories",
        schema: search.SearchIssuesSchema,
        handler: search.searchIssues,
      },
      {
        name: "search_users",
        description: "Search for users on GitHub",
        schema: search.SearchUsersSchema,
        handler: search.searchUsers,
      },
      {
        name: "list_commits",
        description: "Get list of commits of a branch in a GitHub repository",
        schema: commits.ListCommitsSchema,
        handler: async (params: z.infer<typeof commits.ListCommitsSchema>) => {
          const { owner, repo, sha, page, perPage } = params;
          return commits.listCommits(owner, repo, page, perPage, sha);
        },
      },
      {
        name: "create_branch",
        description: "Create a new branch in a GitHub repository",
        schema: branches.CreateBranchSchema,
        handler: async (params: z.infer<typeof branches.CreateBranchSchema>) => {
          const { owner, repo, branch: newBranch, from_branch } = params;
          return branches.createBranchFromRef(owner, repo, newBranch, from_branch);
        },
      }
    ];

    for (const tool of tools) {
      this.server.tool(tool.name, tool.schema.shape, async (params: any, extra: any) => {
        const validatedParams = tool.schema.parse(params);

        // Extract token from multiple possible sources
        let token: string | undefined;
        let authHeader: string | null = null;  // Declare at outer scope
        try {
          // 1. Try X-GitHub-Token header first (our custom header)
          const githubHeader = extra?.request?.headers?.get('X-GitHub-Token');
          if (githubHeader) {
            token = githubHeader;
            if (token) {
              console.log('Token extracted from X-GitHub-Token header:', token.substring(0, 8) + '...');
            }
          }

          // 2. Try Authorization header next
          if (!token) {
            authHeader = extra?.request?.headers?.get('Authorization');
            console.log('Auth header present:', !!authHeader);
            if (authHeader?.startsWith('Bearer ')) {
              token = authHeader.substring(7);
              if (token) {
                console.log('Token extracted from Authorization header:', token.substring(0, 8) + '...');
              }
            }
          }

          // 3. Try params._meta.token next (from MCP client)
          if (!token && params?._meta?.token) {
            token = params._meta.token;
            if (token) {
              console.log('Token extracted from _meta.token:', token.substring(0, 8) + '...');
            }
          }

          // 4. Finally try params.token (direct parameter)
          if (!token && params?.token) {
            token = params.token;
            if (token) {
              console.log('Token extracted from params.token:', token.substring(0, 8) + '...');
            }
          }

          // Log final token state with more detail
          if (token) {
            console.log(`✅ GitHub token found (${token.substring(0, 8)}...) for tool: ${tool.name}`);
            console.log('Token source:',
              extra?.request?.headers?.get('X-GitHub-Token') ? 'X-GitHub-Token header' :
                authHeader?.startsWith('Bearer ') ? 'Authorization header' :
                  params?._meta?.token ? '_meta.token' :
                    params?.token ? 'params.token' : 'unknown'
            );
          } else {
            console.warn(`⚠️ No GitHub token found for tool: ${tool.name}. This may cause API rate limits or authentication errors.`);
          }
        } catch (e) {
          console.error('Error accessing token sources:', e);
        }

        const context: ToolContext = {
          token
        };

        // Temporarily replace githubRequest with token-aware version
        const originalRequest = globalThis.githubRequest;
        try {
          console.log(`🔧 Executing GitHub tool: ${tool.name}`);
          console.log(`📊 Tool parameters:`, JSON.stringify(validatedParams, null, 2).substring(0, 200));

          globalThis.githubRequest = withToken(context.token);
          const result = await tool.handler(validatedParams as any);

          console.log(`✅ Tool ${tool.name} execution successful`);
          return {
            content: [{
              type: "text" as const,
              text: JSON.stringify(result)
            }]
          };
        } catch (error) {
          console.error(`❌ Tool execution error for ${tool.name}:`, error);

          // Improved error handling for specific GitHub errors
          let errorResponse: any = {
            error: error instanceof Error ? error.message : String(error)
          };

          // For operations that fail without a token to public repositories
          if (tool.name.startsWith('get_') && !context.token &&
            (error instanceof GitHubError && (error.status === 401 || error.status === 403 || error.status === 429))) {
            console.log(`🔄 Error might be due to GitHub rate limits or auth requirements`);

            errorResponse = {
              error: "GitHub API access error",
              details: {
                message: `The GitHub API returned an error that might be due to rate limiting or authentication requirements.`,
                original_error: error instanceof Error ? error.message : String(error),
                status: error?.status || 'unknown',
                suggestion: "For public repositories, you may still access content without authentication, but GitHub imposes stricter rate limits for unauthenticated requests. Providing a GitHub token would help avoid these limitations."
              }
            };
          }

          return {
            content: [{
              type: "text" as const,
              text: JSON.stringify(errorResponse)
            }]
          };
        } finally {
          globalThis.githubRequest = originalRequest;
        }
      });
    }
  }
}

export default {
  fetch: async (request: Request, env: any, ctx: any) => {
    const url = new URL(request.url);

    // Extract token from multiple sources
    const urlToken = url.searchParams.get('token');  // Try URL parameter first
    const authHeader = request.headers.get('Authorization');
    const headerToken = authHeader?.startsWith('Bearer ') ? authHeader.substring(7) : null;
    const token = urlToken || headerToken || null;

    console.log("Incoming request to MCP server");
    console.log("Token present:", !!token);
    if (token) {
      console.log("Token found:", token.substring(0, 8) + "...");
    }

    // Handle the homepage route
    if (url.pathname === "/") {
      return new Response("It works! Test via the MCP inspector (explained at https://modelcontextprotocol.io/docs/tools/inspector) by connecting to http://mcp-github.openagents.com/sse", {
        headers: {
          "Content-Type": "text/plain",
          ...(token && { "X-GitHub-Token": token })  // Pass token through response headers
        },
      });
    }

    // Handle the SSE route
    const response = await MyMCP.mount("/sse", {
      corsOptions: {
        origin: "*",
        methods: "GET,POST",
        headers: "*",
      }
    }).fetch(request, env, ctx);

    // Add token to response headers if present
    if (token) {
      response.headers.set("X-GitHub-Token", token);
    }

    return response;
  }
};
