import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import { getAuthUserId } from "@convex-dev/auth/server";

// Get all threads for the authenticated user
export const getThreads = query({
  args: {},
  handler: async (ctx) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    const threads = await ctx.db
      .query("threads")
      .withIndex("by_user", (q) => q.eq("userId", userId))
      .order("desc")
      .collect();

    return threads;
  },
});

// Get a specific thread by ID
export const getThread = query({
  args: { threadId: v.id("threads") },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    const thread = await ctx.db.get(args.threadId);
    if (!thread) {
      throw new Error("Thread not found");
    }

    // Ensure user owns the thread
    if (thread.userId !== userId) {
      throw new Error("Unauthorized");
    }

    return thread;
  },
});

// Get all messages for a thread
export const getMessages = query({
  args: { threadId: v.id("threads") },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    // Verify thread ownership
    const thread = await ctx.db.get(args.threadId);
    if (!thread || thread.userId !== userId) {
      throw new Error("Unauthorized");
    }

    const messages = await ctx.db
      .query("messages")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .order("asc")
      .collect();

    return messages;
  },
});

// Create a new thread
export const createThread = mutation({
  args: {
    title: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    const now = Date.now();
    const threadId = await ctx.db.insert("threads", {
      userId,
      title: args.title,
      createdAt: now,
      updatedAt: now,
    });

    return threadId;
  },
});

// Add a message to a thread
export const addMessage = mutation({
  args: {
    threadId: v.id("threads"),
    role: v.union(v.literal("user"), v.literal("assistant"), v.literal("system")),
    content: v.string(),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    // Verify thread ownership
    const thread = await ctx.db.get(args.threadId);
    if (!thread || thread.userId !== userId) {
      throw new Error("Unauthorized");
    }

    const now = Date.now();

    // Insert the message
    const messageId = await ctx.db.insert("messages", {
      threadId: args.threadId,
      userId,
      role: args.role,
      content: args.content,
      createdAt: now,
    });

    // Update thread's updatedAt timestamp
    await ctx.db.patch(args.threadId, {
      updatedAt: now,
    });

    return messageId;
  },
});

// Update thread title
export const updateThreadTitle = mutation({
  args: {
    threadId: v.id("threads"),
    title: v.string(),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    // Verify thread ownership
    const thread = await ctx.db.get(args.threadId);
    if (!thread || thread.userId !== userId) {
      throw new Error("Unauthorized");
    }

    await ctx.db.patch(args.threadId, {
      title: args.title,
      updatedAt: Date.now(),
    });
  },
});

// Delete a thread (and all its messages)
export const deleteThread = mutation({
  args: { threadId: v.id("threads") },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    // Verify thread ownership
    const thread = await ctx.db.get(args.threadId);
    if (!thread || thread.userId !== userId) {
      throw new Error("Unauthorized");
    }

    // Delete all messages in the thread
    const messages = await ctx.db
      .query("messages")
      .withIndex("by_thread", (q) => q.eq("threadId", args.threadId))
      .collect();

    for (const message of messages) {
      await ctx.db.delete(message._id);
    }

    // Delete the thread
    await ctx.db.delete(args.threadId);
  },
});

// Archive a thread (soft delete)
export const archiveThread = mutation({
  args: { threadId: v.id("threads") },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    // Verify thread ownership
    const thread = await ctx.db.get(args.threadId);
    if (!thread || thread.userId !== userId) {
      throw new Error("Unauthorized");
    }

    await ctx.db.patch(args.threadId, {
      archived: true,
      updatedAt: Date.now(),
    });
  },
});

// Unarchive a thread
export const unarchiveThread = mutation({
  args: { threadId: v.id("threads") },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    // Verify thread ownership
    const thread = await ctx.db.get(args.threadId);
    if (!thread || thread.userId !== userId) {
      throw new Error("Unauthorized");
    }

    await ctx.db.patch(args.threadId, {
      archived: false,
      updatedAt: Date.now(),
    });
  },
});

// Update thread metadata (for Tinyvex compatibility)
export const updateThread = mutation({
  args: {
    threadId: v.id("threads"),
    title: v.optional(v.string()),
    projectId: v.optional(v.id("projects")),
    resumeId: v.optional(v.string()),
    rolloutPath: v.optional(v.string()),
    source: v.optional(v.string()),
    archived: v.optional(v.boolean()),
    workingDirectory: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    // Verify thread ownership
    const thread = await ctx.db.get(args.threadId);
    if (!thread || thread.userId !== userId) {
      throw new Error("Unauthorized");
    }

    const updates: any = {
      updatedAt: Date.now(),
    };

    if (args.title !== undefined) updates.title = args.title;
    if (args.projectId !== undefined) updates.projectId = args.projectId;
    if (args.resumeId !== undefined) updates.resumeId = args.resumeId;
    if (args.rolloutPath !== undefined) updates.rolloutPath = args.rolloutPath;
    if (args.source !== undefined) updates.source = args.source;
    if (args.archived !== undefined) updates.archived = args.archived;
    if (args.workingDirectory !== undefined) updates.workingDirectory = args.workingDirectory;

    await ctx.db.patch(args.threadId, updates);
  },
});

// Upsert a streaming message (create or update)
export const upsertStreamingMessage = mutation({
  args: {
    threadId: v.string(), // Accept session ID as string
    itemId: v.string(),
    role: v.union(v.literal("user"), v.literal("assistant"), v.literal("system")),
    content: v.string(),
    kind: v.optional(v.union(v.literal("message"), v.literal("reason"))),
    partial: v.optional(v.boolean()),
    seq: v.optional(v.number()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    // Note: threadId is now a session ID string, not a Convex ID
    // We don't validate thread ownership here since ACP sessions manage their own threads

    // Check if message with this itemId already exists
    const existing = await ctx.db
      .query("messages")
      .withIndex("by_item", (q) => q.eq("itemId", args.itemId))
      .first();

    const now = Date.now();

    if (existing) {
      // Update existing message
      await ctx.db.patch(existing._id, {
        content: args.content,
        partial: args.partial ?? false,
        seq: args.seq,
      });

      return existing._id;
    } else {
      // Create new message
      const messageId = await ctx.db.insert("messages", {
        threadId: args.threadId,
        userId,
        role: args.role,
        content: args.content,
        kind: args.kind ?? "message",
        itemId: args.itemId,
        partial: args.partial ?? false,
        seq: args.seq,
        createdAt: now,
      });

      return messageId;
    }
  },
});

// Finalize a streaming message (mark as complete)
export const finalizeMessage = mutation({
  args: {
    itemId: v.string(),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    const message = await ctx.db
      .query("messages")
      .withIndex("by_item", (q) => q.eq("itemId", args.itemId))
      .first();

    if (!message) {
      throw new Error("Message not found");
    }

    // Verify ownership
    if (message.userId !== userId) {
      throw new Error("Unauthorized");
    }

    await ctx.db.patch(message._id, {
      partial: false,
    });
  },
});

// Get partial (streaming) messages for a thread
export const getPartialMessages = query({
  args: { threadId: v.id("threads") },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    // Verify thread ownership
    const thread = await ctx.db.get(args.threadId);
    if (!thread || thread.userId !== userId) {
      throw new Error("Unauthorized");
    }

    const messages = await ctx.db
      .query("messages")
      .withIndex("by_partial", (q) => q.eq("threadId", args.threadId).eq("partial", true))
      .collect();

    return messages;
  },
});

// Create thread with extended fields
export const createThreadExtended = mutation({
  args: {
    title: v.optional(v.string()),
    projectId: v.optional(v.id("projects")),
    resumeId: v.optional(v.string()),
    rolloutPath: v.optional(v.string()),
    source: v.optional(v.string()),
    workingDirectory: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    const now = Date.now();
    const threadId = await ctx.db.insert("threads", {
      userId,
      title: args.title,
      projectId: args.projectId,
      resumeId: args.resumeId,
      rolloutPath: args.rolloutPath,
      source: args.source,
      archived: false,
      workingDirectory: args.workingDirectory,
      createdAt: now,
      updatedAt: now,
    });

    return threadId;
  },
});

// Get threads by project
export const getThreadsByProject = query({
  args: {
    projectId: v.id("projects"),
    includeArchived: v.optional(v.boolean()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (userId === null) {
      throw new Error("Not authenticated");
    }

    // Verify project ownership
    const project = await ctx.db.get(args.projectId);
    if (!project || project.userId !== userId) {
      throw new Error("Unauthorized");
    }

    const threads = await ctx.db
      .query("threads")
      .withIndex("by_project", (q) => q.eq("projectId", args.projectId))
      .order("desc")
      .collect();

    // Filter by archived status if needed
    if (!args.includeArchived) {
      return threads.filter((t) => !t.archived);
    }

    return threads;
  },
});
