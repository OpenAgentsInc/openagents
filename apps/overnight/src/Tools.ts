/**
 * Tool definitions for GitHub API tools
 */

// Define the tool schemas for Anthropic's tools format
// Based on error message, Anthropic expects 'custom' for type, not 'function'
export const TOOL_SCHEMAS = [
  {
    type: "custom",
    function: {
      name: "GetGitHubFileContent",
      description: "Fetches the UTF-8 text content of a specified file from a GitHub repository.",
      parameters: {
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
    }
  },
  {
    type: "custom",
    function: {
      name: "GetGitHubIssue",
      description: "Fetches details (title, state, body) of a specific issue from a GitHub repository.",
      parameters: {
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
  }
];