import { Context, Effect, Layer } from "effect";

import type { CredentialRole } from "../contracts.js";
import { CredentialValidationError } from "../errors.js";

const credentialRoleOrder: ReadonlyArray<CredentialRole> = [
  "gateway_invoice",
  "settlement_read",
  "operator_admin",
];

const envFieldByRole: Record<CredentialRole, string> = {
  gateway_invoice: "OA_LIGHTNING_OPS_CRED_GATEWAY_INVOICE",
  settlement_read: "OA_LIGHTNING_OPS_CRED_SETTLEMENT_READ",
  operator_admin: "OA_LIGHTNING_OPS_CRED_OPERATOR_ADMIN",
};

export type OpsCredentialRoleConfig = Readonly<{
  roles: Readonly<Record<CredentialRole, string>>;
}>;

export class OpsCredentialRoleConfigService extends Context.Tag(
  "@openagents/lightning-ops/OpsCredentialRoleConfigService",
)<OpsCredentialRoleConfigService, OpsCredentialRoleConfig>() {}

const normalizeSecret = (value: string | undefined): string | undefined => {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  return trimmed;
};

const validateRoleSecret = (
  role: CredentialRole,
  secret: string | undefined,
): Effect.Effect<string, CredentialValidationError> =>
  Effect.gen(function* () {
    const field = envFieldByRole[role];
    const normalized = normalizeSecret(secret);

    if (!normalized) {
      return yield* CredentialValidationError.make({
        code: "missing_credential_role",
        role,
        field,
        reason: "missing required role credential",
      });
    }

    if (normalized.length < 16 || /\s/.test(normalized)) {
      return yield* CredentialValidationError.make({
        code: "invalid_credential_role",
        role,
        field,
        reason: "credential format failed validation",
      });
    }

    return normalized;
  });

export const validateCredentialRoleMap = (
  input: Partial<Record<CredentialRole, string | undefined>>,
): Effect.Effect<Readonly<Record<CredentialRole, string>>, CredentialValidationError> =>
  Effect.gen(function* () {
    const roles: Partial<Record<CredentialRole, string>> = {};
    for (const role of credentialRoleOrder) {
      const validated = yield* validateRoleSecret(role, input[role]);
      roles[role] = validated;
    }

    return roles as Readonly<Record<CredentialRole, string>>;
  });

const loadCredentialRoleConfigFromEnv = (): Effect.Effect<
  OpsCredentialRoleConfig,
  CredentialValidationError
> =>
  Effect.gen(function* () {
    const roles = yield* validateCredentialRoleMap({
      gateway_invoice: process.env[envFieldByRole.gateway_invoice],
      settlement_read: process.env[envFieldByRole.settlement_read],
      operator_admin: process.env[envFieldByRole.operator_admin],
    });

    return {
      roles,
    };
  });

export const OpsCredentialRoleConfigLive = Layer.effect(
  OpsCredentialRoleConfigService,
  loadCredentialRoleConfigFromEnv(),
);

export const makeOpsCredentialRoleConfigTestLayer = (
  config: OpsCredentialRoleConfig,
) => Layer.succeed(OpsCredentialRoleConfigService, config);

export const credentialRoleEnvFields = envFieldByRole;
