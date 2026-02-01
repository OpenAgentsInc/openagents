import { httpAction, type ActionCtx } from "./_generated/server";
import { internal } from "./_generated/api";
import type { Id } from "./_generated/dataModel";
import { npubEncode } from "nostr-tools/nip19";
import * as nip98 from "nostr-tools/nip98";
import { verifyEvent } from "nostr-tools/pure";

const CONTROL_HEADER = "x-oa-control-key";

function unauthorized(message = "Unauthorized"): Response {
  return new Response(message, { status: 401 });
}

function badRequest(message: string): Response {
  return new Response(message, { status: 400 });
}

function requireControlKey(request: Request): Response | null {
  const secret = process.env.OA_CONTROL_KEY;
  if (!secret) {
    return new Response("Control key not configured", { status: 500 });
  }
  const header = request.headers.get(CONTROL_HEADER);
  if (!header || header !== secret) {
    return unauthorized();
  }
  return null;
}

function extractApiKey(request: Request, payload?: Record<string, unknown>): string | null {
  const headerAuth = request.headers.get("authorization")?.trim();
  if (headerAuth && headerAuth.toLowerCase().startsWith("bearer ")) {
    const token = headerAuth.slice(7).trim();
    if (token) return token;
  }
  const direct = request.headers.get("x-api-key")?.trim();
  if (direct) return direct;
  const bodyValue = payload?.api_key;
  if (typeof bodyValue === "string" && bodyValue.trim()) {
    return bodyValue.trim();
  }
  return null;
}

function extractNostrAuth(request: Request): string | null {
  const forwarded = request.headers.get("x-oa-nostr-auth")?.trim();
  if (forwarded) return forwarded;
  const headerAuth = request.headers.get("authorization")?.trim();
  if (headerAuth && headerAuth.toLowerCase().startsWith("nostr ")) {
    return headerAuth;
  }
  return null;
}

function getOriginalUrl(request: Request): string {
  return request.headers.get("x-oa-original-url")?.trim() || request.url;
}

async function requireApiKey(
  ctx: ActionCtx,
  request: Request,
  payload?: Record<string, unknown>,
): Promise<{ user_id: string; tokenHash: string } | Response> {
  const apiKey = extractApiKey(request, payload);
  if (!apiKey) {
    return unauthorized("Missing api key");
  }

  const resolved = await ctx.runQuery(internal.apiTokens.resolveApiToken, {
    token: apiKey,
  });

  if (!resolved) {
    return unauthorized("Invalid api key");
  }

  return {
    user_id: resolved.user_id,
    tokenHash: resolved.tokenHash,
  };
}

async function touchApiToken(
  ctx: ActionCtx,
  tokenHash: string,
): Promise<void> {
  await ctx.runMutation(internal.apiTokens.updateApiTokenLastUsed, {
    tokenHash,
  });
}

export const register = httpAction(async (ctx, request) => {
  if (request.method !== "POST") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest("Invalid JSON");
  }

  const userIdRaw = payload.user_id ?? payload.userId ?? payload.subject;
  if (typeof userIdRaw !== "string" || !userIdRaw.trim()) {
    return badRequest("user_id is required");
  }

  const user_id = userIdRaw.trim();
  const email = typeof payload.email === "string" ? payload.email : undefined;
  const name = typeof payload.name === "string" ? payload.name : undefined;
  const image = typeof payload.image === "string" ? payload.image : undefined;
  const tokenNameRaw = payload.token_name ?? payload.tokenName ?? "default";
  const token_name = typeof tokenNameRaw === "string" && tokenNameRaw.trim()
    ? tokenNameRaw
    : "default";

  await ctx.runMutation(internal.users.upsertUser, {
    user_id,
    email,
    name,
    image,
  });

  const token = await ctx.runMutation(internal.apiTokens.issueApiTokenForUser, {
    user_id,
    name: token_name,
  });

  return new Response(
    JSON.stringify({
      ok: true,
      user_id,
      api_key: token.token,
      token_id: token.tokenId,
    }),
    {
      status: 200,
      headers: { "content-type": "application/json" },
    },
  );
});

export const createProject = httpAction(async (ctx, request) => {
  if (request.method !== "POST") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest("Invalid JSON");
  }

  const apiKey = extractApiKey(request, payload);
  if (!apiKey) {
    return unauthorized("Missing api key");
  }

  const resolved = await ctx.runQuery(internal.apiTokens.resolveApiToken, {
    token: apiKey,
  });

  if (!resolved) {
    return unauthorized("Invalid api key");
  }

  const name = typeof payload.name === "string" ? payload.name : "";
  if (!name.trim()) {
    return badRequest("name is required");
  }

  const description = typeof payload.description === "string" ? payload.description : undefined;
  const organizationIdRaw =
    typeof payload.organization_id === "string"
      ? payload.organization_id
      : typeof payload.organizationId === "string"
        ? payload.organizationId
        : undefined;
  const organizationId = organizationIdRaw
    ? (organizationIdRaw as Id<"organizations">)
    : undefined;

  const project = await ctx.runMutation(internal.projects.createProjectForUser, {
    user_id: resolved.user_id,
    name: name.trim(),
    description,
    organizationId,
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, project }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const listProjects = httpAction(async (ctx, request) => {
  if (request.method !== "GET") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  const resolved = await requireApiKey(ctx, request);
  if (resolved instanceof Response) return resolved;

  const url = new URL(request.url);
  const organizationId = url.searchParams.get("organization_id")
    ? (url.searchParams.get("organization_id") as Id<"organizations">)
    : undefined;

  const projects = await ctx.runQuery(internal.projects.listProjectsForUser, {
    user_id: resolved.user_id,
    organizationId: organizationId || undefined,
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, projects }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const createOrganization = httpAction(async (ctx, request) => {
  if (request.method !== "POST") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest("Invalid JSON");
  }

  const resolved = await requireApiKey(ctx, request, payload);
  if (resolved instanceof Response) return resolved;

  const name = typeof payload.name === "string" ? payload.name : "";
  if (!name.trim()) {
    return badRequest("name is required");
  }

  const organization = await ctx.runMutation(
    internal.organizations.createOrganizationForUser,
    {
      user_id: resolved.user_id,
      name: name.trim(),
    },
  );

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, organization }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const listOrganizations = httpAction(async (ctx, request) => {
  if (request.method !== "GET") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  const resolved = await requireApiKey(ctx, request);
  if (resolved instanceof Response) return resolved;

  const organizations = await ctx.runQuery(
    internal.organizations.listOrganizationsForUser,
    {
      user_id: resolved.user_id,
    },
  );

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, organizations }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const listIssues = httpAction(async (ctx, request) => {
  if (request.method !== "GET") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  const resolved = await requireApiKey(ctx, request);
  if (resolved instanceof Response) return resolved;

  const url = new URL(request.url);
  const organizationId = url.searchParams.get("organization_id")
    ? (url.searchParams.get("organization_id") as Id<"organizations">)
    : undefined;
  const projectId = url.searchParams.get("project_id")
    ? (url.searchParams.get("project_id") as Id<"projects">)
    : undefined;

  const issues = await ctx.runQuery(internal.issues.listIssuesForUser, {
    userId: resolved.user_id,
    organizationId,
    projectId,
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, issues }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const createIssue = httpAction(async (ctx, request) => {
  if (request.method !== "POST") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest("Invalid JSON");
  }

  const resolved = await requireApiKey(ctx, request, payload);
  if (resolved instanceof Response) return resolved;

  const title = typeof payload.title === "string" ? payload.title : "";
  if (!title.trim()) {
    return badRequest("title is required");
  }

  const statusId =
    typeof payload.status_id === "string"
      ? payload.status_id
      : typeof payload.statusId === "string"
        ? payload.statusId
        : "";
  const priorityId =
    typeof payload.priority_id === "string"
      ? payload.priority_id
      : typeof payload.priorityId === "string"
        ? payload.priorityId
        : "";
  if (!statusId.trim()) {
    return badRequest("status_id is required");
  }
  if (!priorityId.trim()) {
    return badRequest("priority_id is required");
  }

  const organizationIdRaw =
    typeof payload.organization_id === "string"
      ? payload.organization_id
      : typeof payload.organizationId === "string"
        ? payload.organizationId
        : undefined;
  const projectIdRaw =
    typeof payload.project_id === "string"
      ? payload.project_id
      : typeof payload.projectId === "string"
        ? payload.projectId
        : undefined;
  const organizationId = organizationIdRaw
    ? (organizationIdRaw as Id<"organizations">)
    : undefined;
  const projectId = projectIdRaw ? (projectIdRaw as Id<"projects">) : undefined;

  const issue = await ctx.runMutation(internal.issues.createIssueForUser, {
    userId: resolved.user_id,
    title: title.trim(),
    description:
      typeof payload.description === "string" ? payload.description : undefined,
    statusId: statusId.trim(),
    priorityId: priorityId.trim(),
    assigneeId:
      typeof payload.assignee_id === "string"
        ? payload.assignee_id
        : typeof payload.assigneeId === "string"
          ? payload.assigneeId
          : null,
    labelIds: Array.isArray(payload.label_ids)
      ? (payload.label_ids as string[])
      : Array.isArray(payload.labelIds)
        ? (payload.labelIds as string[])
        : undefined,
    projectId,
    organizationId,
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, issue }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const updateIssue = httpAction(async (ctx, request) => {
  if (request.method !== "PATCH") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest("Invalid JSON");
  }

  const resolved = await requireApiKey(ctx, request, payload);
  if (resolved instanceof Response) return resolved;

  const issueIdRaw =
    typeof payload.issue_id === "string"
      ? payload.issue_id
      : typeof payload.issueId === "string"
        ? payload.issueId
        : "";
  if (!issueIdRaw.trim()) {
    return badRequest("issue_id is required");
  }
  const issueId = issueIdRaw as Id<"issues">;

  const projectIdRaw =
    typeof payload.project_id === "string"
      ? payload.project_id
      : typeof payload.projectId === "string"
        ? payload.projectId
        : undefined;

  const dueDateRaw = payload.due_date ?? payload.dueDate;
  const dueDate =
    typeof dueDateRaw === "string" || typeof dueDateRaw === "number"
      ? dueDateRaw
      : undefined;

  const issue = await ctx.runMutation(internal.issues.updateIssueForUser, {
    userId: resolved.user_id,
    issueId,
    title: typeof payload.title === "string" ? payload.title : undefined,
    description:
      typeof payload.description === "string" ? payload.description : undefined,
    statusId:
      typeof payload.status_id === "string"
        ? payload.status_id
        : typeof payload.statusId === "string"
          ? payload.statusId
          : undefined,
    priorityId:
      typeof payload.priority_id === "string"
        ? payload.priority_id
        : typeof payload.priorityId === "string"
          ? payload.priorityId
          : undefined,
    assigneeId:
      typeof payload.assignee_id === "string"
        ? payload.assignee_id
        : typeof payload.assigneeId === "string"
          ? payload.assigneeId
          : null,
    labelIds: Array.isArray(payload.label_ids)
      ? (payload.label_ids as string[])
      : Array.isArray(payload.labelIds)
      ? (payload.labelIds as string[])
        : undefined,
    projectId: projectIdRaw ? (projectIdRaw as Id<"projects">) : undefined,
    dueDate,
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, issue }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const deleteIssue = httpAction(async (ctx, request) => {
  if (request.method !== "DELETE") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest("Invalid JSON");
  }

  const resolved = await requireApiKey(ctx, request, payload);
  if (resolved instanceof Response) return resolved;

  const issueIdRaw =
    typeof payload.issue_id === "string"
      ? payload.issue_id
      : typeof payload.issueId === "string"
        ? payload.issueId
        : "";
  if (!issueIdRaw.trim()) {
    return badRequest("issue_id is required");
  }

  const issueId = issueIdRaw as Id<"issues">;
  const deleted = await ctx.runMutation(internal.issues.deleteIssueForUser, {
    userId: resolved.user_id,
    issueId,
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, issue_id: deleted }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const listRepos = httpAction(async (ctx, request) => {
  if (request.method !== "GET") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  const resolved = await requireApiKey(ctx, request);
  if (resolved instanceof Response) return resolved;

  const url = new URL(request.url);
  const projectIdRaw = url.searchParams.get("project_id");
  if (!projectIdRaw) {
    return badRequest("project_id is required");
  }

  const repos = await ctx.runQuery(internal.projects.getProjectReposForUser, {
    user_id: resolved.user_id,
    projectId: projectIdRaw as Id<"projects">,
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, repos }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const connectRepo = httpAction(async (ctx, request) => {
  if (request.method !== "POST") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest("Invalid JSON");
  }

  const resolved = await requireApiKey(ctx, request, payload);
  if (resolved instanceof Response) return resolved;

  const projectIdRaw =
    typeof payload.project_id === "string"
      ? payload.project_id
      : typeof payload.projectId === "string"
        ? payload.projectId
        : "";
  if (!projectIdRaw.trim()) {
    return badRequest("project_id is required");
  }

  const repo = payload.repo as Record<string, unknown> | undefined;
  if (!repo) {
    return badRequest("repo is required");
  }

  const provider = typeof repo.provider === "string" ? repo.provider : "";
  const owner = typeof repo.owner === "string" ? repo.owner : "";
  const name = typeof repo.name === "string" ? repo.name : "";
  if (!provider || !owner || !name) {
    return badRequest("repo.provider, repo.owner, and repo.name are required");
  }

  const connected = await ctx.runMutation(
    internal.projects.connectProjectRepoForUser,
    {
      user_id: resolved.user_id,
      projectId: projectIdRaw as Id<"projects">,
      repo: {
        provider,
        owner,
        name,
        default_branch:
          typeof repo.default_branch === "string" ? repo.default_branch : undefined,
        url: typeof repo.url === "string" ? repo.url : undefined,
      },
    },
  );

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, repo: connected }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const disconnectRepo = httpAction(async (ctx, request) => {
  if (request.method !== "DELETE") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest("Invalid JSON");
  }

  const resolved = await requireApiKey(ctx, request, payload);
  if (resolved instanceof Response) return resolved;

  const projectIdRaw =
    typeof payload.project_id === "string"
      ? payload.project_id
      : typeof payload.projectId === "string"
        ? payload.projectId
        : "";
  const repoIdRaw =
    typeof payload.repo_id === "string"
      ? payload.repo_id
      : typeof payload.repoId === "string"
        ? payload.repoId
        : "";
  if (!projectIdRaw.trim() || !repoIdRaw.trim()) {
    return badRequest("project_id and repo_id are required");
  }

  await ctx.runMutation(internal.projects.disconnectProjectRepoForUser, {
    user_id: resolved.user_id,
    projectId: projectIdRaw as Id<"projects">,
    repoId: repoIdRaw as Id<"repos">,
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const listTokens = httpAction(async (ctx, request) => {
  if (request.method !== "GET") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  const resolved = await requireApiKey(ctx, request);
  if (resolved instanceof Response) return resolved;

  const tokens = await ctx.runQuery(internal.apiTokens.listApiTokensForUser, {
    user_id: resolved.user_id,
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, tokens }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const createToken = httpAction(async (ctx, request) => {
  if (request.method !== "POST") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest("Invalid JSON");
  }

  const resolved = await requireApiKey(ctx, request, payload);
  if (resolved instanceof Response) return resolved;

  const name = typeof payload.name === "string" ? payload.name : "token";

  const token = await ctx.runMutation(internal.apiTokens.issueApiTokenForUser, {
    user_id: resolved.user_id,
    name,
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(
    JSON.stringify({ ok: true, api_key: token.token, token_id: token.tokenId }),
    {
      status: 200,
      headers: { "content-type": "application/json" },
    },
  );
});

export const revokeToken = httpAction(async (ctx, request) => {
  if (request.method !== "DELETE") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest("Invalid JSON");
  }

  const resolved = await requireApiKey(ctx, request, payload);
  if (resolved instanceof Response) return resolved;

  const tokenIdRaw =
    typeof payload.token_id === "string"
      ? payload.token_id
      : typeof payload.tokenId === "string"
        ? payload.tokenId
        : "";
  if (!tokenIdRaw.trim()) {
    return badRequest("token_id is required");
  }

  await ctx.runMutation(internal.apiTokens.revokeApiTokenForUser, {
    user_id: resolved.user_id,
    tokenId: tokenIdRaw as Id<"api_tokens">,
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const getNostrIdentity = httpAction(async (ctx, request) => {
  if (request.method !== "GET") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  const resolved = await requireApiKey(ctx, request);
  if (resolved instanceof Response) return resolved;

  const identity = await ctx.runQuery(internal.users.getNostrIdentityForUser, {
    user_id: resolved.user_id,
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, identity }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});

export const verifyNostrIdentity = httpAction(async (ctx, request) => {
  if (request.method !== "POST") {
    return new Response("Method Not Allowed", { status: 405 });
  }

  const authError = requireControlKey(request);
  if (authError) return authError;

  let payload: Record<string, unknown> = {};
  try {
    payload = (await request.json()) as Record<string, unknown>;
  } catch {
    return badRequest("Invalid JSON");
  }

  const resolved = await requireApiKey(ctx, request, payload);
  if (resolved instanceof Response) return resolved;

  const nostrAuth = extractNostrAuth(request);
  if (!nostrAuth) {
    return unauthorized("Missing Nostr auth");
  }

  let event: any;
  try {
    event = await nip98.unpackEventFromToken(nostrAuth);
  } catch {
    return unauthorized("Invalid Nostr auth");
  }

  if (!verifyEvent(event)) {
    return unauthorized("Invalid Nostr signature");
  }
  if (!nip98.validateEventKind(event)) {
    return unauthorized("Invalid Nostr auth kind");
  }
  if (!nip98.validateEventTimestamp(event)) {
    return unauthorized("Expired Nostr auth");
  }

  const originalUrl = getOriginalUrl(request);
  if (!nip98.validateEventUrlTag(event, originalUrl)) {
    return unauthorized("Invalid Nostr auth url");
  }
  if (!nip98.validateEventMethodTag(event, request.method)) {
    return unauthorized("Invalid Nostr auth method");
  }

  const payloadTag = event.tags.find((tag) => tag[0] === "payload");
  if (payloadTag && !nip98.validateEventPayloadTag(event, payload)) {
    return unauthorized("Invalid Nostr auth payload");
  }

  const nostr_pubkey = event.pubkey;
  let nostr_npub = "";
  try {
    nostr_npub = npubEncode(nostr_pubkey);
  } catch {
    return badRequest("Invalid Nostr pubkey");
  }

  const identity = await ctx.runMutation(internal.users.linkNostrIdentityForUser, {
    user_id: resolved.user_id,
    pubkey: nostr_pubkey,
    npub: nostr_npub,
    verified_at: Date.now(),
    method: "nip98",
  });

  await touchApiToken(ctx, resolved.tokenHash);

  return new Response(JSON.stringify({ ok: true, identity }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});
