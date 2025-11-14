import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import { getAuthUserId } from "@convex-dev/auth/server";

// List all projects for the authenticated user
export const listProjects = query({
  args: {
    includeArchived: v.optional(v.boolean()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    let projects;
    if (args.includeArchived) {
      // Get all projects
      projects = await ctx.db
        .query("projects")
        .withIndex("by_user", (q) => q.eq("userId", userId))
        .order("desc")
        .collect();
    } else {
      // Get only non-archived projects
      projects = await ctx.db
        .query("projects")
        .withIndex("by_archived", (q) => q.eq("userId", userId).eq("archived", false))
        .order("desc")
        .collect();
    }

    return projects;
  },
});

// List starred projects
export const listStarredProjects = query({
  args: {},
  handler: async (ctx) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    const projects = await ctx.db
      .query("projects")
      .withIndex("by_starred", (q) => q.eq("userId", userId).eq("starred", true))
      .order("desc")
      .collect();

    return projects;
  },
});

// Get a specific project
export const getProject = query({
  args: {
    projectId: v.id("projects"),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    const project = await ctx.db.get(args.projectId);
    if (!project) {
      throw new Error("Project not found");
    }

    // Verify ownership
    if (project.userId !== userId) {
      throw new Error("Unauthorized");
    }

    return project;
  },
});

// Create a new project
export const createProject = mutation({
  args: {
    name: v.string(),
    path: v.string(),
    description: v.optional(v.string()),
    icon: v.optional(v.string()),
    color: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    const now = Date.now();
    const projectId = await ctx.db.insert("projects", {
      userId,
      name: args.name,
      path: args.path,
      description: args.description,
      icon: args.icon,
      color: args.color,
      starred: false,
      archived: false,
      createdAt: now,
      updatedAt: now,
    });

    return projectId;
  },
});

// Update a project
export const updateProject = mutation({
  args: {
    projectId: v.id("projects"),
    name: v.optional(v.string()),
    path: v.optional(v.string()),
    description: v.optional(v.string()),
    icon: v.optional(v.string()),
    color: v.optional(v.string()),
    starred: v.optional(v.boolean()),
    archived: v.optional(v.boolean()),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    const project = await ctx.db.get(args.projectId);
    if (!project) {
      throw new Error("Project not found");
    }

    // Verify ownership
    if (project.userId !== userId) {
      throw new Error("Unauthorized");
    }

    const updates: any = {
      updatedAt: Date.now(),
    };

    if (args.name !== undefined) updates.name = args.name;
    if (args.path !== undefined) updates.path = args.path;
    if (args.description !== undefined) updates.description = args.description;
    if (args.icon !== undefined) updates.icon = args.icon;
    if (args.color !== undefined) updates.color = args.color;
    if (args.starred !== undefined) updates.starred = args.starred;
    if (args.archived !== undefined) updates.archived = args.archived;

    await ctx.db.patch(args.projectId, updates);
  },
});

// Archive a project (soft delete)
export const archiveProject = mutation({
  args: {
    projectId: v.id("projects"),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    const project = await ctx.db.get(args.projectId);
    if (!project) {
      throw new Error("Project not found");
    }

    // Verify ownership
    if (project.userId !== userId) {
      throw new Error("Unauthorized");
    }

    await ctx.db.patch(args.projectId, {
      archived: true,
      updatedAt: Date.now(),
    });
  },
});

// Unarchive a project
export const unarchiveProject = mutation({
  args: {
    projectId: v.id("projects"),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    const project = await ctx.db.get(args.projectId);
    if (!project) {
      throw new Error("Project not found");
    }

    // Verify ownership
    if (project.userId !== userId) {
      throw new Error("Unauthorized");
    }

    await ctx.db.patch(args.projectId, {
      archived: false,
      updatedAt: Date.now(),
    });
  },
});

// Delete a project permanently
export const deleteProject = mutation({
  args: {
    projectId: v.id("projects"),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    const project = await ctx.db.get(args.projectId);
    if (!project) {
      throw new Error("Project not found");
    }

    // Verify ownership
    if (project.userId !== userId) {
      throw new Error("Unauthorized");
    }

    await ctx.db.delete(args.projectId);
  },
});

// Toggle starred status
export const toggleStarred = mutation({
  args: {
    projectId: v.id("projects"),
  },
  handler: async (ctx, args) => {
    const userId = await getAuthUserId(ctx);
    if (!userId) {
      throw new Error("Not authenticated");
    }

    const project = await ctx.db.get(args.projectId);
    if (!project) {
      throw new Error("Project not found");
    }

    // Verify ownership
    if (project.userId !== userId) {
      throw new Error("Unauthorized");
    }

    await ctx.db.patch(args.projectId, {
      starred: !project.starred,
      updatedAt: Date.now(),
    });
  },
});
