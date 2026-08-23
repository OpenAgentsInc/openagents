import { Clock, Duration, Effect, Layer, Option, Redacted, Schedule, Schema } from "effect";
import * as Context from "effect/Context";

import { ApiTransport } from "./api-transport.js";
import {
  ApiError,
  ComputerDisabled,
  ComputerPairingExpired,
  ComputerPairingNetworkFailure,
  ComputerPairingRefused,
  ComputerStatusNetworkFailure,
  ContractError,
  NetworkRefused,
  TransportError,
  type CliError,
} from "./errors.js";

export const ComputerPairingStart = Schema.Struct({
  pairing_id: Schema.String,
  code: Schema.String,
  poll_secret: Schema.String,
  verify_url: Schema.String,
  expires_at: Schema.String,
  interval_seconds: Schema.Number,
});
export interface ComputerPairingStart extends Schema.Schema.Type<typeof ComputerPairingStart> {}

export const ComputerPairingClaim = Schema.Struct({
  status: Schema.Literal("approved"),
  machine_id: Schema.String,
  name: Schema.String,
  token: Schema.String,
});
export interface ComputerPairingClaim extends Schema.Schema.Type<typeof ComputerPairingClaim> {}

export const ComputerStatus = Schema.Struct({
  machine_id: Schema.String,
  name: Schema.String,
  status: Schema.Literal("active"),
  token_expires_at: Schema.String,
});
export interface ComputerStatus extends Schema.Schema.Type<typeof ComputerStatus> {}

export interface ComputerPairingRequest {
  readonly name: string;
  readonly tier: "probe" | "curated" | "shell";
  readonly platform: string;
  readonly agentVersion: string;
  readonly roots: ReadonlyArray<string>;
}

export interface ComputerClientInterface {
  readonly start: (
    origin: string,
    request: ComputerPairingRequest,
  ) => Effect.Effect<ComputerPairingStart, CliError>;
  readonly wait: (
    origin: string,
    pairing: ComputerPairingStart,
  ) => Effect.Effect<ComputerPairingClaim, CliError>;
  readonly status: (
    origin: string,
    token: Redacted.Redacted<string>,
  ) => Effect.Effect<Option.Option<ComputerStatus>, CliError>;
}

export class ComputerClient extends Context.Service<ComputerClient, ComputerClientInterface>()(
  "@openagentsinc/cli/ComputerClient",
) {}

const ErrorBody = Schema.Struct({
  error: Schema.optionalKey(Schema.String),
});

const responseCode = (body: unknown): string | undefined => {
  const decoded = Schema.decodeUnknownOption(ErrorBody)(body);
  return Option.isSome(decoded) ? decoded.value.error : undefined;
};

class PairingPending extends Schema.TaggedErrorClass<PairingPending>()(
  "OpenAgentsCli.Internal.PairingPending",
  {},
) {}

export const computerClientLayer = Layer.effect(
  ComputerClient,
  Effect.gen(function* () {
    const transport = yield* ApiTransport;

    const networkFailure = (origin: string) =>
      new ComputerPairingNetworkFailure({
        message: `The Computer pairing request could not reach ${origin}.`,
      });
    const statusNetworkFailure = (origin: string) =>
      new ComputerStatusNetworkFailure({
        message: `The Computer status request could not reach ${origin}.`,
      });

    const request = <A>(
      effect: Effect.Effect<A, NetworkRefused | TransportError>,
      origin: string,
    ): Effect.Effect<A, ComputerPairingNetworkFailure> =>
      effect.pipe(Effect.mapError(() => networkFailure(origin)));

    const start = Effect.fn("ComputerClient.start")(function* (
      origin: string,
      pairing: ComputerPairingRequest,
    ) {
      const response = yield* request(
        transport.request({
          origin,
          method: "POST",
          path: "/controller/pairings",
          body: {
            name: pairing.name,
            tier: pairing.tier,
            platform: pairing.platform,
            agent_version: pairing.agentVersion,
            roots: pairing.roots,
          },
        }),
        origin,
      );
      if (
        response.status === 404 &&
        responseCode(response.body) === "computer_controller_disabled"
      ) {
        return yield* new ComputerDisabled({
          message: "The OpenAgents Computer surface is not enabled on this server.",
        });
      }
      if (response.status === 422) {
        return yield* new ComputerPairingRefused({
          message: "The OpenAgents server refused this Computer pairing.",
        });
      }
      if (response.status !== 200 && response.status !== 201) {
        return yield* new ApiError({
          operation: "register computer pairing",
          status: response.status,
          message: "OpenAgents could not register this Computer pairing.",
          ...(response.requestId === undefined ? {} : { requestId: response.requestId }),
        });
      }
      return yield* Schema.decodeUnknownEffect(ComputerPairingStart)(response.body).pipe(
        Effect.mapError(
          (cause) =>
            new ContractError({
              operation: "register computer pairing",
              message: "The Computer pairing response did not match the API contract.",
              cause,
            }),
        ),
      );
    });

    const poll = Effect.fn("ComputerClient.poll")(function* (
      origin: string,
      pairing: ComputerPairingStart,
    ) {
      const response = yield* request(
        transport.request({
          origin,
          method: "GET",
          path: `/controller/pairings/${encodeURIComponent(pairing.pairing_id)}`,
          headers: { "x-pairing-secret": pairing.poll_secret },
        }),
        origin,
      );
      if (response.status === 410) {
        return yield* new ComputerPairingExpired({
          message: "The Computer pairing expired before the owner approved it.",
        });
      }
      if (response.status === 404 || response.status === 401 || response.status === 403) {
        return yield* new ComputerPairingRefused({
          message: "The Computer pairing was refused or is no longer available.",
        });
      }
      if (response.status !== 200) {
        return yield* new ApiError({
          operation: "poll computer pairing",
          status: response.status,
          message: "OpenAgents could not poll this Computer pairing.",
          ...(response.requestId === undefined ? {} : { requestId: response.requestId }),
        });
      }
      if (
        typeof response.body === "object" &&
        response.body !== null &&
        "status" in response.body &&
        response.body.status === "pending"
      ) {
        return yield* new PairingPending();
      }
      return yield* Schema.decodeUnknownEffect(ComputerPairingClaim)(response.body).pipe(
        Effect.mapError(
          (cause) =>
            new ContractError({
              operation: "claim computer pairing",
              message: "The Computer claim response did not match the API contract.",
              cause,
            }),
        ),
      );
    });

    const wait = Effect.fn("ComputerClient.wait")(function* (
      origin: string,
      pairing: ComputerPairingStart,
    ) {
      const expiresAtMs = Date.parse(pairing.expires_at);
      if (!Number.isFinite(expiresAtMs)) {
        return yield* new ContractError({
          operation: "poll computer pairing",
          message: "The Computer pairing expiry did not match the API contract.",
          cause: new Error("invalid expires_at"),
        });
      }
      const now = yield* Clock.currentTimeMillis;
      const timeoutMs = Math.max(0, expiresAtMs - now);
      const result = yield* poll(origin, pairing).pipe(
        Effect.retry({
          schedule: Schedule.spaced(Duration.seconds(Math.max(1, pairing.interval_seconds))),
          while: (failure) => failure instanceof PairingPending,
        }),
        Effect.timeoutOption(Duration.millis(timeoutMs)),
        Effect.catchTag("OpenAgentsCli.Internal.PairingPending", () =>
          Effect.succeed(Option.none()),
        ),
      );
      if (Option.isNone(result)) {
        return yield* new ComputerPairingExpired({
          message: "The Computer pairing expired before the owner approved it.",
        });
      }
      return result.value;
    });

    const status = Effect.fn("ComputerClient.status")(function* (
      origin: string,
      token: Redacted.Redacted<string>,
    ) {
      const response = yield* transport
        .request({
          origin,
          method: "GET",
          path: "/controller/status",
          token,
        })
        .pipe(Effect.mapError(() => statusNetworkFailure(origin)));
      if (response.status === 401) {
        return Option.none();
      }
      if (response.status !== 200) {
        const code = responseCode(response.body);
        return yield* new ApiError({
          operation: "read computer status",
          status: response.status,
          message: "OpenAgents could not read this Computer status.",
          ...(code === undefined ? {} : { code }),
          ...(response.requestId === undefined ? {} : { requestId: response.requestId }),
        });
      }
      return yield* Schema.decodeUnknownEffect(ComputerStatus)(response.body).pipe(
        Effect.mapError(
          (cause) =>
            new ContractError({
              operation: "read computer status",
              message: "The Computer status response did not match the API contract.",
              cause,
            }),
        ),
        Effect.map(Option.some),
      );
    });

    return ComputerClient.of({ start, wait, status });
  }),
);
