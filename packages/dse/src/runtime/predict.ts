import { Effect } from "effect";

import type { DseSignature } from "../signature.js";
import type { DseParams } from "../params.js";

import { decodeJsonOutput, type OutputDecodeError } from "./decode.js";
import { LmClientError, LmClientService } from "./lm.js";
import { PolicyRegistryError, PolicyRegistryService } from "./policyRegistry.js";
import { PromptRenderError, renderPromptMessages } from "./render.js";

export type PredictEnv = LmClientService | PolicyRegistryService;

export type PredictError =
  | LmClientError
  | PolicyRegistryError
  | PromptRenderError
  | OutputDecodeError;

function resolveEffectiveParams<I, O>(
  signature: DseSignature<I, O>,
  active: { readonly params: DseParams } | null
): DseParams {
  return active?.params ?? signature.defaults.params;
}

export function make<I, O>(signature: DseSignature<I, O>) {
  return Effect.fn(`dse.Predict(${signature.id})`)(function* (input: I) {
    const lm = yield* LmClientService;
    const registry = yield* PolicyRegistryService;

    const active = yield* registry.getActive(signature.id);
    const params = resolveEffectiveParams(signature, active);

    const messages = yield* renderPromptMessages({
      signature,
      input,
      params
    });

    const response = yield* lm.complete({
      messages,
      modelId: params.model?.modelId,
      temperature: params.model?.temperature,
      topP: params.model?.topP,
      maxTokens: params.model?.maxTokens ?? signature.defaults.constraints.maxTokens
    });

    return yield* decodeJsonOutput(signature.output, response.text);
  });
}

