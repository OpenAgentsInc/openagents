import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  loadBlueprintSignatureRegistry,
  lookupBlueprintSignatures,
  makeProbeToolMenuPlanner,
  planProbeToolMenu,
} from "../src";

async function lookupToolMenuSignature() {
  const registryView = await Effect.runPromise(loadBlueprintSignatureRegistry({ sourceKind: "staticFixture" }));
  return Effect.runPromise(
    lookupBlueprintSignatures({
      backendCapabilityRefs: ["probe.backend.apple_fm_bridge", "probe.blueprint.tool_menu"],
      lookupId: "lookup.tool_menu.test",
      registryView,
      request: {
        actorRef: "actor.probe.test",
        allowedSurfaces: ["agent_api"],
        backendKind: "apple_fm_bridge",
        contextPackRef: "context_pack.test",
        programSignatureIds: ["program_signature.probe.tool_menu.project.v1"],
        riskCeiling: "medium",
      },
    }),
  );
}

describe("Probe tool menu planner", () => {
  test("plans read_file and code_search tools when allowed by lookup and capability", async () => {
    const lookup = await lookupToolMenuSignature();
    const menu = await Effect.runPromise(
      planProbeToolMenu({
        backendKind: "apple_fm_bridge",
        contextPackRefs: ["context_pack.test"],
        deniedToolRefs: [],
        lookup,
        menuId: "menu.test.allowed",
        sourceAuthorityRefs: ["source_authority.test"],
        supportedToolRefs: ["tool.probe.read_file", "tool.probe.code_search", "tool.probe.record_evidence"],
      }),
    );

    expect(menu.tools.map((tool) => tool.toolName)).toContain("read_file");
    expect(menu.tools.map((tool) => tool.toolName)).toContain("code_search");
    expect(menu.tools[0]?.programSignatureId).toBe("program_signature.probe.tool_menu.project.v1");
    expect(menu.tools[0]?.programTypeId).toBe("program_type.probe.tool_menu.project");
    expect(menu.tools[0]?.contextPackRefs).toEqual(["context_pack.test"]);
    expect(menu.tools[0]?.sourceAuthorityRefs).toEqual(["source_authority.test"]);
    expect(menu.receiptRequirementRefs).toContain("receipt.program_run");
  });

  test("marks propose_action scopes as approval_required", async () => {
    const lookup = await lookupToolMenuSignature();
    const planner = makeProbeToolMenuPlanner();
    const menu = await Effect.runPromise(
      planner.plan({
        backendKind: "apple_fm_bridge",
        contextPackRefs: ["context_pack.test"],
        deniedToolRefs: [],
        lookup,
        menuId: "menu.test.approval",
        sourceAuthorityRefs: ["source_authority.test"],
        supportedToolRefs: [
          "tool.probe.read_file",
          "tool.probe.code_search",
          "tool.probe.record_evidence",
          "tool.probe.propose_action_submission",
        ],
      }),
    );
    const proposalTool = menu.tools.find((tool) => tool.toolRef === "tool.probe.propose_action_submission");

    expect(proposalTool?.policy).toBe("approval_required");
    expect(proposalTool?.approvalPolicyRef).toContain("approval_required");
  });

  test("moves denied scopes into deniedTools with a warning", async () => {
    const lookup = await lookupToolMenuSignature();
    const menu = await Effect.runPromise(
      planProbeToolMenu({
        backendKind: "apple_fm_bridge",
        contextPackRefs: ["context_pack.test"],
        deniedToolRefs: ["tool.probe.read_file"],
        lookup,
        menuId: "menu.test.denied",
        sourceAuthorityRefs: ["source_authority.test"],
        supportedToolRefs: ["tool.probe.read_file", "tool.probe.code_search", "tool.probe.record_evidence"],
      }),
    );

    expect(menu.tools.map((tool) => tool.toolRef)).not.toContain("tool.probe.read_file");
    expect(menu.deniedTools.map((tool) => tool.toolRef)).toContain("tool.probe.read_file");
    expect(menu.warnings.map((warning) => warning.kind)).toContain("denied_tool_scope");
  });

  test("omits unsupported backend capability scopes with structured warnings", async () => {
    const lookup = await lookupToolMenuSignature();
    const menu = await Effect.runPromise(
      planProbeToolMenu({
        backendKind: "apple_fm_bridge",
        contextPackRefs: ["context_pack.test"],
        deniedToolRefs: [],
        lookup,
        menuId: "menu.test.unsupported",
        sourceAuthorityRefs: ["source_authority.test"],
        supportedToolRefs: ["tool.probe.read_file"],
      }),
    );

    expect(menu.tools.map((tool) => tool.toolRef)).toEqual(["tool.probe.read_file"]);
    expect(menu.warnings.map((warning) => warning.kind)).toContain("unsupported_tool_scope");
    expect(menu.warnings.map((warning) => warning.toolRef)).toContain("tool.probe.code_search");
  });

  test("enforces maxToolCount without widening context or source authority", async () => {
    const lookup = await lookupToolMenuSignature();
    const menu = await Effect.runPromise(
      planProbeToolMenu({
        backendKind: "apple_fm_bridge",
        contextPackRefs: ["context_pack.test"],
        deniedToolRefs: [],
        lookup,
        maxToolCount: 1,
        menuId: "menu.test.max",
        sourceAuthorityRefs: ["source_authority.test"],
        supportedToolRefs: ["tool.probe.read_file", "tool.probe.code_search", "tool.probe.record_evidence"],
      }),
    );

    expect(menu.tools).toHaveLength(1);
    expect(menu.tools[0]?.contextPackRefs).toEqual(["context_pack.test"]);
    expect(menu.tools[0]?.sourceAuthorityRefs).toEqual(["source_authority.test"]);
    expect(menu.warnings.map((warning) => warning.kind)).toContain("max_tool_count_reached");
  });
});
