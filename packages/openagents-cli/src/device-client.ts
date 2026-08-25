import { Duration, Effect, Layer, Option, Redacted, Schedule, Schema } from "effect";
import * as Context from "effect/Context";

import { ApiTransport } from "./api-transport.js";
import { API_VERSION_PATH } from "./constants.js";
import { ApiError, ContractError, type CliError } from "./errors.js";

export const DeviceAuthorization = Schema.Struct({
  device_code: Schema.String,
  user_code: Schema.String,
  verification_uri: Schema.String,
  verification_uri_complete: Schema.String,
  expires_in: Schema.Number,
  interval: Schema.Number,
});
export interface DeviceAuthorization extends Schema.Schema.Type<typeof DeviceAuthorization> {}

const DeviceToken = Schema.Struct({
  access_token: Schema.String,
  token_type: Schema.Literal("Bearer"),
  scope: Schema.String,
  expires_in: Schema.Number,
});

const DeviceError = Schema.Struct({ code: Schema.String });

class DevicePending extends Schema.TaggedErrorClass<DevicePending>()(
  "OpenAgentsCli.Internal.DevicePending",
  {},
) {}

export interface DeviceClientInterface {
  readonly start: (
    origin: string,
    scopes?: ReadonlyArray<string>,
  ) => Effect.Effect<DeviceAuthorization, CliError>;
  readonly wait: (
    origin: string,
    authorization: DeviceAuthorization,
  ) => Effect.Effect<Redacted.Redacted<string>, CliError>;
}

export class DeviceClient extends Context.Service<DeviceClient, DeviceClientInterface>()(
  "@openagentsinc/cli/DeviceClient",
) {}

export const deviceClientLayer = Layer.effect(
  DeviceClient,
  Effect.gen(function* () {
    const transport = yield* ApiTransport;

    const start = Effect.fn("DeviceClient.start")(function* (
      origin: string,
      scopes?: ReadonlyArray<string>,
    ) {
      const response = yield* transport.request({
        origin,
        method: "POST",
        path: `${API_VERSION_PATH}/device/authorizations`,
        // The server decides the default scope set. Asking for none keeps that
        // decision on the server; asking names exactly what the approval page
        // must show the person approving it.
        body: scopes === undefined || scopes.length === 0 ? {} : { scope: scopes.join(" ") },
      });
      if (response.status !== 201) {
        return yield* new ApiError({
          operation: "start device authorization",
          status: response.status,
          message: "OpenAgents could not start CLI authorization.",
          ...(response.requestId === undefined ? {} : { requestId: response.requestId }),
        });
      }
      return yield* Schema.decodeUnknownEffect(DeviceAuthorization)(response.body).pipe(
        Effect.mapError(
          (cause) =>
            new ContractError({
              operation: "start device authorization",
              message: "The device authorization response did not match the API contract.",
              cause,
            }),
        ),
      );
    });

    const wait = Effect.fn("DeviceClient.wait")(function* (
      origin: string,
      authorization: DeviceAuthorization,
    ) {
      const poll: Effect.Effect<Redacted.Redacted<string>, CliError | DevicePending> = transport
        .request({
          origin,
          method: "POST",
          path: `${API_VERSION_PATH}/device/authorizations/token`,
          body: { device_code: authorization.device_code },
        })
        .pipe(
          Effect.flatMap(
            (
              response,
            ): Effect.Effect<
              Redacted.Redacted<string>,
              ApiError | ContractError | DevicePending
            > => {
              if (response.status === 200) {
                return Schema.decodeUnknownEffect(DeviceToken)(response.body).pipe(
                  Effect.map((token) => Redacted.make(token.access_token)),
                  Effect.mapError(
                    (cause) =>
                      new ContractError({
                        operation: "claim device authorization",
                        message: "The device token response did not match the API contract.",
                        cause,
                      }),
                  ),
                );
              }
              const decoded = Schema.decodeUnknownOption(DeviceError)(response.body);
              if (
                (response.status === 428 || response.status === 429) &&
                Option.isSome(decoded) &&
                (decoded.value.code === "authorization_pending" ||
                  decoded.value.code === "slow_down")
              ) {
                return Effect.fail(new DevicePending());
              }
              return Effect.fail(
                new ApiError({
                  operation: "claim device authorization",
                  status: response.status,
                  message: "CLI authorization was denied, expired, or already claimed.",
                  ...(response.requestId === undefined ? {} : { requestId: response.requestId }),
                }),
              );
            },
          ),
        );

      const result = yield* poll.pipe(
        Effect.retry({
          schedule: Schedule.spaced(Duration.seconds(Math.max(1, authorization.interval))),
          while: (failure) => failure instanceof DevicePending,
        }),
        Effect.timeoutOption(Duration.seconds(Math.max(1, authorization.expires_in))),
        Effect.catch((failure) =>
          failure instanceof DevicePending ? Effect.succeed(Option.none()) : Effect.fail(failure),
        ),
      );

      if (Option.isNone(result)) {
        return yield* new ApiError({
          operation: "claim device authorization",
          status: 400,
          message: "CLI authorization expired before approval.",
        });
      }
      return result.value;
    });

    return DeviceClient.of({ start, wait });
  }),
);

export const deviceClientTestLayer = (client: DeviceClientInterface) =>
  Layer.succeed(DeviceClient, DeviceClient.of(client));
