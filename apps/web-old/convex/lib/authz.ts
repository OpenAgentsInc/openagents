import type { Doc, Id } from "../_generated/dataModel";
import type { DatabaseReader } from "../_generated/server";
import { fail, requireFound } from "./errors";

export const getOrgMembership = async (
  db: DatabaseReader,
  organizationId: Id<"organizations">,
  userId: string,
): Promise<Doc<"organization_members"> | null> => {
  return (
    (await db
      .query("organization_members")
      .withIndex("by_organization_and_user", (q) =>
        q.eq("organization_id", organizationId).eq("user_id", userId),
      )
      .first()) ?? null
  );
};

export const requireOrgMember = async (
  db: DatabaseReader,
  organizationId: Id<"organizations">,
  userId: string,
): Promise<Doc<"organization_members">> => {
  const membership = await getOrgMembership(db, organizationId, userId);
  return requireFound(
    membership,
    "FORBIDDEN",
    "Not a member of this organization",
  );
};

export const getProjectAccess = async (
  db: DatabaseReader,
  projectId: Id<"projects">,
  userId: string,
): Promise<Doc<"projects"> | null> => {
  const project = await db.get(projectId);
  if (!project) {
    return null;
  }

  if (project.organization_id) {
    const membership = await getOrgMembership(
      db,
      project.organization_id,
      userId,
    );
    return membership ? project : null;
  }

  if (project.user_id && project.user_id !== userId) {
    return null;
  }

  if (!project.user_id && !project.organization_id) {
    return null;
  }

  return project;
};

export const requireProjectAccess = async (
  db: DatabaseReader,
  projectId: Id<"projects">,
  userId: string,
): Promise<Doc<"projects">> => {
  const project = requireFound(
    await db.get(projectId),
    "NOT_FOUND",
    "Project not found",
  );

  if (project.organization_id) {
    await requireOrgMember(db, project.organization_id, userId);
    return project;
  }

  if (project.user_id && project.user_id !== userId) {
    fail("FORBIDDEN", "Not authorized to access this project");
  }

  if (!project.user_id && !project.organization_id) {
    fail("BAD_REQUEST", "Invalid project configuration");
  }

  return project;
};

export const requireIssueAccess = async (
  db: DatabaseReader,
  issueId: Id<"issues">,
  userId: string,
): Promise<Doc<"issues">> => {
  const issue = requireFound(
    await db.get(issueId),
    "NOT_FOUND",
    "Issue not found",
  );

  if (issue.project_id) {
    await requireProjectAccess(db, issue.project_id, userId);
    return issue;
  }

  if (issue.organization_id) {
    await requireOrgMember(db, issue.organization_id, userId);
    return issue;
  }

  if (issue.user_id !== userId) {
    fail("FORBIDDEN", "Not authorized to access this issue");
  }

  return issue;
};

export const getIssueAccess = async (
  db: DatabaseReader,
  issueId: Id<"issues">,
  userId: string,
): Promise<Doc<"issues"> | null> => {
  try {
    return await requireIssueAccess(db, issueId, userId);
  } catch {
    return null;
  }
};
