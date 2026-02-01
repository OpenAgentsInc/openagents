import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import { getUser, requireUser } from "./lib/users";
import { getOrgMembership, requireOrgMember, requireProjectAccess } from "./lib/authz";
import { requireFound } from "./lib/errors";

export const getUserProjects = query({
  args: {
    organizationId: v.optional(v.id("organizations")),
  },
  handler: async (ctx, { organizationId }) => {
    const user = await getUser(ctx);
    if (!user) {
      return [];
    }

    if (!organizationId) {
      const projects = await ctx.db
        .query("projects")
        .withIndex("by_user", (q) => q.eq("user_id", user.user_id))
        .collect();
      return projects.filter((project) => project.is_archived === false);
    }

    const membership = await getOrgMembership(
      ctx.db,
      organizationId,
      user.user_id,
    );
    if (!membership) {
      return [];
    }

    const projects = await ctx.db
      .query("projects")
      .withIndex("by_organization", (q) => q.eq("organization_id", organizationId))
      .collect();

    return projects.filter((project) => project.is_archived === false);
  },
});

export const getProjectDetails = query({
  args: { projectId: v.id("projects") },
  handler: async (ctx, { projectId }) => {
    const user = await getUser(ctx);
    if (!user) {
      return null;
    }

    const project = await requireProjectAccess(ctx.db, projectId, user.user_id);

    const threads = await ctx.db
      .query("threads")
      .withIndex("by_project_id", (q) => q.eq("project_id", projectId))
      .collect();

    return {
      ...project,
      threadCount: threads.length,
    };
  },
});

export const getProjectRepos = query({
  args: { projectId: v.id("projects") },
  handler: async (ctx, { projectId }) => {
    const user = await getUser(ctx);
    if (!user) {
      return [];
    }

    await requireProjectAccess(ctx.db, projectId, user.user_id);

    const links = await ctx.db
      .query("project_repos")
      .withIndex("by_project_id", (q) => q.eq("project_id", projectId))
      .collect();

    const repos = [];
    for (const link of links) {
      const repo = await ctx.db.get(link.repo_id);
      if (repo) {
        repos.push(repo);
      }
    }

    return repos;
  },
});

export const createProject = mutation({
  args: {
    name: v.string(),
    description: v.optional(v.string()),
    organizationId: v.optional(v.id("organizations")),
    system_prompt: v.optional(v.string()),
    default_model: v.optional(v.string()),
    default_tools: v.optional(v.array(v.string())),
  },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);
    const { organizationId, ...rest } = args;

    if (organizationId) {
      await requireOrgMember(ctx.db, organizationId, user.user_id);
    }

    const now = Date.now();
    const projectId = await ctx.db.insert("projects", {
      ...rest,
      organization_id: organizationId,
      user_id: organizationId ? undefined : user.user_id,
      created_at: now,
      updated_at: now,
      is_archived: false,
    });

    const project = await ctx.db.get(projectId);
    return requireFound(project, "NOT_FOUND", "Project not found after creation");
  },
});

export const updateProject = mutation({
  args: {
    project_id: v.id("projects"),
    name: v.optional(v.string()),
    description: v.optional(v.string()),
    system_prompt: v.optional(v.string()),
    default_model: v.optional(v.string()),
    default_tools: v.optional(v.array(v.string())),
  },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);
    const { project_id, ...updates } = args;

    await requireProjectAccess(ctx.db, project_id, user.user_id);

    await ctx.db.patch(project_id, {
      ...updates,
      updated_at: Date.now(),
    });

    return project_id;
  },
});

export const setProjectArchiveStatus = mutation({
  args: {
    project_id: v.id("projects"),
    is_archived: v.boolean(),
  },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);
    const { project_id, is_archived } = args;

    await requireProjectAccess(ctx.db, project_id, user.user_id);

    await ctx.db.patch(project_id, {
      is_archived,
      updated_at: Date.now(),
    });

    return project_id;
  },
});

export const updateSystemPrompt = mutation({
  args: {
    project_id: v.id("projects"),
    system_prompt: v.string(),
  },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);
    const { project_id, system_prompt } = args;

    await requireProjectAccess(ctx.db, project_id, user.user_id);

    await ctx.db.patch(project_id, {
      system_prompt,
      updated_at: Date.now(),
    });

    return project_id;
  },
});

export const updateProjectDefaults = mutation({
  args: {
    project_id: v.id("projects"),
    default_model: v.optional(v.string()),
    default_tools: v.optional(v.array(v.string())),
  },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);
    const { project_id, ...defaults } = args;

    await requireProjectAccess(ctx.db, project_id, user.user_id);

    await ctx.db.patch(project_id, {
      ...defaults,
      updated_at: Date.now(),
    });

    return project_id;
  },
});

const projectRepoInput = v.object({
  provider: v.string(),
  owner: v.string(),
  name: v.string(),
  default_branch: v.optional(v.string()),
  url: v.optional(v.string()),
});

export const connectProjectRepo = mutation({
  args: {
    projectId: v.id("projects"),
    repo: projectRepoInput,
  },
  handler: async (ctx, { projectId, repo }) => {
    const user = await requireUser(ctx);
    await requireProjectAccess(ctx.db, projectId, user.user_id);

    const existingRepo = await ctx.db
      .query("repos")
      .withIndex("by_provider_and_owner_and_name", (q) =>
        q.eq("provider", repo.provider).eq("owner", repo.owner).eq("name", repo.name),
      )
      .unique();

    const repoId =
      existingRepo?._id ??
      (await ctx.db.insert("repos", {
        name: repo.name,
        provider: repo.provider,
        owner: repo.owner,
        default_branch: repo.default_branch,
        url: repo.url,
        created_at: Date.now(),
      }));

    if (existingRepo) {
      const shouldPatch = repo.default_branch !== undefined || repo.url !== undefined;
      if (shouldPatch) {
        await ctx.db.patch(repoId, {
          default_branch: repo.default_branch ?? existingRepo.default_branch,
          url: repo.url ?? existingRepo.url,
        });
      }
    }

    const existingLink = await ctx.db
      .query("project_repos")
      .withIndex("by_project_id_and_repo_id", (q) =>
        q.eq("project_id", projectId).eq("repo_id", repoId),
      )
      .unique();

    if (!existingLink) {
      await ctx.db.insert("project_repos", {
        project_id: projectId,
        repo_id: repoId,
        created_at: Date.now(),
      });
    }

    const connectedRepo = await ctx.db.get(repoId);
    return requireFound(connectedRepo, "NOT_FOUND", "Repo not found after connection");
  },
});

export const disconnectProjectRepo = mutation({
  args: {
    projectId: v.id("projects"),
    repoId: v.id("repos"),
  },
  handler: async (ctx, { projectId, repoId }) => {
    const user = await requireUser(ctx);
    await requireProjectAccess(ctx.db, projectId, user.user_id);

    const link = await ctx.db
      .query("project_repos")
      .withIndex("by_project_id_and_repo_id", (q) =>
        q.eq("project_id", projectId).eq("repo_id", repoId),
      )
      .unique();

    if (link) {
      await ctx.db.delete(link._id);
    }

    return null;
  },
});
