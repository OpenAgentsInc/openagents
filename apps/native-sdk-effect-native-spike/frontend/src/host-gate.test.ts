import { describe, expect, test } from "vite-plus/test";

import {
  decodeNativeSdkHostGate,
  nativeSdkAutomationProtocol,
  nativeSdkCommit,
  nativeSdkHostGateFormat,
  nativeSdkHostGateSteps,
  nativeSdkTargetRef,
} from "../../scripts/host-gate.ts";

const digest = `sha256:${"a".repeat(64)}`;
const validGate = () => ({
  formatVersion: nativeSdkHostGateFormat,
  targetRef: nativeSdkTargetRef,
  runNonce: "01234567-89ab-4def-8123-456789abcdef",
  automationProtocol: nativeSdkAutomationProtocol,
  frontendAuthority: "effect-native",
  result: "passed",
  runtime: { os: "darwin", architecture: "arm64", node: "24.13.1", zig: "0.16.0", nativeSdkCommit },
  inputs: { commandDigest: digest, binaryDigest: digest, sidecarBundleDigest: digest, frontendDigest: digest, sourceDigest: digest },
  assurance: null,
  processes: {
    initial: { pid: 101, publisherPid: 101, stopped: true, exitCode: 0, signal: null, forcedKill: false },
    restarted: { pid: 102, publisherPid: 102, stopped: true, exitCode: 0, signal: null, forcedKill: false },
  },
  sidecars: {
    initial: { pid: 91, generation: 1, liveAfterBootstrap: false },
    restarted: { pid: 92, generation: 2, liveAfterBootstrap: false },
  },
  steps: nativeSdkHostGateSteps.map((id) => ({ id, result: "passed", evidence: ["01-composited-window.png"] })),
  evidence: [
    "01-composited-window.png",
    "03-native-shell.png",
    "04-renderer-reload.snapshot.txt",
    "05-process-restart.snapshot.txt",
  ].map((name) => ({ name, digest, bytes: 10 })),
});

describe("Native SDK typed host gate", () => {
  test("accepts only a complete two-process headed receipt", () => {
    expect(decodeNativeSdkHostGate(validGate())).toMatchObject({ result: "passed", targetRef: nativeSdkTargetRef });
  });

  test("rejects missing composited evidence and PID reuse", () => {
    const missing = validGate();
    missing.evidence = missing.evidence.filter((entry) => entry.name !== "01-composited-window.png");
    expect(() => decodeNativeSdkHostGate(missing)).toThrow("native_host_gate_evidence_incomplete");
    const reused = validGate();
    reused.processes.restarted.pid = reused.processes.initial.pid;
    reused.processes.restarted.publisherPid = reused.processes.initial.pid;
    expect(() => decodeNativeSdkHostGate(reused)).toThrow("native_host_gate_process_or_size_invalid");
  });
});
