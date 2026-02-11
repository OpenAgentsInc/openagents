import { Context, Effect, Layer, Schema } from "effect";

import { DesktopConfigService } from "./config";

export type ConnectivityProbeResult = Readonly<{
  readonly openAgentsReachable: boolean;
  readonly convexReachable: boolean;
  readonly checkedAtMs: number;
}>;

export class ConnectivityProbeError extends Schema.TaggedError<ConnectivityProbeError>()(
  "ConnectivityProbeError",
  {
    operation: Schema.String,
    error: Schema.Defect,
  },
) {}

export type ConnectivityProbeApi = Readonly<{
  readonly probe: () => Effect.Effect<ConnectivityProbeResult>;
}>;

export class ConnectivityProbeService extends Context.Tag("@openagents/desktop/ConnectivityProbeService")<
  ConnectivityProbeService,
  ConnectivityProbeApi
>() {}

const requestReachable = Effect.fn("Connectivity.requestReachable")(function* (input: {
  readonly operation: string;
  readonly url: string;
}) {
  const ok = yield* Effect.tryPromise({
    try: async () => {
      const response = await fetch(input.url, {
        method: "GET",
        cache: "no-store",
      });
      return response.status >= 200 && response.status < 500;
    },
    catch: (error) => ConnectivityProbeError.make({ operation: input.operation, error }),
  }).pipe(Effect.catchTag("ConnectivityProbeError", () => Effect.succeed(false)));
  return ok;
});

export const ConnectivityProbeLive = Layer.effect(
  ConnectivityProbeService,
  Effect.gen(function* () {
    const cfg = yield* DesktopConfigService;
    const probe = Effect.fn("Connectivity.probe")(function* () {
      const [openAgentsReachable, convexReachable] = yield* Effect.all([
        requestReachable({
          operation: "probe.openagents",
          url: `${cfg.openAgentsBaseUrl}/api/auth/session`,
        }),
        requestReachable({
          operation: "probe.convex",
          url: cfg.convexUrl,
        }),
      ]);

      return {
        openAgentsReachable,
        convexReachable,
        checkedAtMs: Date.now(),
      } satisfies ConnectivityProbeResult;
    });

    return ConnectivityProbeService.of({ probe: () => probe().pipe(Effect.catchAll(() => Effect.succeed({
      openAgentsReachable: false,
      convexReachable: false,
      checkedAtMs: Date.now(),
    }))) });
  }),
);
