/**
 * The public Loupe verification surface.
 *
 * This entry point deliberately exposes no way to hand the verifier evidence,
 * a worker receipt, an initial verdict, or a lifecycle authority. It exposes a
 * spec, a control-plane transport and a ledger directory. That is the whole of
 * the OFR-007 fix: a caller cannot supply both the evidence and the authority
 * that validates it, because it supplies neither.
 *
 * `evaluateLoupeVerificationSession` and the session brand it checks live in
 * `./verification-contract.ts` and are not re-exported here. There is no
 * package export path that reaches them.
 */

export {
  LOUPE_CONTROL_PLANE_TRANSCRIPT_VERSION,
  LOUPE_VOLATILE_REQUEST_FIELDS,
  conformanceLoupeControlPlane,
  httpLoupeControlPlane,
  loupeControlPlaneRequestDigest,
  recordedLoupeControlPlane,
  recordingLoupeControlPlane,
  type LoupeControlPlaneExchange,
  type LoupeControlPlaneKind,
  type LoupeControlPlaneResponse,
  type LoupeControlPlaneRoute,
  type LoupeControlPlaneTranscript,
  type LoupeControlPlaneTransport,
  type RecordingLoupeControlPlane,
} from "./control-plane.ts";

export {
  LOUPE_FIRST_VERDICT_LEDGER_VERSION,
  durableFirstVerdictLedger,
  type LoupeFirstVerdictLedger,
} from "./verdict-ledger.ts";

export {
  runLoupeVerification,
  type LoupeControlVariant,
  type LoupeSandboxRunSummary,
  type LoupeTargetPin,
  type LoupeVerificationRole,
  type LoupeVerificationRun,
  type LoupeVerificationSpec,
  type RunLoupeVerificationOptions,
} from "./session.ts";

export {
  LOUPE_ADMITTED_WORKER_RECEIPT_VERSION,
  LOUPE_INITIAL_VERDICT_VERSION,
  LOUPE_VERIFICATION_EVIDENCE_VERSION,
  LOUPE_VERIFICATION_PLAN_VERSION,
  LOUPE_VERIFICATION_RELEASE_GATE_VERSION,
  LOUPE_VERIFICATION_RESULT_VERSION,
  LoupeAdmittedWorkerReceiptSchema,
  LoupeAdmittedWorkerSchema,
  LoupeControlTerminationSchema,
  LoupeEvidenceProvenance,
  LoupeInitialVerdictSchema,
  LoupeVerificationEvidenceSchema,
  LoupeVerificationPlanSchema,
  LoupeVerificationReleaseGateSchema,
  LoupeVerificationResultSchema,
  deriveControlTestOutcome,
  evaluateLoupeVerificationReleaseGate,
  loupeVerificationContract,
  type EvaluateLoupeVerificationReleaseGateInput,
  type LoupeAdmittedWorker,
  type LoupeAdmittedWorkerReceipt,
  type LoupeControlTermination,
  type LoupeInitialVerdict,
  type LoupeVerificationEvidence,
  type LoupeVerificationPlan,
  type LoupeVerificationReleaseGate,
  type LoupeVerificationResult,
} from "./verification-contract.ts";
