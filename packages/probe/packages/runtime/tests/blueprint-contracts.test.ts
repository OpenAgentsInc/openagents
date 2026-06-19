import { describe, expect, test } from "bun:test";
import { Effect, Schema as S } from "effect";
import {
  BlueprintContractExportSeed,
  BlueprintProgramRegistryProjection,
  BlueprintSignatureLookupRequest,
  BlueprintSignatureLookupResult,
  ProbeToolMenuPlan,
  STATIC_BLUEPRINT_CONTRACT_EXPORT,
  STATIC_BLUEPRINT_PROGRAM_REGISTRY,
  STATIC_PROBE_TOOL_MENU_PLAN,
  blueprintContractExportSeedIsPrivateDataSafe,
  blueprintProgramRunDetailProjectionIsEvidenceOnly,
  blueprintRegistryProjectionIsPrivateDataSafe,
  blueprintRegistryProjectionIsSafe,
  validateBlueprintRegistryProjection,
} from "../src";

describe("Blueprint consumer contracts", () => {
  test("decodes the static Blueprint registry fixture", async () => {
    const registry = await Effect.runPromise(
      S.decodeUnknownEffect(BlueprintProgramRegistryProjection)(STATIC_BLUEPRINT_PROGRAM_REGISTRY),
    );

    expect(registry.safeProjection).toBe(true);
    expect(registry.programSignatures.map((signature) => signature.id)).toContain(
      "program_signature.probe.signature_lookup.v1",
    );
    expect(registry.programTypes.every((programType) => !programType.directMutationAllowed)).toBe(true);
    expect(registry.runDetails.every(blueprintProgramRunDetailProjectionIsEvidenceOnly)).toBe(true);
    expect(blueprintRegistryProjectionIsSafe(registry)).toBe(true);
    await expect(Effect.runPromise(validateBlueprintRegistryProjection(registry))).resolves.toEqual(registry);
  });

  test("decodes the static Blueprint contract export fixture", async () => {
    const seed = await Effect.runPromise(
      S.decodeUnknownEffect(BlueprintContractExportSeed)(STATIC_BLUEPRINT_CONTRACT_EXPORT),
    );

    expect(seed.consumers).toContain("probe");
    expect(seed.consumers).toContain("pylon");
    expect(seed.openApi.map((entry) => entry.path)).toContain("/api/blueprint/program-registry");
    expect(seed.openApi.map((entry) => entry.path)).toContain("/api/blueprint/tassadar-modules");
    expect(seed.receiptCatalog.map((entry) => entry.receiptRef)).toContain("receipt.program_run");
    expect(blueprintContractExportSeedIsPrivateDataSafe(seed)).toBe(true);
  });

  test("decodes signature lookup and tool menu consumer shapes", async () => {
    const request = await Effect.runPromise(
      S.decodeUnknownEffect(BlueprintSignatureLookupRequest)({
        actorRef: "actor.probe.local",
        allowedSurfaces: ["agent_api", "omni_workroom"],
        backendKind: "apple_fm_bridge",
        preferredFamily: "routing",
        registrySource: "staticFixture",
        riskCeiling: "medium",
      }),
    );

    const result = await Effect.runPromise(
      S.decodeUnknownEffect(BlueprintSignatureLookupResult)({
        entries: STATIC_BLUEPRINT_PROGRAM_REGISTRY.entries,
        moduleVersions: STATIC_BLUEPRINT_PROGRAM_REGISTRY.moduleVersions,
        programSignatures: STATIC_BLUEPRINT_PROGRAM_REGISTRY.programSignatures,
        programTypes: STATIC_BLUEPRINT_PROGRAM_REGISTRY.programTypes,
        registryPolicyRef: STATIC_BLUEPRINT_PROGRAM_REGISTRY.policyRef,
        releaseGates: STATIC_BLUEPRINT_PROGRAM_REGISTRY.releaseGates,
        safeProjection: STATIC_BLUEPRINT_PROGRAM_REGISTRY.safeProjection,
        source: "staticFixture",
      }),
    );

    const plan = await Effect.runPromise(S.decodeUnknownEffect(ProbeToolMenuPlan)(STATIC_PROBE_TOOL_MENU_PLAN));

    expect(request.allowedSurfaces).toEqual(["agent_api", "omni_workroom"]);
    expect(result.programSignatures.length).toBeGreaterThan(0);
    expect(plan.safeProjection).toBe(true);
    expect(plan.evidenceFlags.directMutationDisabled).toBe(true);
  });

  test("rejects unsafe registry projections", async () => {
    const unsafe = {
      ...STATIC_BLUEPRINT_PROGRAM_REGISTRY,
      entries: [
        {
          ...STATIC_BLUEPRINT_PROGRAM_REGISTRY.entries[0],
          directMutationAllowed: true,
        },
      ],
    };
    const decoded = await Effect.runPromise(S.decodeUnknownEffect(BlueprintProgramRegistryProjection)(unsafe));

    expect(blueprintRegistryProjectionIsSafe(decoded)).toBe(false);
    await expect(Effect.runPromise(validateBlueprintRegistryProjection(decoded))).rejects.toMatchObject({
      _tag: "BlueprintProjectionUnsafe",
    });
  });

  test("rejects direct-mutation-enabled Program Run projections", async () => {
    const unsafe = {
      ...STATIC_BLUEPRINT_PROGRAM_REGISTRY,
      runDetails: [
        {
          ...STATIC_BLUEPRINT_PROGRAM_REGISTRY.runDetails[0],
          directMutationDisabled: false,
          noDeploy: false,
        },
      ],
    };
    const decoded = await Effect.runPromise(S.decodeUnknownEffect(BlueprintProgramRegistryProjection)(unsafe));

    expect(decoded.runDetails.every(blueprintProgramRunDetailProjectionIsEvidenceOnly)).toBe(false);
    expect(blueprintRegistryProjectionIsSafe(decoded)).toBe(false);
  });

  test("rejects secret-shaped fixture payloads", async () => {
    const unsafeRegistry = {
      ...STATIC_BLUEPRINT_PROGRAM_REGISTRY,
      policyRef: "policy.blueprint.access_token.raw",
    };
    const unsafeContractExport = {
      ...STATIC_BLUEPRINT_CONTRACT_EXPORT,
      jsonSchemas: [
        {
          ...STATIC_BLUEPRINT_CONTRACT_EXPORT.jsonSchemas[0],
          schemaRef: "schema.blueprint.raw_prompt.private",
        },
      ],
    };

    expect(blueprintRegistryProjectionIsPrivateDataSafe(unsafeRegistry)).toBe(false);
    expect(blueprintContractExportSeedIsPrivateDataSafe(unsafeContractExport)).toBe(false);
  });
});
