import { Context, Effect, Layer, Schema } from "effect";

import type { DseCompiledArtifactV1 } from "../compiledArtifact.js";
import type { SignatureId } from "../signature.js";

export type ActivePolicy = {
  readonly compiledId: string;
};

export class PolicyRegistryError extends Schema.TaggedError<PolicyRegistryError>()(
  "PolicyRegistryError",
  {
    message: Schema.String,
    cause: Schema.optional(Schema.Defect)
  }
) {}

export type PolicyRegistry = {
  readonly getActive: (
    signatureId: SignatureId
  ) => Effect.Effect<ActivePolicy | null, PolicyRegistryError>;
  readonly setActive: (
    signatureId: SignatureId,
    policy: ActivePolicy
  ) => Effect.Effect<void, PolicyRegistryError>;
  readonly clearActive: (
    signatureId: SignatureId
  ) => Effect.Effect<void, PolicyRegistryError>;
  readonly getArtifact: (
    signatureId: SignatureId,
    compiledId: string
  ) => Effect.Effect<DseCompiledArtifactV1 | null, PolicyRegistryError>;
  readonly putArtifact: (
    artifact: DseCompiledArtifactV1
  ) => Effect.Effect<void, PolicyRegistryError>;
};

export class PolicyRegistryService extends Context.Tag(
  "@openagentsinc/dse/PolicyRegistry"
)<PolicyRegistryService, PolicyRegistry>() {}

export function layerInMemory(initial?: {
  readonly activeBySignatureId?: Readonly<Record<string, string>>;
  readonly artifacts?: ReadonlyArray<DseCompiledArtifactV1>;
}): Layer.Layer<PolicyRegistryService> {
  return Layer.sync(PolicyRegistryService, () => {
    const active = new Map<string, ActivePolicy>(
      Object.entries(initial?.activeBySignatureId ?? {}).map(([k, v]) => [
        k,
        { compiledId: v }
      ])
    );

    const getActive: PolicyRegistry["getActive"] = (signatureId) =>
      Effect.sync(() => active.get(signatureId) ?? null);

    const setActive: PolicyRegistry["setActive"] = (signatureId, policy) =>
      Effect.sync(() => void active.set(signatureId, policy));

    const clearActive: PolicyRegistry["clearActive"] = (signatureId) =>
      Effect.sync(() => void active.delete(signatureId));

    const artifacts = new Map<string, DseCompiledArtifactV1>();
    for (const artifact of initial?.artifacts ?? []) {
      artifacts.set(`${artifact.signatureId}::${artifact.compiled_id}`, artifact);
    }

    const getArtifact: PolicyRegistry["getArtifact"] = (signatureId, compiledId) =>
      Effect.sync(() => artifacts.get(`${signatureId}::${compiledId}`) ?? null);

    const putArtifact: PolicyRegistry["putArtifact"] = (artifact) =>
      Effect.sync(
        () => void artifacts.set(`${artifact.signatureId}::${artifact.compiled_id}`, artifact)
      );

    return PolicyRegistryService.of({
      getActive,
      setActive,
      clearActive,
      getArtifact,
      putArtifact
    });
  });
}
