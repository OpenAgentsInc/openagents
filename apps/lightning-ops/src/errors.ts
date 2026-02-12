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
