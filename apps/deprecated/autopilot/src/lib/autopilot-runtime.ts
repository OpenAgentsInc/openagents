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

export interface HomeworkStageProjection {
  id: string;
  label: string;
  state: string;
  detail: string;
}

export interface HomeworkAssignmentProjection {
  kind: string;
  state: string;
  trainingRunId: string | null;
  windowId: string | null;
  assignmentId: string | null;
  leaseId: string | null;
  membershipRevision: string | null;
  role: string | null;
  networkId: string | null;
  runtimeLaneId: string | null;
  runtimeOperation: string | null;
  runtimeWorkClass: string | null;
  runtimeManifestPath: string | null;
  updatedAtMs: number | null;
}

export interface HomeworkRuntimeProjection {
  trainingRunId: string;
  windowId: string;
  assignmentId: string;
  leaseId: string;
  role: string;
  desiredState: string;
  processState: string;
  pid: number | null;
  lastHeartbeatAtMs: number | null;
  lastFailureReason: string | null;
  manifestPath: string;
  runRoot: string;
  launchCount: number;
  restartCount: number;
  updatedAtMs: number;
}

export interface HomeworkCloseoutProjection {
  trainingRunId: string;
  windowId: string;
  assignmentId: string;
  role: string;
  stage: string;
  nextAction: string | null;
  challengeId: string | null;
  acceptanceState: string | null;
  acceptedOutcomeId: string | null;
  payoutState: string | null;
  payoutId: string | null;
  payoutReceiptId: string | null;
  payoutReconciliationStatus: string | null;
  lastError: string | null;
  blockingClass: string | null;
  updatedAtMs: number | null;
}

export interface HomeworkIssueProjection {
  kind: string;
  subjectId: string;
  reason: string;
  blockingClass: string | null;
  owner: string | null;
  retryable: boolean | null;
  observedAtMs: number | null;
}

export interface HomeworkTrainingProjection {
  nodeLabel: string;
  providerPubkey: string | null;
  checkpointServeUrl: string;
  runtimeSurfaceDetected: boolean;
  runtimeSurfaceError: string | null;
  contributorSupported: boolean;
  currentRunId: string | null;
  activeWindowId: string | null;
  manifestCount: number;
  workOfferCount: number;
  pendingPublicationCount: number;
  closeoutCount: number;
  validatorQueueCount: number;
  recentTrnEventCount: number;
  blockedLabelKeys: string[];
  activeRuntime: HomeworkRuntimeProjection | null;
  leasedAssignment: HomeworkAssignmentProjection | null;
  recentWorkOffers: HomeworkAssignmentProjection[];
  recentCloseoutProgress: HomeworkCloseoutProjection[];
  recentIssues: HomeworkIssueProjection[];
}

export interface HomeworkSnapshotProjection {
  assignmentLabel: string;
  status: string;
  detail: string;
  payoutPolicy: string;
  updatedAt: string;
  pylon: PylonStatusProjection;
  training: HomeworkTrainingProjection | null;
  trainingError: string | null;
  proof: ProofRunProjection | null;
  stages: HomeworkStageProjection[];
}

export interface AutopilotHealthMetric {
  label: string;
  value: string;
}

export interface AutopilotHealthSubsystem {
  id: string;
  label: string;
  state: string;
  summary: string;
  detail: string;
  metrics: AutopilotHealthMetric[];
}

export interface AutopilotHealthActiveRun {
  runId: string | null;
  windowId: string | null;
  status: string;
  detail: string;
}

export interface AutopilotHealthFollowup {
  id: string;
  severity: string;
  owner: string;
  action: string;
  detail: string;
}

export interface AutopilotHealthStopState {
  canCancel: boolean;
  state: string;
  reason: string;
}

export interface AutopilotHealthAction {
  id: string;
  state: string;
  summary: string;
  actor: string;
  observedAtUnixMs: number;
}

export interface AutopilotHealthEvent {
  id: string;
  atUnixMs: number;
  state: string;
  title: string;
  detail: string;
  evidence: string;
}

export interface AutopilotHealthPredicate {
  predicateId: string;
  severity: string;
  status: string;
  detail: string;
  remediationHint: string;
}

export interface AutopilotHealthGate {
  gateId: string;
  status: string;
  passed: boolean;
}

export interface AutopilotNexusHealthProjection {
  schemaVersion: number;
  generatedAtUnixMs: number;
  source: string;
  baseUrl: string;
  state: string;
  severity: string;
  summary: string;
  exactCause: string;
  subsystems: AutopilotHealthSubsystem[];
  activeRun: AutopilotHealthActiveRun;
  queuedFollowups: AutopilotHealthFollowup[];
  stopState: AutopilotHealthStopState;
  latestAction: AutopilotHealthAction | null;
  eventTimeline: AutopilotHealthEvent[];
  failedPredicates: AutopilotHealthPredicate[];
  verificationGates: AutopilotHealthGate[];
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

export function nexusHealthStatus() {
  return invoke<AutopilotNexusHealthProjection>("nexus_health_status");
}

export function pylonDetect() {
  return invoke<PylonBinaryStatus>("pylon_detect");
}

export function pylonGetStatus() {
  return invoke<PylonStatusProjection>("pylon_get_status");
}

export function pylonHomeworkGet() {
  return invoke<HomeworkSnapshotProjection>("pylon_homework_get");
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
