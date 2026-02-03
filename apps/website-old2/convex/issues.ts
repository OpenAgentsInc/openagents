import { v } from "convex/values";
import { internalMutation, internalQuery, mutation, query } from "./_generated/server";
import type { Doc, Id } from "./_generated/dataModel";
import type { DatabaseReader, DatabaseWriter } from "./_generated/server";
import { getUser, requireUser } from "./lib/users";
import {
  getIssueAccess,
  getOrgMembership,
  getProjectAccess,
  requireIssueAccess,
  requireProjectAccess,
} from "./lib/authz";
import { requireFound } from "./lib/errors";
import { toTimestamp } from "./lib/time";

type IssueScope = {
  organizationId?: Id<"organizations">;
  projectId?: Id<"projects">;
};

type CreateIssueInput = {
  userId: string;
  title: string;
  description?: string;
  statusId: string;
  priorityId: string;
  assigneeId?: string | null;
  labelIds?: string[];
  projectId?: Id<"projects"> | null;
  organizationId?: Id<"organizations">;
};

const resolveIssueScope = async (
  db: DatabaseReader,
  input: Pick<CreateIssueInput, "userId" | "organizationId" | "projectId">,
): Promise<IssueScope> => {
  let organizationId = input.organizationId;
  let projectId = input.projectId ?? null;

  if (projectId !== null) {
    const project = await db.get(projectId);
    if (!project) {
      projectId = null;
    } else if (project.organization_id) {
      organizationId = project.organization_id;
      const membership = await getOrgMembership(
        db,
        project.organization_id,
        input.userId,
      );
      if (!membership) {
        organizationId = undefined;
        projectId = null;
      }
    } else if (project.user_id && project.user_id !== input.userId) {
      projectId = null;
    }
  }

  if (organizationId) {
    const membership = await getOrgMembership(db, organizationId, input.userId);
    if (!membership) {
      organizationId = undefined;
      projectId = null;
    }
  }

  return {
    organizationId,
    projectId: projectId ?? undefined,
  };
};

const createIssueImpl = async (
  ctx: { db: DatabaseReader & DatabaseWriter },
  input: CreateIssueInput,
): Promise<Doc<"issues">> => {
  const { organizationId, projectId } = await resolveIssueScope(ctx.db, input);

  const now = Date.now();
  const identifier = `ISS-${Date.now().toString(36).toUpperCase()}`;

  const issueId = await ctx.db.insert("issues", {
    user_id: input.userId,
    organization_id: organizationId,
    project_id: projectId,
    identifier,
    title: input.title,
    description: input.description,
    status_id: input.statusId,
    priority_id: input.priorityId,
    assignee_id: input.assigneeId ?? undefined,
    label_ids: input.labelIds ?? [],
    rank: now,
    created_at: now,
    updated_at: now,
    due_date: undefined,
  });

  const issue = await ctx.db.get(issueId);
  return requireFound(issue, "NOT_FOUND", "Issue not found after creation");
};

const listIssuesForUserImpl = async (
  ctx: { db: DatabaseReader },
  userId: string,
  organizationId?: Id<"organizations">,
  projectId?: Id<"projects">,
): Promise<Doc<"issues">[]> => {
  let scopedOrganizationId = organizationId;
  let scopedProjectId = projectId;

  if (scopedProjectId) {
    const project = await getProjectAccess(ctx.db, scopedProjectId, userId);
    if (!project) {
      return [];
    }

    if (project.organization_id) {
      scopedOrganizationId = project.organization_id;
    }
  }

  if (scopedOrganizationId) {
    const membership = await getOrgMembership(
      ctx.db,
      scopedOrganizationId,
      userId,
    );
    if (!membership) {
      return [];
    }
  }

  if (scopedProjectId) {
    return ctx.db
      .query("issues")
      .withIndex("by_project_id", (q) => q.eq("project_id", scopedProjectId))
      .order("desc")
      .collect();
  }

  if (scopedOrganizationId) {
    return ctx.db
      .query("issues")
      .withIndex("by_organization_id", (q) =>
        q.eq("organization_id", scopedOrganizationId),
      )
      .order("desc")
      .collect();
  }

  return ctx.db
    .query("issues")
    .withIndex("by_user_id", (q) => q.eq("user_id", userId))
    .order("desc")
    .collect();
};

const loadUserById = async (ctx: { db: DatabaseReader }, userId: string) => {
  const user = await ctx.db
    .query("users")
    .withIndex("by_user_id", (q) => q.eq("user_id", userId))
    .first();
  return requireFound(user, "NOT_FOUND", "User not found");
};

export const listIssues = query({
  args: {
    organizationId: v.optional(v.id("organizations")),
    projectId: v.optional(v.id("projects")),
  },
  handler: async (ctx, args) => {
    const user = await getUser(ctx);
    if (!user) {
      return [];
    }

    return listIssuesForUserImpl(
      ctx,
      user.user_id,
      args.organizationId,
      args.projectId,
    );
  },
});

export const listIssuesForApi = query({
  args: {
    userId: v.string(),
    organizationId: v.optional(v.id("organizations")),
    projectId: v.optional(v.id("projects")),
  },
  handler: async (ctx, args) => {
    await loadUserById(ctx, args.userId);
    return listIssuesForUserImpl(
      ctx,
      args.userId,
      args.organizationId,
      args.projectId,
    );
  },
});

export const getIssue = query({
  args: {
    issueId: v.id("issues"),
  },
  handler: async (ctx, args) => {
    const user = await getUser(ctx);
    if (!user) {
      return null;
    }

    return getIssueAccess(ctx.db, args.issueId, user.user_id);
  },
});

export const getIssueForApi = query({
  args: {
    userId: v.string(),
    issueId: v.id("issues"),
  },
  handler: async (ctx, args) => {
    await loadUserById(ctx, args.userId);
    return getIssueAccess(ctx.db, args.issueId, args.userId);
  },
});

export const createIssue = mutation({
  args: {
    title: v.string(),
    description: v.optional(v.string()),
    statusId: v.string(),
    priorityId: v.string(),
    assigneeId: v.optional(v.union(v.string(), v.null())),
    labelIds: v.optional(v.array(v.string())),
    projectId: v.optional(v.union(v.id("projects"), v.null())),
    organizationId: v.optional(v.id("organizations")),
  },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);
    return createIssueImpl(ctx, {
      userId: user.user_id,
      title: args.title,
      description: args.description,
      statusId: args.statusId,
      priorityId: args.priorityId,
      assigneeId: args.assigneeId ?? undefined,
      labelIds: args.labelIds,
      projectId: args.projectId,
      organizationId: args.organizationId,
    });
  },
});

export const createIssueForApi = mutation({
  args: {
    userId: v.string(),
    title: v.string(),
    description: v.optional(v.string()),
    statusId: v.string(),
    priorityId: v.string(),
    assigneeId: v.optional(v.union(v.string(), v.null())),
    labelIds: v.optional(v.array(v.string())),
    projectId: v.optional(v.union(v.id("projects"), v.null())),
    organizationId: v.optional(v.id("organizations")),
  },
  handler: async (ctx, args) => {
    await loadUserById(ctx, args.userId);
    return createIssueImpl(ctx, {
      userId: args.userId,
      title: args.title,
      description: args.description,
      statusId: args.statusId,
      priorityId: args.priorityId,
      assigneeId: args.assigneeId ?? undefined,
      labelIds: args.labelIds,
      projectId: args.projectId,
      organizationId: args.organizationId,
    });
  },
});

const buildIssuePatch = async (
  ctx: { db: DatabaseReader },
  userId: string,
  args: {
    title?: string;
    description?: string;
    statusId?: string;
    priorityId?: string;
    assigneeId?: string | null;
    labelIds?: string[];
    projectId?: Id<"projects"> | null;
    dueDate?: string | number;
  },
) => {
  const patch: Record<string, unknown> = {
    updated_at: Date.now(),
  };

  if (args.title !== undefined) {
    patch.title = args.title;
  }
  if (args.description !== undefined) {
    patch.description = args.description;
  }
  if (args.statusId !== undefined) {
    patch.status_id = args.statusId;
  }
  if (args.priorityId !== undefined) {
    patch.priority_id = args.priorityId;
  }
  if (args.assigneeId !== undefined) {
    patch.assignee_id = args.assigneeId ?? undefined;
  }
  if (args.labelIds !== undefined) {
    patch.label_ids = args.labelIds;
  }
  if (args.dueDate !== undefined) {
    const dueDate = toTimestamp(args.dueDate);
    patch.due_date = dueDate;
  }

  if (args.projectId !== undefined) {
    if (args.projectId === null) {
      patch.project_id = undefined;
      patch.organization_id = undefined;
    } else {
      const project = await requireProjectAccess(ctx.db, args.projectId, userId);
      patch.project_id = args.projectId;
      patch.organization_id = project.organization_id ?? undefined;
    }
  }

  return patch;
};

export const updateIssue = mutation({
  args: {
    issueId: v.id("issues"),
    title: v.optional(v.string()),
    description: v.optional(v.string()),
    statusId: v.optional(v.string()),
    priorityId: v.optional(v.string()),
    assigneeId: v.optional(v.union(v.string(), v.null())),
    labelIds: v.optional(v.array(v.string())),
    projectId: v.optional(v.union(v.id("projects"), v.null())),
    dueDate: v.optional(v.union(v.string(), v.number())),
  },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);

    await requireIssueAccess(ctx.db, args.issueId, user.user_id);

    const patch = await buildIssuePatch(ctx, user.user_id, args);

    await ctx.db.patch(args.issueId, patch);

    const updatedIssue = await ctx.db.get(args.issueId);
    return requireFound(
      updatedIssue,
      "NOT_FOUND",
      "Issue not found after update",
    );
  },
});

export const updateIssueForApi = mutation({
  args: {
    userId: v.string(),
    issueId: v.id("issues"),
    title: v.optional(v.string()),
    description: v.optional(v.string()),
    statusId: v.optional(v.string()),
    priorityId: v.optional(v.string()),
    assigneeId: v.optional(v.union(v.string(), v.null())),
    labelIds: v.optional(v.array(v.string())),
    projectId: v.optional(v.union(v.id("projects"), v.null())),
    dueDate: v.optional(v.union(v.string(), v.number())),
  },
  handler: async (ctx, args) => {
    await loadUserById(ctx, args.userId);

    await requireIssueAccess(ctx.db, args.issueId, args.userId);

    const patch = await buildIssuePatch(ctx, args.userId, args);

    await ctx.db.patch(args.issueId, patch);

    const updatedIssue = await ctx.db.get(args.issueId);
    return requireFound(
      updatedIssue,
      "NOT_FOUND",
      "Issue not found after update",
    );
  },
});

export const deleteIssue = mutation({
  args: {
    issueId: v.id("issues"),
  },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);
    const issue = await ctx.db.get(args.issueId);
    if (!issue) {
      return null;
    }

    await requireIssueAccess(ctx.db, args.issueId, user.user_id);

    await ctx.db.delete(args.issueId);
    return args.issueId;
  },
});

export const deleteIssueForApi = mutation({
  args: {
    userId: v.string(),
    issueId: v.id("issues"),
  },
  handler: async (ctx, args) => {
    await loadUserById(ctx, args.userId);
    const issue = await ctx.db.get(args.issueId);
    if (!issue) {
      return null;
    }

    await requireIssueAccess(ctx.db, args.issueId, args.userId);

    await ctx.db.delete(args.issueId);
    return args.issueId;
  },
});

export const listIssuesForUser = internalQuery({
  args: {
    userId: v.string(),
    organizationId: v.optional(v.id("organizations")),
    projectId: v.optional(v.id("projects")),
  },
  handler: async (ctx, args) => {
    return listIssuesForUserImpl(
      ctx,
      args.userId,
      args.organizationId,
      args.projectId,
    );
  },
});

export const getIssueForUser = internalQuery({
  args: {
    userId: v.string(),
    issueId: v.id("issues"),
  },
  handler: async (ctx, args) => {
    return getIssueAccess(ctx.db, args.issueId, args.userId);
  },
});

export const createIssueForUser = internalMutation({
  args: {
    userId: v.string(),
    title: v.string(),
    description: v.optional(v.string()),
    statusId: v.string(),
    priorityId: v.string(),
    assigneeId: v.optional(v.union(v.string(), v.null())),
    labelIds: v.optional(v.array(v.string())),
    projectId: v.optional(v.union(v.id("projects"), v.null())),
    organizationId: v.optional(v.id("organizations")),
  },
  handler: async (ctx, args) => {
    return createIssueImpl(ctx, {
      userId: args.userId,
      title: args.title,
      description: args.description,
      statusId: args.statusId,
      priorityId: args.priorityId,
      assigneeId: args.assigneeId ?? undefined,
      labelIds: args.labelIds,
      projectId: args.projectId,
      organizationId: args.organizationId,
    });
  },
});

export const updateIssueForUser = internalMutation({
  args: {
    userId: v.string(),
    issueId: v.id("issues"),
    title: v.optional(v.string()),
    description: v.optional(v.string()),
    statusId: v.optional(v.string()),
    priorityId: v.optional(v.string()),
    assigneeId: v.optional(v.union(v.string(), v.null())),
    labelIds: v.optional(v.array(v.string())),
    projectId: v.optional(v.union(v.id("projects"), v.null())),
    dueDate: v.optional(v.union(v.string(), v.number())),
  },
  handler: async (ctx, args) => {
    await requireIssueAccess(ctx.db, args.issueId, args.userId);

    const patch = await buildIssuePatch(ctx, args.userId, args);

    await ctx.db.patch(args.issueId, patch);

    const updatedIssue = await ctx.db.get(args.issueId);
    return requireFound(
      updatedIssue,
      "NOT_FOUND",
      "Issue not found after update",
    );
  },
});

export const deleteIssueForUser = internalMutation({
  args: {
    userId: v.string(),
    issueId: v.id("issues"),
  },
  handler: async (ctx, args) => {
    const issue = await ctx.db.get(args.issueId);
    if (!issue) {
      return null;
    }
    await requireIssueAccess(ctx.db, args.issueId, args.userId);

    await ctx.db.delete(args.issueId);
    return args.issueId;
  },
});
