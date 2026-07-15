import { afterEach, describe, expect, test } from "vite-plus/test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

import {
  decodeAndBindNativeSdkHostGate,
  nativeSdkHostGateExpectedSteps,
  nativeSdkHostGateSourcePaths,
  normalizeNativeSdkHostGate,
  observeNativeSdkHostGateInputs,
  sha256Digest,
} from "../src/index.ts";

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

const digest = (character: string): string => `sha256:${character.repeat(64)}`;

const fixture = () => {
  const workspaceRoot = mkdtempSync(join(tmpdir(), "openagents-native-assurance-"));
  roots.push(workspaceRoot);
  for (const path of nativeSdkHostGateSourcePaths) {
    const absolute = resolve(workspaceRoot, path);
    mkdirSync(dirname(absolute), { recursive: true });
    writeFileSync(absolute, `fixture:${path}\n`);
  }
  const packageRoot = resolve(workspaceRoot, "apps/native-sdk-effect-native-spike");
  const binaryPath = resolve(packageRoot, "zig-out/bin/native-sdk-effect-native-spike");
  const frontendPath = resolve(packageRoot, "frontend/dist/assets/main.js");
  const sidecarPath = resolve(packageRoot, "sidecar/dist/native-sidecar-entry.mjs");
  mkdirSync(dirname(binaryPath), { recursive: true });
  mkdirSync(dirname(frontendPath), { recursive: true });
  mkdirSync(dirname(sidecarPath), { recursive: true });
  writeFileSync(binaryPath, "native-binary");
  writeFileSync(frontendPath, "frontend-bundle");
  writeFileSync(sidecarPath, "sidecar-bundle");
  const evidenceRoot = resolve(workspaceRoot, "var/assurance/native/host-smoke");
  mkdirSync(evidenceRoot, { recursive: true });
  const evidenceNames = [
    "01-composited-window.png",
    "02-cw-ac-03-initial.json",
    "03-native-shell.png",
    "04-renderer-reload.snapshot.txt",
    "05-process-restart.snapshot.txt",
    "06-cw-ac-03-restarted.json",
  ];
  const evidence = evidenceNames.map((name) => {
    const bytes = `evidence:${name}`;
    writeFileSync(resolve(evidenceRoot, name), bytes);
    return { name, digest: sha256Digest(bytes), bytes: Buffer.byteLength(bytes) };
  });
  const observed = observeNativeSdkHostGateInputs(workspaceRoot);
  const expected = {
    workspaceRoot,
    evidenceRoot,
    manifestDigest: digest("a"),
    environmentDigest: digest("b"),
    adapterLockDigest: digest("c"),
    targetDescriptorDigest: digest("d"),
    targetSourceDigest: digest("e"),
    environmentRef: "ENV-OA-DESKTOP-NATIVE-SDK-MACOS-1",
    nativeReportRef: "var/assurance/native/host-smoke/host-gate.json",
  };
  const gate = {
    formatVersion: "openagents.native-sdk.host-gate.v5",
    targetRef: "openagents.desktop.native-sdk.mvp",
    runNonce: "01234567-89ab-4def-8123-456789abcdef",
    automationProtocol: 7,
    frontendAuthority: "effect-native",
    result: "passed",
    runtime: {
      os: "darwin",
      architecture: "arm64",
      node: "24.13.1",
      zig: "0.16.0",
      nativeSdkCommit: "f7aa92af6dcece250feba852af4d22e7f5429312",
    },
    inputs: { commandDigest: digest("f"), ...observed },
    assurance: {
      manifestDigest: expected.manifestDigest,
      environmentDigest: expected.environmentDigest,
      adapterLockDigest: expected.adapterLockDigest,
      targetDescriptorDigest: expected.targetDescriptorDigest,
      targetSourceDigest: expected.targetSourceDigest,
    },
    processes: {
      initial: {
        pid: 9_007_199_254_740_001,
        publisherPid: 9_007_199_254_740_001,
        stopped: true,
        exitCode: null,
        signal: "SIGTERM",
        forcedKill: false,
      },
      restarted: {
        pid: 9_007_199_254_740_002,
        publisherPid: 9_007_199_254_740_002,
        stopped: true,
        exitCode: null,
        signal: "SIGTERM",
        forcedKill: false,
      },
    },
    sidecars: {
      initial: {
        pid: 9_007_199_254_739_991,
        generation: 1,
        liveDuringHost: true,
        liveAfterHost: false,
      },
      restarted: {
        pid: 9_007_199_254_739_992,
        generation: 2,
        liveDuringHost: true,
        liveAfterHost: false,
      },
    },
    criterionObservations: {
      schema: "openagents.native-sdk.cw-ac-03.v1",
      criterionRef: "CW-AC-03",
      grantSource: "native_canvas_file_drop",
      initial: {
        generation: 1,
        catalogSessionCount: 1,
        admission: {
          grantRef: "grant.test",
          projectRef: "project.test",
          repositoryRef: "repository.test",
          worktreeRef: "worktree.test",
          workContextRef: "work-context.test",
          sessionRef: "session.test",
        },
      },
      restarted: {
        generation: 2,
        catalogSessionCount: 1,
        admission: {
          grantRef: "grant.test",
          projectRef: "project.test",
          repositoryRef: "repository.test",
          worktreeRef: "worktree.test",
          workContextRef: "work-context.test",
          sessionRef: "session.test",
        },
      },
      aliasCanonicalized: true,
      ambientInputsExcluded: true,
      ambientFalsifiers: {
        initial: { hostname: "host-alpha", port: "port-43101", providerThread: "provider-alpha" },
        restarted: { hostname: "host-beta", port: "port-53202", providerThread: "provider-beta" },
      },
      privateBindingMode: "0600",
    },
    steps: nativeSdkHostGateExpectedSteps.map((id) => ({
      id,
      result: "passed",
      evidence: [evidenceNames[0]!],
    })),
    evidence,
  };
  return { workspaceRoot, evidenceRoot, expected, gate };
};

describe("Native SDK Assurance adapter", () => {
  test("normalizes an independently rehashed, exactly bound host gate", () => {
    const { expected, gate, workspaceRoot } = fixture();
    const result = normalizeNativeSdkHostGate(`${JSON.stringify(gate)}\n`, expected);
    expect(result.status).toBe("ready");
    if (result.status !== "ready") throw new Error(result.code);
    expect(result.receipt).toMatchObject({
      target_ref: "openagents.desktop.native-sdk.mvp",
      manifest_digest: expected.manifestDigest,
      environment_digest: expected.environmentDigest,
      adapter_lock_digest: expected.adapterLockDigest,
      target_descriptor_digest: expected.targetDescriptorDigest,
      target_source_digest: expected.targetSourceDigest,
      verdict: "green",
    });
    expect(result.receiptBytes).not.toContain(workspaceRoot);
    expect(result.receiptBytes).not.toContain("evidence:01-composited");
  });

  test("rejects assurance-binding drift and excess producer fields", () => {
    const { expected, gate } = fixture();
    const drifted = { ...gate, assurance: { ...gate.assurance, manifestDigest: digest("9") } };
    expect(() => decodeAndBindNativeSdkHostGate(JSON.stringify(drifted), expected)).toThrow(
      "manifestDigest",
    );
    const excess = { ...gate, producerVerdict: "trust-me" };
    expect(normalizeNativeSdkHostGate(JSON.stringify(excess), expected)).toMatchObject({
      status: "inconclusive",
      code: "host_gate_schema_invalid",
    });
  });

  test("rejects stale or traversal evidence instead of emitting a receipt", () => {
    const { expected, gate, evidenceRoot } = fixture();
    writeFileSync(resolve(evidenceRoot, gate.evidence[0]!.name), "tampered");
    expect(normalizeNativeSdkHostGate(JSON.stringify(gate), expected)).toMatchObject({
      status: "inconclusive",
      code: "host_gate_evidence_mismatch",
    });
    const fresh = fixture();
    const traversal = {
      ...fresh.gate,
      evidence: [...fresh.gate.evidence, { ...fresh.gate.evidence[0]!, name: "../escape.txt" }],
    };
    expect(normalizeNativeSdkHostGate(JSON.stringify(traversal), fresh.expected)).toMatchObject({
      status: "inconclusive",
      code: "host_gate_evidence_path_invalid",
    });
  });

  test("rejects source drift and forced process termination", () => {
    const { expected, gate, workspaceRoot } = fixture();
    writeFileSync(resolve(workspaceRoot, nativeSdkHostGateSourcePaths[0]!), "drift");
    expect(normalizeNativeSdkHostGate(JSON.stringify(gate), expected)).toMatchObject({
      status: "inconclusive",
      code: "host_gate_input_mismatch",
    });
    const fresh = fixture();
    const forced = {
      ...fresh.gate,
      processes: {
        ...fresh.gate.processes,
        restarted: { ...fresh.gate.processes.restarted, forcedKill: true },
      },
    };
    expect(normalizeNativeSdkHostGate(JSON.stringify(forced), fresh.expected)).toMatchObject({
      status: "inconclusive",
      code: "host_gate_process_or_size_invalid",
    });
  });

  test("rejects work identity drift across the Native restart", () => {
    const { expected, gate } = fixture();
    const drifted = {
      ...gate,
      criterionObservations: {
        ...gate.criterionObservations,
        restarted: {
          ...gate.criterionObservations.restarted,
          admission: {
            ...gate.criterionObservations.restarted.admission,
            sessionRef: "session.drifted",
          },
        },
      },
    };
    expect(normalizeNativeSdkHostGate(JSON.stringify(drifted), expected)).toMatchObject({
      status: "inconclusive",
      code: "host_gate_work_identity_drift",
    });
  });
});
