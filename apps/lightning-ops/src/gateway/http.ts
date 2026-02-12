import { Effect, Layer } from "effect";

import { GatewayRuntimeError } from "../errors.js";

import { GatewayService, type GatewayDeploymentSnapshot } from "./service.js";

export type HttpGatewayConfig = Readonly<{
  baseUrl: string;
  challengeUrl: string;
  proxyUrl: string;
  healthPath?: string;
  opsToken?: string;
  proxyAuthorizationHeader?: string;
}>;

const normalizeUrl = (input: string): string => input.replace(/\/$/, "");

const operationHeaders = (config: HttpGatewayConfig): HeadersInit => {
  if (!config.opsToken) return { "content-type": "application/json" };
  return {
    "content-type": "application/json",
    authorization: `Bearer ${config.opsToken}`,
  };
};

const parseDeploymentSnapshot = (
  input: unknown,
  stage: "active_lookup" | "apply" | "rollback",
): Effect.Effect<GatewayDeploymentSnapshot, GatewayRuntimeError> =>
  Effect.sync(() => {
    if (!input || typeof input !== "object") {
      throw GatewayRuntimeError.make({ stage, reason: "invalid_gateway_payload" });
    }

    const value = input as Record<string, unknown>;
    const deploymentId = typeof value.deploymentId === "string" ? value.deploymentId : "";
    const configHash = typeof value.configHash === "string" ? value.configHash : "";
    const imageDigest = typeof value.imageDigest === "string" ? value.imageDigest : undefined;

    if (!deploymentId || !configHash) {
      throw GatewayRuntimeError.make({ stage, reason: "invalid_gateway_payload" });
    }

    return {
      deploymentId,
      configHash,
      ...(imageDigest ? { imageDigest } : {}),
    };
  }).pipe(
    Effect.catchAll((error) =>
      Effect.fail(
        GatewayRuntimeError.make({
          stage,
          reason: String(error),
        }),
      ),
    ),
  );

const fetchJson = <A>(
  input: RequestInfo | URL,
  init: RequestInit,
  stage: "active_lookup" | "apply" | "rollback",
  selector: (json: unknown) => Effect.Effect<A, GatewayRuntimeError>,
): Effect.Effect<A, GatewayRuntimeError> =>
  Effect.tryPromise({
    try: () => fetch(input, init),
    catch: (error) =>
      GatewayRuntimeError.make({
        stage,
        reason: String(error),
      }),
  }).pipe(
    Effect.flatMap((response) =>
      response.ok
        ? Effect.tryPromise({
            try: () => response.json(),
            catch: (error) =>
              GatewayRuntimeError.make({
                stage,
                reason: String(error),
              }),
          })
        : Effect.fail(
            GatewayRuntimeError.make({
              stage,
              reason: `status_${response.status}`,
            }),
          ),
    ),
    Effect.flatMap(selector),
  );

export const makeHttpGatewayLayer = (config: HttpGatewayConfig) => {
  const baseUrl = normalizeUrl(config.baseUrl);
  const healthPath = config.healthPath ?? "/healthz";

  return Layer.succeed(
    GatewayService,
    GatewayService.of({
      getActiveDeployment: () =>
        fetchJson(
          `${baseUrl}/ops/deployment`,
          {
            method: "GET",
            headers: operationHeaders(config),
          },
          "active_lookup",
          (json) =>
            Effect.sync(() => {
              if (json && typeof json === "object" && "deployment" in (json as Record<string, unknown>)) {
                return (json as Record<string, unknown>).deployment;
              }
              return null;
            }).pipe(
              Effect.flatMap((deployment) =>
                deployment ? parseDeploymentSnapshot(deployment, "active_lookup") : Effect.succeed(null),
              ),
            ),
        ),

      applyConfig: (args) =>
        fetchJson(
          `${baseUrl}/ops/deploy`,
          {
            method: "POST",
            headers: operationHeaders(config),
            body: JSON.stringify({
              requestId: args.requestId,
              deploymentId: args.deploymentId,
              configHash: args.configHash,
              apertureYaml: args.apertureYaml,
            }),
          },
          "apply",
          (json) =>
            Effect.sync(() => {
              if (json && typeof json === "object" && "deployment" in (json as Record<string, unknown>)) {
                return (json as Record<string, unknown>).deployment;
              }
              return json;
            }).pipe(Effect.flatMap((deployment) => parseDeploymentSnapshot(deployment, "apply"))),
        ),

      checkHealth: () =>
        Effect.tryPromise({
          try: () => fetch(`${baseUrl}${healthPath}`, { method: "GET" }),
          catch: (error) =>
            GatewayRuntimeError.make({
              stage: "health",
              reason: String(error),
            }),
        }).pipe(
          Effect.flatMap((response) =>
            response.ok
              ? Effect.succeed({ ok: true as const, statusCode: response.status })
              : Effect.fail(
                  GatewayRuntimeError.make({
                    stage: "health",
                    reason: `status_${response.status}`,
                  }),
                ),
          ),
        ),

      checkChallenge: () =>
        Effect.tryPromise({
          try: () => fetch(config.challengeUrl, { method: "GET" }),
          catch: (error) =>
            GatewayRuntimeError.make({
              stage: "challenge",
              reason: String(error),
            }),
        }).pipe(
          Effect.flatMap((response) => {
            if (response.status !== 402) {
              return Effect.fail(
                GatewayRuntimeError.make({
                  stage: "challenge",
                  reason: `status_${response.status}`,
                }),
              );
            }

            const fromHeader = response.headers.get("x-oa-smoke-auth") ?? undefined;
            const authorizationHeader = config.proxyAuthorizationHeader ?? fromHeader;
            if (!authorizationHeader) {
              return Effect.fail(
                GatewayRuntimeError.make({
                  stage: "challenge",
                  reason: "missing_proxy_authorization_header",
                }),
              );
            }

            return Effect.succeed({
              ok: true as const,
              statusCode: response.status,
              authorizationHeader,
            });
          }),
        ),

      checkProxy: (args) =>
        Effect.tryPromise({
          try: () =>
            fetch(config.proxyUrl, {
              method: "GET",
              headers: {
                authorization: args.authorizationHeader,
              },
            }),
          catch: (error) =>
            GatewayRuntimeError.make({
              stage: "proxy",
              reason: String(error),
            }),
        }).pipe(
          Effect.flatMap((response) =>
            response.ok
              ? Effect.succeed({ ok: true as const, statusCode: response.status })
              : Effect.fail(
                  GatewayRuntimeError.make({
                    stage: "proxy",
                    reason: `status_${response.status}`,
                  }),
                ),
          ),
        ),

      rollbackTo: (args) =>
        fetchJson(
          `${baseUrl}/ops/rollback`,
          {
            method: "POST",
            headers: operationHeaders(config),
            body: JSON.stringify({
              requestId: args.requestId,
              deploymentId: args.deploymentId,
              targetConfigHash: args.target.configHash,
              imageDigest: args.target.imageDigest,
            }),
          },
          "rollback",
          (json) =>
            Effect.sync(() => {
              if (json && typeof json === "object" && "deployment" in (json as Record<string, unknown>)) {
                return (json as Record<string, unknown>).deployment;
              }
              return json;
            }).pipe(Effect.flatMap((deployment) => parseDeploymentSnapshot(deployment, "rollback"))),
        ),
    }),
  );
};
