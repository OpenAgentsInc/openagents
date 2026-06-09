import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  STATIC_BLUEPRINT_PROGRAM_REGISTRY,
  loadBlueprintSignatureRegistry,
  lookupBlueprintSignatures,
  makeBlueprintSignatureLookupService,
} from "../src";

const view = () => loadBlueprintSignatureRegistry({ sourceKind: "staticFixture" });

describe("Blueprint signature lookup service", () => {
  test("selects exact Program Signature refs when valid", async () => {
    const registryView = await Effect.runPromise(view());
    const selection = await Effect.runPromise(
      lookupBlueprintSignatures({
        backendCapabilityRefs: ["probe.backend.apple_fm_bridge", "probe.blueprint.tool_menu"],
        lookupId: "lookup.test.exact",
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

    expect(selection.lookupId).toBe("lookup.test.exact");
    expect(selection.programSignatureIds).toEqual(["program_signature.probe.tool_menu.project.v1"]);
    expect(selection.programTypeIds).toEqual(["program_type.probe.tool_menu.project"]);
    expect(selection.moduleVersionIds).toContain("module_version.probe.tool_menu.seed.v1");
    expect(selection.releaseGateRefs).toContain("release_gate.probe.tool_menu.seed.v1");
    expect(selection.directMutationAllowed).toBe(false);
    expect(selection.actionSubmissionRequiredForDirectEffects).toBe(true);
    expect(selection.toolScopes.map((scope) => scope.toolRef)).toContain("tool.probe.propose_action_submission");
  });

  test("selects through structured filters without exact refs", async () => {
    const registryView = await Effect.runPromise(view());
    const service = makeBlueprintSignatureLookupService();
    const selection = await Effect.runPromise(
      service.lookup({
        backendCapabilityRefs: ["probe.backend.apple_fm_bridge", "probe.blueprint.signature_lookup"],
        lookupId: "lookup.test.structured",
        maxToolCount: 2,
        registryView,
        request: {
          actorRef: "actor.probe.test",
          allowedSurfaces: ["agent_api"],
          backendKind: "apple_fm_bridge",
          contextPackRef: "context_pack.test",
          preferredFamily: "routing",
          riskCeiling: "low",
        },
      }),
    );

    expect(selection.programSignatureIds).toEqual(["program_signature.probe.signature_lookup.v1"]);
    expect(selection.toolScopes).toHaveLength(2);
    expect(selection.backendCapabilityRefs).toEqual([
      "probe.backend.apple_fm_bridge",
      "probe.blueprint.signature_lookup",
    ]);
  });

  test("refuses missing context pack refs when selected signatures require context", async () => {
    const registryView = await Effect.runPromise(view());

    await expect(
      Effect.runPromise(
        lookupBlueprintSignatures({
          backendCapabilityRefs: ["probe.backend.apple_fm_bridge", "probe.blueprint.signature_lookup"],
          lookupId: "lookup.test.missing_context",
          registryView,
          request: {
            actorRef: "actor.probe.test",
            allowedSurfaces: ["agent_api"],
            backendKind: "apple_fm_bridge",
            preferredFamily: "routing",
            riskCeiling: "low",
          },
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "BlueprintSignatureLookupError",
      lookupId: "lookup.test.missing_context",
    });
  });

  test("refuses signatures outside the risk ceiling", async () => {
    const registryView = await Effect.runPromise(view());

    await expect(
      Effect.runPromise(
        lookupBlueprintSignatures({
          backendCapabilityRefs: ["probe.backend.apple_fm_bridge", "probe.blueprint.tool_menu"],
          lookupId: "lookup.test.risk",
          registryView,
          request: {
            actorRef: "actor.probe.test",
            allowedSurfaces: ["agent_api"],
            backendKind: "apple_fm_bridge",
            contextPackRef: "context_pack.test",
            programSignatureIds: ["program_signature.probe.tool_menu.project.v1"],
            riskCeiling: "low",
          },
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "BlueprintSignatureLookupError",
      lookupId: "lookup.test.risk",
    });
  });

  test("refuses signatures without an allowed surface", async () => {
    const registryView = await Effect.runPromise(view());

    await expect(
      Effect.runPromise(
        lookupBlueprintSignatures({
          backendCapabilityRefs: ["probe.backend.apple_fm_bridge", "probe.blueprint.signature_lookup"],
          lookupId: "lookup.test.surface",
          registryView,
          request: {
            actorRef: "actor.probe.test",
            allowedSurfaces: ["public_site"],
            backendKind: "apple_fm_bridge",
            contextPackRef: "context_pack.test",
            programSignatureIds: ["program_signature.probe.signature_lookup.v1"],
            riskCeiling: "low",
          },
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "BlueprintSignatureLookupError",
      lookupId: "lookup.test.surface",
    });
  });

  test("refuses unsafe or direct-mutation registry entries", async () => {
    const registryView = await Effect.runPromise(view());
    const unsafeRegistryView = {
      ...registryView,
      registry: {
        ...STATIC_BLUEPRINT_PROGRAM_REGISTRY,
        entries: [
          {
            ...STATIC_BLUEPRINT_PROGRAM_REGISTRY.entries[0],
            directMutationAllowed: true,
          },
        ],
      },
    };

    await expect(
      Effect.runPromise(
        lookupBlueprintSignatures({
          backendCapabilityRefs: ["probe.backend.apple_fm_bridge", "probe.blueprint.signature_lookup"],
          lookupId: "lookup.test.unsafe",
          registryView: unsafeRegistryView,
          request: {
            actorRef: "actor.probe.test",
            allowedSurfaces: ["agent_api"],
            backendKind: "apple_fm_bridge",
            contextPackRef: "context_pack.test",
            preferredFamily: "routing",
            riskCeiling: "low",
          },
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "BlueprintSignatureLookupError",
      lookupId: "lookup.test.unsafe",
    });
  });

  test("does not need prompt keyword routing for structured lookup", async () => {
    const registryView = await Effect.runPromise(view());
    const baseInput = {
      backendCapabilityRefs: ["probe.backend.apple_fm_bridge", "probe.blueprint.signature_lookup"],
      registryView,
      request: {
        actorRef: "actor.probe.test",
        allowedSurfaces: ["agent_api"] as const,
        backendKind: "apple_fm_bridge",
        contextPackRef: "context_pack.test",
        preferredFamily: "routing" as const,
        riskCeiling: "low" as const,
      },
    };

    const first = await Effect.runPromise(
      lookupBlueprintSignatures({
        ...baseInput,
        lookupId: "lookup.test.no_keywords.1",
      }),
    );
    const second = await Effect.runPromise(
      lookupBlueprintSignatures({
        ...baseInput,
        lookupId: "lookup.test.no_keywords.2",
        request: {
          ...baseInput.request,
          objectiveRef: "objective.ref.that.is.not.parsed.for.prompt_words",
        },
      }),
    );

    expect(first.programSignatureIds).toEqual(second.programSignatureIds);
    expect(first.toolScopes.map((scope) => scope.toolRef)).toEqual(second.toolScopes.map((scope) => scope.toolRef));
  });
});
