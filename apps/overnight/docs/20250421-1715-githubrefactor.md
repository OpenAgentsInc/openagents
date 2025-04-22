Okay, let's break down the analysis and create refactoring instructions for a coding agent.

**Analysis:**

1.  **Project Goal:** The overall project is an "Overnight Coding Agent" that interacts with GitHub (fetching issues, commenting, creating PRs) using the Effect-TS framework.
2.  **Current State (GitHub Module):**
    *   Two separate modules exist for GitHub interactions: `src/github/GitHubApi.ts` (for files) and `src/github/GitHubIssueApi.ts` (for issues).
    *   Each module defines its own:
        *   Interface (`GitHubApiClient`, `GitHubIssueClient`) and `Context.Tag`.
        *   Live implementation (`GitHubApiClientLive`, `GitHubIssueClientLive`) that uses the native `fetch` API directly.
        *   Specific payload/response schemas (`@effect/schema`).
        *   Error types using `Data.TaggedError` (e.g., `FileNotFoundError`, `IssueNotFoundError`).
        *   *Duplicate* common error types (`GitHubApiError`, `RateLimitExceededError`).
        *   Layer factory function (`createGitHubApiClient`, `createGitHubIssueClient`).
    *   Mocking is inconsistent: `GitHubIssueApi.ts` includes a mock implementation (`GitHubIssueClientMock`) and layer factory (`createMockGitHubIssueClient`), while `GitHubApi.test.ts` defines its mock (`GitHubApiClientMock`) *within the test file*.
    *   `Program.ts` demonstrates using both clients by merging their respective layers.
    *   Tests exist for both modules, using their respective mocks (or the inline one for the file API).
3.  **Identified Problems & Refactoring Opportunities:**
    *   **Duplication:** The core HTTP request logic (headers, auth, fetch call, rate limit check, basic error handling) and common error types are duplicated across `GitHubApiClientLive` and `GitHubIssueClientLive`.
    *   **Maintainability:** Adding new GitHub API features (e.g., Pull Requests, Comments) would require duplicating the structure again, leading to bloat and potential inconsistencies.
    *   **Idiomatic Effect:** The code uses native `fetch` directly. While functional, using Effect's built-in HTTP client (`@effect/platform/Http/Client`) would be more idiomatic, integrating better with Effect's ecosystem (e.g., interruption, retries, structured responses/errors). The comments in `GitHubApi.ts` even acknowledge this limitation.
    *   **Structure:** The separation into "File API" and "Issue API" is logical, but the underlying *request execution* and *common error handling* should be centralized.
    *   **Mocking:** The mocking strategy is inconsistent and the file API mock is hidden within its test file.
4.  **Desired Outcome (Based on Docs & Best Practices):**
    *   A unified internal mechanism for making authenticated requests to the GitHub API using `@effect/platform/Http/Client`.
    *   Centralized definition of common GitHub API errors.
    *   Refactored `File` and `Issue` clients that *use* the central request mechanism, focusing only on endpoint-specific logic and schemas.
    *   Consistent and easily accessible mock implementations for testing.
    *   A cleaner, more extensible structure for adding future GitHub API interactions.

**Refactoring Instructions for Coding Agent:**

Okay, coding agent, your task is to refactor the existing GitHub API client code (`src/github/`) to improve its structure, reduce duplication, and make it more extensible, following Effect-TS best practices. You have no prior knowledge of this codebase, so follow these steps precisely.

**Goal:** Centralize GitHub API request logic and common error handling, refactor existing file and issue clients to use this central logic, and standardize mocking.

**Location of Relevant Files:**

*   `src/github/GitHubApi.ts` (Current File API client)
*   `src/github/GitHubIssueApi.ts` (Current Issue API client)
*   `test/github/GitHubApi.test.ts` (Tests for File API client, contains a mock)
*   `test/github/GitHubIssueApi.test.ts` (Tests for Issue API client)
*   `src/Program.ts` (Example usage)
*   `package.json` (To check dependencies like `@effect/platform`)

**Refactoring Steps:**

1.  **Create New Directory Structure:**
    *   Inside `src/github/`, create the following files:
        *   `src/github/Client.ts` (Will hold the core HTTP executor)
        *   `src/github/Errors.ts` (Will hold common error definitions)
        *   `src/github/Types.ts` (Optional, for shared types if needed, can start empty)
        *   `src/github/FileClient.ts` (Will hold the refactored File API client)
        *   `src/github/IssueClient.ts` (Will hold the refactored Issue API client)

2.  **Centralize Common Errors (`src/github/Errors.ts`):**
    *   Go to `src/github/GitHubApi.ts` and `src/github/GitHubIssueApi.ts`.
    *   Identify the common error types: `GitHubApiError` and `RateLimitExceededError`.
    *   Move their definitions (using `Data.TaggedError`) into `src/github/Errors.ts`.
    *   Define a more generic `NotFoundError` in `Errors.ts` as well, which specific clients can potentially wrap.
    *   Define a generic `HttpError` for non-specific HTTP failures.
    *   *Delete* the duplicate definitions of `GitHubApiError` and `RateLimitExceededError` from `GitHubApi.ts` and `GitHubIssueApi.ts`.
    *   Update imports in all files that used the old error locations.

3.  **Implement Core HTTP Executor (`src/github/Client.ts`):**
    *   Define a configuration interface/schema for the GitHub client (e.g., `GitHubConfig`) containing `baseUrl` and optional `token`.
    *   Create a `Context.Tag` for this config (e.g., `GitHubConfig`).
    *   Import `HttpClient` from `@effect/platform/Http/Client`. If `@effect/platform` or `@effect/platform-node` is not installed, add it (`pnpm add @effect/platform @effect/platform-node`).
    *   Define an internal interface `GitHubHttpExecutor` with a method like `execute(request: HttpClientRequest.HttpClientRequest)`.
    *   Create a `Context.Tag` for this executor (e.g., `GitHubHttpExecutor`).
    *   Implement `GitHubHttpExecutorLive` which:
        *   Takes `GitHubConfig` and `HttpClient` as dependencies.
        *   The `execute` method should:
            *   Get the `baseUrl` and `token` from `GitHubConfig`.
            *   Clone the incoming `request`.
            *   Prepend the `baseUrl` to the request URL if it's relative.
            *   Add standard headers: `Accept: application/vnd.github.v3+json`.
            *   If a `token` exists in the config, add the `Authorization: token <token>` header.
            *   Use the injected `HttpClient` to execute the modified request (`HttpClient.execute`).
            *   Handle the response:
                *   Check for rate limiting headers (`x-ratelimit-remaining`, `x-ratelimit-reset`) and fail with the centralized `RateLimitExceededError` if exceeded.
                *   Check for common status codes (404, 403, 5xx) and fail with the appropriate centralized errors (`NotFoundError`, `GitHubApiError`, `HttpError`).
                *   If the response is successful (e.g., 2xx), attempt to parse it as JSON (`HttpClientResponse.schemaBodyJson`).
                *   Return the successful response or fail with appropriate errors.
    *   Create a `Layer` (e.g., `githubHttpExecutorLayer`) that provides the `GitHubHttpExecutorLive` implementation, taking `GitHubConfig` and `HttpClient` layers as input.
    *   Create a convenience Layer (e.g., `defaultGitHubLayer`) that provides the `GitHubConfig` (using default `https://api.github.com` and maybe reading token from env var later), the `HttpClient` (from `@effect/platform-node`), and the `GitHubHttpExecutor`.

4.  **Refactor File Client (`src/github/FileClient.ts`):**
    *   Move the `FetchFilePayload` type/schema and `GitHubFileContent` type/schema from `src/github/GitHubApi.ts` to `src/github/FileClient.ts`.
    *   Move the `FileNotFoundError` definition from `src/github/GitHubApi.ts` to `src/github/FileClient.ts`. Make it extend the common `NotFoundError` or just be specific to files.
    *   Define the `GitHubFileClient` interface with the `fetchFile` method signature (similar to the old `GitHubApiClient`). Ensure its error types use the centralized `GitHubApiError`, `RateLimitExceededError` and the specific `FileNotFoundError`.
    *   Create a `Context.Tag` for `GitHubFileClient`.
    *   Implement `GitHubFileClientLive`:
        *   It should depend on the `GitHubHttpExecutor` tag.
        *   The `fetchFile` method should:
            *   Construct the specific API path (`/repos/${owner}/${repo}/contents/${path}`).
            *   Create an `HttpClientRequest` (e.g., `HttpClientRequest.get(path)`). Add query params like `ref` if provided.
            *   Call `GitHubHttpExecutor.execute(request)`.
            *   Map the successful response body using the `GitHubFileContentSchema`.
            *   Map errors from the executor if necessary (e.g., map generic `NotFoundError` to `FileNotFoundError`).
    *   Create a `Layer` (e.g., `githubFileClientLayer`) that provides `GitHubFileClientLive`, taking `GitHubHttpExecutor` as input.
    *   Implement a `GitHubFileClientMock` implementing `GitHubFileClient` (move the logic from `test/github/GitHubApi.test.ts`'s mock here).
    *   Create a `Layer` for the mock (e.g., `mockGitHubFileClientLayer`).

5.  **Refactor Issue Client (`src/github/IssueClient.ts`):**
    *   Perform similar steps as for the File Client:
        *   Move `FetchIssuePayload`, `GitHubIssue`, `IssueNotFoundError`, and schemas from `src/github/GitHubIssueApi.ts` to `src/github/IssueClient.ts`.
        *   Define `GitHubIssueClient` interface and `Context.Tag`.
        *   Implement `GitHubIssueClientLive` using `GitHubHttpExecutor`, similar to `GitHubFileClientLive`, calling the correct API path (`/repos/${owner}/${repo}/issues/${issueNumber}`). Map errors appropriately (e.g., `NotFoundError` to `IssueNotFoundError`).
        *   Create a `Layer` for the live implementation (`githubIssueClientLayer`).
        *   Move the `GitHubIssueClientMock` implementation from `src/github/GitHubIssueApi.ts` here.
        *   Create a `Layer` for the mock (`mockGitHubIssueClientLayer`).

6.  **Clean Up Old Files:**
    *   Delete the original `src/github/GitHubApi.ts` and `src/github/GitHubIssueApi.ts` files.

7.  **Update Usage (`src/Program.ts`):**
    *   Update imports to use the new client tags (`GitHubFileClient`, `GitHubIssueClient`) and errors from their new locations.
    *   Update the layer composition. Instead of merging two separate client layers, you might now provide a base layer (like `defaultGitHubLayer` which includes the executor) and then layers for the specific clients (`githubFileClientLayer`, `githubIssueClientLayer`), or compose them differently based on how you structured the layers in steps 3-5. The goal is to provide `GitHubFileClient` and `GitHubIssueClient` to the program.

8.  **Update Tests:**
    *   Rename `test/github/GitHubApi.test.ts` to `test/github/FileClient.test.ts`.
    *   Rename `test/github/GitHubIssueApi.test.ts` to `test/github/IssueClient.test.ts`.
    *   Update imports in both test files to reflect the new file structure, error locations, and client tags.
    *   Modify `FileClient.test.ts` to use the `mockGitHubFileClientLayer` instead of the inline mock implementation (delete the inline mock class).
    *   Modify `IssueClient.test.ts` to use the `mockGitHubIssueClientLayer` (it might already be using the factory function, just ensure paths are correct).
    *   Ensure tests cover success cases, specific errors (file/issue not found), and common errors (rate limit, generic API error) using the new structure and mocks.
    *   Keep the skipped real API tests in `IssueClient.test.ts` as they were, just update imports. Consider adding similar (skipped) real tests for the file client.

9.  **Verification:**
    *   Run `pnpm check` to ensure there are no TypeScript errors.
    *   Run `pnpm lint` to check for linting issues.
    *   Run `pnpm test` to ensure all tests pass with the new structure and mocks.
    *   Run `pnpm dev` (or equivalent) to ensure `src/Program.ts` still executes correctly against the live API (or demonstrates the expected errors like "file not found").

You have completed the refactoring. The GitHub interaction code should now be more organized, less repetitive, easier to test, and ready for adding more API features.
