import { Buffer } from "node:buffer";

import { Context, Effect, Layer, Redacted, Schema } from "effect";

import { ForgeGitConfiguration, type ForgeGitConfigurationShape } from "./config.js";
import { ForgeGitAuthError, type ForgeGitScope, ForgeGitSession } from "./model.js";

const tokenPrefix = "oa_forge_git_";

const ForgeGitPolicyAuthorizationResponse = Schema.Struct({
  session: ForgeGitSession,
});

export interface ForgeGitAuthShape {
  readonly authenticate: (input: {
    readonly authorization: string | null;
    readonly nowIso: string;
    readonly repositoryRef: string;
    readonly requiredScope: ForgeGitScope;
    readonly tenantRef: string;
  }) => Effect.Effect<ForgeGitSession, ForgeGitAuthError>;
}

export class ForgeGitAuth extends Context.Service<ForgeGitAuth, ForgeGitAuthShape>()(
  "@openagentsinc/forge-git-service/Auth",
) {}

const unauthorized = () =>
  new ForgeGitAuthError({
    code: "forge_git_unauthorized",
    status: 401,
  });

const forbidden = () =>
  new ForgeGitAuthError({
    code: "forge_git_tenant_forbidden",
    status: 403,
  });

const policyForbidden = () =>
  new ForgeGitAuthError({
    code: "forge_git_membership_forbidden",
    status: 403,
  });

export const readForgeGitToken = (authorization: string | null): string | undefined => {
  if (authorization === null) return undefined;
  const [scheme, encoded] = authorization.trim().split(/\s+/, 2);
  if (scheme?.toLowerCase() === "bearer" && encoded?.startsWith(tokenPrefix)) {
    return encoded;
  }
  if (scheme?.toLowerCase() !== "basic" || encoded === undefined) {
    return undefined;
  }
  try {
    const decoded = Buffer.from(encoded, "base64").toString("utf8");
    const separator = decoded.indexOf(":");
    const username = separator === -1 ? decoded : decoded.slice(0, separator);
    const password = separator === -1 ? "" : decoded.slice(separator + 1);
    if (password.startsWith(tokenPrefix)) return password;
    if (username.startsWith(tokenPrefix)) return username;
    return undefined;
  } catch {
    return undefined;
  }
};

export type ForgeGitPolicyFetch = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>;

const makePolicyAuthorityAuth = (
  configuration: ForgeGitConfigurationShape,
  policyFetch: ForgeGitPolicyFetch,
): ForgeGitAuthShape =>
  ForgeGitAuth.of({
    authenticate: Effect.fn("ForgeGitAuth.authenticate")(function* (input) {
      const token = readForgeGitToken(input.authorization);
      if (token === undefined) return yield* unauthorized();

      const response = yield* Effect.tryPromise({
        try: () =>
          policyFetch(configuration.policyAuthorityUrl, {
            body: JSON.stringify({
              repositoryRef: input.repositoryRef,
              requiredScope: input.requiredScope,
              tenantRef: input.tenantRef,
              transportAuthorization: `Bearer ${token}`,
            }),
            headers: {
              authorization: `Bearer ${Redacted.value(configuration.policyAuthorityToken)}`,
              "content-type": "application/json",
            },
            method: "POST",
          }),
        catch: unauthorized,
      });
      if (response.status === 403) {
        return yield* policyForbidden();
      }
      if (!response.ok) {
        return yield* unauthorized();
      }

      const body = yield* Effect.tryPromise({
        try: () => response.json(),
        catch: unauthorized,
      });
      const decoded = yield* Schema.decodeUnknownEffect(ForgeGitPolicyAuthorizationResponse)(
        body,
      ).pipe(Effect.mapError(unauthorized));
      if (
        decoded.session.tenantRef !== input.tenantRef ||
        decoded.session.repositoryRef !== input.repositoryRef
      ) {
        return yield* forbidden();
      }
      return decoded.session;
    }),
  });

export const makePolicyAuthorityAuthLayer = (
  configuration: ForgeGitConfigurationShape,
  policyFetch: ForgeGitPolicyFetch = fetch,
): Layer.Layer<ForgeGitAuth> =>
  Layer.succeed(ForgeGitAuth, makePolicyAuthorityAuth(configuration, policyFetch));

export const layerAuth = Layer.effect(
  ForgeGitAuth,
  Effect.gen(function* () {
    const configuration = yield* ForgeGitConfiguration;
    return makePolicyAuthorityAuth(configuration, fetch);
  }),
);

export const makeStaticAuthLayer = (
  expectedToken: string,
  session: ForgeGitSession,
  allowedScopes: ReadonlyArray<ForgeGitScope> = ["git:receive-pack", "git:upload-pack"],
): Layer.Layer<ForgeGitAuth> =>
  Layer.succeed(
    ForgeGitAuth,
    ForgeGitAuth.of({
      authenticate: Effect.fn("ForgeGitAuth.test.authenticate")(function* (input) {
        const token = readForgeGitToken(input.authorization);
        if (token !== expectedToken) return yield* unauthorized();
        if (input.tenantRef !== session.tenantRef) {
          return yield* forbidden();
        }
        if (input.repositoryRef !== session.repositoryRef) {
          return yield* unauthorized();
        }
        if (!allowedScopes.includes(input.requiredScope)) {
          return yield* unauthorized();
        }
        return session;
      }),
    }),
  );
