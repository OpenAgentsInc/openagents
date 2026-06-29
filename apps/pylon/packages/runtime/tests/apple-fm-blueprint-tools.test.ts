import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  loadBlueprintSignatureRegistry,
  lookupBlueprintSignatures,
  planProbeToolMenu,
  projectProbeToolMenuToAppleFm,
  type ProbeToolMenu,
} from "../src";

async function readFileMenu(): Promise<ProbeToolMenu> {
  const registryView = await Effect.runPromise(loadBlueprintSignatureRegistry({ sourceKind: "staticFixture" }));
  const lookup = await Effect.runPromise(
    lookupBlueprintSignatures({
      backendCapabilityRefs: ["probe.backend.apple_fm_bridge", "probe.blueprint.tool_menu"],
      lookupId: "lookup.apple_projection.test",
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

  return Effect.runPromise(
    planProbeToolMenu({
      backendKind: "apple_fm_bridge",
      contextPackRefs: ["context_pack.test"],
      deniedToolRefs: [],
      lookup,
      maxToolCount: 1,
      menuId: "menu.apple_projection.test",
      sourceAuthorityRefs: ["source_authority.test"],
      supportedToolRefs: ["tool.probe.read_file"],
    }),
  );
}

describe("Apple FM Blueprint tool projection", () => {
  test("projects a Blueprint-selected read_file menu into Apple FM tool definitions", async () => {
    const menu = await readFileMenu();
    const projected = await Effect.runPromise(
      projectProbeToolMenuToAppleFm({
        enumHints: {
          "tool.probe.read_file": {
            path: ["README.md"],
          },
        },
        executors: {
          "tool.probe.read_file": (input) => Effect.succeed({ path: input.path, content: "# Probe" }),
        },
        menu,
      }),
    );

    expect(projected.toolDefinitions).toHaveLength(1);
    expect(projected.toolDefinitions[0]?.name).toBe("read_file");
    expect(projected.toolDefinitions[0]?.inputSchema.type).toBe("object");
    expect(projected.toolDefinitions[0]?.inputSchema["x-order"]).toEqual(["path"]);
    expect((projected.toolDefinitions[0]?.inputSchema.properties as any).path.enum).toEqual(["README.md"]);
    expect(projected.projection.lookupId).toBe("lookup.apple_projection.test");
    expect(projected.projection.toolRefs[0]?.toolRef).toBe("tool.probe.read_file");
    expect(projected.projection.toolRefs[0]?.programSignatureId).toBe("program_signature.probe.tool_menu.project.v1");
  });

  test("fails before Apple FM session creation for unsupported Probe tools", async () => {
    const menu = await readFileMenu();
    const unsupportedMenu: ProbeToolMenu = {
      ...menu,
      tools: [
        {
          ...menu.tools[0]!,
          toolName: "record_evidence",
          toolRef: "tool.probe.record_evidence",
        },
      ],
    };

    await expect(
      Effect.runPromise(
        projectProbeToolMenuToAppleFm({
          executors: {
            "tool.probe.record_evidence": () => Effect.succeed({ ok: true }),
          },
          menu: unsupportedMenu,
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "AppleFmBlueprintToolProjectionError",
      toolRef: "tool.probe.record_evidence",
    });
  });

  test("projects Action Submission proposals as approval-required Apple FM tools", async () => {
    const registryView = await Effect.runPromise(loadBlueprintSignatureRegistry({ sourceKind: "staticFixture" }));
    const lookup = await Effect.runPromise(
      lookupBlueprintSignatures({
        backendCapabilityRefs: ["probe.backend.apple_fm_bridge", "probe.blueprint.tool_menu"],
        lookupId: "lookup.apple_projection.action_submission.test",
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
    const menu = await Effect.runPromise(
      planProbeToolMenu({
        backendKind: "apple_fm_bridge",
        contextPackRefs: ["context_pack.test"],
        deniedToolRefs: [],
        lookup,
        menuId: "menu.apple_projection.action_submission.test",
        sourceAuthorityRefs: ["source_authority.test"],
        supportedToolRefs: [
          "tool.probe.read_file",
          "tool.probe.code_search",
          "tool.probe.propose_action_submission",
        ],
      }),
    );

    const projected = await Effect.runPromise(
      projectProbeToolMenuToAppleFm({
        executors: {
          "tool.probe.code_search": () => Effect.succeed({ resultRefs: [] }),
          "tool.probe.propose_action_submission": () => Effect.succeed({ proposalRef: "action_submission.test" }),
          "tool.probe.read_file": () => Effect.succeed({ path: "README.md", contentRef: "artifact.readme" }),
        },
        menu,
      }),
    );
    const proposalTool = projected.toolDefinitions.find((tool) => tool.name === "propose_action_submission");

    expect(proposalTool?.policy).toBe("approval_required");
    expect(proposalTool?.inputSchema.additionalProperties).toBe(false);
    expect(projected.projection.toolRefs.map((tool) => tool.toolRef)).toContain("tool.probe.propose_action_submission");
  });

  test("fails before Apple FM session creation for unbounded object schemas", async () => {
    const menu = await readFileMenu();
    const unsafeMenu: ProbeToolMenu = {
      ...menu,
      tools: [
        {
          ...menu.tools[0]!,
          inputSchema: {
            type: "object",
            properties: {
              path: { type: "string" },
            },
            additionalProperties: true,
          },
        },
      ],
    };

    await expect(
      Effect.runPromise(
        projectProbeToolMenuToAppleFm({
          executors: {
            "tool.probe.read_file": () => Effect.succeed({ ok: true }),
          },
          menu: unsafeMenu,
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "AppleFmBlueprintToolProjectionError",
      toolRef: "tool.probe.read_file",
    });
  });
});
