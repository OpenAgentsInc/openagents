import { Effect, Option, Schema } from "effect";
import {
  ConfectActionCtx,
  ConfectMutationCtx,
  ConfectQueryCtx,
  action,
  internalMutation,
  mutation,
  query,
} from "./confect";
import { api } from "../_generated/api";
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


// Force refresh repositories (for testing)
export const forceRefreshRepositories = mutation({
  args: Schema.Struct({}),
  returns: FetchUserRepositoriesResult,
  handler: () =>
    Effect.gen(function* () {
      console.log(`🔄 [GITHUB_API] Force refresh triggered by user`);
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

      // Force refresh - skip cache check and try to fetch repositories
      console.log(`🔄 [GITHUB_API] ${timestamp} Force fetching repositories (ignoring cache)`);

      // Get GitHub OAuth token from user's stored data
      if (!userData.githubAccessToken) {
        console.log(`⚠️ [GITHUB_API] ${timestamp} No GitHub access token found, trying OpenAuth token`);
        
        // Fallback: try using OpenAuth token directly 
        const identity = yield* auth.getUserIdentity();
        if (Option.isNone(identity) || !identity.value.tokenIdentifier) {
          return yield* Effect.fail(
            new GitHubAuthError("No GitHub or OpenAuth token available - user needs to re-authenticate")
          );
        }
        
        // Extract the actual JWT token and try it with GitHub API
        const jwtToken = identity.value.tokenIdentifier.split(' ')[1]; // Remove "Bearer " prefix
        console.log(`🔑 [GITHUB_API] ${timestamp} Testing OpenAuth token with GitHub API`);
        
        // Try calling GitHub API with OpenAuth token
        const reposUrl = `${GITHUB_API_BASE}/user/repos?sort=updated&per_page=${MAX_REPOSITORIES}&type=all`;
        
        const response = yield* Effect.promise(() =>
          fetch(reposUrl, {
            headers: {
              Authorization: `Bearer ${jwtToken}`,
              Accept: "application/vnd.github.v3+json",
              "User-Agent": "OpenAgents-Mobile/1.0",
            },
          })
        );

        if (!response.ok) {
          const errorBody = yield* Effect.promise(() => response.text());
          console.log(`❌ [GITHUB_API] ${timestamp} OpenAuth token failed: ${response.status} - ${errorBody}`);
          return yield* Effect.fail(
            new GitHubAuthError(`GitHub API failed with OpenAuth token: ${response.statusText}`)
          );
        }

        console.log(`✅ [GITHUB_API] ${timestamp} SUCCESS! OpenAuth token works with GitHub API`);
        const apiResponse = yield* Effect.promise(() => response.json());
        
        // Process repositories
        const repositories = [];
        for (const repo of apiResponse) {
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
        }

        console.log(`✅ [GITHUB_API] ${timestamp} Successfully fetched ${repositories.length} repositories using OpenAuth token`);

        return yield* Effect.succeed({
          repositories,
          totalCount: repositories.length,
          isCached: false,
          lastFetched: Date.now(),
        });
      }

      return yield* Effect.fail(new GitHubAPIError("Not implemented yet for GitHub access token"));
    }),
});

// Fetch user repositories from GitHub API
export const fetchUserRepositories = action({
  args: FetchUserRepositoriesArgs,
  returns: FetchUserRepositoriesResult,
  handler: ({ forceRefresh = false }): any =>
    Effect.gen(function* (): any {
      const { runQuery, runMutation, auth } = yield* ConfectActionCtx;
      const timestamp = new Date().toISOString();

      // Ensure user is authenticated  
      const identity = yield* auth.getUserIdentity();
      if (Option.isNone(identity)) {
        return yield* Effect.fail(new GitHubAuthError("Not authenticated"));
      }

      // Get the user record using internal helper
      console.log(`🔍 [GITHUB_API] ${timestamp} Getting authenticated user`);
      
      const user = yield* runQuery((api as any)["confect/github"]._getAuthenticatedUserForAction, {});

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

      // Get GitHub OAuth token from user's stored data
      if (!userData.githubAccessToken) {
        console.log(`⚠️ [GITHUB_API] ${timestamp} No GitHub access token found, trying OpenAuth token`);
        
        // Fallback: try using OpenAuth token directly 
        // OpenAuth might proxy GitHub API calls or the token might work directly
        const identity = yield* auth.getUserIdentity();
        if (Option.isNone(identity) || !identity.value.tokenIdentifier) {
          return yield* Effect.fail(
            new GitHubAuthError("No GitHub or OpenAuth token available - user needs to re-authenticate")
          );
        }
        
        // Extract the actual JWT token and try it with GitHub API
        const jwtToken = identity.value.tokenIdentifier.split(' ')[1]; // Remove "Bearer " prefix
        console.log(`🔑 [GITHUB_API] ${timestamp} Trying OpenAuth JWT token as fallback`);
        
        // Try calling GitHub API with OpenAuth token (might be proxied)
        const reposUrl = `${GITHUB_API_BASE}/user/repos?sort=updated&per_page=${MAX_REPOSITORIES}&type=all`;
        
        console.log(`🔍 [GITHUB_API] ${timestamp} Testing GitHub API with OpenAuth token: ${reposUrl}`);
        const response = yield* Effect.promise(() =>
          fetch(reposUrl, {
            headers: {
              Authorization: `Bearer ${jwtToken}`,
              Accept: "application/vnd.github.v3+json",
              "User-Agent": "OpenAgents-Mobile/1.0",
            },
          })
        );

        if (!response.ok) {
          const errorBody = yield* Effect.promise(() => response.text());
          console.log(`❌ [GITHUB_API] ${timestamp} OpenAuth token failed for GitHub API: ${response.status} - ${errorBody}`);
          
          if (response.status === 401 || response.status === 403) {
            return yield* Effect.fail(
              new GitHubAuthError(`GitHub authentication failed with OpenAuth token: ${response.statusText}`)
            );
          }
          
          return yield* Effect.fail(
            new GitHubAPIError(
              `GitHub API request failed with OpenAuth token: ${response.statusText}`,
              response.status,
              errorBody
            )
          );
        }

        console.log(`✅ [GITHUB_API] ${timestamp} SUCCESS! OpenAuth token works with GitHub API`);
        const apiResponse = yield* Effect.promise(() => response.json());
        
        // Process the response same as normal GitHub token flow
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

        console.log(`✅ [GITHUB_API] ${timestamp} Successfully fetched ${repositories.length} repositories using OpenAuth token`);

        return yield* Effect.succeed({
          repositories,
          totalCount: repositories.length,
          isCached: false,
          lastFetched: Date.now(),
        });
      }

      const oauthToken = userData.githubAccessToken;
      console.log(`🔑 [GITHUB_API] ${timestamp} Using stored GitHub access token`);

      // Fetch repositories from GitHub API
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

      // Save repositories using internal helper
      const lastFetched = Date.now();
      const githubMetadata = {
        publicRepos: userData.githubMetadata?.publicRepos || 0,
        totalPrivateRepos: userData.githubMetadata?.totalPrivateRepos || 0,
        ownedPrivateRepos: userData.githubMetadata?.ownedPrivateRepos || 0,
        reposUrl: userData.githubMetadata?.reposUrl || `${GITHUB_API_BASE}/users/${userData.githubUsername}/repos`,
        cachedRepos: repositories,
        lastReposFetch: lastFetched,
        lastReposFetchError: undefined, // Clear any previous errors
      };

      yield* runMutation((api as any)["confect/github"]._saveRepositoryDataFromAction, { 
        userId: userData._id, 
        githubMetadata 
      });

      console.log(`✅ [GITHUB_API] ${timestamp} Successfully cached ${repositories.length} repositories`);

      return yield* Effect.succeed({
        repositories,
        totalCount: repositories.length,
        isCached: false,
        lastFetched,
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

// Internal helper to get authenticated user for actions
export const _getAuthenticatedUserForAction = query({
  args: Schema.Struct({}),
  returns: Schema.Option(Schema.Struct({
    _id: Schema.String,
    githubAccessToken: Schema.optional(Schema.String),
    githubMetadata: Schema.optional(Schema.Any),
    githubUsername: Schema.String,
  })),
  handler: () =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectQueryCtx;

      // Ensure user is authenticated  
      const identity = yield* auth.getUserIdentity();
      if (Option.isNone(identity)) {
        return Option.none();
      }

      // Get the user record by OpenAuth subject
      const authSubject = identity.value.subject;
      
      const user = yield* db
        .query("users")
        .withIndex("by_openauth_subject", (q: any) => q.eq("openAuthSubject", authSubject))
        .first();

      return user;
    }),
});

// Internal helper to save repository data from actions
export const _saveRepositoryDataFromAction = internalMutation({
  args: Schema.Struct({
    userId: Schema.String,
    githubMetadata: Schema.Any,
  }),
  returns: Schema.String,
  handler: ({ userId, githubMetadata }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      yield* db.patch(userId as any, { githubMetadata });

      return "success";
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

