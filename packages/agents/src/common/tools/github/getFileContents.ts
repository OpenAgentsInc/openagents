import { tool } from "ai";
import { z } from "zod";
import type { ToolContext } from "@openagents/core/src/tools/toolContext";

interface GitHubFileResponse {
  content: string;
  encoding?: string;
}

interface GetFileContentsParams {
  owner?: string;
  repo?: string;
  path: string;
  branch?: string;
}

/**
 * Tool for getting file contents from GitHub
 */
export function getFileContentsTool(context: ToolContext) {
  const toolConfig = {
    description: "Get the contents of a file from GitHub",
    parameters: z.object({
      owner: z.string().optional().describe("The owner of the repository (username or organization)"),
      repo: z.string().optional().describe("The name of the repository"),
      path: z.string().describe("The path to the file in the repository"),
      branch: z.string().optional().describe("The branch to get the file from (defaults to main/master)"),
    }),
    execute: async ({ owner, repo, path, branch }: GetFileContentsParams) => {
      console.log(`[get_file_contents] Getting file contents from ${owner}/${repo}:${path}`);

      if (!context.githubToken) {
        throw new Error("GitHub token is required but not provided");
      }

      const token = context.githubToken;

      try {
        // Use provided owner/repo or fallback to context values
        const repoOwner = owner || context.currentRepoOwner;
        const repoName = repo || context.currentRepoName;
        const branchName = branch || context.currentBranch || 'main';

        if (!repoOwner || !repoName) {
          throw new Error("Repository owner and name are required but not provided");
        }

        // Construct GitHub API URL for getting file contents
        const url = `https://api.github.com/repos/${repoOwner}/${repoName}/contents/${path}?ref=${branchName}`;

        // Make API request
        const response = await fetch(url, {
          headers: {
            'Authorization': `token ${token}`,
            'Accept': 'application/vnd.github.v3+json',
            'User-Agent': 'OpenAgents'
          }
        });

        if (!response.ok) {
          const errorText = await response.text();
          console.error(`[get_file_contents] GitHub API error: ${response.status} ${response.statusText}`, errorText);
          throw new Error(`GitHub API error (${response.status}): ${errorText}`);
        }

        const data = await response.json() as GitHubFileResponse;

        if (!Array.isArray(data) && data.content) {
          if (data.encoding === 'base64') {
            try {
              // First try the simple base64 decode
              const cleanBase64 = data.content.replace(/\s/g, '');
              return Buffer.from(cleanBase64, 'base64').toString('utf-8');
            } catch (error) {
              try {
                // If that fails, try a more aggressive cleanup
                const cleanBase64 = data.content.replace(/[\s\r\n]+/g, '');
                return Buffer.from(cleanBase64, 'base64').toString('utf-8');
              } catch (fallbackError) {
                console.error('[get_file_contents] Error decoding base64:', fallbackError);
                throw new Error(`Failed to decode file content: ${(fallbackError as Error).message}`);
              }
            }
          } else {
            return data.content;
          }
        }

        throw new Error('Invalid response format from GitHub API');
      } catch (error) {
        console.error(`[get_file_contents] Error:`, error);
        throw error;
      }
    }
  };

  return tool(toolConfig);
}
