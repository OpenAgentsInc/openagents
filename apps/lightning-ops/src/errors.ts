import { Schema } from "effect";

import { CompileDiagnostic } from "./contracts.js";

export class ApertureCompileValidationError extends Schema.TaggedError<ApertureCompileValidationError>()(
  "ApertureCompileValidationError",
  {
    diagnostics: Schema.Array(CompileDiagnostic),
  },
) {}

export class ControlPlaneDecodeError extends Schema.TaggedError<ControlPlaneDecodeError>()(
  "ControlPlaneDecodeError",
  {
    operation: Schema.String,
    reason: Schema.String,
  },
) {}

export class ControlPlaneTransportError extends Schema.TaggedError<ControlPlaneTransportError>()(
  "ControlPlaneTransportError",
  {
    operation: Schema.String,
    reason: Schema.String,
  },
) {}

export class ConfigError extends Schema.TaggedError<ConfigError>()("ConfigError", {
  field: Schema.String,
  message: Schema.String,
}) {}

export class CredentialValidationError extends Schema.TaggedError<CredentialValidationError>()(
  "CredentialValidationError",
  {
    code: Schema.Literal("missing_credential_role", "invalid_credential_role"),
    role: Schema.Literal("gateway_invoice", "settlement_read", "operator_admin"),
    field: Schema.String,
    reason: Schema.String,
  },
) {}

export class GatewayRuntimeError extends Schema.TaggedError<GatewayRuntimeError>()("GatewayRuntimeError", {
  stage: Schema.Literal("active_lookup", "apply", "health", "challenge", "proxy", "rollback"),
  reason: Schema.String,
}) {}
