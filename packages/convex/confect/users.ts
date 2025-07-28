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

// Get or create user from JWT claims
export const getOrCreateUser = mutation({
  args: GetOrCreateUserArgs,
  returns: GetOrCreateUserResult,
  handler: ({ email, name, avatar, githubId, githubUsername }) =>
    Effect.gen(function* () {
      const { db, auth } = yield* ConfectMutationCtx;

      // Ensure user is authenticated
      const identity = yield* Effect.promise(() => auth.getUserIdentity());
      if (!identity) {
        return yield* Effect.fail(new Error("Not authenticated"));
      }

      // Check if user already exists
      const existingUser = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", githubId))
        .first();

      return yield* Option.match(existingUser, {
        onSome: (user) =>
          // Update existing user
          db.patch(user._id, {
            lastLogin: Date.now(),
            email,
            name,
            avatar,
            githubUsername,
          }).pipe(Effect.as(user._id)),
        
        onNone: () =>
          // Create new user
          db.insert("users", {
            email,
            name,
            avatar,
            githubId,
            githubUsername,
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

      const identity = yield* Effect.promise(() => auth.getUserIdentity());
      if (!identity) {
        return Option.none();
      }

      // Extract GitHub ID from identity subject
      const githubId = identity.subject;
      
      const user = yield* db
        .query("users")
        .withIndex("by_github_id", (q) => q.eq("githubId", githubId))
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