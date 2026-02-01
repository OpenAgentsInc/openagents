import { httpAction } from "./_generated/server";
import { internal } from "./_generated/api";
import type { Id } from "./_generated/dataModel";

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

  await ctx.runMutation(internal.apiTokens.updateApiTokenLastUsed, {
    tokenHash: resolved.tokenHash,
  });

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

  const apiKey = extractApiKey(request);
  if (!apiKey) {
    return unauthorized("Missing api key");
  }

  const resolved = await ctx.runQuery(internal.apiTokens.resolveApiToken, {
    token: apiKey,
  });

  if (!resolved) {
    return unauthorized("Invalid api key");
  }

  const url = new URL(request.url);
  const organizationId = url.searchParams.get("organization_id")
    ? (url.searchParams.get("organization_id") as Id<"organizations">)
    : undefined;

  const projects = await ctx.runQuery(internal.projects.listProjectsForUser, {
    user_id: resolved.user_id,
    organizationId: organizationId || undefined,
  });

  await ctx.runMutation(internal.apiTokens.updateApiTokenLastUsed, {
    tokenHash: resolved.tokenHash,
  });

  return new Response(JSON.stringify({ ok: true, projects }), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
});
