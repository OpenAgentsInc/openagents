import type { Schema } from "effect";

import type { DseParams } from "./params.js";
import { emptyParamsV1 } from "./params.js";
import type { PromptIR } from "./promptIr.js";

export type SignatureId = string;

export type SignatureConstraints = {
  readonly timeoutMs?: number;
  readonly maxTokens?: number;
  readonly allowTools?: boolean;
};

export type DseSignature<I, O> = {
  readonly id: SignatureId;
  readonly input: Schema.Schema<I>;
  readonly output: Schema.Schema<O>;
  readonly prompt: PromptIR<I, O>;
  readonly defaults: {
    readonly params: DseParams;
    readonly constraints: SignatureConstraints;
  };
};

export function make<I, O>(options: {
  readonly id: SignatureId;
  readonly input: Schema.Schema<I>;
  readonly output: Schema.Schema<O>;
  readonly prompt: PromptIR<I, O>;
  readonly defaults?: Partial<{
    readonly params: DseParams;
    readonly constraints: SignatureConstraints;
  }>;
}): DseSignature<I, O> {
  return {
    id: options.id,
    input: options.input,
    output: options.output,
    prompt: options.prompt,
    defaults: {
      params: options.defaults?.params ?? emptyParamsV1,
      constraints: options.defaults?.constraints ?? {}
    }
  };
}

