import { ConvexHttpClient } from "convex/browser";
import { Context, Effect, Layer } from "effect";

import { ControlPlaneTransportError } from "../errors.js";
import { OpsRuntimeConfigService } from "../runtime/config.js";

export type ConvexTransportApi = Readonly<{
  query: (functionName: string, args: Record<string, unknown>) => Effect.Effect<unknown, ControlPlaneTransportError>;
  mutation: (
    functionName: string,
    args: Record<string, unknown>,
  ) => Effect.Effect<unknown, ControlPlaneTransportError>;
}>;

export class ConvexTransportService extends Context.Tag("@openagents/lightning-ops/ConvexTransportService")<
  ConvexTransportService,
  ConvexTransportApi
>() {}

export const ConvexTransportLive = Layer.effect(
  ConvexTransportService,
  Effect.gen(function* () {
    const config = yield* OpsRuntimeConfigService;
    const convexUrl = config.convexUrl?.trim() ?? "";

    if (!convexUrl) {
      return yield* ControlPlaneTransportError.make({
        operation: "config:OA_LIGHTNING_OPS_CONVEX_URL",
        reason: "missing required environment variable for convex control-plane mode",
      });
    }

    const client = new ConvexHttpClient(convexUrl, { logger: false });

    const query: ConvexTransportApi["query"] = (functionName, args) =>
      Effect.tryPromise({
        try: () => client.query(functionName as any, args as any),
        catch: (error) =>
          ControlPlaneTransportError.make({
            operation: `query:${functionName}`,
            reason: String(error),
          }),
      });

    const mutation: ConvexTransportApi["mutation"] = (functionName, args) =>
      Effect.tryPromise({
        try: () => client.mutation(functionName as any, args as any),
        catch: (error) =>
          ControlPlaneTransportError.make({
            operation: `mutation:${functionName}`,
            reason: String(error),
          }),
      });

    return ConvexTransportService.of({ query, mutation });
  }),
);

export const makeConvexTransportTestLayer = (transport: ConvexTransportApi) =>
  Layer.succeed(ConvexTransportService, transport);
