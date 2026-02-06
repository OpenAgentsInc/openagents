import type { Schema } from "effect";
import { JSONSchema } from "effect";

export type ToolName = string;

/**
 * A typed contract for a tool call (input/output schemas + metadata).
 *
 * This is the tool analogue of a DSPy "Signature": strict IO + durable naming.
 */
export type DseToolContract<I, O = unknown> = {
  readonly name: ToolName;
  readonly description: string;
  readonly usage?: string;
  readonly input: Schema.Schema<I>;
  readonly output?: Schema.Schema<O>;
  readonly inputExamples?: ReadonlyArray<{ readonly input: I }>;
};

export function make<I, O>(options: {
  readonly name: ToolName;
  readonly description: string;
  readonly usage?: string;
  readonly input: Schema.Schema<I>;
  readonly output?: Schema.Schema<O>;
  readonly inputExamples?: ReadonlyArray<{ readonly input: I }>;
}): DseToolContract<I, O> {
  return {
    name: options.name,
    description: options.description,
    ...(options.usage ? { usage: options.usage } : {}),
    input: options.input,
    ...(options.output ? { output: options.output } : {}),
    ...(options.inputExamples ? { inputExamples: options.inputExamples } : {})
  };
}

export function inputJsonSchema<I, O>(
  contract: DseToolContract<I, O>
): JSONSchema.JsonSchema7Root {
  return JSONSchema.make(contract.input);
}

export function outputJsonSchema<I, O>(
  contract: DseToolContract<I, O>
): JSONSchema.JsonSchema7Root | null {
  return contract.output ? JSONSchema.make(contract.output) : null;
}

