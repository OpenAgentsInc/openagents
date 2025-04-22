# GitHub API Refactoring Implementation

## Overview

This document outlines the implementation of the GitHub API refactoring requested in `docs/20250421-1715-githubrefactor.md`. The refactoring centralizes the GitHub API interaction logic, reduces code duplication, and creates a more maintainable and extensible architecture for interacting with GitHub's API.

## Implementation Details

### 1. Centralized Error Handling

Created `src/github/Errors.ts` to define common error types:

- `GitHubApiError`: Generic error for GitHub API interactions
- `RateLimitExceededError`: Specific error for rate limit issues 
- `HttpError`: Error for HTTP-related issues
- `NotFoundError`: Generic "not found" error that client-specific errors can extend

### 2. Core HTTP Executor

Implemented a central HTTP executor in `src/github/Client.ts`:

- `GitHubHttpExecutor` interface with an `execute<A>()` method that handles all HTTP requests
- `GitHubHttpExecutorLive` implementation that:
  - Prepares requests with proper URL, headers, and authentication
  - Handles common HTTP status codes and errors
  - Parses JSON responses
  - Provides detailed error handling for GitHub API responses
  - Uses native `fetch` for HTTP requests to avoid Effect.js HTTP client issues

### 3. Configuration Management

Created a `GitHubConfig` interface and Context Tag for dependency injection:

- Centralized configuration with `baseUrl` and optional auth `token`
- Created layer factories that make it easy to configure GitHub clients

### 4. Refactored File Client

Converted the original file API client to use the central executor:

- Moved file-specific logic to `src/github/FileClient.ts`
- Defined `GitHubFileClient` interface that depends on `GitHubHttpExecutor`
- Implemented `GitHubFileClientLive` for real API interactions
- Kept `GitHubFileClientMock` for testing
- Created proper Layer factories for both implementations

### 5. Refactored Issue Client

Similarly converted the issue API client:

- Moved issue-specific logic to `src/github/IssueClient.ts`
- Defined `GitHubIssueClient` interface
- Implemented both live and mock versions
- Created proper Layer factories

### 6. Dependency Management

Implemented proper dependency injection using Effect's Context and Layer patterns:

- Each client's Layer depends on the central executor
- The executor Layer depends on the configuration
- Created convenient helper functions to compose these Layers
- Updated Program.ts to use the new Layers

### 7. Testing Infrastructure

Updated the test files to use the refactored structure:

- Renamed test files to match their source files
- Updated imports and fixed type issues
- Ensured all tests pass with the new implementation
- Added strongly-typed error handling

## Results

The refactoring successfully:

1. **Eliminated Duplication**: Common HTTP logic and error handling now lives in one place
2. **Improved Type Safety**: No use of `any` type and proper error typing throughout
3. **Enhanced Maintainability**: Clear separation between HTTP execution and API-specific logic
4. **Simplified Testing**: Consistent mocking approach for all GitHub clients
5. **Enabled Extensibility**: Easy to add new API clients following the established pattern

## Future GitHub API Client Implementations

To add a new GitHub API client (e.g., Pull Request, Comments), follow these steps:

1. Create a new file `src/github/[Feature]Client.ts`
2. Define the response schema and interfaces
3. Define feature-specific errors extending common ones where appropriate
4. Create a client interface with methods for the specific API endpoints
5. Implement both live and mock versions of the client
6. Create Layer factories for both implementations
7. Create corresponding test file with appropriate tests

All clients will automatically benefit from the central error handling, configuration, and HTTP execution logic.

## Technical Notes

- Used standard `fetch` API for HTTP requests due to issues with Effect's HTTP client
- Implemented proper TypeScript error handling with discriminated unions
- Used the Effect.js Context and Layer patterns for dependency injection
- Ensured all code passes TypeScript strict checks
- Followed the project's code style guidelines