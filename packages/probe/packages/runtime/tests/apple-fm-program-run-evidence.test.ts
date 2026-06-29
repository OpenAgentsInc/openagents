import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  APPLE_FM_DEFAULT_MODEL_ID,
  makeAppleFmToolStreamProgramRunEvidence,
  probeProgramRunEvidenceIsEvidenceOnly,
  validateProbeProgramRunEvidence,
  type AppleFmToolStreamResult,
  type ProbeBlueprintProgramRunEvidence,
  type ProbeToolMenu,
} from "../src";

const menu = (): ProbeToolMenu => ({
  actionSubmissionRequiredForDirectEffects: true,
  backendKind: "apple_fm_bridge",
  deniedTools: [],
  evidenceRequirementRefs: ["evidence.blueprint.tool_menu.projected_from_signature_scopes"],
  lookupId: "lookup.evidence.test",
  menuId: "menu.evidence.test",
  moduleVersionIds: ["module_version.probe.tool_menu.seed.v1"],
  policyRef: "policy.blueprint.probe_registry_fixture.public_refs_only.v1",
  programSignatureIds: ["program_signature.probe.tool_menu.project.v1"],
  programTypeIds: ["program_type.probe.tool_menu.project"],
  receiptRequirementRefs: ["receipt.program_run"],
  registryVersionRef: "blueprint_registry.probe_static_fixture.v1",
  safeProjection: true,
  sourceKind: "staticFixture",
  tools: [],
  warnings: [],
});

const result = (): AppleFmToolStreamResult => ({
  bridgeSessionId: "sess-evidence",
  callbackServer: {
    callbackUrl: "[redacted]",
  },
  completion: {
    profile: {
      id: "apple-fm-local",
      kind: "apple_fm_bridge",
      model: APPLE_FM_DEFAULT_MODEL_ID,
      baseUrl: "http://127.0.0.1:11439",
      readinessPath: "/health",
      attachMode: "attach_existing",
      authMode: "none",
      streamMode: "snapshot",
    },
    receipt: {
      kind: "probe_backend_transcript",
      backendKind: "apple_fm_bridge",
      profileId: "apple-fm-local",
      model: APPLE_FM_DEFAULT_MODEL_ID,
      usage: { truth: "estimated", totalTokens: 12 },
      observedAt: "2026-06-07T00:00:00.000Z",
      contentRedacted: true,
    },
    response: {
      model: APPLE_FM_DEFAULT_MODEL_ID,
      choices: [
        {
          message: {
            role: "assistant",
            content: "README.md first heading is Probe.",
          },
        },
      ],
      usage: { truth: "estimated", totalTokens: 12 },
    },
    text: "README.md first heading is Probe.",
    usage: { truth: "estimated", totalTokens: 12 },
  },
  events: [],
  profile: {
    id: "apple-fm-local",
    kind: "apple_fm_bridge",
    model: APPLE_FM_DEFAULT_MODEL_ID,
    baseUrl: "http://127.0.0.1:11439",
    readinessPath: "/health",
    attachMode: "attach_existing",
    authMode: "none",
    streamMode: "snapshot",
  },
  toolTranscript: [
    {
      kind: "probe_tool_callback",
      backendKind: "apple_fm_bridge",
      sessionId: "session-evidence",
      toolCallId: "tool-call-1",
      toolName: "read_file",
      status: "success",
      input: { path: "README.md" },
      observedAt: "2026-06-07T00:00:00.000Z",
      callbackTokenRedacted: true,
      contentRedacted: true,
    },
  ],
});

describe("Apple FM Program Run evidence", () => {
  test("creates evidence-only Program Run records for Apple FM tool streams", async () => {
    const record = await Effect.runPromise(
      makeAppleFmToolStreamProgramRunEvidence({
        actorRef: "actor.probe.test",
        menu: menu(),
        observedAt: "2026-06-07T00:00:00.000Z",
        promptSummaryRef: "prompt_summary.test",
        projection: {
          lookupId: "lookup.evidence.test",
          menuId: "menu.evidence.test",
          programSignatureIds: ["program_signature.probe.tool_menu.project.v1"],
          registryVersionRef: "blueprint_registry.probe_static_fixture.v1",
          toolRefs: [],
          warnings: [],
        },
        result: result(),
      }),
    );

    expect(record.kind).toBe("probe_blueprint_program_run_evidence");
    expect(record.programSignatureId).toBe("program_signature.probe.tool_menu.project.v1");
    expect(record.moduleVersionId).toBe("module_version.probe.tool_menu.seed.v1");
    expect(record.toolCallbackRefs).toContain("tool_callback.session-evidence.tool-call-1.success");
    expect(record.receiptRefs).toContain("receipt.program_run");
    expect(record.usage.truth).toBe("estimated");
    expect(record.contentRedacted).toBe(true);
    expect(probeProgramRunEvidenceIsEvidenceOnly(record)).toBe(true);
    expect(JSON.stringify(record)).not.toContain("README.md first heading is Probe.");
  });

  test("rejects Program Run records that claim external write authority", async () => {
    const unsafe: ProbeBlueprintProgramRunEvidence = {
      ...(await Effect.runPromise(
        makeAppleFmToolStreamProgramRunEvidence({
          actorRef: "actor.probe.test",
          menu: menu(),
          promptSummaryRef: "prompt_summary.test",
          projection: {
            lookupId: "lookup.evidence.test",
            menuId: "menu.evidence.test",
            programSignatureIds: ["program_signature.probe.tool_menu.project.v1"],
            registryVersionRef: "blueprint_registry.probe_static_fixture.v1",
            toolRefs: [],
            warnings: [],
          },
          result: result(),
        }),
      )),
      noDeploy: false,
    };

    await expect(Effect.runPromise(validateProbeProgramRunEvidence(unsafe))).rejects.toMatchObject({
      _tag: "ProbeBlueprintProgramRunEvidenceUnsafe",
    });
  });

  test("rejects Program Run evidence with private-data-shaped payloads", async () => {
    const unsafe: ProbeBlueprintProgramRunEvidence = {
      ...(await Effect.runPromise(
        makeAppleFmToolStreamProgramRunEvidence({
          actorRef: "actor.probe.test",
          menu: menu(),
          promptSummaryRef: "prompt_summary.test",
          projection: {
            lookupId: "lookup.evidence.test",
            menuId: "menu.evidence.test",
            programSignatureIds: ["program_signature.probe.tool_menu.project.v1"],
            registryVersionRef: "blueprint_registry.probe_static_fixture.v1",
            toolRefs: [],
            warnings: [],
          },
          result: result(),
        }),
      )),
      typedOutput: {
        raw_prompt: "do not store me",
      },
    };

    await expect(Effect.runPromise(validateProbeProgramRunEvidence(unsafe))).rejects.toMatchObject({
      _tag: "ProbeBlueprintProgramRunEvidenceUnsafe",
    });
  });
});
