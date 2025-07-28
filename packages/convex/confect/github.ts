import { Effect, Option } from "effect";
import {
  ConfectMutationCtx,
  ConfectQueryCtx,
  mutation,
  query,
} from "./confect";
import {
  FetchUserRepositoriesArgs,
  FetchUserRepositoriesResult,
  GetUserRepositoriesArgs,
  GetUserRepositoriesResult,
  UpdateGitHubMetadataArgs,
  UpdateGitHubMetadataResult,
  GitHubRepositorySchema,
  GitHubAPIError,
  GitHubRateLimitError,
  GitHubAuthError,
} from "./github.schemas";

// Constants
const GITHUB_API_BASE = "https://api.github.com";
const CACHE_DURATION_MS = 60 * 60 * 1000; // 1 hour
const MAX_REPOSITORIES = 5;

// Helper function to fetch from GitHub API with error handling
const fetchFromGitHubAPI = (url: string, token: string) =>
  Effect.gen(function* () {
    const timestamp = new Date().toISOString();
    console.log(`🔍 [GITHUB_API] ${timestamp} Fetching: ${url}`);

    const response = yield* Effect.promise(() =>
      fetch(url, {
        headers: {
          Authorization: `Bearer ${token}`,
          Accept: "application/vnd.github.v3+json",
          "User-Agent": "OpenAgents-Mobile/1.0",
        },
      })
    );

    if (!response.ok) {
      const errorBody = yield* Effect.promise(() => response.text());
      const timestamp = new Date().toISOString();
      
      console.error(`❌ [GITHUB_API] ${timestamp} API Error:`, {
        status: response.status,
        statusText: response.statusText,
        body: errorBody,
        url,
      });

      if (response.status === 401 || response.status === 403) {
        if (response.headers.get("X-RateLimit-Remaining") === "0") {
          const resetTime = parseInt(response.headers.get("X-RateLimit-Reset") || "0");
          return yield* Effect.fail(
            new GitHubRateLimitError(
              "GitHub API rate limit exceeded",
              resetTime,
              0
            )
          );
        }
        return yield* Effect.fail(
          new GitHubAuthError(`GitHub authentication failed: ${response.statusText}`)
        );
      }

      return yield* Effect.fail(
        new GitHubAPIError(
          `GitHub API request failed: ${response.statusText}`,
          response.status,
          errorBody
        )
      );
    }

    const data = yield* Effect.promise(() => response.json());
    console.log(`✅ [GITHUB_API] ${timestamp} API request successful`);
    return data;
  });

// Transform GitHub API response to our schema
const transformGitHubRepo = (repo: any) =>
  Effect.gen(function* () {
    try {
      return {
        id: repo.id,
        name: repo.name,
        fullName: repo.full_name,
        owner: repo.owner.login,
        isPrivate: repo.private,
        defaultBranch: repo.default_branch || "main",
        updatedAt: repo.updated_at,
        description: repo.description || undefined,
        language: repo.language || undefined,
        htmlUrl: repo.html_url,
        cloneUrl: repo.clone_url,
        sshUrl: repo.ssh_url,
      };
    } catch (error) {
      return yield* Effect.fail(
        new GitHubAPIError(`Failed to transform repository data: ${error}`)
      );
    }
  });

// Fetch user repositories from GitHub API
export const fetchUserRepositories = mutation({
  args: FetchUserRepositoriesArgs,
  returns: FetchUserRepositoriesResult,
  handler: ({ forceRefresh = false }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;
      const timestamp = new Date().toISOString();

      // Ensure user is authenticated  
      const identity = yield* auth.getUserIdentity();
      if (!identity) {
        return yield* Effect.fail(new GitHubAuthError("Not authenticated"));
      }

      // Get the user record
      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
        .first();

      if (Option.isNone(user)) {
        return yield* Effect.fail(new GitHubAuthError("User not found"));
      }

      const userData = user.value;

      // Check if we have cached data and it's still valid
      if (!forceRefresh && userData.githubMetadata) {
        const cacheAge = Date.now() - userData.githubMetadata.lastReposFetch;
        if (cacheAge < CACHE_DURATION_MS) {
          console.log(`📦 [GITHUB_API] ${timestamp} Using cached repositories (age: ${Math.round(cacheAge / 1000)}s)`);
          return {
            repositories: userData.githubMetadata.cachedRepos,
            totalCount: userData.githubMetadata.cachedRepos.length,
            isCached: true,
            lastFetched: userData.githubMetadata.lastReposFetch,
          };
        }
      }

      console.log(`🔄 [GITHUB_API] ${timestamp} Fetching fresh repositories from GitHub API`);

      // TODO: Get OAuth token from secure storage or auth context
      // For now, we'll need to get this from the authentication system
      const oauthToken = "placeholder_token"; // This needs to be implemented

      // Fetch repositories from GitHub API
      const reposUrl = `${GITHUB_API_BASE}/user/repos?sort=updated&per_page=${MAX_REPOSITORIES}&type=all`;
      const apiResponse = yield* fetchFromGitHubAPI(reposUrl, oauthToken);

      // Transform API response to our schema
      const repositories = yield* Effect.all(
        apiResponse.map((repo: any) => transformGitHubRepo(repo))
      );

      // Update user's GitHub metadata
      const githubMetadata = {
        publicRepos: userData.githubMetadata?.publicRepos || 0,
        totalPrivateRepos: userData.githubMetadata?.totalPrivateRepos || 0,
        ownedPrivateRepos: userData.githubMetadata?.ownedPrivateRepos || 0,
        reposUrl: userData.githubMetadata?.reposUrl || `${GITHUB_API_BASE}/users/${userData.githubUsername}/repos`,
        cachedRepos: repositories,
        lastReposFetch: Date.now(),
        lastReposFetchError: undefined, // Clear any previous errors
      };

      yield* db.patch(userData._id, { githubMetadata });

      console.log(`✅ [GITHUB_API] ${timestamp} Successfully cached ${repositories.length} repositories`);

      return {
        repositories,
        totalCount: repositories.length,
        isCached: false,
        lastFetched: Date.now(),
      };
    }),
});

// Get cached user repositories
export const getUserRepositories = query({
  args: GetUserRepositoriesArgs,
  returns: GetUserRepositoriesResult,
  handler: () =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectQueryCtx;
      const timestamp = new Date().toISOString();

      // Ensure user is authenticated
      const identity = yield* auth.getUserIdentity();
      if (!identity) {
        console.log(`⚠️ [GITHUB_API] ${timestamp} User not authenticated`);
        return Option.none();
      }

      // Get the user record
      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
        .first();

      if (Option.isNone(user)) {
        console.log(`⚠️ [GITHUB_API] ${timestamp} User not found in database`);
        return Option.none();
      }

      const userData = user.value;

      if (!userData.githubMetadata) {
        console.log(`📦 [GITHUB_API] ${timestamp} No GitHub metadata found for user`);
        return Option.none();
      }

      const cacheAge = Date.now() - userData.githubMetadata.lastReposFetch;
      const isStale = cacheAge > CACHE_DURATION_MS;

      console.log(`📦 [GITHUB_API] ${timestamp} Returning cached repositories`, {
        count: userData.githubMetadata.cachedRepos.length,
        cacheAge: Math.round(cacheAge / 1000),
        isStale,
        hasError: !!userData.githubMetadata.lastReposFetchError,
      });

      return Option.some({
        repositories: userData.githubMetadata.cachedRepos,
        totalCount: userData.githubMetadata.cachedRepos.length,
        isCached: true,
        lastFetched: userData.githubMetadata.lastReposFetch,
        hasError: !!userData.githubMetadata.lastReposFetchError,
        errorMessage: userData.githubMetadata.lastReposFetchError,
      });
    }),
});

// Update GitHub metadata (internal helper)
export const updateGitHubMetadata = mutation({
  args: UpdateGitHubMetadataArgs,
  returns: UpdateGitHubMetadataResult,
  handler: ({ githubMetadata }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      // Ensure user is authenticated  
      const identity = yield* auth.getUserIdentity();
      if (!identity) {
        return yield* Effect.fail(new GitHubAuthError("Not authenticated"));
      }

      // Get the user record
      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", identity.subject))
        .first();

      if (Option.isNone(user)) {
        return yield* Effect.fail(new GitHubAuthError("User not found"));
      }

      // Update the user's GitHub metadata
      yield* db.patch(user.value._id, { githubMetadata });

      return user.value._id;
    }),
});