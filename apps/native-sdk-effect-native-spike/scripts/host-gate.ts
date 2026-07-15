import { Schema } from "@effect-native/core/effect";

export const nativeSdkHostGateFormat = "openagents.native-sdk.host-gate.v5" as const;
export const nativeSdkTargetRef = "openagents.desktop.native-sdk.mvp" as const;
export const nativeSdkCommit = "f7aa92af6dcece250feba852af4d22e7f5429312" as const;
export const nativeSdkAutomationProtocol = 7 as const;
export const nativeSdkHostGateSteps = [
  "initial-projection",
  "runtime-sidecar-bootstrap",
  "repository-grant-admitted",
  "composited-window-capture",
  "session-selection",
  "workspace-round-trip",
  "native-canvas-screenshot",
  "renderer-reload-restored",
  "process-restart-restored",
  "repository-identity-restored",
  "new-chat-after-restart",
  "clean-teardown",
] as const;

const DigestSchema = Schema.String;
const PositiveIntegerSchema = Schema.Number;
const CodingRefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);
const AdmissionSchema = Schema.Struct({
  grantRef: CodingRefSchema,
  projectRef: CodingRefSchema,
  repositoryRef: CodingRefSchema,
  worktreeRef: CodingRefSchema,
  workContextRef: CodingRefSchema,
  sessionRef: CodingRefSchema,
});

export const NativeSdkHostGateSchema = Schema.Struct({
  formatVersion: Schema.Literal(nativeSdkHostGateFormat),
  targetRef: Schema.Literal(nativeSdkTargetRef),
  runNonce: Schema.String,
  automationProtocol: Schema.Literal(nativeSdkAutomationProtocol),
  frontendAuthority: Schema.Literal("effect-native"),
  result: Schema.Literal("passed"),
  runtime: Schema.Struct({
    os: Schema.Literal("darwin"),
    architecture: Schema.Literal("arm64"),
    node: Schema.Literal("24.13.1"),
    zig: Schema.Literal("0.16.0"),
    nativeSdkCommit: Schema.Literal(nativeSdkCommit),
  }),
  inputs: Schema.Struct({
    commandDigest: DigestSchema,
    binaryDigest: DigestSchema,
    sidecarBundleDigest: DigestSchema,
    frontendDigest: DigestSchema,
    sourceDigest: DigestSchema,
  }),
  assurance: Schema.NullOr(
    Schema.Struct({
      manifestDigest: DigestSchema,
      environmentDigest: DigestSchema,
      adapterLockDigest: DigestSchema,
      targetDescriptorDigest: DigestSchema,
      targetSourceDigest: DigestSchema,
    }),
  ),
  processes: Schema.Struct({
    initial: Schema.Struct({
      pid: PositiveIntegerSchema,
      publisherPid: PositiveIntegerSchema,
      stopped: Schema.Literal(true),
      exitCode: Schema.NullOr(Schema.Number),
      signal: Schema.NullOr(Schema.String),
      forcedKill: Schema.Boolean,
    }),
    restarted: Schema.Struct({
      pid: PositiveIntegerSchema,
      publisherPid: PositiveIntegerSchema,
      stopped: Schema.Literal(true),
      exitCode: Schema.NullOr(Schema.Number),
      signal: Schema.NullOr(Schema.String),
      forcedKill: Schema.Boolean,
    }),
  }),
  sidecars: Schema.Struct({
    initial: Schema.Struct({
      pid: PositiveIntegerSchema,
      generation: Schema.Literal(1),
      liveDuringHost: Schema.Literal(true),
      liveAfterHost: Schema.Literal(false),
    }),
    restarted: Schema.Struct({
      pid: PositiveIntegerSchema,
      generation: Schema.Literal(2),
      liveDuringHost: Schema.Literal(true),
      liveAfterHost: Schema.Literal(false),
    }),
  }),
  criterionObservations: Schema.Struct({
    schema: Schema.Literal("openagents.native-sdk.cw-ac-03.v1"),
    criterionRef: Schema.Literal("CW-AC-03"),
    grantSource: Schema.Literal("native_canvas_file_drop"),
    initial: Schema.Struct({
      generation: Schema.Literal(1),
      catalogSessionCount: Schema.Literal(1),
      admission: AdmissionSchema,
    }),
    restarted: Schema.Struct({
      generation: Schema.Literal(2),
      catalogSessionCount: Schema.Literal(1),
      admission: AdmissionSchema,
    }),
    aliasCanonicalized: Schema.Literal(true),
    ambientInputsExcluded: Schema.Literal(true),
    ambientFalsifiers: Schema.Struct({
      initial: Schema.Struct({
        hostname: CodingRefSchema,
        port: CodingRefSchema,
        providerThread: CodingRefSchema,
      }),
      restarted: Schema.Struct({
        hostname: CodingRefSchema,
        port: CodingRefSchema,
        providerThread: CodingRefSchema,
      }),
    }),
    privateBindingMode: Schema.Literal("0600"),
  }),
  steps: Schema.Array(
    Schema.Struct({
      id: Schema.String,
      result: Schema.Literal("passed"),
      evidence: Schema.Array(Schema.String),
    }),
  ),
  evidence: Schema.Array(
    Schema.Struct({
      name: Schema.String,
      digest: DigestSchema,
      bytes: PositiveIntegerSchema,
    }),
  ),
});

export type NativeSdkHostGate = typeof NativeSdkHostGateSchema.Type;
const decodeSchema = Schema.decodeUnknownSync(NativeSdkHostGateSchema);
const digestPattern = /^sha256:[a-f0-9]{64}$/u;

/** Decode the private headed-gate artifact and reject structurally green but incomplete evidence. */
export const decodeNativeSdkHostGate = (candidate: unknown): NativeSdkHostGate => {
  const gate = decodeSchema(candidate, { onExcessProperty: "error" });
  if (
    !/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/u.test(gate.runNonce)
  ) {
    throw new Error("native_host_gate_nonce_invalid");
  }
  const stepIds = gate.steps.map((step) => step.id);
  if (JSON.stringify(stepIds) !== JSON.stringify(nativeSdkHostGateSteps)) {
    throw new Error("native_host_gate_steps_incomplete");
  }
  if (new Set(gate.evidence.map((entry) => entry.name)).size !== gate.evidence.length) {
    throw new Error("native_host_gate_evidence_duplicate");
  }
  const requiredEvidence = [
    "01-composited-window.png",
    "02-cw-ac-03-initial.json",
    "03-native-shell.png",
    "04-renderer-reload.snapshot.txt",
    "05-process-restart.snapshot.txt",
    "06-cw-ac-03-restarted.json",
  ];
  if (!requiredEvidence.every((name) => gate.evidence.some((entry) => entry.name === name))) {
    throw new Error("native_host_gate_evidence_incomplete");
  }
  const evidenceNames = new Set(gate.evidence.map((entry) => entry.name));
  if (gate.steps.some((step) => step.evidence.some((name) => !evidenceNames.has(name)))) {
    throw new Error("native_host_gate_step_evidence_unbound");
  }
  if (gate.steps.some((step) => step.evidence.length === 0)) {
    throw new Error("native_host_gate_step_evidence_empty");
  }
  const digests = [
    gate.inputs.commandDigest,
    gate.inputs.binaryDigest,
    gate.inputs.sidecarBundleDigest,
    gate.inputs.frontendDigest,
    gate.inputs.sourceDigest,
    ...(gate.assurance === null
      ? []
      : [
          gate.assurance.manifestDigest,
          gate.assurance.environmentDigest,
          gate.assurance.adapterLockDigest,
          gate.assurance.targetDescriptorDigest,
          gate.assurance.targetSourceDigest,
        ]),
    ...gate.evidence.map((entry) => entry.digest),
  ];
  if (!digests.every((digest) => digestPattern.test(digest)))
    throw new Error("native_host_gate_digest_invalid");
  if (
    !Number.isSafeInteger(gate.processes.initial.pid) ||
    !Number.isSafeInteger(gate.processes.restarted.pid) ||
    gate.processes.initial.pid <= 0 ||
    gate.processes.restarted.pid <= 0 ||
    gate.processes.initial.pid !== gate.processes.initial.publisherPid ||
    gate.processes.restarted.pid !== gate.processes.restarted.publisherPid ||
    gate.processes.initial.pid === gate.processes.restarted.pid ||
    gate.sidecars.initial.pid === gate.sidecars.restarted.pid ||
    gate.sidecars.initial.pid === gate.processes.initial.pid ||
    gate.sidecars.restarted.pid === gate.processes.restarted.pid ||
    gate.processes.initial.forcedKill ||
    gate.processes.restarted.forcedKill ||
    gate.evidence.some((entry) => !Number.isSafeInteger(entry.bytes) || entry.bytes <= 0)
  ) {
    throw new Error("native_host_gate_process_or_size_invalid");
  }
  const initialAdmission = JSON.stringify(gate.criterionObservations.initial.admission);
  const restartedAdmission = JSON.stringify(gate.criterionObservations.restarted.admission);
  if (initialAdmission !== restartedAdmission) {
    throw new Error("native_host_gate_work_identity_drift");
  }
  const falsifiers = Object.values(gate.criterionObservations.ambientFalsifiers).flatMap(
    Object.values,
  );
  if (
    JSON.stringify(gate.criterionObservations.ambientFalsifiers.initial) ===
      JSON.stringify(gate.criterionObservations.ambientFalsifiers.restarted) ||
    falsifiers.some((value) => initialAdmission.includes(value))
  ) {
    throw new Error("native_host_gate_ambient_identity_derived");
  }
  return gate;
};
