import { Schema } from "effect";
import { Id } from "@rjdellecese/confect/server";

// GitHub repository structure matching GitHub API response
export const GitHubRepositorySchema = Schema.Struct({
  id: Schema.Number,
  name: Schema.String.pipe(Schema.nonEmptyString()),
  fullName: Schema.String.pipe(Schema.nonEmptyString()),
  owner: Schema.String.pipe(Schema.nonEmptyString()),
  isPrivate: Schema.Boolean,
  defaultBranch: Schema.optional(Schema.String),
  updatedAt: Schema.String, // ISO timestamp
  description: Schema.optional(Schema.String),
  language: Schema.optional(Schema.String),
  htmlUrl: Schema.String,
  cloneUrl: Schema.String,
  sshUrl: Schema.String,
});

// GitHub user metadata
export const GitHubMetadataSchema = Schema.Struct({
  publicRepos: Schema.Number,
  totalPrivateRepos: Schema.Number,
  ownedPrivateRepos: Schema.Number,
  reposUrl: Schema.String,
  cachedRepos: Schema.Array(GitHubRepositorySchema),
  lastReposFetch: Schema.Number,
  lastReposFetchError: Schema.optional(Schema.String),
});

// Fetch User Repositories
export const FetchUserRepositoriesArgs = Schema.Struct({
  forceRefresh: Schema.optional(Schema.Boolean),
});

export const FetchUserRepositoriesResult = Schema.Struct({
  repositories: Schema.Any, // Simplified for type compatibility
  totalCount: Schema.Number,
  isCached: Schema.Boolean,
  lastFetched: Schema.Number,
});

// Get User Repositories (from cache)
export const GetUserRepositoriesArgs = Schema.Struct({});

export const GetUserRepositoriesResult = Schema.Option(
  Schema.Struct({
    repositories: Schema.Any, // Simplified for type compatibility
    totalCount: Schema.Number,
    isCached: Schema.Boolean,
    lastFetched: Schema.Number,
    hasError: Schema.Boolean,
    errorMessage: Schema.optional(Schema.String),
  })
);

// Update GitHub metadata
export const UpdateGitHubMetadataArgs = Schema.Struct({
  githubMetadata: GitHubMetadataSchema,
});

export const UpdateGitHubMetadataResult = Id.Id("users");

// GitHub API error types
export class GitHubAPIError {
  readonly _tag = "GitHubAPIError";
  constructor(
    readonly message: string,
    readonly status?: number,
    readonly response?: unknown
  ) {}
}

export class GitHubRateLimitError {
  readonly _tag = "GitHubRateLimitError";
  constructor(
    readonly message: string,
    readonly resetTime: number,
    readonly remaining: number
  ) {}
}

export class GitHubAuthError {
  readonly _tag = "GitHubAuthError";
  constructor(readonly message: string) {}
}