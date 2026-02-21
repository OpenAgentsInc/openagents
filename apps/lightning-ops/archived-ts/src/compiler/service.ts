import { Context, Effect } from "effect";

import type { CompiledApertureArtifact, ControlPlanePaywall } from "../contracts.js";
import type { ApertureCompileValidationError } from "../errors.js";

export type ApertureConfigCompilerApi = Readonly<{
  compile: (
    paywalls: ReadonlyArray<ControlPlanePaywall>,
  ) => Effect.Effect<CompiledApertureArtifact, ApertureCompileValidationError>;
  snapshotHash: (paywalls: ReadonlyArray<ControlPlanePaywall>) => string;
}>;

export class ApertureConfigCompilerService extends Context.Tag(
  "@openagents/lightning-ops/ApertureConfigCompilerService",
)<ApertureConfigCompilerService, ApertureConfigCompilerApi>() {}
