import { Type, type Static } from "@sinclair/typebox";
import Ajv, { type ValidateFunction } from "ajv";

const RoleSchema = Type.Union(
  [
    Type.Literal("system"),
    Type.Literal("user"),
    Type.Literal("assistant"),
    Type.Literal("tool")
  ],
  { additionalProperties: false }
);

const MessagePartSchema = Type.Object(
  {
    type: Type.String()
  },
  { additionalProperties: true }
);

export const SkyMessageSchema = Type.Object(
  {
    id: Type.String(),
    role: RoleSchema,
    parts: Type.Array(MessagePartSchema),
    metadata: Type.Optional(Type.Record(Type.String(), Type.Unknown()))
  },
  { additionalProperties: true }
);

export type SkyMessage = Static<typeof SkyMessageSchema>;

export const SkyRunSchema = Type.Object(
  {
    run_id: Type.String(),
    thread_id: Type.String(),
    started_at: Type.Number(),
    completed_at: Type.Union([Type.Number(), Type.Null()]),
    status: Type.String(),
    model_config_id: Type.String(),
    error_code: Type.Union([Type.String(), Type.Null()]),
    schema_version: Type.Number()
  },
  { additionalProperties: false }
);

export type SkyRun = Static<typeof SkyRunSchema>;

export const SkyMemorySchema = Type.Object(
  {
    thread_id: Type.String(),
    summary: Type.Union([Type.String(), Type.Null()]),
    updated_at: Type.Union([Type.Number(), Type.Null()]),
    schema_version: Type.Number()
  },
  { additionalProperties: false }
);

export type SkyMemory = Static<typeof SkyMemorySchema>;

const SkyRunReceiptSchema = Type.Object(
  {
    schema_version: Type.Number(),
    cf_sky_version: Type.String(),
    type: Type.Literal("run"),
    run_id: Type.String(),
    thread_id: Type.String(),
    model_config_id: Type.String(),
    input_hash: Type.Union([Type.String(), Type.Null()]),
    output_hash: Type.Union([Type.String(), Type.Null()]),
    started_at: Type.Number(),
    completed_at: Type.Number(),
    duration_ms: Type.Number(),
    status: Type.String(),
    finish_reason: Type.Union([Type.String(), Type.Null()]),
    error_code: Type.Union([Type.String(), Type.Null()])
  },
  { additionalProperties: false }
);

const SkyToolReceiptSchema = Type.Object(
  {
    schema_version: Type.Number(),
    cf_sky_version: Type.String(),
    type: Type.Literal("tool"),
    run_id: Type.String(),
    thread_id: Type.String(),
    tool_call_id: Type.String(),
    tool_name: Type.String(),
    args_hash: Type.Union([Type.String(), Type.Null()]),
    output_hash: Type.Union([Type.String(), Type.Null()]),
    patch_hash: Type.Optional(Type.Union([Type.String(), Type.Null()])),
    started_at: Type.Number(),
    completed_at: Type.Number(),
    duration_ms: Type.Number(),
    status: Type.Union([Type.Literal("success"), Type.Literal("error")]),
    error_code: Type.Union([Type.String(), Type.Null()])
  },
  { additionalProperties: false }
);

export const SkyReceiptSchema = Type.Union([
  SkyRunReceiptSchema,
  SkyToolReceiptSchema
]);

export type SkyReceipt = Static<typeof SkyReceiptSchema>;

const RunStartedSchema = Type.Object(
  {
    thread_id: Type.String(),
    model_config_id: Type.String(),
    started_at: Type.Number(),
    schema_version: Type.Number()
  },
  { additionalProperties: false }
);

const ModelDeltaSchema = Type.Object(
  {
    kind: Type.Union([Type.Literal("text-delta"), Type.Literal("reasoning-delta")]),
    delta: Type.String()
  },
  { additionalProperties: false }
);

const ModelCompletedSchema = Type.Object(
  {
    finish_reason: Type.Union([Type.String(), Type.Null()]),
    text_length: Type.Number(),
    fallback: Type.Optional(Type.Boolean())
  },
  { additionalProperties: false }
);

const RunErrorSchema = Type.Object(
  {
    error: Type.String()
  },
  { additionalProperties: false }
);

const RunCompletedSchema = Type.Object(
  {
    status: Type.String(),
    finish_reason: Type.Union([Type.String(), Type.Null()]),
    duration_ms: Type.Number()
  },
  { additionalProperties: false }
);

const ToolCallStartedSchema = Type.Object(
  {
    tool_call_id: Type.String(),
    tool_name: Type.String()
  },
  { additionalProperties: false }
);

const ToolArgsDeltaSchema = Type.Object(
  {
    tool_call_id: Type.String(),
    tool_name: Type.String(),
    delta: Type.String(),
    format: Type.Literal("json")
  },
  { additionalProperties: false }
);

const ToolArgsCompletedSchema = Type.Object(
  {
    tool_call_id: Type.String(),
    tool_name: Type.String(),
    args: Type.Unknown()
  },
  { additionalProperties: false }
);

const ToolCallCompletedSchema = Type.Object(
  {
    tool_call_id: Type.String(),
    tool_name: Type.String(),
    status: Type.Union([Type.Literal("success"), Type.Literal("error")]),
    duration_ms: Type.Number()
  },
  { additionalProperties: false }
);

const ToolResultSchema = Type.Object(
  {
    tool_call_id: Type.String(),
    tool_name: Type.String(),
    status: Type.Union([Type.Literal("success"), Type.Literal("error")]),
    output_hash: Type.Union([Type.String(), Type.Null()])
  },
  { additionalProperties: false }
);

export const SKY_EVENT_PAYLOAD_SCHEMAS = {
  "run.started": RunStartedSchema,
  "model.delta": ModelDeltaSchema,
  "model.completed": ModelCompletedSchema,
  "run.error": RunErrorSchema,
  "run.completed": RunCompletedSchema,
  "tool.call.started": ToolCallStartedSchema,
  "tool.call.args.delta": ToolArgsDeltaSchema,
  "tool.call.args.completed": ToolArgsCompletedSchema,
  "tool.call.completed": ToolCallCompletedSchema,
  "tool.result": ToolResultSchema
} as const;

export type SkyEventType = keyof typeof SKY_EVENT_PAYLOAD_SCHEMAS;

export type SkyEventPayload = {
  [Key in SkyEventType]: Static<(typeof SKY_EVENT_PAYLOAD_SCHEMAS)[Key]>;
}[SkyEventType];

export type SkyEventEnvelope = {
  run_id: string;
  event_id: number;
  type: SkyEventType;
  payload: SkyEventPayload;
  created_at: number;
  schema_version: number;
};

export type SkyValidators = {
  validateMessage: ValidateFunction<SkyMessage>;
  validateRun: ValidateFunction<SkyRun>;
  validateMemory: ValidateFunction<SkyMemory>;
  validateReceipt: ValidateFunction<SkyReceipt>;
  validateEventPayload: (type: SkyEventType, payload: unknown) => boolean;
  getEventPayloadErrors: () => string[];
};

export const createSkyValidators = (): SkyValidators => {
  const ajv = new Ajv({
    allErrors: true,
    strict: false,
    allowUnionTypes: true
  });

  const validateMessage = ajv.compile(SkyMessageSchema);
  const validateRun = ajv.compile(SkyRunSchema);
  const validateMemory = ajv.compile(SkyMemorySchema);
  const validateReceipt = ajv.compile(SkyReceiptSchema);

  const eventValidators = new Map<
    SkyEventType,
    ValidateFunction<SkyEventPayload>
  >();

  for (const [type, schema] of Object.entries(SKY_EVENT_PAYLOAD_SCHEMAS)) {
    eventValidators.set(type as SkyEventType, ajv.compile(schema));
  }

  let lastEventErrors: string[] = [];

  const validateEventPayload = (type: SkyEventType, payload: unknown) => {
    const validator = eventValidators.get(type);
    if (!validator) {
      lastEventErrors = [`No validator for event type ${type}`];
      return false;
    }

    const ok = validator(payload);
    lastEventErrors = ok
      ? []
      : (validator.errors ?? []).map((error) => error.message ?? "invalid");
    return Boolean(ok);
  };

  return {
    validateMessage,
    validateRun,
    validateMemory,
    validateReceipt,
    validateEventPayload,
    getEventPayloadErrors: () => lastEventErrors
  };
};
