import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  APPLE_FM_DEFAULT_MODEL_ID,
  PROBE_APPLE_FM_BACKEND_CAPABILITY,
  STATIC_BLUEPRINT_PROGRAM_REGISTRY,
  STATIC_BLUEPRINT_REGISTRY_VERSION_REF,
  reportAppleFmBackendCapability,
  type ProbeRunnerIdentity,
} from "../src";

const runner = (kind: ProbeRunnerIdentity["kind"] = "pylon"): ProbeRunnerIdentity => ({
  runnerId: `runner_${kind}_1`,
  kind,
  linkedSubject: "provider_1",
  linkedAt: "2026-06-07T00:00:00.000Z",
  capabilities: ["probe.run", PROBE_APPLE_FM_BACKEND_CAPABILITY],
});

describe("Probe backend capability reporting", () => {
  test("advertises Apple FM capability only after live health is ready", async () => {
    const report = await Effect.runPromise(
      reportAppleFmBackendCapability({
        runner: runner("pylon"),
        trustedBackendBaseUrl: "http://user:secret@127.0.0.1:11435/?token=hidden",
        fetch: async (input) => {
          expect(new URL(String(input)).pathname).toBe("/health");
          return Response.json({
            ready: true,
            modelId: APPLE_FM_DEFAULT_MODEL_ID,
            platform: "apple-silicon",
            version: "fake",
          });
        },
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(report.available).toBe(true);
    expect(report.advertisedCapabilities).toContain(PROBE_APPLE_FM_BACKEND_CAPABILITY);
    expect(report.advertisedCapabilities).toContain("probe.blueprint.tool_menu");
    expect(report.advertisedCapabilities).toContain("adapter.probe.apple_fm.blueprint_tools.v1");
    expect(report.advertisedCapabilities).toContain("probe.program_run.evidence.local_offline");
    expect(report.backendKind).toBe("apple_fm_bridge");
    expect(report.model).toBe(APPLE_FM_DEFAULT_MODEL_ID);
    expect(report.support).toEqual({ snapshotStreaming: true, toolCallbacks: true });
    expect(report.blueprintSupport.registryVersionRefs).toEqual([STATIC_BLUEPRINT_REGISTRY_VERSION_REF]);
    expect(report.blueprintSupport.programFamilies).toContain("action_planning");
    expect(report.blueprintSupport.programSignatureRefs).toContain("program_signature.probe.tool_menu.project.v1");
    expect(report.blueprintSupport.toolRefs).toContain("tool.probe.read_file");
    expect(report.blueprintSupport.appleFmSchemaProjection).toMatchObject({
      maxProjectedToolCount: 8,
      supported: true,
    });
    expect(report.blueprintSupport.backendAvailability).toEqual({ api: false, local: true, swarm: false });
    expect(report.blueprintSupport.localProgramRunEvidenceOffline).toBe(true);
    expect(report.baseUrl).toBe("http://127.0.0.1:11435");
    expect(JSON.stringify(report)).not.toContain("secret");
    expect(JSON.stringify(report)).not.toContain("hidden");
  });

  test("reports unsupported Apple FM state without advertising the capability", async () => {
    const report = await Effect.runPromise(
      reportAppleFmBackendCapability({
        runner: runner("shc"),
        fetch: async () =>
          Response.json({
            ready: false,
            modelId: APPLE_FM_DEFAULT_MODEL_ID,
            unavailableReason: "unsupported_hardware",
            message: "Apple Intelligence unavailable",
          }),
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(report.available).toBe(false);
    expect(report.status).toBe("unsupported");
    expect(report.advertisedCapabilities).toEqual([]);
    expect(report.blueprintSupport.safeProjection).toBe(true);
    expect(report.blueprintSupport.backendAvailability.local).toBe(false);
    expect(report.unavailableReason).toBe("unsupported_hardware");
    expect(report.receipt).toMatchObject({
      kind: "probe_backend_availability",
      ready: false,
      unavailableReason: "unsupported_hardware",
    });
  });

  test("keeps backend identity distinct for Omega routing", async () => {
    const report = await Effect.runPromise(
      reportAppleFmBackendCapability({
        runner: runner("sandbox"),
        fetch: async () =>
          Response.json({
            ready: true,
            modelId: APPLE_FM_DEFAULT_MODEL_ID,
          }),
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(report.backendKind).toBe("apple_fm_bridge");
    expect(report.capability).toBe(PROBE_APPLE_FM_BACKEND_CAPABILITY);
    expect(JSON.stringify(report)).not.toContain("local inference");
    expect(JSON.stringify(report)).not.toContain("callbackToken");
  });

  test("does not advertise runnable capabilities when Blueprint registry support is malformed", async () => {
    const report = await Effect.runPromise(
      reportAppleFmBackendCapability({
        runner: runner("pylon"),
        blueprintRegistry: {
          ...STATIC_BLUEPRINT_PROGRAM_REGISTRY,
          safeProjection: false,
          policyRef: "policy.blueprint.callbackToken.raw",
        },
        fetch: async () =>
          Response.json({
            ready: true,
            modelId: APPLE_FM_DEFAULT_MODEL_ID,
          }),
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(report.available).toBe(false);
    expect(report.status).toBe("malformed");
    expect(report.advertisedCapabilities).toEqual([]);
    expect(report.unavailableReason).toBe("malformed_blueprint_support");
    expect(report.blueprintSupport.safeProjection).toBe(false);
    expect(report.blueprintSupport.appleFmSchemaProjection.supported).toBe(false);
    expect(JSON.stringify(report)).not.toContain("callbackToken");
  });

  test("does not advertise runnable capabilities when Apple FM tool projection support is malformed", async () => {
    const report = await Effect.runPromise(
      reportAppleFmBackendCapability({
        runner: runner("pylon"),
        maxProjectedAppleFmToolCount: 0,
        backendAvailability: {
          swarm: true,
        },
        fetch: async () =>
          Response.json({
            ready: true,
            modelId: APPLE_FM_DEFAULT_MODEL_ID,
          }),
        now: new Date("2026-06-07T00:00:00.000Z"),
      }),
    );

    expect(report.available).toBe(false);
    expect(report.status).toBe("malformed");
    expect(report.advertisedCapabilities).toEqual([]);
    expect(report.blueprintSupport.appleFmSchemaProjection).toMatchObject({
      maxProjectedToolCount: 0,
      supported: false,
      unsupportedReason: "unsupported_tool_schema_projection",
    });
    expect(report.blueprintSupport.backendAvailability).toEqual({ api: false, local: true, swarm: true });
  });
});
