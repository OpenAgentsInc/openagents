import type {
  ManagedSandboxPrivateIngressCapability,
  ManagedSandboxPrivatePreviewResponse,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect } from "effect";

import type { HttpHeadersDecorator } from "./http/responses";
import { ManagedSandboxPrivatePreviewTargetError } from "./managed-sandbox-private-preview-target";

export const MANAGED_SANDBOX_PRIVATE_INGRESS_PATH_PREFIX =
  "/api/managed-sandboxes/private-ingress/" as const;

const CapabilityRefPattern = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const privateTopology =
  /(?:\b10(?:\.\d{1,3}){3}\b|\b192\.168(?:\.\d{1,3}){2}\b|\b172\.(?:1[6-9]|2\d|3[01])(?:\.\d{1,3}){2}\b|\.internal\b|metadata\.google\.internal|compute\.googleapis\.com)/iu;

type AuthenticatedAudience = Readonly<{
  userId: string;
  decorateResponseHeaders?: HttpHeadersDecorator | undefined;
}>;

export type ManagedSandboxPrivatePreviewRouteDependencies<Bindings> = Readonly<{
  authenticateAudience: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<AuthenticatedAudience | undefined>;
  enabled: (env: Bindings) => boolean;
  readCapability: (
    env: Bindings,
    input: { audienceRef: string; capabilityRef: string },
  ) => Promise<ManagedSandboxPrivateIngressCapability | undefined>;
  usePreview: (
    env: Bindings,
    input: {
      requestRef: string;
      capability: ManagedSandboxPrivateIngressCapability;
      audienceRef: string;
      path: string;
      encoding: "utf8";
    },
  ) => Promise<ManagedSandboxPrivatePreviewResponse>;
  now?: () => Date;
  makeRequestRef?: () => string;
  accessUrlDigest?: (capabilityRef: string) => Promise<string>;
}>;

const response = (
  body: BodyInit | null,
  init: ResponseInit,
  decorate?: HttpHeadersDecorator,
): Response => {
  const headers = new Headers(init.headers);
  headers.set("cache-control", "no-store, private");
  headers.set("referrer-policy", "no-referrer");
  headers.set("x-content-type-options", "nosniff");
  decorate?.(headers);
  return new Response(body, { ...init, headers });
};

const jsonError = (error: string, status: number, decorate?: HttpHeadersDecorator): Response =>
  response(
    JSON.stringify({ error }),
    {
      status,
      headers: { "content-type": "application/json; charset=utf-8" },
    },
    decorate,
  );

const previewPath = (url: URL): string | undefined => {
  const value = url.searchParams.get("path") ?? "/workspace/.openagents/preview.html";
  return value.length <= 1_024 && value.startsWith("/workspace/") && !value.includes("\0")
    ? value
    : undefined;
};

const capabilityRefFromPath = (pathname: string): string | undefined => {
  try {
    return decodeURIComponent(pathname.slice(MANAGED_SANDBOX_PRIVATE_INGRESS_PATH_PREFIX.length));
  } catch {
    return undefined;
  }
};

export const makeManagedSandboxPrivatePreviewRoutes = <Bindings>(
  deps: ManagedSandboxPrivatePreviewRouteDependencies<Bindings>,
) => {
  const now = deps.now ?? (() => new Date());
  const makeRequestRef =
    deps.makeRequestRef ??
    (() => `operation.sbx10.preview.${crypto.randomUUID().replaceAll("-", "")}`);
  const accessUrlDigest =
    deps.accessUrlDigest ??
    (async (capabilityRef: string) => {
      const bytes = new TextEncoder().encode(
        `https://openagents.com${MANAGED_SANDBOX_PRIVATE_INGRESS_PATH_PREFIX}${capabilityRef}`,
      );
      const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
      return `sha256:${Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
    });

  const route = (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<Response> | undefined => {
    const url = new URL(request.url);
    if (!url.pathname.startsWith(MANAGED_SANDBOX_PRIVATE_INGRESS_PATH_PREFIX)) return undefined;
    const capabilityRef = capabilityRefFromPath(url.pathname);
    if (
      capabilityRef === undefined ||
      !CapabilityRefPattern.test(capabilityRef) ||
      capabilityRef.includes("/")
    ) {
      return Effect.succeed(jsonError("not_found", 404));
    }
    return Effect.gen(function* () {
      if (request.method !== "GET") return jsonError("method_not_allowed", 405);
      const audience = yield* Effect.promise(() =>
        deps.authenticateAudience(request, env, ctx).catch(() => undefined),
      );
      if (audience === undefined) return jsonError("unauthorized", 401);
      if (!deps.enabled(env)) {
        return jsonError("private_preview_unavailable", 503, audience.decorateResponseHeaders);
      }
      const path = previewPath(url);
      if (path === undefined) {
        return jsonError("preview_path_invalid", 400, audience.decorateResponseHeaders);
      }
      const capability = yield* Effect.promise(() =>
        deps
          .readCapability(env, {
            audienceRef: audience.userId,
            capabilityRef,
          })
          .catch(() => undefined),
      );
      if (
        capability === undefined ||
        capability.capabilityRef !== capabilityRef ||
        capability.audienceRef !== audience.userId
      ) {
        return jsonError("not_found", 404, audience.decorateResponseHeaders);
      }
      if (capability["_tag"] !== "Active") {
        return jsonError("capability_terminal", 410, audience.decorateResponseHeaders);
      }
      if (capability.kind !== "preview" || Date.parse(capability.expiresAt) <= now().getTime()) {
        return jsonError("capability_expired", 410, audience.decorateResponseHeaders);
      }
      const expectedAccessUrlDigest = yield* Effect.promise(() =>
        accessUrlDigest(capabilityRef).catch(() => "unavailable"),
      );
      if (expectedAccessUrlDigest !== capability.accessUrlDigest) {
        return jsonError("not_found", 404, audience.decorateResponseHeaders);
      }
      const used = yield* Effect.promise(async () => {
        try {
          return {
            ok: true as const,
            value: await deps.usePreview(env, {
              requestRef: makeRequestRef(),
              capability,
              audienceRef: audience.userId,
              path,
              encoding: "utf8",
            }),
          };
        } catch (error) {
          return { ok: false as const, error };
        }
      });
      if (!used.ok) {
        const error = used.error;
        const status =
          error instanceof ManagedSandboxPrivatePreviewTargetError ? error.status : 503;
        return jsonError(
          status === 410 ? "capability_terminal" : "private_preview_unavailable",
          status,
          audience.decorateResponseHeaders,
        );
      }
      const preview = used.value.preview;
      if (
        preview.action !== "read_file" ||
        preview.encoding !== "utf8" ||
        preview.binary ||
        privateTopology.test(preview.content) ||
        preview.receipt.secretScan !== "clean" ||
        !preview.receipt.egressDenied ||
        preview.receipt.networkBytes !== 0
      ) {
        return jsonError("private_preview_refused", 502, audience.decorateResponseHeaders);
      }
      const html = /\.html?$/iu.test(path);
      return response(
        preview.content,
        {
          status: 200,
          headers: {
            "content-type": html ? "text/html; charset=utf-8" : "text/plain; charset=utf-8",
            "content-security-policy":
              "sandbox; default-src 'none'; img-src data:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'",
            "x-openagents-content-digest": preview.contentDigest,
            "x-openagents-receipt-ref": preview.receipt.receiptRef,
          },
        },
        audience.decorateResponseHeaders,
      );
    });
  };

  return { route } as const;
};
