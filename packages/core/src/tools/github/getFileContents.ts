import { tool } from 'ai';
import { githubRequest } from "./common/utils";
import { GitHubContentSchema, GitHubFileContentSchema } from "./common/types";
import { z } from "zod";
import type { ToolContext } from '../toolContext';

export const GetFileContentsSchema = z.object({
  owner: z.string().describe("Repository owner (username or organization)"),
  repo: z.string().describe("Repository name"),
  path: z.string().describe("Path to the file or directory"),
  branch: z.string().describe("Branch to get contents from"),
});

// Function implementations
export async function getFileContents(
  owner: string,
  repo: string,
  path: string,
  branch: string,
  token?: string
) {
  let url = `https://api.github.com/repos/${owner}/${repo}/contents/${path}`;

  console.log('in getFileContents with token', token);

  console.log("Getting file contents for", url);
  console.log("Checking we have token", token?.slice(0, 13));

  if (branch) {
    url += `?ref=${branch}`;
  }

  const response = await githubRequest(url, { headers: { Authorization: `Bearer ${token}` } });
  const data = GitHubContentSchema.parse(response);

  // If it's a file, decode the content
  if (!Array.isArray(data) && data.content) {
    // Replace newlines and spaces that GitHub adds to base64
    const cleanContent = data.content.replace(/\n/g, '');
    data.content = atob(cleanContent);
  }

  return data;
}

// @ts-ignore - Ignoring type mismatch issues due to version differences in tool schema
export const getFileContentsTool = (context: ToolContext) => tool({
  description: "Get the contents of a file or directory from a GitHub repository",
  parameters: GetFileContentsSchema,
  execute: async (args) => {
    const { owner, repo, path, branch } = args;
    console.log('in getFileContentsToolwith context', context.githubToken);
    return getFileContents(owner, repo, path, branch, context.githubToken);
  },
});
