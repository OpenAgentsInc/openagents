import { Effect, Layer } from "effect";

import { ControlPlaneTransportError } from "../errors.js";

import { ControlPlaneTransportService, type ControlPlaneTransportApi } from "./transport.js";

const normalizeBaseUrl = (value: string): string => value.replace(/\/+$/, "");

const buildEndpoint = (baseUrl: string, kind: "query" | "mutation"): string =>
  `${normalizeBaseUrl(baseUrl)}/api/internal/lightning-ops/control-plane/${kind}`;

const parseJsonResponse = (
  response: Response,
  operation: string,
): Effect.Effect<unknown, ControlPlaneTransportError> =>
  Effect.tryPromise({
    try: () => response.json(),
    catch: (error) =>
      ControlPlaneTransportError.make({
        operation,
        reason: String(error),
      }),
  });

const requestControlPlane = (
  kind: "query" | "mutation",
  endpoint: string,
  functionName: string,
  args: Record<string, unknown>,
): Effect.Effect<unknown, ControlPlaneTransportError> => {
  const operation = `${kind}:${functionName}`;

  return Effect.tryPromise({
    try: () =>
      fetch(endpoint, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          accept: "application/json",
        },
        body: JSON.stringify({
          functionName,
          args,
        }),
      }),
    catch: (error) =>
      ControlPlaneTransportError.make({
        operation,
        reason: String(error),
      }),
  }).pipe(
    Effect.flatMap((response) => {
      if (!response.ok) {
        return parseJsonResponse(response, operation).pipe(
          Effect.flatMap((json) =>
            Effect.fail(
              ControlPlaneTransportError.make({
                operation,
                reason: `status_${response.status}:${JSON.stringify(json)}`,
              }),
            ),
          ),
          Effect.catchAll(() =>
            Effect.fail(
              ControlPlaneTransportError.make({
                operation,
                reason: `status_${response.status}`,
              }),
            ),
          ),
        );
      }

      return parseJsonResponse(response, operation).pipe(
        Effect.flatMap((json) => {
          if (
            json &&
            typeof json === "object" &&
            "error" in (json as Record<string, unknown>) &&
            (json as Record<string, unknown>).error
          ) {
            return Effect.fail(
              ControlPlaneTransportError.make({
                operation,
                reason: JSON.stringify((json as Record<string, unknown>).error),
              }),
            );
          }

          return Effect.succeed(json);
        }),
      );
    }),
  );
};

export const ApiTransportLive = Layer.sync(ControlPlaneTransportService, () => {
  const baseUrl = process.env.OA_LIGHTNING_OPS_API_BASE_URL?.trim() ?? "";
  if (!baseUrl) {
    throw ControlPlaneTransportError.make({
      operation: "config:OA_LIGHTNING_OPS_API_BASE_URL",
      reason: "missing required environment variable",
    });
  }

  const queryEndpoint = buildEndpoint(baseUrl, "query");
  const mutationEndpoint = buildEndpoint(baseUrl, "mutation");

  const query: ControlPlaneTransportApi["query"] = (functionName, args) =>
    requestControlPlane("query", queryEndpoint, functionName, args);
  const mutation: ControlPlaneTransportApi["mutation"] = (functionName, args) =>
    requestControlPlane("mutation", mutationEndpoint, functionName, args);

  return ControlPlaneTransportService.of({
    query,
    mutation,
  });
});

export const makeApiTransportTestLayer = (transport: ControlPlaneTransportApi) =>
  Layer.succeed(ControlPlaneTransportService, transport);
