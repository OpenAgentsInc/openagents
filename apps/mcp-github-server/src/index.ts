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
      this.server.tool(tool.name, tool.schema.shape, async (params: Record<string, unknown>) => {
        const validatedParams = tool.schema.parse(params);
        const context: ToolContext = {
          token: (params as { token?: string }).token,
        };

        // Temporarily replace githubRequest with token-aware version
        const originalRequest = globalThis.githubRequest;
        try {
          globalThis.githubRequest = withToken(context.token);
          const result = await tool.handler(validatedParams as any);
          return {
            content: [{
              type: "text" as const,
              text: JSON.stringify(result)
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

    // Handle the homepage route
    if (url.pathname === "/") {
      return new Response("It works! Test via the MCP inspector (explained at https://modelcontextprotocol.io/docs/tools/inspector) by connecting to http://mcp-github.openagents.com/sse", {
        headers: { "Content-Type": "text/plain" },
      });
    }

    // Handle the SSE route
    return MyMCP.mount("/sse", {
      corsOptions: {
        origin: "*",
        methods: "GET,POST",
        headers: "*",
      },
    }).fetch(request, env, ctx);
  }
};
