import { Context, Effect, Layer, Schema } from "effect";

import type { DseParams } from "../params.js";
import type { SignatureId } from "../signature.js";

export type ActivePolicy = {
  readonly compiledId: string;
  readonly params: DseParams;
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
};

export class PolicyRegistryService extends Context.Tag(
  "@openagentsinc/dse/PolicyRegistry"
)<PolicyRegistryService, PolicyRegistry>() {}

export function layerInMemory(initial?: {
  readonly activeBySignatureId?: Readonly<Record<string, ActivePolicy>>;
}): Layer.Layer<PolicyRegistryService> {
  return Layer.sync(PolicyRegistryService, () => {
    const active = new Map<string, ActivePolicy>(
      Object.entries(initial?.activeBySignatureId ?? {})
    );

    const getActive: PolicyRegistry["getActive"] = (signatureId) =>
      Effect.sync(() => active.get(signatureId) ?? null);

    const setActive: PolicyRegistry["setActive"] = (signatureId, policy) =>
      Effect.sync(() => void active.set(signatureId, policy));

    const clearActive: PolicyRegistry["clearActive"] = (signatureId) =>
      Effect.sync(() => void active.delete(signatureId));

    return PolicyRegistryService.of({ getActive, setActive, clearActive });
  });
}

