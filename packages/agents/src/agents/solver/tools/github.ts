import { z } from "zod";
import { Effect, Data, Option, Cause } from "effect";
import { Solver, solverContext } from "../index"; // Adjust path if needed
import type { BaseAgentState } from "../../../common/types"; // Adjust path
import { effectTool } from "./effect-tool";

// --- Zod Schema for Parameters ---
export const GetFileContentsParams = z.object({
  owner: z.string().describe("Repository owner (username or organization)"),
  repo: z.string().describe("Repository name"),
  path: z.string().describe("Path to the file"),
  branch: z.string().optional().describe("Branch, tag, or commit SHA"),
  token: z.string().optional().describe("GitHub token (optional - will use agent token if not provided)"),
});

// --- Specific Error Types for this Tool ---
// Using Data.TaggedError for type-safe error handling

export class GitHubApiError extends Data.TaggedError("GitHubApiError")<{
  message: string;
  status?: number; // Optional HTTP status
  url: string; // Include URL for context
}> {}

export class FileNotFoundError extends Data.TaggedError("FileNotFoundError")<{
  path: string;
  repo: string;
  owner: string;
  branch?: string;
  url: string;
}> {}

export class InvalidPathError extends Data.TaggedError("InvalidPathError")<{
  path: string;
  message: string; // e.g., "Path refers to a directory, not a file"
  url: string;
}> {}

export class ContentDecodingError extends Data.TaggedError("ContentDecodingError")<{
  path: string;
  encoding: string;
  cause: unknown; // Original error if available
  url: string;
  message: string; // Error message
}> {}

// Union type for expected failures of this tool
type FetchFileContentError =
  | GitHubApiError
  | FileNotFoundError
  | InvalidPathError
  | ContentDecodingError;

// --- GitHub API Response Schema (Simplified for content) ---
interface GitHubFileResponse {
  type: "file" | "dir" | string; // other types exist but we care about 'file'
  encoding?: "base64" | string;
  content?: string; // Base64 encoded content if it's a file
  sha: string;
  path: string;
  url: string;
  // ... other fields
}

interface GitHubDirectoryResponse extends Array<GitHubFileResponse> {} // Directory listing

type GitHubContentResponse = GitHubFileResponse | GitHubDirectoryResponse;

// --- GitHub Effect Request Utility ---
// Basic Effect wrapper for fetch
function githubRequestEffect(
  url: string,
  options?: RequestInit
): Effect.Effect<Response, GitHubApiError> {
  return Effect.tryPromise({
    try: () => fetch(url, options),
    catch: (unknown) => new GitHubApiError({
      message: `Network error: ${unknown instanceof Error ? unknown.message : String(unknown)}`,
      url: url
    })
  });
}

// --- The Tool Definition ---
export const fetchFileContents = effectTool({
  description: "Fetches the contents of a specific file from a GitHub repository.",
  parameters: GetFileContentsParams,
  execute: ({ owner, repo, path, branch, token: explicitToken }) => {
    // Return an Effect for better error handling and composition
    return Effect.gen(function* () {
      // Access agent state via AsyncLocalStorage
      const agent = yield* Effect.sync(() => solverContext.getStore());
      
      // Debug agent context status
      console.log("[fetchFileContents] Agent context check:", {
        contextExists: !!agent,
        contextType: agent ? typeof agent : 'undefined',
        hasState: agent && 'state' in agent,
        stateHasToken: agent && 'state' in agent && 'githubToken' in agent.state,
      });
      
      // Try to recover without dying if agent context is missing but we have an explicit token
      if (!agent) {
        if (explicitToken) {
          console.log("[fetchFileContents] Agent context missing but explicit token provided - continuing");
          // We can proceed with the explicit token
        } else {
          // This is likely a programming error if context is missing
          return yield* Effect.fail(
            new GitHubApiError({
              message: "Agent context not found for fetchFileContents tool. This is likely an internal error.",
              status: 500,
              url: `https://api.github.com/repos/${owner}/${repo}/contents/${path}${branch ? `?ref=${branch}` : ''}`
            })
          );
        }
      }

      // Try to get a token, checking both explicit parameter and agent state
      // Handle the case where agent might be null
      let token = explicitToken;
      if (agent && agent.state && agent.state.githubToken) {
        token = token || agent.state.githubToken;
      }
      console.log("[fetchFileContents] Token check:", {
        agentExists: !!agent,
        stateExists: !!agent?.state,
        hasToken: !!token,
        tokenLength: token ? token.length : 0,
        hasExplicitToken: !!explicitToken
      });
        
      if (!token) {
        // Change from dieMessage to fail with GitHubApiError for better error handling
        // This is an expected condition that should be reported to the user
        return yield* Effect.fail(
          new GitHubApiError({
            message: "GitHub token is missing. Please reconnect the agent with a valid GitHub token or provide a token parameter.",
            status: 401,
            url: `https://api.github.com/repos/${owner}/${repo}/contents/${path}${branch ? `?ref=${branch}` : ''}`
          })
        );
      }
      
      // If we received an explicit token but it's not in the agent state, update the state
      if (agent && agent.state && explicitToken && 
          (!agent.state.githubToken || agent.state.githubToken !== explicitToken)) {
        console.log("[fetchFileContents] Updating agent state with token from parameters");
        try {
          // Only try to update agent state if agent exists and has state
          if (typeof agent.updateState === 'function') {
            // Use updateState instead of setGithubToken to avoid async method issues
            yield* Effect.tryPromise({
              try: () => Promise.resolve(agent.updateState({ githubToken: explicitToken })),
              catch: (error) => new GitHubApiError({
                message: `Failed to update token in agent state: ${error}`,
                url: `https://api.github.com/repos/${owner}/${repo}/contents/${path}`
              })
            });
          } else {
            console.warn("[fetchFileContents] Agent exists but updateState method not found");
          }
        } catch (e) {
          console.error("[fetchFileContents] Error updating token:", e);
          // Continue with the request even if the state update fails
        }
      }

      // Construct API URL
      let url = `https://api.github.com/repos/${owner}/${repo}/contents/${path}`;
      if (branch) {
        url += `?ref=${branch}`;
      }
      const urlFinal = url; // Capture final URL for error reporting

      // Make API Request using Effect-based utility
      const response = yield* githubRequestEffect(urlFinal, {
        headers: {
          Authorization: `Bearer ${token}`,
          Accept: "application/vnd.github.v3+json",
          "User-Agent": "OpenAgents-Solver", // Identify our agent
        },
      });

      // Handle HTTP Errors
      if (!response.ok) {
        const status = response.status;
        const errorText = yield* Effect.tryPromise({
          try: () => response.text(),
          catch: (unknown) =>
            new GitHubApiError({
              message: `Failed to read error response body: ${unknown}`,
              status,
              url: urlFinal,
            }),
        });

        if (status === 404) {
          return yield* Effect.fail(
            new FileNotFoundError({ owner, repo, path, branch, url: urlFinal })
          );
        }
        // Other errors (403, 401, 5xx etc.)
        return yield* Effect.fail(
          new GitHubApiError({ message: errorText, status, url: urlFinal })
        );
      }

      // Parse and Validate Response
      const data = yield* Effect.tryPromise({
        try: () => response.json() as Promise<GitHubContentResponse>,
        catch: (unknown) =>
          new GitHubApiError({
            message: `Failed to parse JSON response: ${unknown}`,
            status: response.status,
            url: urlFinal,
          }),
      });

      // Check if it's a file and has content
      if (Array.isArray(data) || data.type !== "file") {
        return yield* Effect.fail(
          new InvalidPathError({
            path,
            message: `Path does not point to a file (type: ${
              Array.isArray(data) ? "dir" : data.type
            })`,
            url: urlFinal,
          })
        );
      }

      if (!data.content) {
        return yield* Effect.fail(
          new InvalidPathError({
            path,
            message: "File content is missing in the API response.",
            url: urlFinal,
          })
        );
      }

      if (data.encoding !== "base64") {
        return yield* Effect.fail(
          new ContentDecodingError({
            path,
            encoding: data.encoding ?? "unknown",
            message: `Unsupported content encoding: ${
              data.encoding ?? "unknown"
            }. Expected base64.`,
            url: urlFinal,
            cause: null,
          })
        );
      }

      // Decode Content
      const decodedContent = yield* Effect.try({
        try: () => Buffer.from(data.content!, "base64").toString("utf8"),
        catch: (unknown) =>
          new ContentDecodingError({
            path,
            encoding: "base64",
            message: `Failed to decode base64 content.`,
            cause: unknown,
            url: urlFinal,
          }),
      });

      // Return Success
      return decodedContent;
    });
  }
});