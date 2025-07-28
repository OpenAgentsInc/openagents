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
      if (Option.isNone(identity)) {
        return yield* Effect.fail(new GitHubAuthError("Not authenticated"));
      }

      // Get the user record by OpenAuth subject
      const authSubject = identity.value.subject;
      
      console.log(`🔍 [GITHUB_API] ${timestamp} Looking for user with OpenAuth subject: ${authSubject}`);
      
      const user = yield* db
        .query("users")
        .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
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
          return yield* Effect.succeed({
            repositories: userData.githubMetadata.cachedRepos,
            totalCount: userData.githubMetadata.cachedRepos.length,
            isCached: true,
            lastFetched: userData.githubMetadata.lastReposFetch,
          });
        }
      }

      console.log(`🔄 [GITHUB_API] ${timestamp} Fetching fresh repositories from GitHub API`);

      // TODO: Get OAuth token from secure storage or auth context
      // For now, we'll need to get this from the authentication system
      const oauthToken = "placeholder_token"; // This needs to be implemented

      // Fetch repositories from GitHub API inline to avoid context pollution
      const reposUrl = `${GITHUB_API_BASE}/user/repos?sort=updated&per_page=${MAX_REPOSITORIES}&type=all`;
      
      console.log(`🔍 [GITHUB_API] ${timestamp} Fetching: ${reposUrl}`);
      const response = yield* Effect.promise(() =>
        fetch(reposUrl, {
          headers: {
            Authorization: `Bearer ${oauthToken}`,
            Accept: "application/vnd.github.v3+json",
            "User-Agent": "OpenAgents-Mobile/1.0",
          },
        })
      );

      if (!response.ok) {
        const errorBody = yield* Effect.promise(() => response.text());
        console.error(`❌ [GITHUB_API] ${timestamp} API Error:`, {
          status: response.status,
          statusText: response.statusText,
          body: errorBody,
          url: reposUrl,
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

      const apiResponse = yield* Effect.promise(() => response.json());
      console.log(`✅ [GITHUB_API] ${timestamp} API request successful`);

      // Transform repositories inline to avoid context pollution
      const repositories = [];
      for (const repo of apiResponse) {
        try {
          repositories.push({
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
          });
        } catch (error) {
          return yield* Effect.fail(
            new GitHubAPIError(`Failed to transform repository data: ${error}`)
          );
        }
      }

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

      return yield* Effect.succeed({
        repositories,
        totalCount: repositories.length,
        isCached: false,
        lastFetched: Date.now(),
      });
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
      if (Option.isNone(identity)) {
        console.log(`⚠️ [GITHUB_API] ${timestamp} User not authenticated`);
        return Option.none();
      }

      // Get the user record by OpenAuth subject
      const authSubject = identity.value.subject;
      
      console.log(`🔍 [GITHUB_API] ${timestamp} Looking for user with OpenAuth subject: ${authSubject}`);
      
      const user = yield* db
        .query("users")
        .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
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
      const timestamp = new Date().toISOString();

      // Ensure user is authenticated  
      const identity = yield* auth.getUserIdentity();
      if (Option.isNone(identity)) {
        return yield* Effect.fail(new GitHubAuthError("Not authenticated"));
      }

      // Get the user record by OpenAuth subject
      const authSubject = identity.value.subject;
      
      console.log(`🔍 [GITHUB_API] ${timestamp} Looking for user with OpenAuth subject: ${authSubject}`);
      
      const user = yield* db
        .query("users")
        .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
        .first();

      if (Option.isNone(user)) {
        return yield* Effect.fail(new GitHubAuthError("User not found"));
      }

      // Update the user's GitHub metadata
      yield* db.patch(user.value._id, { githubMetadata });

      return user.value._id;
    }),
});