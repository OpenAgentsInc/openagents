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

// No need for global withToken wrapper or global declaration anymore

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
        handler: async (params: z.infer<typeof files.CreateOrUpdateFileSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, path, content, message, branch, sha } = params;
          return files.createOrUpdateFile(owner, repo, path, content, message, branch, sha, { token });
        },
      },
      {
        name: "get_file_contents",
        description: "Get the contents of a file or directory from a GitHub repository",
        schema: files.GetFileContentsSchema,
        handler: async (params: z.infer<typeof files.GetFileContentsSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, path, branch } = params;
          return files.getFileContents(owner, repo, path, branch, token);
        },
      },
      {
        name: "push_files",
        description: "Push multiple files to a GitHub repository in a single commit",
        schema: files.PushFilesSchema,
        handler: async (params: z.infer<typeof files.PushFilesSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, branch, files: filesList, message } = params;
          return files.pushFiles(owner, repo, branch, filesList, message, { token });
        },
      },
      {
        name: "search_repositories",
        description: "Search for GitHub repositories",
        schema: repository.SearchRepositoriesSchema,
        handler: async (params: z.infer<typeof repository.SearchRepositoriesSchema>, { token }: { token?: string } = {}) => {
          const { query, page, perPage } = params;
          return repository.searchRepositories(query, page, perPage, { token });
        },
      },
      {
        name: "create_repository",
        description: "Create a new GitHub repository in your account",
        schema: repository.CreateRepositoryOptionsSchema,
        handler: async (params: z.infer<typeof repository.CreateRepositoryOptionsSchema>, { token }: { token?: string } = {}) => {
          return repository.createRepository(params, { token });
        },
      },
      {
        name: "fork_repository",
        description: "Fork a GitHub repository to your account or specified organization",
        schema: repository.ForkRepositorySchema,
        handler: async (params: z.infer<typeof repository.ForkRepositorySchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, organization } = params;
          return repository.forkRepository(owner, repo, organization, { token });
        },
      },
      {
        name: "create_issue",
        description: "Create a new issue in a GitHub repository",
        schema: issues.CreateIssueSchema,
        handler: async (params: z.infer<typeof issues.CreateIssueSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, ...options } = params;
          return issues.createIssue(owner, repo, options, { token });
        },
      },
      {
        name: "list_issues",
        description: "List issues in a GitHub repository with filtering options",
        schema: issues.ListIssuesOptionsSchema,
        handler: async (params: z.infer<typeof issues.ListIssuesOptionsSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, ...options } = params;
          return issues.listIssues(owner, repo, options, { token });
        },
      },
      {
        name: "update_issue",
        description: "Update an existing issue in a GitHub repository",
        schema: issues.UpdateIssueOptionsSchema,
        handler: async (params: z.infer<typeof issues.UpdateIssueOptionsSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, issue_number, ...options } = params;
          return issues.updateIssue(owner, repo, issue_number, options, { token });
        },
      },
      {
        name: "add_issue_comment",
        description: "Add a comment to an existing issue",
        schema: issues.IssueCommentSchema,
        handler: async (params: z.infer<typeof issues.IssueCommentSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, issue_number, body } = params;
          return issues.addIssueComment(owner, repo, issue_number, body, { token });
        },
      },
      {
        name: "get_issue",
        description: "Get details of a specific issue in a GitHub repository",
        schema: issues.GetIssueSchema,
        handler: async (params: z.infer<typeof issues.GetIssueSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, issue_number } = params;
          return issues.getIssue(owner, repo, issue_number, { token });
        },
      },
      {
        name: "create_pull_request",
        description: "Create a new pull request in a GitHub repository",
        schema: pulls.CreatePullRequestSchema,
        handler: async (params: z.infer<typeof pulls.CreatePullRequestSchema>, { token }: { token?: string } = {}) => {
          return pulls.createPullRequest(params, { token });
        },
      },
      {
        name: "get_pull_request",
        description: "Get details of a specific pull request",
        schema: pulls.GetPullRequestSchema,
        handler: async (params: z.infer<typeof pulls.GetPullRequestSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, pull_number } = params;
          return pulls.getPullRequest(owner, repo, pull_number, { token });
        },
      },
      {
        name: "list_pull_requests",
        description: "List and filter repository pull requests",
        schema: pulls.ListPullRequestsSchema,
        handler: async (params: z.infer<typeof pulls.ListPullRequestsSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, ...options } = params;
          return pulls.listPullRequests(owner, repo, options, { token });
        },
      },
      {
        name: "create_pull_request_review",
        description: "Create a review on a pull request",
        schema: pulls.CreatePullRequestReviewSchema,
        handler: async (params: z.infer<typeof pulls.CreatePullRequestReviewSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, pull_number, ...options } = params;
          return pulls.createPullRequestReview(owner, repo, pull_number, options, { token });
        },
      },
      {
        name: "merge_pull_request",
        description: "Merge a pull request",
        schema: pulls.MergePullRequestSchema,
        handler: async (params: z.infer<typeof pulls.MergePullRequestSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, pull_number, ...options } = params;
          return pulls.mergePullRequest(owner, repo, pull_number, options, { token });
        },
      },
      {
        name: "get_pull_request_files",
        description: "Get the list of files changed in a pull request",
        schema: pulls.GetPullRequestFilesSchema,
        handler: async (params: z.infer<typeof pulls.GetPullRequestFilesSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, pull_number } = params;
          return pulls.getPullRequestFiles(owner, repo, pull_number, { token });
        },
      },
      {
        name: "get_pull_request_status",
        description: "Get the combined status of all status checks for a pull request",
        schema: pulls.GetPullRequestStatusSchema,
        handler: async (params: z.infer<typeof pulls.GetPullRequestStatusSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, pull_number } = params;
          return pulls.getPullRequestStatus(owner, repo, pull_number, { token });
        },
      },
      {
        name: "update_pull_request_branch",
        description: "Update a pull request branch with the latest changes from the base branch",
        schema: pulls.UpdatePullRequestBranchSchema,
        handler: async (params: z.infer<typeof pulls.UpdatePullRequestBranchSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, pull_number, expected_head_sha } = params;
          return pulls.updatePullRequestBranch(owner, repo, pull_number, expected_head_sha, { token });
        },
      },
      {
        name: "get_pull_request_comments",
        description: "Get the review comments on a pull request",
        schema: pulls.GetPullRequestCommentsSchema,
        handler: async (params: z.infer<typeof pulls.GetPullRequestCommentsSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, pull_number } = params;
          return pulls.getPullRequestComments(owner, repo, pull_number, { token });
        },
      },
      {
        name: "get_pull_request_reviews",
        description: "Get the reviews on a pull request",
        schema: pulls.GetPullRequestReviewsSchema,
        handler: async (params: z.infer<typeof pulls.GetPullRequestReviewsSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, pull_number } = params;
          return pulls.getPullRequestReviews(owner, repo, pull_number, { token });
        },
      },
      {
        name: "search_code",
        description: "Search for code across GitHub repositories",
        schema: search.SearchCodeSchema,
        handler: async (params: z.infer<typeof search.SearchCodeSchema>, { token }: { token?: string } = {}) => {
          return search.searchCode(params, { token });
        },
      },
      {
        name: "search_issues",
        description: "Search for issues and pull requests across GitHub repositories",
        schema: search.SearchIssuesSchema,
        handler: async (params: z.infer<typeof search.SearchIssuesSchema>, { token }: { token?: string } = {}) => {
          return search.searchIssues(params, { token });
        },
      },
      {
        name: "search_users",
        description: "Search for users on GitHub",
        schema: search.SearchUsersSchema,
        handler: async (params: z.infer<typeof search.SearchUsersSchema>, { token }: { token?: string } = {}) => {
          return search.searchUsers(params, { token });
        },
      },
      {
        name: "list_commits",
        description: "Get list of commits of a branch in a GitHub repository",
        schema: commits.ListCommitsSchema,
        handler: async (params: z.infer<typeof commits.ListCommitsSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, sha, page, perPage } = params;
          return commits.listCommits(owner, repo, page, perPage, sha, { token });
        },
      },
      {
        name: "create_branch",
        description: "Create a new branch in a GitHub repository",
        schema: branches.CreateBranchSchema,
        handler: async (params: z.infer<typeof branches.CreateBranchSchema>, { token }: { token?: string } = {}) => {
          const { owner, repo, branch: newBranch, from_branch } = params;
          return branches.createBranchFromRef(owner, repo, newBranch, from_branch, { token });
        },
      }
    ];

    for (const tool of tools) {
      this.server.tool(tool.name, tool.schema.shape, async (params: any, extra: any) => {
        const validatedParams = tool.schema.parse(params);

        // Extract token ONLY from Authorization header sent by the client wrapper
        let token: string | undefined;
        const authHeader = extra?.request?.headers?.get('Authorization');
        if (authHeader?.startsWith('Bearer ')) {
          token = authHeader.substring(7);
          console.log(`🔑 Token found in Authorization header for tool ${tool.name}`);
        } else {
          console.log(`⚠️ No Authorization header token found for tool ${tool.name}`);
        }

        try {
          console.log(`🔧 Executing GitHub tool: ${tool.name}`);
          console.log(`📊 Tool parameters (keys): ${Object.keys(validatedParams)}`);
          console.log(`🔑 Passing token to handler: ${!!token}`);

          // Call the handler, passing the extracted token
          // Need to cast validatedParams and token to any due to type incompatibility across different tool schemas
          const result = await tool.handler(validatedParams as any, { token } as any);

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
          if (tool.name.startsWith('get_') && !token && 
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
        }
      });
    }
  }
}

export default {
  fetch: async (request: Request, env: any, ctx: any) => {
    const url = new URL(request.url);

    console.log("Incoming request to MCP server");

    // Handle the homepage route
    if (url.pathname === "/") {
      return new Response("It works! Test via the MCP inspector (explained at https://modelcontextprotocol.io/docs/tools/inspector) by connecting to http://mcp-github.openagents.com/sse", {
        headers: {
          "Content-Type": "text/plain"
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

    return response;
  }
};
