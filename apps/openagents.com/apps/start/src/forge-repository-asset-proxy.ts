import { getStartRequestContext } from "@openagentsinc/effect-start";

import { FORGE_MAX_IMAGE_BYTES } from "./features/forge/repository-read";

const ASSET_PREFIX = "/internal/v1/repositories/";
const ALLOWED_IMAGE_TYPES = new Set([
  "image/gif",
  "image/jpeg",
  "image/png",
  "image/svg+xml",
  "image/webp",
]);

type ForgeAssetProxyEnv = Readonly<{
  OPENAGENTS_FORGE_GIT_SERVICE_AUTH_TOKEN?: unknown;
  OPENAGENTS_FORGE_READ_BASE_URL?: unknown;
}>;

export type ForgeRepositoryAssetProxyDeps = Readonly<{
  fetch: (input: string, init?: RequestInit) => Promise<Response>;
  serviceAuthToken: string;
  upstreamBaseUrl: string;
}>;

const noStoreJson = (body: unknown, status: number): Response =>
  Response.json(body, {
    headers: {
      "cache-control": "no-store",
      "content-type": "application/json; charset=utf-8",
    },
    status,
  });

const decodeSegment = (segment: string): string | undefined => {
  try {
    const decoded = decodeURIComponent(segment);
    return decoded === "" || decoded === "." || decoded === ".." || decoded.includes("/")
      ? undefined
      : decoded;
  } catch {
    return undefined;
  }
};

const validatedAssetPath = (url: URL): string | undefined => {
  if (!url.pathname.startsWith(ASSET_PREFIX)) return undefined;
  const parts = url.pathname.split("/").filter((part) => part !== "");
  if (
    parts.length < 7 ||
    parts[0] !== "internal" ||
    parts[1] !== "v1" ||
    parts[2] !== "repositories" ||
    parts[5] !== "web-read-asset"
  ) {
    return undefined;
  }
  const decoded = parts.slice(3).map(decodeSegment);
  return decoded.some((segment) => segment === undefined) ? undefined : url.pathname;
};

export const routeForgeRepositoryAssetRequestWithDeps = async (
  request: Request,
  deps: ForgeRepositoryAssetProxyDeps,
): Promise<Response | undefined> => {
  const requestUrl = new URL(request.url);
  const assetPath = validatedAssetPath(requestUrl);
  if (assetPath === undefined) return undefined;
  if (request.method !== "GET") {
    return new Response(null, {
      headers: {
        allow: "GET",
        "cache-control": "no-store",
      },
      status: 405,
    });
  }

  const objectId = requestUrl.searchParams.get("object");
  const commitId = requestUrl.searchParams.get("commit");
  const objectIdPattern = /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/u;
  if (
    objectId === null ||
    commitId === null ||
    !objectIdPattern.test(objectId) ||
    !objectIdPattern.test(commitId)
  ) {
    return noStoreJson({ error: "not_found" }, 404);
  }

  const upstreamUrl = new URL(assetPath, deps.upstreamBaseUrl);
  upstreamUrl.searchParams.set("object", objectId);
  upstreamUrl.searchParams.set("commit", commitId);
  upstreamUrl.searchParams.set("max_image_bytes", String(FORGE_MAX_IMAGE_BYTES));

  let upstream: Response;
  try {
    upstream = await deps.fetch(upstreamUrl.href, {
      method: "GET",
      signal: request.signal,
      headers: {
        accept: "image/*",
        authorization: `Bearer ${deps.serviceAuthToken}`,
        ...(request.headers.get("cookie") === null
          ? {}
          : { cookie: request.headers.get("cookie") as string }),
      },
    });
  } catch {
    return noStoreJson({ error: "upstream_unavailable", retryable: true }, 503);
  }

  if (!upstream.ok) {
    const status = [401, 403, 404].includes(upstream.status) ? upstream.status : 502;
    return noStoreJson(
      {
        error: status === 502 ? "upstream_unavailable" : "not_found",
        ...(status === 502 ? { retryable: true } : {}),
      },
      status,
    );
  }

  const contentType = upstream.headers.get("content-type")?.split(";", 1)[0]?.trim();
  const contentLength = Number(upstream.headers.get("content-length"));
  if (
    contentType === undefined ||
    !ALLOWED_IMAGE_TYPES.has(contentType) ||
    (Number.isFinite(contentLength) && contentLength > FORGE_MAX_IMAGE_BYTES)
  ) {
    return noStoreJson({ error: "unsupported_image" }, 415);
  }

  const body = await upstream.arrayBuffer();
  if (body.byteLength > FORGE_MAX_IMAGE_BYTES) {
    return noStoreJson({ error: "image_too_large" }, 413);
  }

  return new Response(body, {
    headers: {
      "cache-control": "private, no-store",
      "content-disposition": "inline",
      "content-security-policy": "default-src 'none'; sandbox",
      "content-type": contentType,
      "x-content-type-options": "nosniff",
    },
    status: 200,
  });
};

const configuredString = (value: unknown): string | undefined =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : undefined;

export const routeForgeRepositoryAssetRequest = (
  request: Request,
): Promise<Response | undefined> => {
  const env = getStartRequestContext<ForgeAssetProxyEnv>()?.env;
  const upstreamBaseUrl = configuredString(env?.OPENAGENTS_FORGE_READ_BASE_URL);
  const serviceAuthToken = configuredString(env?.OPENAGENTS_FORGE_GIT_SERVICE_AUTH_TOKEN);
  if (upstreamBaseUrl === undefined || serviceAuthToken === undefined) {
    return Promise.resolve(
      validatedAssetPath(new URL(request.url)) === undefined
        ? undefined
        : noStoreJson({ error: "forge_read_unavailable", retryable: true }, 503),
    );
  }
  return routeForgeRepositoryAssetRequestWithDeps(request, {
    fetch: globalThis.fetch.bind(globalThis),
    serviceAuthToken,
    upstreamBaseUrl,
  });
};
