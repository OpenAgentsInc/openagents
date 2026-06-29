import { Effect, Schema as S } from "effect";
import {
  BlueprintProgramToolScope,
  BlueprintTassadarModuleStepBinding,
  BlueprintToolAccess,
  ProbeToolMenuPlan,
  type BlueprintProgramToolScope as BlueprintProgramToolScopeType,
  type BlueprintToolAccess as BlueprintToolAccessType,
} from "./contracts";
import { BlueprintSignatureLookupSelection } from "./signature-lookup";

export const ProbeToolName = S.Literals([
  "read_file",
  "code_search",
  "record_evidence",
  "propose_action_submission",
  "execute_tassadar_module_step",
  "show_proof_replay_bundle",
]);
export type ProbeToolName = typeof ProbeToolName.Type;

export const ProbeToolPolicy = S.Literals(["allow", "approval_required", "deny"]);
export type ProbeToolPolicy = typeof ProbeToolPolicy.Type;

export const ProbeToolMenuWarningKind = S.Literals([
  "denied_tool_scope",
  "max_tool_count_reached",
  "unsupported_tool_scope",
]);
export type ProbeToolMenuWarningKind = typeof ProbeToolMenuWarningKind.Type;

export const ProbeToolMenuWarning = S.Struct({
  kind: ProbeToolMenuWarningKind,
  message: S.String,
  toolRef: S.optional(S.String),
});
export type ProbeToolMenuWarning = typeof ProbeToolMenuWarning.Type;

export const ProbeToolDefinition = S.Struct({
  approvalPolicyRef: S.String,
  contextPackRefs: S.Array(S.String),
  description: S.String,
  evidenceRequirementRefs: S.Array(S.String),
  inputSchema: S.Record(S.String, S.Unknown),
  inputSchemaRef: S.String,
  outputSchemaRef: S.String,
  policy: ProbeToolPolicy,
  programSignatureId: S.String,
  programTypeId: S.String,
  receiptRequirementRefs: S.Array(S.String),
  sourceAuthorityRefs: S.Array(S.String),
  tassadarModuleStep: S.optional(BlueprintTassadarModuleStepBinding),
  toolName: ProbeToolName,
  toolRef: S.String,
});
export type ProbeToolDefinition = typeof ProbeToolDefinition.Type;

export const ProbeToolMenu = S.Struct({
  actionSubmissionRequiredForDirectEffects: S.Literal(true),
  backendKind: S.String,
  deniedTools: S.Array(ProbeToolDefinition),
  evidenceRequirementRefs: S.Array(S.String),
  lookupId: S.String,
  menuId: S.String,
  moduleVersionIds: S.Array(S.String),
  policyRef: S.String,
  programSignatureIds: S.Array(S.String),
  programTypeIds: S.Array(S.String),
  receiptRequirementRefs: S.Array(S.String),
  registryVersionRef: S.String,
  safeProjection: S.Boolean,
  sourceKind: S.String,
  tools: S.Array(ProbeToolDefinition),
  warnings: S.Array(ProbeToolMenuWarning),
});
export type ProbeToolMenu = typeof ProbeToolMenu.Type;

export const ProbeToolMenuPlannerInput = S.Struct({
  backendKind: S.String,
  contextPackRefs: S.Array(S.String),
  deniedToolRefs: S.Array(S.String),
  lookup: BlueprintSignatureLookupSelection,
  maxToolCount: S.optional(S.Number),
  menuId: S.String,
  sourceAuthorityRefs: S.Array(S.String),
  supportedToolRefs: S.Array(S.String),
});
export type ProbeToolMenuPlannerInput = typeof ProbeToolMenuPlannerInput.Type;

export class ProbeToolMenuPlannerError extends S.TaggedErrorClass<ProbeToolMenuPlannerError>()(
  "ProbeToolMenuPlannerError",
  {
    menuId: S.String,
    reason: S.String,
  },
) {}

export interface ProbeToolMenuPlanner {
  readonly plan: (input: ProbeToolMenuPlannerInput) => Effect.Effect<ProbeToolMenu, ProbeToolMenuPlannerError>;
}

type ToolCatalogEntry = Readonly<{
  description: string;
  inputSchema: Readonly<Record<string, unknown>>;
  inputSchemaRef: string;
  name: ProbeToolName;
  outputSchemaRef: string;
}>;

const TOOL_CATALOG: Readonly<Record<string, ToolCatalogEntry>> = {
  "tool.probe.code_search": {
    description: "Search repository text and code symbols.",
    inputSchema: {
      type: "object",
      properties: {
        query: { type: "string" },
        path: { type: "string" },
      },
      required: ["query"],
      additionalProperties: false,
    },
    inputSchemaRef: "schema.probe.tool.code_search.input.v1",
    name: "code_search",
    outputSchemaRef: "schema.probe.tool.code_search.output.v1",
  },
  "tool.probe.propose_action_submission": {
    description: "Propose an external write-side effect for Blueprint review.",
    inputSchema: {
      type: "object",
      properties: {
        actionKind: { type: "string" },
        programRunRef: { type: "string" },
        evidenceRef: { type: "string" },
        summaryRef: { type: "string" },
      },
      required: ["actionKind", "programRunRef", "evidenceRef", "summaryRef"],
      additionalProperties: false,
    },
    inputSchemaRef: "schema.probe.tool.propose_action_submission.input.v1",
    name: "propose_action_submission",
    outputSchemaRef: "schema.probe.tool.propose_action_submission.output.v1",
  },
  "tool.probe.read_file": {
    description: "Read an allowed repository file.",
    inputSchema: {
      type: "object",
      properties: {
        path: { type: "string" },
      },
      required: ["path"],
      additionalProperties: false,
    },
    inputSchemaRef: "schema.probe.tool.read_file.input.v1",
    name: "read_file",
    outputSchemaRef: "schema.probe.tool.read_file.output.v1",
  },
  "tool.probe.record_evidence": {
    description: "Record a redacted evidence reference for the current run.",
    inputSchema: {
      type: "object",
      properties: {
        evidenceRef: { type: "string" },
      },
      required: ["evidenceRef"],
      additionalProperties: false,
    },
    inputSchemaRef: "schema.probe.tool.record_evidence.input.v1",
    name: "record_evidence",
    outputSchemaRef: "schema.probe.tool.record_evidence.output.v1",
  },
  "tool.tassadar.module.execute": {
    description: "Execute a bound Tassadar module step and record exact-replay evidence.",
    inputSchema: {
      type: "object",
      properties: {
        stepRef: { type: "string" },
      },
      required: ["stepRef"],
      additionalProperties: false,
    },
    inputSchemaRef: "schema.blueprint.tassadar_module_step.input.v1",
    name: "execute_tassadar_module_step",
    outputSchemaRef: "schema.blueprint.BlueprintTassadarModuleStepEvidence.v1",
  },
  "tool.proof_replay.bundle.show": {
    description: "Load a public-safe proof replay bundle through the Blueprint replay module.",
    inputSchema: {
      type: "object",
      properties: {
        intentRef: { type: "string" },
        replaySlug: { type: "string" },
        targetRef: { type: "string" },
      },
      required: ["replaySlug", "intentRef"],
      additionalProperties: false,
    },
    inputSchemaRef: "schema.blueprint.ShowReplayInput.v1",
    name: "show_proof_replay_bundle",
    outputSchemaRef: "schema.blueprint.BlueprintReplayModuleEvidence.v1",
  },
};

export function makeProbeToolMenuPlanner(): ProbeToolMenuPlanner {
  return {
    plan: planProbeToolMenu,
  };
}

export function planProbeToolMenu(
  input: ProbeToolMenuPlannerInput,
): Effect.Effect<ProbeToolMenu, ProbeToolMenuPlannerError> {
  return Effect.gen(function* () {
    if (input.lookup.programSignatureIds.length === 0 || input.lookup.programTypeIds.length === 0) {
      return yield* failMenu(input.menuId, "lookup result is missing Program Signature or Program Type refs");
    }

    const warnings: ProbeToolMenuWarning[] = [];
    const allowedTools: ProbeToolDefinition[] = [];
    const deniedTools: ProbeToolDefinition[] = [];
    const maxToolCount = input.maxToolCount ?? input.lookup.toolScopes.length;

    for (const scope of input.lookup.toolScopes) {
      const catalogEntry = TOOL_CATALOG[scope.toolRef];

      if (catalogEntry === undefined || !input.supportedToolRefs.includes(scope.toolRef)) {
        warnings.push({
          kind: "unsupported_tool_scope",
          message: "Tool scope is not supported by this backend capability set",
          toolRef: scope.toolRef,
        });
        continue;
      }

      const policy = toolPolicy(scope, input.deniedToolRefs);
      const definition = toolDefinitionFromScope(input, scope, catalogEntry, policy);

      if (policy === "deny") {
        deniedTools.push(definition);
        warnings.push({
          kind: "denied_tool_scope",
          message: "Tool scope was denied by assignment or backend policy",
          toolRef: scope.toolRef,
        });
        continue;
      }

      if (allowedTools.length >= maxToolCount) {
        warnings.push({
          kind: "max_tool_count_reached",
          message: "Tool scope was omitted because the menu reached maxToolCount",
          toolRef: scope.toolRef,
        });
        continue;
      }

      allowedTools.push(definition);
    }

    if (allowedTools.length === 0 && deniedTools.length === 0) {
      return yield* failMenu(input.menuId, "tool menu planner produced no supported tool definitions");
    }

    return {
      actionSubmissionRequiredForDirectEffects: true,
      backendKind: input.backendKind,
      deniedTools,
      evidenceRequirementRefs: input.lookup.evidenceRequirementRefs,
      lookupId: input.lookup.lookupId,
      menuId: input.menuId,
      moduleVersionIds: input.lookup.moduleVersionIds,
      policyRef: input.lookup.policyRef,
      programSignatureIds: input.lookup.programSignatureIds,
      programTypeIds: input.lookup.programTypeIds,
      receiptRequirementRefs: input.lookup.receiptRequirementRefs,
      registryVersionRef: input.lookup.registryVersionRef,
      safeProjection: input.lookup.safeProjection,
      sourceKind: input.lookup.sourceKind,
      tools: allowedTools,
      warnings,
    };
  });
}

export function probeToolMenuPlanFromMenu(menu: ProbeToolMenu): ProbeToolMenuPlan {
  return {
    backendKind: menu.backendKind,
    evidenceFlags: {
      authorityBoundary: "evidence_only",
      directMutationDisabled: true,
      noDeploy: true,
      noEmail: true,
      noSourceMutation: true,
      noSpend: true,
    },
    programSignatureIds: menu.programSignatureIds,
    registryPolicyRef: menu.policyRef,
    releaseGateIds: [],
    safeProjection: menu.safeProjection,
    tools: menu.tools.map((tool) => ({
      access: accessFromPolicy(tool.policy),
      allowedSurfaces: [],
      inputSchemaRef: tool.inputSchemaRef,
      programSignatureId: tool.programSignatureId,
      requiresApproval: tool.policy === "approval_required",
      tassadarModuleStep: tool.tassadarModuleStep,
      toolRef: tool.toolRef,
    })),
  };
}

function toolPolicy(scope: BlueprintProgramToolScopeType, deniedToolRefs: ReadonlyArray<string>): ProbeToolPolicy {
  if (deniedToolRefs.includes(scope.toolRef)) {
    return "deny";
  }

  return scope.requiresApproval || scope.access === "propose_action" ? "approval_required" : "allow";
}

function toolDefinitionFromScope(
  input: ProbeToolMenuPlannerInput,
  scope: BlueprintProgramToolScopeType,
  catalogEntry: ToolCatalogEntry,
  policy: ProbeToolPolicy,
): ProbeToolDefinition {
  return {
    approvalPolicyRef: approvalPolicyRef(scope, policy),
    contextPackRefs: input.contextPackRefs,
    description: catalogEntry.description,
    evidenceRequirementRefs: input.lookup.evidenceRequirementRefs,
    inputSchema: { ...catalogEntry.inputSchema },
    inputSchemaRef: catalogEntry.inputSchemaRef,
    outputSchemaRef: catalogEntry.outputSchemaRef,
    policy,
    programSignatureId: input.lookup.programSignatureIds[0] ?? "program_signature.unknown",
    programTypeId: input.lookup.programTypeIds[0] ?? "program_type.unknown",
    receiptRequirementRefs: input.lookup.receiptRequirementRefs,
    sourceAuthorityRefs: input.sourceAuthorityRefs,
    tassadarModuleStep: scope.tassadarModuleStep,
    toolName: catalogEntry.name,
    toolRef: scope.toolRef,
  };
}

function approvalPolicyRef(scope: BlueprintProgramToolScopeType, policy: ProbeToolPolicy): string {
  return `policy.probe.${scope.toolRef}.${policy}.v1`;
}

function accessFromPolicy(policy: ProbeToolPolicy): BlueprintToolAccessType {
  return policy === "approval_required" ? "propose_action" : "read";
}

function failMenu(menuId: string, reason: string): Effect.Effect<never, ProbeToolMenuPlannerError> {
  return Effect.fail(new ProbeToolMenuPlannerError({ menuId, reason }));
}
