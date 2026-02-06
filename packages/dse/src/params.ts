export type DseParamsV1 = {
  readonly paramsVersion: 1;

  readonly instruction?: {
    readonly text?: string;
  };

  readonly fewShot?: {
    readonly exampleIds: ReadonlyArray<string>;
    readonly k?: number;
  };

  readonly model?: {
    readonly modelId?: string;
    readonly temperature?: number;
    readonly topP?: number;
    readonly maxTokens?: number;
  };

  readonly decode?: {
    readonly mode: "strict_json" | "jsonish";
    readonly maxRepairs?: number;
  };

  readonly tools?: {
    readonly allowedToolNames?: ReadonlyArray<string>;
    readonly maxToolCalls?: number;
    readonly timeoutMsByToolName?: Readonly<Record<string, number>>;
  };
};

export type DseParams = DseParamsV1;

export const emptyParamsV1: DseParamsV1 = { paramsVersion: 1 };

