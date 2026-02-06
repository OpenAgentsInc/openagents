import { JSONSchema } from "effect";

import type { DseParams } from "./params.js";
import type { PromptIR } from "./promptIr.js";
import type { DseSignature, SignatureConstraints } from "./signature.js";

export type SignatureContractExportV1 = {
  readonly format: "openagents.dse.signature_contract";
  readonly formatVersion: 1;

  readonly signatureId: string;

  readonly inputSchemaJson: unknown;
  readonly outputSchemaJson: unknown;

  readonly promptIr: PromptIR<unknown, unknown>;

  readonly defaultParams: DseParams;
  readonly defaultConstraints: SignatureConstraints;
};

export function exportContractV1<I, O>(
  signature: DseSignature<I, O>
): SignatureContractExportV1 {
  return {
    format: "openagents.dse.signature_contract",
    formatVersion: 1,

    signatureId: signature.id,

    inputSchemaJson: JSONSchema.make(signature.input),
    outputSchemaJson: JSONSchema.make(signature.output),

    // Prompt IR is already a JSON-serializable AST by construction.
    promptIr: signature.prompt as unknown as PromptIR<unknown, unknown>,

    defaultParams: signature.defaults.params,
    defaultConstraints: signature.defaults.constraints
  };
}

