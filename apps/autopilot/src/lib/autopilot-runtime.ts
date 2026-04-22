import { invoke } from "@tauri-apps/api/core";

export type ProviderMode = "online" | "offline" | "pause" | "resume";
export type ProofLane =
  | "cs336-a1"
  | "cs336-a1-stale-recovery"
  | "cs336-a1-replacement-attempt";

export interface AutopilotStatus {
  product: string;
  shell: string;
  rustAuthority: string;
  runtimeLane: string;
}

export interface WorkbenchWorkspace {
  id: string;
  name: string;
  path: string;
  branch: string;
  trust: string;
  policy: string;
}

export interface WorkbenchSession {
  id: string;
  title: string;
  goal: string;
  state: string;
  permissionMode: string;
  resumeState: string;
  engine: string;
}

export interface WorkbenchTimelineEvent {
  id: string;
  time: string;
  state: string;
  label: string;
  detail: string;
  owner: string;
  evidence: string;
}

export interface WorkbenchApproval {
  id: string;
  state: string;
  risk: string;
  request: string;
  policy: string;
  paths: string[];
}

export interface WorkbenchDiff {
  id: string;
  state: string;
  file: string;
  summary: string;
  additions: number;
  deletions: number;
}

export interface WorkbenchVerification {
  id: string;
  state: string;
  command: string;
  elapsedMs: number;
  detail: string;
}

export interface WorkbenchEvidence {
  id: string;
  kind: string;
  state: string;
  location: string;
  owner: string;
}

export interface WorkbenchScorecard {
  firstToolEventSeconds: number;
  verifiedDiffMinutes: number;
  recoveryState: string;
  humanInterventions: number;
  satsEarnedToday: number;
}

export interface AutopilotWorkbenchSnapshot {
  product: string;
  visibleSurface: string;
  generatedAtUnixMs: number;
  workspace: WorkbenchWorkspace;
  session: WorkbenchSession;
  timeline: WorkbenchTimelineEvent[];
  approvals: WorkbenchApproval[];
  diffs: WorkbenchDiff[];
  verification: WorkbenchVerification[];
  evidence: WorkbenchEvidence[];
  scorecard: WorkbenchScorecard;
}

export interface PylonBinaryStatus {
  installed: boolean;
  binaryName: string;
  binaryPath: string | null;
  source: string;
  detail: string | null;
}

export interface PylonStatusProjection {
  installed: boolean;
  configured: boolean;
  processState: string;
  providerState: string;
  desiredMode: string | null;
  pid: number | null;
  listenAddr: string | null;
  binaryPath: string | null;
  configPath: string | null;
  pylonHome: string | null;
  executionBackend: string | null;
  readyModel: string | null;
  productsVisible: number | null;
  productsEligible: number | null;
  queueDepth: number | null;
  uptimeSeconds: number | null;
  blockerCodes: string[];
  lastAction: string | null;
  lastError: string | null;
  lastExitCode: number | null;
  lastUpdatedAt: string;
}

export interface ProofNodeProjection {
  role: string;
  index: number;
  label: string;
  running: boolean;
  pid: number | null;
  eligibility: string | null;
  hardGateReasons: string[];
  retainedStateFixtureId: string | null;
  trainingStatus: string | null;
  trainingError: string | null;
}

export interface ProofTransportProjection {
  authority: string;
  relay: string;
  artifactStore: string;
  nodeSurfaces: string;
}

export interface ProofFailedAuthorityWrite {
  source: string;
  method: string | null;
  url: string | null;
  status: number | null;
  responseBody: string | null;
  detail: string;
}

export interface ProofArtifactsProjection {
  root: string;
  runReportPath: string | null;
  authorityTracePath: string | null;
  summaryPath: string | null;
  artifactTracePath: string | null;
}

export interface ProofRunProjection {
  namespace: string;
  lane: string;
  status: string;
  firstRedStage: string | null;
  firstRedSubject: string | null;
  blockerId: string | null;
  detail: string | null;
  runId: string | null;
  windowId: string | null;
  assignmentId: string | null;
  leaseId: string | null;
  membershipRevision: string | null;
  closeoutStage: string | null;
  closeoutNextAction: string | null;
  closeoutLastError: string | null;
  workers: ProofNodeProjection[];
  validators: ProofNodeProjection[];
  transport: ProofTransportProjection;
  artifacts: ProofArtifactsProjection;
  firstFailedAuthorityWrite: ProofFailedAuthorityWrite | null;
  localSimulation: boolean;
  simulatedTreasury: boolean;
  updatedAt: string;
}

export interface ProofRuntimeProjection {
  namespace: string;
  status: string;
  detail: string;
  artifacts: ProofArtifactsProjection;
  updatedAt: string;
}

export interface ProofRunOptions {
  lane: ProofLane;
  namespace?: string;
  workers?: number;
  validators?: number;
  timeoutSeconds?: number;
}

export function autopilotStatus() {
  return invoke<AutopilotStatus>("autopilot_status");
}

export function autopilotWorkbenchSnapshot() {
  return invoke<AutopilotWorkbenchSnapshot>("autopilot_workbench_snapshot");
}

export function pylonDetect() {
  return invoke<PylonBinaryStatus>("pylon_detect");
}

export function pylonGetStatus() {
  return invoke<PylonStatusProjection>("pylon_get_status");
}

export function pylonStart() {
  return invoke<PylonStatusProjection>("pylon_start");
}

export function pylonStop() {
  return invoke<PylonStatusProjection>("pylon_stop");
}

export function pylonRestart() {
  return invoke<PylonStatusProjection>("pylon_restart");
}

export function pylonSetMode(mode: ProviderMode) {
  return invoke<PylonStatusProjection>("pylon_set_mode", { mode });
}

export function pylonOpenLogs() {
  return invoke<string>("pylon_open_logs");
}

export function proofRun(options: ProofRunOptions) {
  return invoke<ProofRunProjection>("proof_run", { options });
}

export function proofGet(namespace: string) {
  return invoke<ProofRunProjection>("proof_get", { namespace });
}

export function proofDoctor(namespace: string) {
  return invoke<ProofRunProjection>("proof_doctor", { namespace });
}

export function proofStop(namespace: string) {
  return invoke<ProofRuntimeProjection>("proof_stop", { namespace });
}

export function proofReset(namespace: string) {
  return invoke<ProofRuntimeProjection>("proof_reset", { namespace });
}

export function proofOpenArtifacts(namespace: string) {
  return invoke<string>("proof_open_artifacts", { namespace });
}
