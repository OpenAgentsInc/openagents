import type { BehaviorContract } from "./contract"

const authority = "Voice data carries no independent command, outcome, authentication, storage, Sync, or runtime authority. Raw media never enters Runtime Gateway projections, Khala Sync, logs, analytics, traces, or support bundles; Rust media code owns no transcript, command, Sync, or retention-policy schema."
const source = { channel: "owner-codex-session", statedBy: "owner", statedOn: "2026-07-12" } as const
const enforced = (contractId: string, statement: string, ref: string): BehaviorContract => ({
  authorityBoundary: authority, blockerRefs: [], contractId, enforcementTier: "test-sweep",
  evidenceRefs: ["packages/audio-contract/README.md", "github:OpenAgentsInc/openagents#8734"],
  oracles: [{ description: "Executable bounded audio contract/model regression.", id: `${contractId}.model`, kind: "bun-test", mode: "unit", ref }],
  productArea: "persistent voice", source, state: "enforced", statement,
  surface: "openagents-desktop", verification: "pnpm run test:audio-contract runs the canonical Effect Schema and lifecycle model regressions; Rust consumes the same media accept/reject corpus.",
})
const pending = (contractId: string, statement: string, issue: number): BehaviorContract => ({
  authorityBoundary: authority, blockerRefs: [`github:OpenAgentsInc/openagents#${issue}`], contractId, enforcementTier: "unenforced",
  evidenceRefs: ["packages/audio-contract/README.md", `github:OpenAgentsInc/openagents#${issue}`],
  oracles: [{ description: "Planned Desktop UI/action oracle.", id: `${contractId}.planned`, kind: "planned", mode: "dom", ref: `github:OpenAgentsInc/openagents#${issue}` }],
  productArea: "persistent voice", source, state: "pending", statement,
  surface: "openagents-desktop", verification: `Pending #${issue}.`,
})

export const audioBehaviorContracts: readonly BehaviorContract[] = [
  enforced("openagents_voice.capture_egress_retention_truth.v1", "Persistent voice separately reports microphone capture, network egress, and raw-audio retention from authoritative lifecycle/policy state; one state never implies another, and restart never silently claims or resumes capture.", "packages/audio-contract/src/lifecycle-model.test.ts"),
  enforced("openagents_voice.mute_and_stop_fail_closed.v1", "Mute immediately stops new microphone egress while preserving an explicitly muted session; stop or revoke fences the active generation, and restart stays stopped until a new disclosed start.", "packages/audio-contract/src/lifecycle-model.test.ts"),
  enforced("openagents_voice.retention_requires_policy_receipt.v1", "Raw audio is never retained without a matching disclosure and retention-policy receipt for the exact owner/device/thread/session/generation.", "packages/audio-contract/src/lifecycle-model.test.ts"),
  enforced("openagents_voice.command_outcome_receipt_authority.v1", "ASR, transcripts, assistant/model prose, and TTS never propose, confirm, execute, or complete a command without the typed action path and durable outcome ref.", "packages/audio-contract/src/authority.test.ts"),
  enforced("openagents_voice.replay_delivery_only.v1", "Reconnect replay redelivers already-produced frames only and never reruns side effects or final publication.", "packages/audio-contract/src/lifecycle-model.test.ts"),
  pending("openagents_voice.visible_text_fallback_and_media_failure.v1", "Canonical text remains visible through listening, speaking, muted, reconnecting, degraded, and media-failure states.", 8738),
  pending("openagents_voice.explicit_command_confirmation.v1", "Risky or ambiguous voice-derived actions use the same explicit typed confirmation and durable outcome path as text; speech and playback never count as confirmation.", 8739),
]
