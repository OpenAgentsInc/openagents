import { Schema as S } from "effect";

import { ExampleId, SignatureId } from "./refs.js";

/**
 * The typed signature and its serializable Prompt IR.
 *
 * A `DseSignature<I, O>` is the runtime object: it carries the Effect Schema
 * input and output contracts used to decode a model result, its default Prompt
 * IR, and a language-neutral `SignatureContract` export for outside inspection.
 * The Effect schemas never serialize; the `SignatureContract` does, and the
 * generated signature catalog is derived from it.
 */

export const PROMPT_IR_SCHEMA_LITERAL = "openagents.dse.prompt_ir.v1" as const;
export const SIGNATURE_CONTRACT_SCHEMA_LITERAL = "openagents.dse.signature_contract.v1" as const;

/**
 * Structured Prompt IR. The stable compiled blocks are the system frame, the
 * instruction, the ordered few-shot example identity, the tool policy, and the
 * output format. The per-call context is injected at render time and is never
 * part of the compiled artifact.
 */
export const PromptIr = S.Struct({
  schema: S.Literal(PROMPT_IR_SCHEMA_LITERAL),
  system: S.String.check(S.isMaxLength(8000)),
  instruction: S.String.check(S.isMinLength(1), S.isMaxLength(8000)),
  fewShotExampleIds: S.Array(ExampleId).check(S.isMaxLength(32)),
  toolPolicy: S.String.check(S.isMaxLength(4000)),
  outputFormat: S.String.check(S.isMaxLength(4000)),
});
export type PromptIr = typeof PromptIr.Type;

/** A language-neutral field descriptor for the serializable contract export. */
export const FieldDescriptor = S.Struct({
  name: S.String.check(S.isMinLength(1), S.isMaxLength(128)),
  type: S.Literals(["string", "number", "boolean", "string_array", "enum", "json"]),
  required: S.Boolean,
  description: S.String.check(S.isMaxLength(1000)),
});
export type FieldDescriptor = typeof FieldDescriptor.Type;

/**
 * The serializable signature contract. It lets another system inspect a
 * signature without the TypeScript type system, and it is the byte source for
 * the generated signature catalog entry.
 */
export const SignatureContract = S.Struct({
  schema: S.Literal(SIGNATURE_CONTRACT_SCHEMA_LITERAL),
  signatureId: SignatureId,
  title: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  inputFields: S.Array(FieldDescriptor).check(S.isMinLength(1), S.isMaxLength(64)),
  outputFields: S.Array(FieldDescriptor).check(S.isMinLength(1), S.isMaxLength(64)),
  defaultPromptIr: PromptIr,
});
export type SignatureContract = typeof SignatureContract.Type;

/**
 * The runtime signature object. `input` and `output` are Effect Schemas used by
 * `Predict` to decode a model result. `contract` is the serializable export.
 */
export interface DseSignature<I, O> {
  readonly signatureId: SignatureId;
  readonly input: S.Codec<I>;
  readonly output: S.Codec<O>;
  readonly defaultPromptIr: PromptIr;
  readonly contract: SignatureContract;
}

export interface MakeSignatureArgs<I, O> {
  readonly signatureId: SignatureId;
  readonly title: string;
  readonly input: S.Codec<I>;
  readonly output: S.Codec<O>;
  readonly inputFields: ReadonlyArray<FieldDescriptor>;
  readonly outputFields: ReadonlyArray<FieldDescriptor>;
  readonly defaultPromptIr: PromptIr;
}

const decodePromptIr = S.decodeUnknownSync(PromptIr);
const decodeContract = S.decodeUnknownSync(SignatureContract);

/**
 * Construct a signature and derive its serializable contract deterministically.
 * The derivation is total: the same arguments always produce the same contract
 * bytes, which is what makes the generated catalog claim mechanical.
 */
export const makeSignature = <I, O>(args: MakeSignatureArgs<I, O>): DseSignature<I, O> => {
  const defaultPromptIr = decodePromptIr(args.defaultPromptIr);
  const contract = decodeContract({
    schema: SIGNATURE_CONTRACT_SCHEMA_LITERAL,
    signatureId: args.signatureId,
    title: args.title,
    inputFields: args.inputFields,
    outputFields: args.outputFields,
    defaultPromptIr,
  });
  return {
    signatureId: args.signatureId,
    input: args.input,
    output: args.output,
    defaultPromptIr,
    contract,
  };
};
