import { v } from "convex/values";
import { mutation, query } from "./_generated/server";
import type { Doc } from "./_generated/dataModel";
import type { DatabaseReader } from "./_generated/server";
import { isAdminUser } from "./lib/admin";
import { getUser, requireUser } from "./lib/users";
import { getOrgMembership, requireOrgMember } from "./lib/authz";
import { fail, requireFound } from "./lib/errors";

const toMemberDetail = async (
  db: DatabaseReader,
  member: Doc<"organization_members">,
): Promise<{
  userId: string;
  role: string;
  joinedAt: number;
  name?: string;
  email?: string;
  image?: string;
} | null> => {
  const user = await db
    .query("users")
    .withIndex("by_user_id", (q) => q.eq("user_id", member.user_id))
    .first();
  if (!user) {
    return null;
  }
  return {
    userId: member.user_id,
    role: member.role,
    joinedAt: member.joined_at,
    name: user.name,
    email: user.email,
    image: user.image,
  };
};

export const createOrganization = mutation({
  args: { name: v.string() },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);
    const now = Date.now();

    const organizationId = await ctx.db.insert("organizations", {
      name: args.name,
      owner_id: user.user_id,
      plan: "startup",
      credits: 0,
      created_at: now,
    });

    await ctx.db.insert("organization_members", {
      organization_id: organizationId,
      user_id: user.user_id,
      role: "owner",
      joined_at: now,
    });

    const organization = await ctx.db.get(organizationId);
    return requireFound(
      organization,
      "NOT_FOUND",
      "Organization not found after creation",
    );
  },
});

export const addUserToOrganization = mutation({
  args: {
    userId: v.string(),
    organizationId: v.id("organizations"),
  },
  handler: async (ctx, args) => {
    const existingMember = await getOrgMembership(
      ctx.db,
      args.organizationId,
      args.userId,
    );

    if (existingMember) {
      fail("CONFLICT", "User is already a member of this organization");
    }

    const user = await ctx.db
      .query("users")
      .withIndex("by_user_id", (q) => q.eq("user_id", args.userId))
      .first();
    if (!user) {
      fail("NOT_FOUND", "User not found");
    }

    await ctx.db.insert("organization_members", {
      organization_id: args.organizationId,
      user_id: args.userId,
      role: "member",
      joined_at: Date.now(),
    });

    return { success: true };
  },
});

export const getUserOrganizations = query({
  args: {},
  handler: async (ctx) => {
    const user = await getUser(ctx);
    if (!user) {
      return [];
    }

    const memberships = await ctx.db
      .query("organization_members")
      .withIndex("by_user", (q) => q.eq("user_id", user.user_id))
      .collect();

    const organizations = await Promise.all(
      memberships.map(async (membership) => {
        const organization = await ctx.db.get(membership.organization_id);
        if (!organization) {
          return null;
        }
        return {
          ...organization,
          role: membership.role,
        };
      }),
    );

    return organizations.filter(
      (organization): organization is NonNullable<typeof organization> =>
        organization !== null,
    );
  },
});

export const getOrganizationDetails = query({
  args: { organizationId: v.id("organizations") },
  handler: async (ctx, { organizationId }) => {
    const user = await getUser(ctx);
    if (!user) {
      return null;
    }

    const membership = await requireOrgMember(
      ctx.db,
      organizationId,
      user.user_id,
    );

    const organization = requireFound(
      await ctx.db.get(organizationId),
      "NOT_FOUND",
      "Organization not found",
    );

    const members = await ctx.db
      .query("organization_members")
      .withIndex("by_organization", (q) =>
        q.eq("organization_id", organizationId),
      )
      .collect();

    const memberDetails = await Promise.all(
      members.map((member) => toMemberDetail(ctx.db, member)),
    );

    return {
      ...organization,
      members: memberDetails.filter(
        (member): member is NonNullable<typeof member> => member !== null,
      ),
      userRole: membership.role,
    };
  },
});

export const listAllOrganizationMemberships = query({
  args: {},
  handler: async (ctx) => {
    const user = await getUser(ctx);
    if (!user || !isAdminUser(user)) {
      fail("FORBIDDEN", "Not authorized");
    }

    const memberships = await ctx.db.query("organization_members").collect();

    return Promise.all(
      memberships.map(async (membership) => {
        const organization = await ctx.db.get(membership.organization_id);
        const memberUser = await ctx.db
          .query("users")
          .withIndex("by_user_id", (q) => q.eq("user_id", membership.user_id))
          .first();
        return {
          organizationId: membership.organization_id,
          organizationName: organization?.name,
          userId: membership.user_id,
          userEmail: memberUser?.email,
          role: membership.role,
          joinedAt: membership.joined_at,
        };
      }),
    );
  },
});

export const listOrganizations = query({
  args: {},
  handler: async (ctx) => {
    const user = await getUser(ctx);
    if (!user || !isAdminUser(user)) {
      fail("FORBIDDEN", "Not authorized to list all organizations");
    }

    const organizations = await ctx.db.query("organizations").collect();

    return Promise.all(
      organizations.map(async (organization) => {
        const members = await ctx.db
          .query("organization_members")
          .withIndex("by_organization", (q) =>
            q.eq("organization_id", organization._id),
          )
          .collect();
        return {
          ...organization,
          memberCount: members.length,
        };
      }),
    );
  },
});

export const listOrganizationMembers = query({
  args: { organizationId: v.id("organizations") },
  handler: async (ctx, args) => {
    const user = await getUser(ctx);
    if (!user) {
      return [];
    }

    const membership = await getOrgMembership(
      ctx.db,
      args.organizationId,
      user.user_id,
    );
    if (!membership) {
      return [];
    }

    const members = await ctx.db
      .query("organization_members")
      .withIndex("by_organization", (q) =>
        q.eq("organization_id", args.organizationId),
      )
      .collect();

    const users = await Promise.all(
      members.map(async (member) => {
        const memberUser = await ctx.db
          .query("users")
          .withIndex("by_user_id", (q) => q.eq("user_id", member.user_id))
          .first();
        if (!memberUser) {
          return null;
        }
        return {
          user: memberUser,
          role: member.role,
          joinedAt: member.joined_at,
        };
      }),
    );

    return users.filter(
      (member): member is NonNullable<typeof member> => member !== null,
    );
  },
});

export const addOrganizationMember = mutation({
  args: {
    organizationId: v.id("organizations"),
    userEmail: v.string(),
    role: v.string(),
  },
  handler: async (ctx, { organizationId, userEmail, role }) => {
    const user = await requireUser(ctx);
    const admin = isAdminUser(user);

    if (!admin) {
      const membership = await requireOrgMember(
        ctx.db,
        organizationId,
        user.user_id,
      );

      if (membership.role !== "owner") {
        fail("FORBIDDEN", "Not authorized to add members");
      }
    }

    const userToAdd = requireFound(
      await ctx.db
        .query("users")
        .withIndex("by_email", (q) => q.eq("email", userEmail))
        .first(),
      "NOT_FOUND",
      "User not found",
    );

    const existingMembership = await ctx.db
      .query("organization_members")
      .withIndex("by_organization_and_user", (q) =>
        q.eq("organization_id", organizationId).eq("user_id", userToAdd.user_id),
      )
      .first();

    if (existingMembership) {
      fail("CONFLICT", "User is already a member of this organization");
    }

    return ctx.db.insert("organization_members", {
      organization_id: organizationId,
      user_id: userToAdd.user_id,
      role,
      joined_at: Date.now(),
    });
  },
});

export const removeOrganizationMember = mutation({
  args: {
    organizationId: v.id("organizations"),
    userId: v.string(),
  },
  handler: async (ctx, { organizationId, userId }) => {
    const currentUser = await requireUser(ctx);
    const admin = isAdminUser(currentUser);

    if (!admin) {
      const currentMembership = await requireOrgMember(
        ctx.db,
        organizationId,
        currentUser.user_id,
      );

      if (currentMembership.role !== "owner") {
        fail("FORBIDDEN", "Not authorized to remove members");
      }
    }

    const membershipToRemove = requireFound(
      await ctx.db
        .query("organization_members")
        .withIndex("by_organization_and_user", (q) =>
          q.eq("organization_id", organizationId).eq("user_id", userId),
        )
        .first(),
      "NOT_FOUND",
      "Member not found",
    );

    if (!admin && membershipToRemove.role === "owner") {
      const owners = await ctx.db
        .query("organization_members")
        .withIndex("by_organization", (q) =>
          q.eq("organization_id", organizationId),
        )
        .collect();
      const ownerCount = owners.filter((member) => member.role === "owner").length;

      if (ownerCount <= 1) {
        fail("CONFLICT", "Cannot remove the last owner");
      }
    }

    await ctx.db.delete(membershipToRemove._id);
    return null;
  },
});

export const updateMemberRole = mutation({
  args: {
    organizationId: v.id("organizations"),
    userId: v.string(),
    newRole: v.string(),
  },
  handler: async (ctx, { organizationId, userId, newRole }) => {
    const currentUser = await requireUser(ctx);
    const admin = isAdminUser(currentUser);

    if (!admin) {
      const currentMembership = await requireOrgMember(
        ctx.db,
        organizationId,
        currentUser.user_id,
      );

      if (currentMembership.role !== "owner") {
        fail("FORBIDDEN", "Not authorized to update roles");
      }
    }

    const membershipToUpdate = requireFound(
      await ctx.db
        .query("organization_members")
        .withIndex("by_organization_and_user", (q) =>
          q.eq("organization_id", organizationId).eq("user_id", userId),
        )
        .first(),
      "NOT_FOUND",
      "Member not found",
    );

    if (!admin && membershipToUpdate.role === "owner" && newRole !== "owner") {
      const owners = await ctx.db
        .query("organization_members")
        .withIndex("by_organization", (q) =>
          q.eq("organization_id", organizationId),
        )
        .collect();
      const ownerCount = owners.filter((member) => member.role === "owner").length;

      if (ownerCount <= 1) {
        fail("CONFLICT", "Cannot demote the last owner");
      }
    }

    await ctx.db.patch(membershipToUpdate._id, { role: newRole });
    return null;
  },
});

export const updateOrganization = mutation({
  args: {
    organizationId: v.id("organizations"),
    name: v.optional(v.string()),
    logo: v.optional(v.string()),
  },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);
    const admin = isAdminUser(user);

    if (!admin) {
      const membership = await requireOrgMember(
        ctx.db,
        args.organizationId,
        user.user_id,
      );
      if (membership.role !== "owner") {
        fail("FORBIDDEN", "Not authorized to update organization");
      }
    }

    const updates: { name?: string; logo?: string } = {};
    if (args.name !== undefined) {
      updates.name = args.name;
    }
    if (args.logo !== undefined) {
      updates.logo = args.logo;
    }

    if (Object.keys(updates).length > 0) {
      await ctx.db.patch(args.organizationId, updates);
    }

    const organization = await ctx.db.get(args.organizationId);
    return requireFound(
      organization,
      "NOT_FOUND",
      "Organization not found",
    );
  },
});

export const deleteOrganization = mutation({
  args: { organizationId: v.id("organizations") },
  handler: async (ctx, args) => {
    const user = await requireUser(ctx);
    const admin = isAdminUser(user);

    if (!admin) {
      const membership = await requireOrgMember(
        ctx.db,
        args.organizationId,
        user.user_id,
      );
      if (membership.role !== "owner") {
        fail("FORBIDDEN", "Not authorized to delete organization");
      }
    }

    const organization = await ctx.db.get(args.organizationId);
    if (!organization) {
      fail("NOT_FOUND", "Organization not found");
    }

    const messageEmbeddings = await ctx.db
      .query("message_embeddings")
      .withIndex("by_organization_id", (q) =>
        q.eq("organization_id", args.organizationId),
      )
      .collect();
    for (const embedding of messageEmbeddings) {
      await ctx.db.delete(embedding._id);
    }

    const messages = await ctx.db
      .query("messages")
      .withIndex("by_organization_id", (q) =>
        q.eq("organization_id", args.organizationId),
      )
      .collect();
    for (const message of messages) {
      await ctx.db.delete(message._id);
    }

    const knowledgeDocs = await ctx.db
      .query("knowledge")
      .withIndex("by_organization", (q) =>
        q.eq("organization_id", args.organizationId),
      )
      .collect();
    for (const doc of knowledgeDocs) {
      await ctx.db.delete(doc._id);
    }

    const threads = await ctx.db
      .query("threads")
      .withIndex("by_organization_id", (q) =>
        q.eq("organization_id", args.organizationId),
      )
      .collect();
    for (const thread of threads) {
      await ctx.db.delete(thread._id);
    }

    const issues = await ctx.db
      .query("issues")
      .withIndex("by_organization_id", (q) =>
        q.eq("organization_id", args.organizationId),
      )
      .collect();
    for (const issue of issues) {
      await ctx.db.delete(issue._id);
    }

    const projects = await ctx.db
      .query("projects")
      .withIndex("by_organization", (q) =>
        q.eq("organization_id", args.organizationId),
      )
      .collect();
    for (const project of projects) {
      await ctx.db.delete(project._id);
    }

    const memberships = await ctx.db
      .query("organization_members")
      .withIndex("by_organization", (q) =>
        q.eq("organization_id", args.organizationId),
      )
      .collect();
    for (const membership of memberships) {
      await ctx.db.delete(membership._id);
    }

    await ctx.db.delete(args.organizationId);
    return null;
  },
});

export const getOrganizationBalance = query({
  args: { organizationId: v.id("organizations") },
  handler: async (ctx, { organizationId }) => {
    const user = await getUser(ctx);
    if (!user) {
      fail("UNAUTHORIZED", "Not authenticated");
    }

    const userValue = user as Doc<"users">;
    await requireOrgMember(ctx.db, organizationId, userValue.user_id);

    const organization = requireFound(
      await ctx.db.get(organizationId),
      "NOT_FOUND",
      "Organization not found",
    );

    return organization.credits ?? 0;
  },
});
