import { Context, Effect, Schema } from "effect";

export type LmRole = "system" | "user" | "assistant";

export type LmMessage = {
  readonly role: LmRole;
  readonly content: string;
};

export type LmRequest = {
  readonly messages: ReadonlyArray<LmMessage>;
  readonly modelId?: string;
  readonly temperature?: number;
  readonly topP?: number;
  readonly maxTokens?: number;
};

export type LmUsage = {
  readonly promptTokens?: number;
  readonly completionTokens?: number;
  readonly totalTokens?: number;
};

export type LmResponse = {
  readonly text: string;
  readonly usage?: LmUsage;
};

export class LmClientError extends Schema.TaggedError<LmClientError>()(
  "LmClientError",
  {
    message: Schema.String,
    cause: Schema.optional(Schema.Defect)
  }
) {}

export type LmClient = {
  readonly complete: (
    request: LmRequest
  ) => Effect.Effect<LmResponse, LmClientError>;
};

export class LmClientService extends Context.Tag("@openagentsinc/dse/LmClient")<
  LmClientService,
  LmClient
>() {}

