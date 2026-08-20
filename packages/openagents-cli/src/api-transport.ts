import { Effect, Layer, Redacted, Schema } from "effect";
import * as Context from "effect/Context";
import { HttpClient, HttpClientRequest } from "effect/unstable/http";

import { loopbackHostname } from "./endpoint.js";
import { NetworkRefused, TransportError } from "./errors.js";

export type HttpMethod = "GET" | "POST";

export interface ApiRequest {
  readonly origin: string;
  readonly method: HttpMethod;
  readonly path: string;
  readonly token: Redacted.Redacted<string>;
  readonly body?: unknown;
}

export interface ApiResponse {
  readonly status: number;
  readonly body: unknown;
  readonly requestId?: string;
}

export interface NetworkPolicyInterface {
  readonly assertAllowed: (origin: string) => Effect.Effect<void, NetworkRefused>;
}

export class NetworkPolicy extends Context.Service<NetworkPolicy, NetworkPolicyInterface>()(
  "@openagentsinc/cli/NetworkPolicy",
) {}

export const networkPolicyLiveLayer = Layer.succeed(
  NetworkPolicy,
  NetworkPolicy.of({
    assertAllowed: Effect.fn("NetworkPolicy.Live.assertAllowed")((_) => Effect.void),
  }),
);

export const networkPolicyTestLoopbackLayer = Layer.succeed(
  NetworkPolicy,
  NetworkPolicy.of({
    assertAllowed: Effect.fn("NetworkPolicy.Test.assertAllowed")(function* (origin: string) {
      const url = yield* Effect.try({
        try: () => new URL(origin),
        catch: () =>
          new NetworkRefused({ origin, message: `Tests refuse the invalid API origin ${origin}.` }),
      });
      if (!loopbackHostname(url.hostname)) {
        return yield* new NetworkRefused({
          origin,
          message: `Tests refuse network access to ${origin}. Use a loopback fake server.`,
        });
      }
    }),
  }),
);

export interface ApiTransportInterface {
  readonly request: (
    input: ApiRequest,
  ) => Effect.Effect<ApiResponse, NetworkRefused | TransportError>;
}

export class ApiTransport extends Context.Service<ApiTransport, ApiTransportInterface>()(
  "@openagentsinc/cli/ApiTransport",
) {}

const transportError = (operation: string, cause: unknown) =>
  new TransportError({
    operation,
    message: `The OpenAgents API request failed during ${operation}.`,
    cause,
  });

export const apiTransportNodeLayer = Layer.effect(
  ApiTransport,
  Effect.gen(function* () {
    const client = yield* HttpClient.HttpClient;
    const policy = yield* NetworkPolicy;

    const request = Effect.fn("ApiTransport.request")(function* (input: ApiRequest) {
      yield* policy.assertAllowed(input.origin);
      const url = new URL(input.path, `${input.origin}/`);
      let httpRequest = HttpClientRequest.make(input.method)(url).pipe(
        HttpClientRequest.acceptJson,
        HttpClientRequest.bearerToken(input.token),
      );
      if (input.body !== undefined) {
        httpRequest = yield* HttpClientRequest.bodyJson(httpRequest, input.body).pipe(
          Effect.mapError((cause) => transportError("encoding the request", cause)),
        );
      }

      const response = yield* client
        .execute(httpRequest)
        .pipe(Effect.mapError((cause) => transportError("sending the request", cause)));
      const text = yield* response.text.pipe(
        Effect.mapError((cause) => transportError("reading the response", cause)),
      );
      const body =
        text.trim().length === 0
          ? null
          : yield* Schema.decodeUnknownEffect(Schema.fromJsonString(Schema.Json))(text).pipe(
              Effect.orElseSucceed(() => text.slice(0, 2_000)),
            );
      const requestId = response.headers["x-request-id"];
      return requestId === undefined
        ? { status: response.status, body }
        : { status: response.status, body, requestId };
    });

    return ApiTransport.of({ request });
  }),
);

export const apiTransportTestLayer = (
  handler: ApiTransportInterface["request"],
): Layer.Layer<ApiTransport> => Layer.succeed(ApiTransport, ApiTransport.of({ request: handler }));
