// Public, OSS, BYO-model programmatic surface for @openagentsinc/qa-runner.
//
// This barrel exposes the local-first, runtime-agnostic core required by issue
// #6191: build a target, bring any OpenAI-compatible model, drive an autonomous
// session against a local backend, and distill it into a committed e2e test —
// with NO OpenAgents login. The Khala / Cloud / receipt / settlement modules are
// deliberately NOT re-exported here; they are optional add-ons, importable by
// their own paths if you opt in, never part of the core surface.

// Bring-your-own-model config + client (the OSS credential path).
export {
  resolveByoModelConfig,
  makeByoChatClient,
  ByoModelConfigError,
  type ByoModelConfig,
  type ByoModelFlags,
  type ResolveByoModelOptions,
} from "./byo-model";

// The CLI command (also runnable as the `qa` bin).
export { runCommand } from "./byo";

// Targets — a deployment seen from outside; swap baseUrl for dev/prod.
export {
  makeTarget,
  resolveTarget,
  isReadOnly,
  checkStepAllowed,
  type Target,
  type TargetCapability,
  type TargetRestriction,
} from "./target";

// Brains — the pluggable decision-makers (scripted is deterministic-now).
export {
  scriptedBrain,
  type Brain,
  type BrainContext,
  type BrainStep,
} from "./brain";

// Backends — the isolation abstraction; localBackend is the OSS default.
export { localBackend, type Backend, type BackendSession, type LocalBackendOptions } from "./backend";

// The fixed-step runner (runQaSession) and the autonomous model-driven runner.
export { runQaSession, type RunInput, type RunOutcome } from "./runner";
export { runKhalaSession, type KhalaSessionInput, type KhalaSessionOutcome } from "./khala-session";

// The minimal chat-client contract any BYO model must satisfy.
export type { ChatClient, ChatMessage } from "./khala-driver";

// The distiller — lower a recorded session into a committed e2e test.
export { distill, assessCandidate, type DistillResult, type CandidateAssessment } from "./distiller";

// The public-safe run result schema (also imported by the /pro read model).
export { type QaRunResult, type QaRunStep } from "./result";
