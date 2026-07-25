import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";

import { Context, Effect, Layer, Schema } from "effect";

import { ForgeGitDatabase } from "./database.js";
import { ForgeGitAuthError, type ForgeGitScope, ForgeGitSession } from "./model.js";

const tokenPrefix = "oa_forge_git_";

const ForgeGitAuthRow = Schema.Struct({
  expires_at: Schema.String,
  ref_restrictions_json: Schema.String,
  repository_ref: Schema.String,
  subject_ref: Schema.String,
  tenant_ref: Schema.String,
  token_ref: Schema.String,
});

const refRestrictionsSchema = Schema.Array(Schema.String);

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

const decodeRestrictions = (
  value: string,
): Effect.Effect<ReadonlyArray<string>, ForgeGitAuthError> =>
  Effect.try({
    try: () => JSON.parse(value) as unknown,
    catch: unauthorized,
  }).pipe(
    Effect.flatMap(Schema.decodeUnknownEffect(refRestrictionsSchema)),
    Effect.mapError(unauthorized),
  );

export const layerAuth = Layer.effect(
  ForgeGitAuth,
  Effect.gen(function* () {
    const database = yield* ForgeGitDatabase;

    const authenticate = Effect.fn("ForgeGitAuth.authenticate")(function* (
      input: Parameters<ForgeGitAuthShape["authenticate"]>[0],
    ) {
      const token = readForgeGitToken(input.authorization);
      if (token === undefined) return yield* unauthorized();

      const tokenHash = createHash("sha256").update(token).digest("hex");
      const rows = yield* Effect.tryPromise({
        try: () =>
          database.sql`
            SELECT
              tokens.expires_at,
              tokens.ref_restrictions_json,
              tokens.repository_ref,
              tokens.subject_ref,
              tokens.tenant_ref,
              tokens.token_ref
            FROM forge_git_access_tokens tokens
            INNER JOIN forge_tenants tenants
              ON tenants.tenant_ref = tokens.tenant_ref
            WHERE tokens.token_hash = ${tokenHash}
              AND tokens.repository_ref = ${input.repositoryRef}
              AND tokens.state = 'active'
              AND tenants.state = 'active'
              AND EXISTS (
                SELECT 1
                FROM forge_git_access_token_scopes scopes
                WHERE scopes.tenant_ref = tokens.tenant_ref
                  AND scopes.token_ref = tokens.token_ref
                  AND scopes.scope IN (${input.requiredScope}, 'git:admin')
              )
            LIMIT 1
          `,
        catch: unauthorized,
      });
      const raw = rows[0];
      if (raw === undefined) return yield* unauthorized();
      const row = yield* Schema.decodeUnknownEffect(ForgeGitAuthRow)(raw).pipe(
        Effect.mapError(unauthorized),
      );
      if (row.tenant_ref !== input.tenantRef) {
        return yield* forbidden();
      }
      if (Date.parse(row.expires_at) <= Date.parse(input.nowIso)) {
        yield* Effect.tryPromise({
          try: () =>
            database.sql`
              UPDATE forge_git_access_tokens
              SET state = 'expired'
              WHERE tenant_ref = ${row.tenant_ref}
                AND token_ref = ${row.token_ref}
                AND state = 'active'
            `,
          catch: unauthorized,
        });
        return yield* unauthorized();
      }

      const refRestrictions = yield* decodeRestrictions(row.ref_restrictions_json);
      yield* Effect.tryPromise({
        try: () =>
          database.sql`
            UPDATE forge_git_access_tokens
            SET last_used_at = ${input.nowIso}
            WHERE tenant_ref = ${row.tenant_ref}
              AND token_ref = ${row.token_ref}
          `,
        catch: unauthorized,
      });

      return ForgeGitSession.make({
        authenticatedAt: input.nowIso,
        refRestrictions,
        repositoryRef: row.repository_ref,
        subjectRef: row.subject_ref,
        tenantRef: row.tenant_ref,
        tokenRef: row.token_ref,
      });
    });

    return ForgeGitAuth.of({ authenticate });
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
