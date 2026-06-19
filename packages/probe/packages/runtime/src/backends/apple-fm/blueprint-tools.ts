import { Effect, Schema as S } from "effect";
import {
  type ProbeToolDefinition,
  type ProbeToolMenu,
  type ProbeToolName,
  type ProbeToolPolicy,
} from "../../blueprint/tool-menu";
import { type AppleFmToolDefinition, type AppleFmToolName } from "./tools";

export const AppleFmBlueprintProjectedToolRef = S.Struct({
  approvalPolicyRef: S.String,
  inputSchemaRef: S.String,
  outputSchemaRef: S.String,
  policy: S.Literals(["allow", "approval_required", "deny"]),
  programSignatureId: S.String,
  programTypeId: S.String,
  toolName: S.String,
  toolRef: S.String,
});
export type AppleFmBlueprintProjectedToolRef = typeof AppleFmBlueprintProjectedToolRef.Type;

export const AppleFmBlueprintToolProjection = S.Struct({
  lookupId: S.String,
  menuId: S.String,
  programSignatureIds: S.Array(S.String),
  registryVersionRef: S.String,
  toolRefs: S.Array(AppleFmBlueprintProjectedToolRef),
  warnings: S.Array(S.String),
});
export type AppleFmBlueprintToolProjection = typeof AppleFmBlueprintToolProjection.Type;

export class AppleFmBlueprintToolProjectionError extends S.TaggedErrorClass<AppleFmBlueprintToolProjectionError>()(
  "AppleFmBlueprintToolProjectionError",
  {
    menuId: S.String,
    reason: S.String,
    toolRef: S.optional(S.String),
  },
) {}

export type AppleFmToolExecutor = (
  input: Readonly<Record<string, unknown>>,
  tool: ProbeToolDefinition,
) => Effect.Effect<unknown, never>;

export interface ProjectProbeToolMenuToAppleFmInput {
  readonly enumHints?: Readonly<Record<string, Readonly<Record<string, ReadonlyArray<string>>>>>;
  readonly executors: Readonly<Record<string, AppleFmToolExecutor>>;
  readonly menu: ProbeToolMenu;
}

export interface AppleFmProjectedProbeToolMenu {
  readonly projection: AppleFmBlueprintToolProjection;
  readonly toolDefinitions: ReadonlyArray<AppleFmToolDefinition>;
}

export function projectProbeToolMenuToAppleFm(
  input: ProjectProbeToolMenuToAppleFmInput,
): Effect.Effect<AppleFmProjectedProbeToolMenu, AppleFmBlueprintToolProjectionError> {
  return Effect.gen(function* () {
    const toolDefinitions: AppleFmToolDefinition[] = [];
    const toolRefs: AppleFmBlueprintProjectedToolRef[] = [];
    const warnings: string[] = [];

    for (const tool of input.menu.tools) {
      const executor = input.executors[tool.toolRef];

      if (executor === undefined) {
        return yield* failProjection(input.menu.menuId, `missing executor for selected Probe tool ${tool.toolRef}`, tool.toolRef);
      }

      const appleToolName = appleFmToolNameFromProbe(tool.toolName);
      if (appleToolName === undefined) {
        return yield* failProjection(input.menu.menuId, `Probe tool ${tool.toolName} cannot be projected to Apple FM`, tool.toolRef);
      }

      const inputSchema = yield* normalizeAppleFmInputSchema(input.menu.menuId, tool, input.enumHints?.[tool.toolRef]);
      toolDefinitions.push({
        name: appleToolName,
        description: tool.description,
        inputSchema,
        policy: appleFmPolicy(tool.policy),
        execute: (toolInput) => executor(toolInput, tool),
      });
      toolRefs.push({
        approvalPolicyRef: tool.approvalPolicyRef,
        inputSchemaRef: tool.inputSchemaRef,
        outputSchemaRef: tool.outputSchemaRef,
        policy: tool.policy,
        programSignatureId: tool.programSignatureId,
        programTypeId: tool.programTypeId,
        toolName: appleToolName,
        toolRef: tool.toolRef,
      });
    }

    for (const warning of input.menu.warnings) {
      warnings.push(`${warning.kind}${warning.toolRef === undefined ? "" : `:${warning.toolRef}`}`);
    }

    return {
      projection: {
        lookupId: input.menu.lookupId,
        menuId: input.menu.menuId,
        programSignatureIds: input.menu.programSignatureIds,
        registryVersionRef: input.menu.registryVersionRef,
        toolRefs,
        warnings,
      },
      toolDefinitions,
    };
  });
}

function normalizeAppleFmInputSchema(
  menuId: string,
  tool: ProbeToolDefinition,
  enumHints: Readonly<Record<string, ReadonlyArray<string>>> | undefined,
): Effect.Effect<Record<string, unknown>, AppleFmBlueprintToolProjectionError> {
  if (tool.inputSchema.additionalProperties !== false) {
    return failProjection(menuId, "Apple FM tool schema cannot allow arbitrary additional properties", tool.toolRef);
  }

  const schema = {
    ...tool.inputSchema,
    type: "object",
    title: pascalCaseToolName(tool.toolName),
    additionalProperties: false,
  } as Record<string, unknown>;
  const properties = schema.properties;

  if (typeof properties !== "object" || properties === null || Array.isArray(properties)) {
    return failProjection(menuId, "Apple FM tool schema must have object properties", tool.toolRef);
  }

  const orderedProperties = Object.keys(properties);

  for (const [fieldName, values] of Object.entries(enumHints ?? {})) {
    const property = (properties as Record<string, unknown>)[fieldName];

    if (typeof property === "object" && property !== null && !Array.isArray(property)) {
      (property as Record<string, unknown>).enum = [...values];
    }
  }

  return Effect.succeed({
    ...schema,
    properties,
    "x-order": Array.isArray(schema["x-order"]) ? schema["x-order"] : orderedProperties,
  });
}

function appleFmToolNameFromProbe(toolName: ProbeToolName): AppleFmToolName | undefined {
  switch (toolName) {
    case "read_file":
    case "code_search":
    case "propose_action_submission":
      return toolName;
    case "record_evidence":
    case "execute_tassadar_module_step":
    case "show_proof_replay_bundle":
      return undefined;
  }
}

function appleFmPolicy(policy: ProbeToolPolicy): "allow" | "approval_required" | "deny" {
  return policy;
}

function pascalCaseToolName(toolName: string): string {
  return toolName
    .split("_")
    .map((part) => `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`)
    .join("");
}

function failProjection(
  menuId: string,
  reason: string,
  toolRef?: string,
): Effect.Effect<never, AppleFmBlueprintToolProjectionError> {
  return Effect.fail(new AppleFmBlueprintToolProjectionError({ menuId, reason, toolRef }));
}
