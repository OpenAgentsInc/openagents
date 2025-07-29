import { Effect, Option } from "effect";
import {
  ConfectMutationCtx,
  ConfectQueryCtx,
  mutation,
  query,
} from "./confect";
import {
  GetOrCreateUserArgs,
  GetOrCreateUserResult,
  GetCurrentUserArgs,
  GetCurrentUserResult,
  GetUserByIdArgs,
  GetUserByIdResult,
} from "./users.schemas";

// Helper function to get authenticated user (for internal use by other functions)
export async function getAuthenticatedUser(ctx: any) {
  const identity = await ctx.auth.getUserIdentity();
  if (!identity) {
    throw new Error("Not authenticated");
  }

  // Look up user by OpenAuth subject first, fallback to GitHub ID for backwards compatibility
  let user = await ctx.db
    .query("users")
    .withIndex("by_openauth_subject", (q: any) => q.eq("openAuthSubject", identity.subject))
    .first();

  // Fallback: try looking up by GitHub ID (for backwards compatibility with old users)
  if (!user) {
    user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q: any) => q.eq("githubId", identity.subject))
      .first();
  }

  if (!user) {
    throw new Error("User not found");
  }

  return user;
}

// Get or create user from JWT claims
export const getOrCreateUser = mutation({
  args: GetOrCreateUserArgs,
  returns: GetOrCreateUserResult,
  handler: ({ email, name, avatar, githubId, githubUsername, openAuthSubject, githubAccessToken }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      // Ensure user is authenticated
      const identity = yield* auth.getUserIdentity();
      if (Option.isNone(identity)) {
        return yield* Effect.fail(new Error("Not authenticated"));
      }

      // Check if user already exists by GitHub ID
      const existingUser = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", githubId))
        .first();

      return yield* Option.match(existingUser, {
        onSome: (user) =>
          // Update existing user with new token and auth subject
          db.patch(user._id, {
            lastLogin: Date.now(),
            email,
            name,
            avatar,
            githubUsername,
            openAuthSubject,
            githubAccessToken,
          }).pipe(Effect.as(user._id)),
        
        onNone: () =>
          // Create new user
          db.insert("users", {
            email,
            name,
            avatar,
            githubId,
            githubUsername,
            openAuthSubject,
            githubAccessToken,
            createdAt: Date.now(),
            lastLogin: Date.now(),
          })
      });
    }),
});

// Get current user
export const getCurrentUser = query({
  args: GetCurrentUserArgs,
  returns: GetCurrentUserResult,
  handler: () =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectQueryCtx;

      const identity = yield* auth.getUserIdentity();
      if (Option.isNone(identity)) {
        return Option.none();
      }

      // Extract user by OpenAuth subject
      const authSubject = identity.value.subject;
      
      console.log(`🔍 [USERS] Looking for user with OpenAuth subject: ${authSubject}`);
      
      const user = yield* db
        .query("users")
        .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
        .first();

      return user;
    }),
});

// Get user by ID
export const getUserById = query({
  args: GetUserByIdArgs,
  returns: GetUserByIdResult,
  handler: ({ userId }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;

      return yield* db.get(userId);
    }),
});