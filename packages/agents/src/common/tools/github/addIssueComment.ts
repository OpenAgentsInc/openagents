import { tool } from "ai";
import { z } from "zod";
import type { ToolContext } from "@openagents/core/src/tools/toolContext";

interface GitHubCommentResponse {
  id: number;
  html_url: string;
}

interface AddIssueCommentParams {
  owner: string;
  repo: string;
  issueNumber: number;
  body: string;
}

/**
 * Tool for adding comments to GitHub issues
 */
export function addIssueCommentTool(context: ToolContext) {
  const toolConfig = {
    description: "Add a comment to a GitHub issue",
    parameters: z.object({
      owner: z.string().describe("The owner of the repository (username or organization)"),
      repo: z.string().describe("The name of the repository"),
      issueNumber: z.number().describe("The issue number"),
      body: z.string().describe("The comment text to add to the issue"),
    }),
    execute: async ({ owner, repo, issueNumber, body }: AddIssueCommentParams) => {
      console.log(`[add_issue_comment] Adding comment to ${owner}/${repo}#${issueNumber}`);

      if (!context.githubToken) {
        throw new Error("GitHub token is required but not provided");
      }

      const token = context.githubToken;

      try {
        // Construct GitHub API URL for adding comments
        const url = `https://api.github.com/repos/${owner}/${repo}/issues/${issueNumber}/comments`;

        // Make API request
        const response = await fetch(url, {
          method: 'POST',
          headers: {
            'Authorization': `token ${token}`,
            'Accept': 'application/vnd.github.v3+json',
            'Content-Type': 'application/json',
            'User-Agent': 'OpenAgents'
          },
          body: JSON.stringify({ body })
        });

        if (!response.ok) {
          const errorText = await response.text();
          console.error(`[add_issue_comment] GitHub API error: ${response.status} ${response.statusText}`, errorText);
          throw new Error(`GitHub API error (${response.status}): ${errorText}`);
        }

        const data = await response.json() as GitHubCommentResponse;

        return {
          success: true,
          commentId: data.id,
          url: data.html_url,
          message: `Comment added successfully to ${owner}/${repo}#${issueNumber}`
        };
      } catch (error) {
        console.error(`[add_issue_comment] Error:`, error);
        throw error;
      }
    }
  };

  return tool(toolConfig);
}
