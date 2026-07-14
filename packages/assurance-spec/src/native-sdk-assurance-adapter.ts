import { Schema as S } from "effect"
import { createHash } from "node:crypto"
import { readFileSync, readdirSync } from "node:fs"
import { basename, dirname, relative, resolve, sep } from "node:path"

import { canonicalArtifact } from "./artifact.ts"
import type { AssuranceEnvironmentProfileDocument } from "./environment.ts"
import type { AssuranceExecutionUnit, AssuranceManifest } from "./manifest.ts"
import { sha256Digest } from "./tooling.ts"
import {
  executeVitePlusTestUnitWithIdentity,
  type VitePlusTestAdapterResult,
} from "./vite-plus-test-adapter.ts"

export const OPENAGENTS_NATIVE_SDK_ASSURANCE_ADAPTER_REF = "openagents.native_sdk_assurance.v1" as const
export const OPENAGENTS_NATIVE_SDK_ASSURANCE_ADAPTER_VERSION = "0.1.0" as const
export const OPENAGENTS_NATIVE_SDK_HOST_GATE_ADAPTER_REF = "openagents.native_sdk_host_gate.v1" as const
export const OPENAGENTS_NATIVE_SDK_HOST_GATE_ADAPTER_VERSION = "0.1.0" as const
export const OPENAGENTS_NATIVE_SDK_HOST_GATE_FORMAT = "openagents.native-sdk.host-gate.v4" as const
export const OPENAGENTS_NATIVE_SDK_TARGET_REF = "openagents.desktop.native-sdk.mvp" as const

export const nativeSdkHostGateExpectedSteps = [
  "initial-projection",
  "runtime-sidecar-bootstrap",
  "composited-window-capture",
  "session-selection",
  "workspace-round-trip",
  "native-canvas-screenshot",
  "renderer-reload-restored",
  "process-restart-restored",
  "new-chat-after-restart",
  "clean-teardown",
] as const

const Digest = S.String
const Process = S.Struct({
  pid: S.Number,
  publisherPid: S.Number,
  stopped: S.Literal(true),
  exitCode: S.NullOr(S.Number),
  signal: S.NullOr(S.String),
  forcedKill: S.Boolean,
})
const AssuranceBinding = S.Struct({
  manifestDigest: Digest,
  environmentDigest: Digest,
  adapterLockDigest: Digest,
  targetDescriptorDigest: Digest,
  targetSourceDigest: Digest,
})

/** Independent verifier schema. It intentionally does not import the app-owned producer decoder. */
export const NativeSdkAssuranceHostGateSchema = S.Struct({
  formatVersion: S.Literal(OPENAGENTS_NATIVE_SDK_HOST_GATE_FORMAT),
  targetRef: S.Literal(OPENAGENTS_NATIVE_SDK_TARGET_REF),
  runNonce: S.String,
  automationProtocol: S.Literal(7),
  frontendAuthority: S.Literal("effect-native"),
  result: S.Literal("passed"),
  runtime: S.Struct({
    os: S.Literal("darwin"),
    architecture: S.Literal("arm64"),
    node: S.Literal("24.13.1"),
    zig: S.Literal("0.16.0"),
    nativeSdkCommit: S.Literal("f7aa92af6dcece250feba852af4d22e7f5429312"),
  }),
  inputs: S.Struct({
    commandDigest: Digest,
    binaryDigest: Digest,
    sidecarBundleDigest: Digest,
    frontendDigest: Digest,
    sourceDigest: Digest,
  }),
  assurance: AssuranceBinding,
  processes: S.Struct({ initial: Process, restarted: Process }),
  sidecars: S.Struct({
    initial: S.Struct({ pid: S.Number, generation: S.Literal(1), liveAfterBootstrap: S.Literal(false) }),
    restarted: S.Struct({ pid: S.Number, generation: S.Literal(2), liveAfterBootstrap: S.Literal(false) }),
  }),
  steps: S.Array(S.Struct({
    id: S.String,
    result: S.Literal("passed"),
    evidence: S.Array(S.String),
  })),
  evidence: S.Array(S.Struct({ name: S.String, digest: Digest, bytes: S.Number })),
})

export type NativeSdkAssuranceHostGate = typeof NativeSdkAssuranceHostGateSchema.Type

export type NativeSdkHostGateExpectedBinding = Readonly<{
  workspaceRoot: string
  evidenceRoot: string
  manifestDigest: string
  environmentDigest: string
  adapterLockDigest: string
  targetDescriptorDigest: string
  targetSourceDigest: string
  environmentRef: string
  nativeReportRef: string
}>

export const nativeSdkHostGateSourcePaths = [
  "apps/native-sdk-effect-native-spike/app.zon",
  "apps/native-sdk-effect-native-spike/build.zig",
  "apps/native-sdk-effect-native-spike/build.zig.zon",
  "apps/native-sdk-effect-native-spike/package.json",
  "apps/native-sdk-effect-native-spike/vite.config.ts",
  "apps/native-sdk-effect-native-spike/frontend/index.html",
  "apps/native-sdk-effect-native-spike/src/main.zig",
  "apps/native-sdk-effect-native-spike/src/tests.zig",
  "apps/native-sdk-effect-native-spike/frontend/src/main.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/native-bridge.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/native-sdk-component-adoption.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/production-command-parity.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/program.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/state-storage.ts",
  "apps/native-sdk-effect-native-spike/frontend/src/style.css",
  "apps/native-sdk-effect-native-spike/scripts/host-gate.ts",
  "apps/native-sdk-effect-native-spike/scripts/run-host-smoke.ts",
  "apps/openagents-desktop/src/native-sidecar-contract.ts",
  "apps/openagents-desktop/src/native-sidecar-contract.test.ts",
  "apps/openagents-desktop/src/native-sidecar-entry.ts",
  "apps/openagents-desktop/src/desktop-command-contract.ts",
  "apps/openagents-desktop/package.json",
  "apps/openagents-desktop/src/chat-contract.ts",
  "apps/openagents-desktop/src/desktop-coding-catalog.ts",
  "apps/openagents-desktop/src/renderer/app.css",
  "apps/openagents-desktop/src/renderer/command-registry.ts",
  "apps/openagents-desktop/src/renderer/portable.ts",
  "apps/openagents-desktop/src/renderer/shell.ts",
] as const

export type NativeSdkHostGateReceipt = Readonly<{
  native_sdk_host_gate_receipt_format_version: "0.1"
  adapter_ref: typeof OPENAGENTS_NATIVE_SDK_HOST_GATE_ADAPTER_REF
  adapter_version: typeof OPENAGENTS_NATIVE_SDK_HOST_GATE_ADAPTER_VERSION
  target_ref: typeof OPENAGENTS_NATIVE_SDK_TARGET_REF
  environment_ref: string
  manifest_digest: string
  environment_digest: string
  adapter_lock_digest: string
  target_descriptor_digest: string
  target_source_digest: string
  native_report_ref: string
  native_report_digest: string
  command_digest: string
  binary_digest: string
  sidecar_bundle_digest: string
  frontend_digest: string
  source_digest: string
  evidence_digest: string
  runtime: NativeSdkAssuranceHostGate["runtime"]
  process_generations: 2
  verdict: "green"
  public_safety: Readonly<{ classification: "reviewed_public_safe"; contains_raw_output: false }>
}>

export type NativeSdkHostGateNormalization =
  | Readonly<{ status: "ready"; gate: NativeSdkAssuranceHostGate; receipt: NativeSdkHostGateReceipt; receiptBytes: string; receiptDigest: string }>
  | Readonly<{ status: "inconclusive"; code: string; message: string }>

export class NativeSdkAssuranceAdapterError extends Error {
  readonly code: string
  constructor(code: string, message: string) {
    super(message)
    this.name = "NativeSdkAssuranceAdapterError"
    this.code = code
  }
}

const fail = (code: string, message: string): never => {
  throw new NativeSdkAssuranceAdapterError(code, message)
}

const digestPattern = /^sha256:[a-f0-9]{64}$/u
const noncePattern = /^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/u

const sha256Bytes = (bytes: Buffer | string): string =>
  `sha256:${createHash("sha256").update(bytes).digest("hex")}`

const filesUnder = (directory: string): ReadonlyArray<string> =>
  readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = resolve(directory, entry.name)
    return entry.isDirectory() ? filesUnder(absolute) : [absolute]
  })

const fileSetDigest = (paths: ReadonlyArray<string>, base: string): string => sha256Bytes(JSON.stringify(
  [...paths].sort().map((absolute) => ({
    path: relative(base, absolute).split(sep).join("/"),
    digest: sha256Bytes(readFileSync(absolute)),
  })),
))

const processIsLive = (pid: number): boolean => {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

export const observeNativeSdkHostGateInputs = (workspaceRootInput: string): Readonly<{
  binaryDigest: string
  sidecarBundleDigest: string
  frontendDigest: string
  sourceDigest: string
}> => {
  const workspaceRoot = resolve(workspaceRootInput)
  const packageRoot = resolve(workspaceRoot, "apps/native-sdk-effect-native-spike")
  return {
    binaryDigest: sha256Bytes(readFileSync(resolve(packageRoot, "zig-out/bin/native-sdk-effect-native-spike"))),
    sidecarBundleDigest: sha256Bytes(readFileSync(resolve(packageRoot, "sidecar/dist/native-sidecar-entry.mjs"))),
    frontendDigest: fileSetDigest(filesUnder(resolve(packageRoot, "frontend/dist")), packageRoot),
    sourceDigest: fileSetDigest(
      nativeSdkHostGateSourcePaths.map((path) => resolve(workspaceRoot, path)),
      workspaceRoot,
    ),
  }
}

const exactBinding = (
  gate: NativeSdkAssuranceHostGate,
  expected: NativeSdkHostGateExpectedBinding,
): void => {
  for (const field of [
    "manifestDigest",
    "environmentDigest",
    "adapterLockDigest",
    "targetDescriptorDigest",
    "targetSourceDigest",
  ] as const) {
    if (gate.assurance[field] !== expected[field]) {
      fail("host_gate_binding_mismatch", `Native host gate ${field} differs from the active assurance run.`)
    }
  }
}

export const decodeAndBindNativeSdkHostGate = (
  bytes: string,
  expected: NativeSdkHostGateExpectedBinding,
): NativeSdkAssuranceHostGate => {
  if (expected.nativeReportRef.startsWith("/") || expected.nativeReportRef.includes("..") || expected.nativeReportRef.trim() === "") {
    return fail("host_gate_report_ref_invalid", "Native host report reference must be a safe repository-relative path.")
  }
  let candidate: unknown
  try {
    candidate = JSON.parse(bytes)
  } catch {
    return fail("host_gate_json_invalid", "Native host gate is not valid JSON.")
  }
  let gate: NativeSdkAssuranceHostGate
  try {
    gate = S.decodeUnknownSync(NativeSdkAssuranceHostGateSchema)(candidate, { onExcessProperty: "error" })
  } catch {
    return fail("host_gate_schema_invalid", "Native host gate does not match the independently owned verifier schema.")
  }
  if (!noncePattern.test(gate.runNonce)) fail("host_gate_nonce_invalid", "Native host gate nonce is malformed.")
  if (JSON.stringify(gate.steps.map((step) => step.id)) !== JSON.stringify(nativeSdkHostGateExpectedSteps)) {
    fail("host_gate_steps_incomplete", "Native host gate does not contain the exact ordered headed journey.")
  }
  const evidenceNames = gate.evidence.map((entry) => entry.name)
  const evidenceSet = new Set(evidenceNames)
  if (evidenceSet.size !== evidenceNames.length) fail("host_gate_evidence_duplicate", "Native host evidence names must be unique.")
  for (const name of [
    "01-composited-window.png",
    "03-native-shell.png",
    "04-renderer-reload.snapshot.txt",
    "05-process-restart.snapshot.txt",
  ]) {
    if (!evidenceSet.has(name)) fail("host_gate_evidence_incomplete", `Native host evidence is missing ${name}.`)
  }
  if (gate.steps.some((step) => step.evidence.some((name) => !evidenceSet.has(name)))) {
    fail("host_gate_step_evidence_unbound", "A Native host journey step names absent evidence.")
  }
  if (gate.steps.some((step) => step.evidence.length === 0)) {
    fail("host_gate_step_evidence_empty", "Every Native host journey step must bind evidence.")
  }
  const evidenceRoot = resolve(expected.evidenceRoot)
  for (const evidence of gate.evidence) {
    if (evidence.name !== basename(evidence.name) || evidence.name.includes("..") || evidence.name.includes("/") || evidence.name.includes("\\")) {
      fail("host_gate_evidence_path_invalid", "Native host evidence names must be safe basenames.")
    }
    const evidencePath = resolve(evidenceRoot, evidence.name)
    if (dirname(evidencePath) !== evidenceRoot) fail("host_gate_evidence_path_invalid", "Native host evidence escaped its run root.")
    let bytes: Buffer
    try {
      bytes = readFileSync(evidencePath)
    } catch {
      return fail("host_gate_evidence_missing", `Native host evidence is unavailable: ${evidence.name}.`)
    }
    if (bytes.length !== evidence.bytes || sha256Bytes(bytes) !== evidence.digest) {
      fail("host_gate_evidence_mismatch", `Native host evidence differs from the host report: ${evidence.name}.`)
    }
  }
  const allDigests = [
    gate.inputs.commandDigest,
    gate.inputs.binaryDigest,
    gate.inputs.sidecarBundleDigest,
    gate.inputs.frontendDigest,
    gate.inputs.sourceDigest,
    ...Object.values(gate.assurance),
    ...gate.evidence.map((entry) => entry.digest),
  ]
  if (!allDigests.every((digest) => digestPattern.test(digest))) {
    fail("host_gate_digest_invalid", "Native host gate contains a malformed digest.")
  }
  const { initial, restarted } = gate.processes
  if (
    !Number.isSafeInteger(initial.pid) || initial.pid <= 0 || initial.pid !== initial.publisherPid ||
    !Number.isSafeInteger(restarted.pid) || restarted.pid <= 0 || restarted.pid !== restarted.publisherPid ||
    initial.pid === restarted.pid || initial.forcedKill || restarted.forcedKill ||
    gate.sidecars.initial.pid === gate.sidecars.restarted.pid ||
    gate.sidecars.initial.pid === initial.pid || gate.sidecars.restarted.pid === restarted.pid ||
    gate.evidence.some((entry) => !Number.isSafeInteger(entry.bytes) || entry.bytes <= 0)
  ) {
    fail("host_gate_process_or_size_invalid", "Native host process generations or evidence sizes are invalid.")
  }
  if (
    processIsLive(initial.publisherPid) || processIsLive(restarted.publisherPid) ||
    processIsLive(gate.sidecars.initial.pid) || processIsLive(gate.sidecars.restarted.pid)
  ) {
    fail("host_gate_publisher_live", "A reported Native SDK host publisher remains live after the gate.")
  }
  let observed: ReturnType<typeof observeNativeSdkHostGateInputs>
  try {
    observed = observeNativeSdkHostGateInputs(expected.workspaceRoot)
  } catch {
    return fail("host_gate_input_unavailable", "Native host gate input artifacts could not be re-read.")
  }
  if (
    observed.binaryDigest !== gate.inputs.binaryDigest ||
    observed.sidecarBundleDigest !== gate.inputs.sidecarBundleDigest ||
    observed.frontendDigest !== gate.inputs.frontendDigest ||
    observed.sourceDigest !== gate.inputs.sourceDigest
  ) {
    fail("host_gate_input_mismatch", "Native host binary, frontend, or source bytes differ from the host report.")
  }
  exactBinding(gate, expected)
  return gate
}

export const normalizeNativeSdkHostGate = (
  bytes: string,
  expected: NativeSdkHostGateExpectedBinding,
): NativeSdkHostGateNormalization => {
  try {
    const gate = decodeAndBindNativeSdkHostGate(bytes, expected)
    const receipt: NativeSdkHostGateReceipt = {
      native_sdk_host_gate_receipt_format_version: "0.1",
      adapter_ref: OPENAGENTS_NATIVE_SDK_HOST_GATE_ADAPTER_REF,
      adapter_version: OPENAGENTS_NATIVE_SDK_HOST_GATE_ADAPTER_VERSION,
      target_ref: OPENAGENTS_NATIVE_SDK_TARGET_REF,
      environment_ref: expected.environmentRef,
      manifest_digest: expected.manifestDigest,
      environment_digest: expected.environmentDigest,
      adapter_lock_digest: expected.adapterLockDigest,
      target_descriptor_digest: expected.targetDescriptorDigest,
      target_source_digest: expected.targetSourceDigest,
      native_report_ref: expected.nativeReportRef,
      native_report_digest: sha256Digest(bytes),
      command_digest: gate.inputs.commandDigest,
      binary_digest: gate.inputs.binaryDigest,
      sidecar_bundle_digest: gate.inputs.sidecarBundleDigest,
      frontend_digest: gate.inputs.frontendDigest,
      source_digest: gate.inputs.sourceDigest,
      evidence_digest: canonicalArtifact(gate.evidence).digest,
      runtime: gate.runtime,
      process_generations: 2,
      verdict: "green",
      public_safety: { classification: "reviewed_public_safe", contains_raw_output: false },
    }
    const artifact = canonicalArtifact(receipt)
    return { status: "ready", gate, receipt, receiptBytes: artifact.bytes, receiptDigest: artifact.digest }
  } catch (error) {
    const known = error instanceof NativeSdkAssuranceAdapterError
      ? error
      : new NativeSdkAssuranceAdapterError("host_gate_unavailable", "Native host gate could not be normalized.")
    return { status: "inconclusive", code: known.code, message: known.message }
  }
}

export const executeNativeSdkCriterionUnit = (input: Readonly<{
  workspaceRoot: string
  runRoot: string
  manifest: AssuranceManifest
  manifestDigest: string
  environment: AssuranceEnvironmentProfileDocument
  unit: AssuranceExecutionUnit
  producerRef: string
  reviewerRef: string
  sourceDigest: string
  vitePlusExecutable?: string
}>): VitePlusTestAdapterResult => {
  for (const capability of ["vite_plus_test", "junit", "native_sdk_criterion_catalog"]) {
    if (!input.environment.capabilities.includes(capability)) {
      fail("missing_environment_capability", `Native SDK environment lacks ${capability}.`)
    }
  }
  return executeVitePlusTestUnitWithIdentity(input, {
    adapterRef: OPENAGENTS_NATIVE_SDK_ASSURANCE_ADAPTER_REF,
    adapterVersion: OPENAGENTS_NATIVE_SDK_ASSURANCE_ADAPTER_VERSION,
  })
}
