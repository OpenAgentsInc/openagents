import { createHash } from "node:crypto";

import {
  PortableRef,
  type PortableTargetDescriptor,
} from "@openagentsinc/portable-session-contract";
import { Effect, Schema } from "effect";

import type {
  PortableCommandTargetInstallationPortResolution,
  PortableCommandTargetInstallationPortResolver,
} from "./portable-command-broker-factory.js";
import {
  ManagedPortableCapabilityInstallationPort,
  createPostgresManagedPortableCapabilityResourceResolver,
  type ManagedPortableCapabilityInstallationConfig,
} from "./portable-capability-installation-ports.js";
import type { SyncSql } from "./sql.js";

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;

type Fetch = (input: string | URL | Request, init?: RequestInit) => Promise<Response>;

type AuthorityRow = Readonly<{
  target_ref: string;
  target_owner_ref: string;
  target_class: string;
  adapter_ref: string;
  compatibility_ref: string;
  isolation: string;
  data_posture: string;
  health: string;
  claim_ref: string;
  claim_owner_ref: string;
  session_ref: string;
  executor_environment_ref: string;
  destination_target_ref: string;
  claim_state: string;
  terminal_status: string | null;
  lease_expires_at: Date | string;
}>;

const stableFailureRef = (code: string, scopeRef: string): string =>
  `failure.portable-managed-installation-resolver.${createHash("sha256")
    .update(`${code}\u0000${scopeRef}`)
    .digest("hex")}`;

export class PortableManagedCommandInstallationResolverError extends Schema.TaggedErrorClass<PortableManagedCommandInstallationResolverError>()(
  "PortableManagedCommandInstallationResolverError",
  {
    code: Schema.Literals([
      "invalid_configuration",
      "invalid_scope",
      "authority_missing",
      "authority_mismatch",
      "authority_expired",
    ]),
    failureRef: PortableRef,
  },
) {}

export type PostgresManagedPortableCommandInstallationPortResolverConfig = Readonly<{
  sql: SyncSql;
  baseUrl: string;
  bearerToken: string;
  fetch?: Fetch;
  timeoutMs?: number;
  now?: () => string;
}>;

/**
 * Resolves only OpenAgents-managed installation ports. The returned port
 * checks the command claim again when it resolves the staged resource.
 */
export const createPostgresManagedPortableCommandInstallationPortResolver = (
  config: PostgresManagedPortableCommandInstallationPortResolverConfig,
): PortableCommandTargetInstallationPortResolver => {
  if (
    typeof config.sql !== "function" ||
    !config.baseUrl.startsWith("https://") ||
    config.bearerToken.length < 16 ||
    (config.timeoutMs !== undefined &&
      (!Number.isSafeInteger(config.timeoutMs) || config.timeoutMs <= 0))
  ) {
    throw new PortableManagedCommandInstallationResolverError({
      code: "invalid_configuration",
      failureRef: stableFailureRef("invalid_configuration", "configuration"),
    });
  }

  const failure = (
    code: PortableManagedCommandInstallationResolverError["code"],
    scopeRef: string,
  ) =>
    new PortableManagedCommandInstallationResolverError({
      code,
      failureRef: stableFailureRef(code, scopeRef),
    });

  const now = (scopeRef: string): Date => {
    const value = new Date(config.now?.() ?? new Date().toISOString());
    if (!Number.isFinite(value.valueOf())) throw failure("invalid_scope", scopeRef);
    return value;
  };

  const assertAuthority = async (
    input: Readonly<{
      commandExecutionClaimRef: string;
      ownerRef: string;
      sessionRef: string;
      target: PortableTargetDescriptor;
    }>,
  ): Promise<void> => {
    if (
      ![
        input.commandExecutionClaimRef,
        input.ownerRef,
        input.sessionRef,
        input.target.targetRef,
        input.target.adapterRef,
      ].every((ref) => SAFE_REF.test(ref)) ||
      input.target.ownerRef !== input.ownerRef
    ) {
      throw failure("invalid_scope", input.commandExecutionClaimRef);
    }
    const rows: ReadonlyArray<AuthorityRow> = await config.sql`
      SELECT target.target_ref, target.owner_user_id AS target_owner_ref,
             target.target_class, target.adapter_ref, target.compatibility_ref,
             target.isolation, target.data_posture, target.health,
             claim.claim_ref, claim.owner_user_id AS claim_owner_ref,
             claim.session_ref, claim.executor_environment_ref,
             claim.destination_target_ref, claim.state AS claim_state,
             claim.terminal_status, claim.lease_expires_at
      FROM khala_sync_portable_targets AS target
      JOIN khala_sync_portable_command_executions AS claim
        ON claim.owner_user_id = target.owner_user_id
       AND (claim.executor_environment_ref = target.target_ref
         OR claim.destination_target_ref = target.target_ref)
      WHERE target.target_ref = ${input.target.targetRef}
        AND target.owner_user_id = ${input.ownerRef}
        AND claim.claim_ref = ${input.commandExecutionClaimRef}
        AND claim.session_ref = ${input.sessionRef}
    `;
    if (rows.length !== 1) throw failure("authority_missing", input.target.targetRef);
    const row = rows[0]!;
    if (
      row.target_ref !== input.target.targetRef ||
      row.target_owner_ref !== input.ownerRef ||
      row.target_class !== "openagents_managed" ||
      row.target_class !== input.target.targetClass ||
      row.adapter_ref !== input.target.adapterRef ||
      row.compatibility_ref !== input.target.compatibilityRef ||
      row.isolation !== input.target.isolation ||
      row.data_posture !== input.target.dataPosture ||
      row.health !== "ready" ||
      row.health !== input.target.health ||
      row.claim_ref !== input.commandExecutionClaimRef ||
      row.claim_owner_ref !== input.ownerRef ||
      row.session_ref !== input.sessionRef ||
      ![row.executor_environment_ref, row.destination_target_ref].includes(
        input.target.targetRef,
      ) ||
      row.claim_state !== "claimed" ||
      row.terminal_status !== null
    ) {
      throw failure("authority_mismatch", input.target.targetRef);
    }
    const expiresAt = new Date(row.lease_expires_at);
    if (!Number.isFinite(expiresAt.valueOf()) || expiresAt <= now(input.commandExecutionClaimRef)) {
      throw failure("authority_expired", input.commandExecutionClaimRef);
    }
  };

  const resolveEffect = Effect.fn("PostgresManagedPortableCommandInstallationPortResolver.resolve")(
    (input: Parameters<PortableCommandTargetInstallationPortResolver["resolve"]>[0]) =>
      Effect.tryPromise({
        try: async (): Promise<PortableCommandTargetInstallationPortResolution | null> => {
          if (input.target.targetClass !== "openagents_managed") return null;
          await assertAuthority(input);
          const resolveManagedResource = createPostgresManagedPortableCapabilityResourceResolver({
            sql: config.sql,
            ownerRef: input.ownerRef,
            targetRef: input.target.targetRef,
            sessionRef: input.sessionRef,
          });
          const resolveResource: ManagedPortableCapabilityInstallationConfig["resolveResource"] =
            async (binding) => {
              await assertAuthority(input);
              return resolveManagedResource(binding);
            };
          const port = new ManagedPortableCapabilityInstallationPort({
            baseUrl: config.baseUrl,
            bearerToken: config.bearerToken,
            ownerRef: input.ownerRef,
            targetRef: input.target.targetRef,
            sessionRef: input.sessionRef,
            resolveResource,
            ...(config.fetch === undefined ? {} : { fetch: config.fetch }),
            ...(config.timeoutMs === undefined ? {} : { timeoutMs: config.timeoutMs }),
          });
          return {
            targetRef: input.target.targetRef,
            targetClass: "openagents_managed",
            adapterRef: input.target.adapterRef,
            port,
          };
        },
        catch: (cause) =>
          cause instanceof PortableManagedCommandInstallationResolverError
            ? cause
            : failure("authority_missing", input.commandExecutionClaimRef),
      }),
  );

  return { resolve: (input) => Effect.runPromise(resolveEffect(input)) };
};
