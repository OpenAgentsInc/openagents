import { mutation, query } from "./_generated/server";
import { v } from "convex/values";

// Get or create user from JWT claims
export const getOrCreateUser = mutation({
  args: {
    email: v.string(),
    name: v.optional(v.string()),
    avatar: v.optional(v.string()),
    githubId: v.string(),
    githubUsername: v.string(),
  },
  handler: async (ctx, args) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) {
      throw new Error("Not authenticated");
    }

    // Check if user already exists
    const existingUser = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", args.githubId))
      .first();

    if (existingUser) {
      // Update last login and any changed info
      await ctx.db.patch(existingUser._id, {
        lastLogin: Date.now(),
        email: args.email,
        name: args.name,
        avatar: args.avatar,
        githubUsername: args.githubUsername,
      });
      return existingUser._id;
    }

    // Create new user
    const userId = await ctx.db.insert("users", {
      email: args.email,
      name: args.name,
      avatar: args.avatar,
      githubId: args.githubId,
      githubUsername: args.githubUsername,
      createdAt: Date.now(),
      lastLogin: Date.now(),
    });

    return userId;
  },
});

// Get current user
export const getCurrentUser = query({
  args: {},
  handler: async (ctx) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) {
      return null;
    }

    // Extract GitHub ID from identity subject
    const githubId = identity.subject;
    
    const user = await ctx.db
      .query("users")
      .withIndex("by_github_id", (q) => q.eq("githubId", githubId))
      .first();

    return user;
  },
});

// Get user by ID
export const getUserById = query({
  args: { userId: v.id("users") },
  handler: async (ctx, args) => {
    return await ctx.db.get(args.userId);
  },
});