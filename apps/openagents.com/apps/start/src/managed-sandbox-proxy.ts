import { getStartRequestContext } from "@openagentsinc/effect-start";

import { readKhalaSyncCredentials } from "./khala-sync-proxy";

export const MANAGED_SANDBOX_WEB_PROXY_PATH = "/api/managed-sandboxes/web/supervision" as const;

export type ManagedSandboxProxyDeps = Readonly<{
  fetch: (input: string, init?: RequestInit) => Promise<Response>;
  upstreamBaseUrl: string;
}>;

const noStoreJson = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": "no-store",
    },
  });

const upstreamUrl = (deps: ManagedSandboxProxyDeps): string =>
  `${deps.upstreamBaseUrl.replace(/\/$/u, "")}${MANAGED_SANDBOX_WEB_PROXY_PATH}`;

export const routeManagedSandboxProxyRequestWithDeps = async (
  request: Request,
  deps: ManagedSandboxProxyDeps,
): Promise<Response | undefined> => {
  if (new URL(request.url).pathname !== MANAGED_SANDBOX_WEB_PROXY_PATH) return undefined;
  if (request.method !== "GET" && request.method !== "POST") {
    return noStoreJson({ error: "method_not_allowed" }, 405);
  }
  const credentials = readKhalaSyncCredentials(request);
  if (credentials === undefined) {
    return noStoreJson({ error: "unauthorized" }, 401);
  }
  try {
    const upstream = await deps.fetch(upstreamUrl(deps), {
      method: request.method,
      headers: {
        authorization: `Bearer ${credentials.token}`,
        ...(request.method === "POST" ? { "content-type": "application/json" } : {}),
      },
      ...(request.method === "POST" ? { body: await request.text() } : {}),
    });
    return new Response(await upstream.text(), {
      status: upstream.status,
      headers: {
        "content-type": "application/json; charset=utf-8",
        "cache-control": "no-store",
      },
    });
  } catch {
    return noStoreJson({ error: "upstream_unavailable", retryable: true }, 503);
  }
};

type StartEnvLike = Readonly<{
  MANAGED_SANDBOX_UPSTREAM_BASE_URL?: unknown;
  KHALA_SYNC_UPSTREAM_BASE_URL?: unknown;
}>;

const resolveUpstreamBaseUrl = (): string => {
  const env = getStartRequestContext<StartEnvLike>()?.env;
  const explicit = env?.MANAGED_SANDBOX_UPSTREAM_BASE_URL;
  if (typeof explicit === "string" && explicit.trim() !== "") return explicit.trim();
  const sync = env?.KHALA_SYNC_UPSTREAM_BASE_URL;
  return typeof sync === "string" && sync.trim() !== "" ? sync.trim() : "https://openagents.com";
};

export const routeManagedSandboxProxyRequest = (request: Request): Promise<Response | undefined> =>
  routeManagedSandboxProxyRequestWithDeps(request, {
    fetch: globalThis.fetch.bind(globalThis),
    upstreamBaseUrl: resolveUpstreamBaseUrl(),
  });
