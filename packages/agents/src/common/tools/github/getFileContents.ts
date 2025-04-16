import { tool } from "ai";
import { z } from "zod";
import type { ToolContext } from "@openagents/core/src/tools/toolContext";

/**
 * Tool for retrieving file contents from GitHub repositories
 */
export function getFileContentsTool(context: ToolContext) {
  return tool({
    name: "get_file_contents",
    description: "Get the contents of a file from a GitHub repository",
    parameters: z.object({
      owner: z.string().optional().describe("The owner of the repository (username or organization)"),
      repo: z.string().optional().describe("The name of the repository"),
      path: z.string().describe("The path to the file within the repository"),
      branch: z.string().optional().describe("The branch to use (defaults to main)"),
    }),
    execute: async ({ owner, repo, path, branch = "main" }) => {
      console.log(`[get_file_contents] Getting file: ${path} from ${owner}/${repo}:${branch}`);
      
      if (!context.githubToken) {
        throw new Error("GitHub token is required but not provided");
      }
      
      const token = context.githubToken;
      const normalizedPath = path.startsWith('/') ? path.substring(1) : path;
      
      try {
        // Construct GitHub API URL
        const url = `https://api.github.com/repos/${owner}/${repo}/contents/${normalizedPath}?ref=${branch}`;
        
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
        
        const data = await response.json();
        
        // If it's a file with content (not a directory)
        if (!Array.isArray(data) && data.content) {
          if (data.encoding === 'base64') {
            try {
              // Decode base64 content
              const cleanBase64 = data.content.replace(/\s/g, '');
              const binaryStr = Buffer.from(cleanBase64, 'base64');
              const decodedContent = new TextDecoder().decode(binaryStr);
              return decodedContent;
            } catch (e) {
              console.error(`[get_file_contents] Error decoding content:`, e);
              
              // Fallback to simpler method
              try {
                const cleanBase64 = data.content.replace(/[\s\r\n]+/g, '');
                const decodedContent = Buffer.from(cleanBase64, 'base64').toString('utf8');
                return decodedContent;
              } catch (fallbackError) {
                console.error(`[get_file_contents] All decode methods failed:`, fallbackError);
                throw new Error(`Failed to decode file content: ${fallbackError.message}`);
              }
            }
          } else {
            // If not base64 encoded, return as-is
            return data.content;
          }
        } else if (Array.isArray(data)) {
          // It's a directory listing, return as-is
          return data;
        } else {
          throw new Error(`Unexpected response format from GitHub API`);
        }
      } catch (error) {
        console.error(`[get_file_contents] Error:`, error);
        throw error;
      }
    }
  });
}