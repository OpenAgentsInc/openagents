export type PromptIrVersion = 1;

export type PromptIR<I, O> = {
  readonly version: PromptIrVersion;
  readonly blocks: ReadonlyArray<PromptBlock<I, O>>;
};

export type PromptBlock<I, O> =
  | SystemBlock
  | InstructionBlock
  | FewShotBlock<I, O>
  | ToolPolicyBlock
  | OutputFormatBlock
  | ContextBlock;

export type SystemBlock = {
  readonly _tag: "System";
  readonly text: string;
};

export type InstructionBlock = {
  readonly _tag: "Instruction";
  readonly text: string;
};

export type FewShotExample<I, O> = {
  readonly id: string;
  readonly input: I;
  readonly output: O;
};

export type FewShotBlock<I, O> = {
  readonly _tag: "FewShot";
  readonly examples: ReadonlyArray<FewShotExample<I, O>>;
};

export type ToolPolicy = {
  readonly allowedToolNames?: ReadonlyArray<string>;
  readonly maxToolCalls?: number;
  readonly timeoutMsByToolName?: Readonly<Record<string, number>>;
};

export type ToolPolicyBlock = {
  readonly _tag: "ToolPolicy";
  readonly policy: ToolPolicy;
};

export type OutputFormatSpec =
  | { readonly _tag: "JsonOnly" }
  | {
      readonly _tag: "JsonSchema";
      readonly schema: unknown;
      readonly schemaHash?: string;
    };

export type OutputFormatBlock = {
  readonly _tag: "OutputFormat";
  readonly format: OutputFormatSpec;
};

export type ContextEntry = {
  readonly key: string;
  readonly value: unknown;
};

export type ContextBlock = {
  readonly _tag: "Context";
  readonly entries: ReadonlyArray<ContextEntry>;
};

export const system = (text: string): SystemBlock => ({ _tag: "System", text });
export const instruction = (text: string): InstructionBlock => ({
  _tag: "Instruction",
  text
});
export const fewShot = <I, O>(
  examples: ReadonlyArray<FewShotExample<I, O>>
): FewShotBlock<I, O> => ({ _tag: "FewShot", examples });
export const toolPolicy = (policy: ToolPolicy): ToolPolicyBlock => ({
  _tag: "ToolPolicy",
  policy
});
export const outputJsonOnly = (): OutputFormatBlock => ({
  _tag: "OutputFormat",
  format: { _tag: "JsonOnly" }
});
export const outputJsonSchema = (schema: unknown): OutputFormatBlock => ({
  _tag: "OutputFormat",
  format: { _tag: "JsonSchema", schema }
});
export const context = (entries: ReadonlyArray<ContextEntry>): ContextBlock => ({
  _tag: "Context",
  entries
});

