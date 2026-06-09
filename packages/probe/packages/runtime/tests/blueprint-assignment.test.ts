import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  PROBE_APPLE_FM_BACKEND_CAPABILITY,
  STATIC_BLUEPRINT_CONTRACT_EXPORT,
  STATIC_BLUEPRINT_PROGRAM_REGISTRY,
  STATIC_BLUEPRINT_REGISTRY_VERSION_REF,
  assignmentInlineBlueprintRegistrySource,
  decodeProbeRunAssignment,
  loadBlueprintSignatureRegistry,
  sanitizeProbeRunAssignmentProjection,
} from "../src";

const assignment = () => ({
  assignmentId: "assignment_blueprint_1",
  runnerSessionId: "runner_session_1",
  goal: "Project the allowed local repo tools.",
  backend: {
    kind: "apple_fm_bridge",
    profile: "apple-fm-local",
  },
  blueprint: {
    actionSubmissionPolicyRef: "policy.blueprint.action_submission.proposals_only.v1",
    backendCapabilityRefs: [PROBE_APPLE_FM_BACKEND_CAPABILITY],
    contextPackRefs: ["context_pack.openagents.thread_1"],
    contractExport: STATIC_BLUEPRINT_CONTRACT_EXPORT,
    moduleVersionRefs: ["module_version.probe.tool_menu.seed.v1"],
    programRunPurposeRef: "purpose.probe.tool_menu.project",
    programSignatureRefs: ["program_signature.probe.tool_menu.project.v1"],
    programTypeRefs: ["program_type.probe.tool_menu.project"],
    registry: STATIC_BLUEPRINT_PROGRAM_REGISTRY,
    registryVersionRef: STATIC_BLUEPRINT_REGISTRY_VERSION_REF,
    releaseGateRefs: ["release_gate.probe.tool_menu.seed.v1"],
    sourceAuthorityRefs: ["source_authority.repo.openagents.probe"],
    toolScopeRefs: ["tool.probe.read_file", "tool.probe.code_search"],
  },
});

describe("Probe Blueprint assignment scope", () => {
  test("accepts a valid Blueprint-scoped Apple FM assignment", async () => {
    const parsed = await Effect.runPromise(decodeProbeRunAssignment(assignment()));
    const registryView = await Effect.runPromise(
      loadBlueprintSignatureRegistry({
        assignment: assignmentInlineBlueprintRegistrySource(parsed),
        sourceKind: "assignmentInline",
      }),
    );

    expect(parsed.blueprint?.registryVersionRef).toBe(STATIC_BLUEPRINT_REGISTRY_VERSION_REF);
    expect(parsed.blueprint?.programSignatureRefs).toContain("program_signature.probe.tool_menu.project.v1");
    expect(parsed.blueprint?.backendCapabilityRefs).toEqual([PROBE_APPLE_FM_BACKEND_CAPABILITY]);
    expect(registryView.sourceKind).toBe("assignmentInline");
    expect(registryView.registryVersionRef).toBe(STATIC_BLUEPRINT_REGISTRY_VERSION_REF);
  });

  test("keeps Apple FM no-auth assignments working without Blueprint fields", async () => {
    const parsed = await Effect.runPromise(
      decodeProbeRunAssignment({
        assignmentId: "assignment_apple_fm_no_blueprint",
        runnerSessionId: "runner_session_1",
        goal: "Summarize this repo locally.",
        backend: {
          kind: "apple_fm_bridge",
          profile: "apple-fm-local",
        },
      }),
    );

    expect(parsed.blueprint).toBeUndefined();
    expect(parsed.providerAccountRef).toBeUndefined();
    expect(parsed.authGrantRef).toBeUndefined();
  });

  test("rejects missing and invalid Blueprint registry version refs", async () => {
    const missingRegistryVersion = {
      ...assignment(),
      blueprint: {
        ...assignment().blueprint,
        registryVersionRef: undefined,
      },
    };
    const invalidRegistryVersion = {
      ...assignment(),
      blueprint: {
        ...assignment().blueprint,
        registryVersionRef: "registry.assignment_inline.test.v1",
      },
    };

    await expect(Effect.runPromise(decodeProbeRunAssignment(missingRegistryVersion))).rejects.toMatchObject({
      _tag: "ProbeAssignmentParseError",
    });
    await expect(Effect.runPromise(decodeProbeRunAssignment(invalidRegistryVersion))).rejects.toMatchObject({
      _tag: "ProbeAssignmentParseError",
    });
  });

  test("rejects private-data-shaped material in the Blueprint section", async () => {
    const unsafeBlueprints = [
      { raw_prompt: "copy this full private prompt" },
      { rawPrompt: "copy this full private prompt" },
      { callback_token: "callback_token secret" },
      { callbackToken: "callback_token secret" },
      { callbackUrl: "http://127.0.0.1/callback" },
      { provider_payload: { id_token: "id_token secret" } },
      { providerPayload: { idToken: "id_token secret" } },
      { private_repo_content: "uncommitted private source" },
      { privateRepo: "git@github.com:OpenAgentsInc/private.git" },
      { wallet_mnemonic: "wallet mnemonic words" },
      { customer_email: "person@example.com" },
      { customerEmail: "person@example.com" },
    ];

    for (const unsafe of unsafeBlueprints) {
      await expect(
        Effect.runPromise(
          decodeProbeRunAssignment({
            ...assignment(),
            blueprint: {
              ...assignment().blueprint,
              ...unsafe,
            },
          }),
        ),
      ).rejects.toMatchObject({
        _tag: "ProbePublicProjectionUnsafe",
      });
    }
  });

  test("sanitizes assignments while preserving Blueprint refs", () => {
    const sanitized = sanitizeProbeRunAssignmentProjection({
      ...assignment(),
      blueprint: {
        ...assignment().blueprint,
        callback_token: "callback_token secret",
        nested: {
          provider_payload: "raw provider payload",
        },
      },
      sandbox: {
        access_token: "raw-token",
      },
    });

    expect(sanitized.blueprint.toolScopeRefs).toContain("tool.probe.read_file");
    expect("callback_token" in sanitized.blueprint).toBe(false);
    expect(sanitized.blueprint.nested).toEqual({});
    expect(sanitized.sandbox.access_token).toBe("[redacted]");
  });

  test("rejects unsafe inline registry slices", async () => {
    await expect(
      Effect.runPromise(
        decodeProbeRunAssignment({
          ...assignment(),
          blueprint: {
            ...assignment().blueprint,
            registry: {
              ...STATIC_BLUEPRINT_PROGRAM_REGISTRY,
              entries: [
                {
                  ...STATIC_BLUEPRINT_PROGRAM_REGISTRY.entries[0],
                  directMutationAllowed: true,
                },
              ],
            },
          },
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "BlueprintProjectionUnsafe",
    });
  });

  test("rejects backend capability refs that widen the selected backend authority", async () => {
    await expect(
      Effect.runPromise(
        decodeProbeRunAssignment({
          ...assignment(),
          blueprint: {
            ...assignment().blueprint,
            backendCapabilityRefs: ["probe.backend.openai_responses"],
          },
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeAssignmentParseError",
    });
  });

  test("rejects Blueprint refs outside the attached inline registry slice", async () => {
    await expect(
      Effect.runPromise(
        decodeProbeRunAssignment({
          ...assignment(),
          blueprint: {
            ...assignment().blueprint,
            programSignatureRefs: ["program_signature.probe.missing.v1"],
          },
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeAssignmentParseError",
    });
  });
});
