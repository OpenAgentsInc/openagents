import { Effect, Layer } from "effect";

// Define type for file content parameters
export interface FileContentParams {
  owner: string;
  repo: string;
  path: string;
  ref?: string;
}

// Define type for issue parameters
export interface IssueParams {
  owner: string;
  repo: string;
  issueNumber: number;
}

// Define handlers context for tools
export class GitHubTools extends Effect.Tag("GitHubTools")<
  GitHubTools,
  {
    getFileContent: (params: FileContentParams) => Effect.Effect<string, string, never>;
    getIssue: (params: IssueParams) => Effect.Effect<string, string, never>;
  }
>() {}

// Create file content function
const fetchFile = (params: FileContentParams): string => {
  return `Fetched file content for ${params.owner}/${params.repo}/${params.path}` +
    (params.ref ? ` at ref ${params.ref}` : '');
};

// Create issue content function
const fetchIssue = (params: IssueParams): string => {
  return `Fetched issue #${params.issueNumber} from ${params.owner}/${params.repo}`;
};

// Provide implementation of GitHubTools with simple functions
export const GitHubToolsLive = Layer.succeed(
  GitHubTools,
  {
    getFileContent: (params: FileContentParams) => 
      Effect.succeed(fetchFile(params)),
    getIssue: (params: IssueParams) => 
      Effect.succeed(fetchIssue(params))
  }
);

// Import the client layers - needed for the consumers of this module
import { githubFileClientLayer } from "./github/FileClient.js";
import { githubIssueClientLayer } from "./github/IssueClient.js";

// Layer that provides the Tool Implementation.
// It requires the layers for the actual GitHub clients.
export const AiServiceLive = Layer.provide(
  GitHubToolsLive,
  Layer.merge(githubFileClientLayer, githubIssueClientLayer)
);