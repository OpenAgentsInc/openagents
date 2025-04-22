// src/AiService.ts (Corrected Implementation)
import { Effect, Layer, Console } from "effect";
import { GitHubFileClient, FileNotFoundError, FetchFilePayload } from "./github/FileClient.js";
import { GitHubIssueClient, IssueNotFoundError } from "./github/IssueClient.js";
import { GitHubApiError, RateLimitExceededError, HttpError } from "./github/Errors.js";
import { Buffer } from "node:buffer"; // For Base64 decoding

// --- Tool Parameter Interfaces (Exported for type safety in Server.ts) ---
export interface FileContentParams {
    owner: string;
    repo: string;
    path: string;
    ref?: string | undefined;
}

export interface IssueParams {
    owner: string;
    repo: string;
    issueNumber: number;
}

// --- Error Stringification Helper ---
const stringifyError = (error: unknown): string => {
    if (error instanceof FileNotFoundError) return `File not found: ${error.owner}/${error.repo}/${error.path}`;
    if (error instanceof IssueNotFoundError) return `Issue not found: ${error.owner}/${error.repo}#${error.issueNumber}`;
    if (error instanceof RateLimitExceededError) return `GitHub API rate limit exceeded. Resets at ${error.resetAt.toISOString()}`;
    if (error instanceof HttpError) return `GitHub API HTTP Error: ${error.status}`;
    if (error instanceof GitHubApiError) return `GitHub API Error: ${error.message}`;
    if (error instanceof Error) return error.message;
    return String(error);
};

// --- Effect Service Definition for Tools ---
export class GitHubTools extends Effect.Tag("GitHubTools")<
    GitHubTools,
    {
        // These functions MUST return an Effect
        getFileContent: (params: FileContentParams) => Effect.Effect<string, string>; // Success string, Failure string
        getIssue: (params: IssueParams) => Effect.Effect<string, string>; // Success string, Failure string
    }
>() {}

// --- Live Implementation Layer for GitHubTools ---
export const GitHubToolsLive = Layer.effect( // Use Layer.effect to access other services
    GitHubTools,
    Effect.gen(function*() {
        // Get the actual GitHub clients from the Effect context
        const fileClient = yield* GitHubFileClient;
        const issueClient = yield* GitHubIssueClient;

        // Implement the service methods
        return {
            getFileContent: (params: FileContentParams): Effect.Effect<string, string> =>
                Effect.gen(function*() {
                    yield* Console.log("🛠️ Tool Effect: GetGitHubFileContent");
                    yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2));

                    // Ensure payload matches client expectations
                    const payload: FetchFilePayload = {
                        owner: params.owner,
                        repo: params.repo,
                        path: params.path,
                        ...(params.ref ? { ref: params.ref } : {})
                    };

                    const file = yield* fileClient.fetchFile(payload); // Call the actual client
                    const content = Buffer.from(file.content, "base64").toString("utf-8");
                    yield* Console.log(`✅ Tool Effect Result: Content length ${content.length}`);
                    return content;
                }).pipe(
                    // Map specific errors or any other error to a failure string
                    Effect.catchAll((error) => {
                        const errorString = stringifyError(error);
                        return Effect.gen(function*() {
                            yield* Console.error(`Tool GetGitHubFileContent failed: ${errorString}`);
                            return yield* Effect.fail(errorString); // Fail the Effect with the error string
                        });
                    })
                ),

            getIssue: (params: IssueParams): Effect.Effect<string, string> =>
                Effect.gen(function*() {
                    yield* Console.log("🛠️ Tool Effect: GetGitHubIssue");
                    yield* Console.log("📝 Parameters:", JSON.stringify(params, null, 2));
                    const issue = yield* issueClient.fetchIssue(params); // Call the actual client
                    const summary = `Issue #${issue.number}: ${issue.title} (${issue.state})\nBody: ${issue.body.substring(0, 200)}...`;
                    yield* Console.log(`✅ Tool Effect Result: ${summary.substring(0, 100)}...`);
                    return summary;
                }).pipe(
                    // Map specific errors or any other error to a failure string
                    Effect.catchAll((error) => {
                        const errorString = stringifyError(error);
                        return Effect.gen(function*() {
                            yield* Console.error(`Tool GetGitHubIssue failed: ${errorString}`);
                            return yield* Effect.fail(errorString); // Fail the Effect with the error string
                        });
                    })
                ),
        };
    })
);

// Import the client layers
import { githubFileClientLayer } from "./github/FileClient.js";
import { githubIssueClientLayer } from "./github/IssueClient.js";

// Layer containing the GitHub client implementations needed by GitHubToolsLive
export const gitHubClientLayers = Layer.merge(githubFileClientLayer, githubIssueClientLayer);

// Combined layer providing GitHubToolsLive and its dependencies
export const AiServiceLive = Layer.provide(GitHubToolsLive, gitHubClientLayers);