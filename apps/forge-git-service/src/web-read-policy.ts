import { Buffer } from "node:buffer";
import { timingSafeEqual } from "node:crypto";

import { Context, Effect, Layer, Redacted, Schema } from "effect";

import { ForgeGitConfiguration, type ForgeGitConfigurationShape } from "./config.js";
import { ForgeGitWebReadError } from "./model.js";
import {
  ForgeWebReadPolicyDecision,
  type ForgeWebReadPolicyDecision as ForgeWebReadPolicyDecisionType,
} from "./web-read-model.js";

export interface ForgeWebReadPolicyShape {
  readonly authorize: (input: {
    readonly authorization: string | null;
    readonly cookie: string | null;
    readonly owner: string;
    readonly repo: string;
  }) => Effect.Effect<ForgeWebReadPolicyDecisionType, ForgeGitWebReadError>;
}

export class ForgeWebReadPolicy extends Context.Service<
  ForgeWebReadPolicy,
  ForgeWebReadPolicyShape
>()("@openagentsinc/forge-git-service/WebReadPolicy") {}

const policyError = (
  operation: string,
  code: string,
  status: number,
  cause?: unknown,
): ForgeGitWebReadError =>
  new ForgeGitWebReadError({
    ...(cause === undefined ? {} : { cause }),
    code,
    operation,
    status,
  });

const decodePolicyDecision = Schema.decodeUnknownEffect(ForgeWebReadPolicyDecision);

const bearerToken = (authorization: string | null): string | undefined => {
  if (authorization === null) return undefined;
  const [scheme, token] = authorization.trim().split(/\s+/, 2);
  return scheme?.toLowerCase() === "bearer" && token !== undefined ? token : undefined;
};

const tokenMatches = (presented: string | undefined, expected: string): boolean => {
  if (presented === undefined) return false;
  const presentedBytes = Buffer.from(presented);
  const expectedBytes = Buffer.from(expected);
  return (
    presentedBytes.byteLength === expectedBytes.byteLength &&
    timingSafeEqual(presentedBytes, expectedBytes)
  );
};

export type ForgeWebReadPolicyFetch = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>;

const makePolicy = (
  configuration: ForgeGitConfigurationShape,
  policyFetch: ForgeWebReadPolicyFetch,
): ForgeWebReadPolicyShape =>
  ForgeWebReadPolicy.of({
    authorize: Effect.fn("ForgeWebReadPolicy.authorize")(function* (input) {
      const serviceToken = Redacted.value(configuration.policyAuthorityToken);
      if (!tokenMatches(bearerToken(input.authorization), serviceToken)) {
        return yield* policyError(
          "ForgeWebReadPolicy.authenticateService",
          "forge_web_read_service_unauthorized",
          401,
        );
      }

      const response = yield* Effect.tryPromise({
        try: (signal) =>
          policyFetch(configuration.webReadPolicyUrl, {
            body: JSON.stringify({
              repositoryRef: input.repo,
              tenantRef: input.owner,
            }),
            headers: {
              authorization: `Bearer ${serviceToken}`,
              "content-type": "application/json",
              ...(input.cookie === null ? {} : { cookie: input.cookie }),
            },
            method: "POST",
            signal,
          }),
        catch: (cause) =>
          policyError(
            "ForgeWebReadPolicy.request",
            "forge_web_read_policy_unavailable",
            503,
            cause,
          ),
      });

      if (response.status === 401) {
        return yield* policyError(
          "ForgeWebReadPolicy.authorize",
          "forge_web_read_authentication_required",
          401,
        );
      }
      if (response.status === 403) {
        return yield* policyError(
          "ForgeWebReadPolicy.authorize",
          "forge_web_read_membership_required",
          403,
        );
      }
      if (!response.ok) {
        return yield* policyError(
          "ForgeWebReadPolicy.authorize",
          "forge_web_read_policy_unavailable",
          503,
        );
      }

      const payload = yield* Effect.tryPromise({
        try: () => response.json(),
        catch: (cause) =>
          policyError("ForgeWebReadPolicy.decodeJson", "forge_web_read_policy_invalid", 502, cause),
      });
      return yield* decodePolicyDecision(payload).pipe(
        Effect.mapError((cause) =>
          policyError("ForgeWebReadPolicy.decode", "forge_web_read_policy_invalid", 502, cause),
        ),
      );
    }),
  });

export const makeForgeWebReadPolicyLayer = (
  configuration: ForgeGitConfigurationShape,
  policyFetch: ForgeWebReadPolicyFetch,
): Layer.Layer<ForgeWebReadPolicy> =>
  Layer.succeed(ForgeWebReadPolicy, makePolicy(configuration, policyFetch));

export const makeStaticForgeWebReadPolicyLayer = (
  decision: ForgeWebReadPolicyDecisionType,
): Layer.Layer<ForgeWebReadPolicy> =>
  Layer.succeed(
    ForgeWebReadPolicy,
    ForgeWebReadPolicy.of({
      authorize: Effect.fn("ForgeWebReadPolicy.test.authorize")(() => Effect.succeed(decision)),
    }),
  );

export const layerForgeWebReadPolicy = Layer.effect(
  ForgeWebReadPolicy,
  Effect.gen(function* () {
    const configuration = yield* ForgeGitConfiguration;
    return makePolicy(configuration, fetch);
  }),
);
