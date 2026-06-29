import { Schema as S } from "effect";
import { ProbeLlmMessage, makeProbeLlmMessage, type ProbeLlmMessageInput } from "./messages.js";
import { ProbeLlmToolDefinition } from "./tool.js";

export const ProbeLlmGenerationOptions = S.Struct({
  maxTokens: S.optional(S.Number),
  temperature: S.optional(S.Number),
  topP: S.optional(S.Number),
  topK: S.optional(S.Number),
  stop: S.optional(S.Array(S.String)),
});
export type ProbeLlmGenerationOptions = typeof ProbeLlmGenerationOptions.Type;

export const ProbeLlmToolChoice = S.Union([
  S.Struct({ type: S.Literal("auto") }),
  S.Struct({ type: S.Literal("none") }),
  S.Struct({ type: S.Literal("required") }),
  S.Struct({ type: S.Literal("tool"), name: S.String }),
]);
export type ProbeLlmToolChoice = typeof ProbeLlmToolChoice.Type;

export const ProbeLlmModelRef = S.Struct({
  provider: S.String,
  model: S.String,
});
export type ProbeLlmModelRef = typeof ProbeLlmModelRef.Type;

export const ProbeLlmRequest = S.Struct({
  id: S.optional(S.String),
  model: ProbeLlmModelRef,
  system: S.Array(ProbeLlmMessage),
  messages: S.Array(ProbeLlmMessage),
  tools: S.Array(ProbeLlmToolDefinition),
  toolChoice: S.optional(ProbeLlmToolChoice),
  generation: S.optional(ProbeLlmGenerationOptions),
  providerOptions: S.optional(S.Record(S.String, S.Record(S.String, S.Unknown))),
});
export type ProbeLlmRequest = typeof ProbeLlmRequest.Type;

export interface MakeProbeLlmRequestInput {
  readonly id?: string;
  readonly model: ProbeLlmModelRef;
  readonly system?: string | ProbeLlmMessage | ReadonlyArray<ProbeLlmMessage>;
  readonly prompt?: ProbeLlmMessageInput;
  readonly messages?: ReadonlyArray<ProbeLlmMessage>;
  readonly tools?: ReadonlyArray<ProbeLlmToolDefinition>;
  readonly toolChoice?: ProbeLlmToolChoice;
  readonly generation?: ProbeLlmGenerationOptions;
  readonly providerOptions?: ProbeLlmRequest["providerOptions"];
}

export function makeProbeLlmRequest(input: MakeProbeLlmRequestInput): ProbeLlmRequest {
  return {
    id: input.id,
    model: input.model,
    system: normalizeSystem(input.system),
    messages: [...(input.messages ?? []), ...(input.prompt === undefined ? [] : [makeProbeLlmMessage("user", input.prompt)])],
    tools: [...(input.tools ?? [])],
    toolChoice: input.toolChoice,
    generation: input.generation,
    providerOptions: input.providerOptions,
  };
}

function normalizeSystem(input: MakeProbeLlmRequestInput["system"]): ReadonlyArray<ProbeLlmMessage> {
  if (input === undefined) {
    return [];
  }

  if (typeof input === "string") {
    return [makeProbeLlmMessage("system", input)];
  }

  // `Array.isArray` does not narrow a `ReadonlyArray<...>` out of the union, so
  // guard on the readonly array shape explicitly to keep both branches typed.
  if (Array.isArray(input)) {
    return input as ReadonlyArray<ProbeLlmMessage>;
  }

  return [input as ProbeLlmMessage];
}
