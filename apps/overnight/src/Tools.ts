/**
 * Tool definitions for GitHub API tools
 */

// Define the tool schemas for Anthropic's tools format
// Updated to latest Anthropic API format (v0.39.0)
export const TOOL_SCHEMAS = [
  {
    name: "GetGitHubFileContent", 
    description: "Fetches the UTF-8 text content of a specified file from a GitHub repository.",
    input_schema: {
      type: "object",
      properties: {
        owner: {
          type: "string",
          description: "The owner of the GitHub repository (user or organization)"
        },
        repo: {
          type: "string",
          description: "The name of the GitHub repository"
        },
        path: {
          type: "string",
          description: "The full path to the file within the repository"
        },
        ref: {
          type: "string",
          description: "Optional branch, tag, or commit SHA (defaults to default branch)"
        }
      },
      required: ["owner", "repo", "path"]
    }
  },
  {
    name: "GetGitHubIssue",
    description: "Fetches details (title, state, body) of a specific issue from a GitHub repository.",
    input_schema: {
      type: "object",
      properties: {
        owner: {
          type: "string",
          description: "The owner of the GitHub repository (user or organization)"
        },
        repo: {
          type: "string",
          description: "The name of the GitHub repository"
        },
        issueNumber: {
          type: "number",
          description: "The number of the issue to fetch"
        }
      },
      required: ["owner", "repo", "issueNumber"]
    }
  }
];