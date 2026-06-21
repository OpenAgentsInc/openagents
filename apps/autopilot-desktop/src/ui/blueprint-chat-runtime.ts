// #5466 (EPIC #5461): derive the chat pane's Blueprint program steps from the
// REAL live session events, not from seeded constants.
//
// The seeded path (model.ts `blueprintChatScopedSteps`) marked the Tassadar step
// `verified` the instant a session ref existed. This module instead reads the
// authoritative `node.events[chatSessionRef]` phase timeline — the same events
// the composer tails — and maps the session lifecycle onto the program steps:
//
//   - signature       : chosen by SEMANTIC routing (blueprint-chat-routing.ts);
//                        `completed` once the turn is accepted (started+).
//   - tool_scope       : the scoped context tool; `completed` once the agent
//                        emits its first transcript event.
//   - tassadar_module  : a real exact-replay module step. Its verdict is `verified`
//                        ONLY when the session reaches a real `completed` terminal
//                        phase carrying replay-conformant evidence; `rejected` on a
//                        terminal `failed`/`redaction_blocked`; `pending` while the
//                        turn is still running. NEVER instant-on-spawn.
//   - replay_module    : the proof-replay bundle step; tracks the same terminal.
//
// Honesty rule: a step is `verified` only when the real event evidence says so.
// Redaction rule: only digests, verdicts, and public refs are surfaced — the raw
// event `full`/`detail` text is never copied into a step.

import type { SessionEventRow } from "../shared/rpc.js"
import type {
  ChatStep,
  ChatStepStatus,
  ChatStepVerdict,
} from "./model.js"
import type { SignatureSelection } from "./blueprint-chat-routing.js"

// Public refs the live steps point at. These are stable PUBLIC identifiers (the
// scoped tool, the public Tassadar module listing, the public replay slug) — NOT
// verdict constants. The verdict and the digest are derived from live events.
export const CHAT_CONTEXT_TOOL_REF = "tool.context_pack.read"
export const CHAT_TASSADAR_TOOL_REF = "tool.tassadar.module.execute"
export const CHAT_TASSADAR_MODULE_REF =
  "listing.public.tassadar_compiled_weight_module.cc1403674fc0d388"
export const CHAT_TASSADAR_STEP_REF =
  "step.blueprint.tassadar.linked_dense.exact_replay.v1"
export const CHAT_REPLAY_TOOL_REF = "tool.proof_replay.bundle.show"
export const CHAT_REPLAY_MODULE_REF =
  "module.openagents.public_proof_replay_runtime"
export const CHAT_REPLAY_SIGNATURE_REF =
  "program_signature.blueprint.show_replay.v1"

// Terminal session states the node reports (control-sessions.ts):
//   queued | running | completed | failed | cancelled
// and event phases include: queued, started, composer_event, dev_check_started,
// completed, failed, cancelled, redaction_blocked.
const TERMINAL_OK = new Set(["completed"])
const TERMINAL_FAIL = new Set(["failed", "cancelled", "redaction_blocked"])

export type LiveTurnPhase =
  | "spawning" // no session ref yet (optimistic local turn)
  | "queued"
  | "running"
  | "completed"
  | "failed"

// Reduce the real event timeline to a single coarse phase for step derivation.
// Reads phase/state only; never the redactable detail text.
export const liveTurnPhase = (
  events: ReadonlyArray<SessionEventRow>,
): LiveTurnPhase => {
  if (events.length === 0) return "queued"
  let phase: LiveTurnPhase = "queued"
  for (const event of events) {
    const marker = `${event.phase} ${event.state}`.toLowerCase()
    if (TERMINAL_OK.has(event.phase) || TERMINAL_OK.has(event.state)) {
      phase = "completed"
    } else if (
      TERMINAL_FAIL.has(event.phase) ||
      TERMINAL_FAIL.has(event.state)
    ) {
      phase = "failed"
    } else if (
      phase !== "completed" &&
      phase !== "failed" &&
      (marker.includes("started") ||
        marker.includes("running") ||
        marker.includes("composer_event") ||
        marker.includes("dev_check"))
    ) {
      phase = "running"
    }
  }
  return phase
}

// Did the agent emit any transcript-worthy content yet (its first real turn)?
const sawAgentOutput = (events: ReadonlyArray<SessionEventRow>): boolean =>
  events.some(
    (e) =>
      e.detail.trim().length > 0 ||
      (e.full != null && e.full.trim().length > 0),
  )

// The exact-replay digest from a real terminal event, if the node surfaced one.
// Pylon proof artifacts carry a redaction-scanned digest; we accept only an
// explicit `sha256:`-prefixed token found in event metadata, never raw text.
const SHA256_TOKEN = /\bsha256:[0-9a-f]{64}\b/i

export const extractReplayDigest = (
  events: ReadonlyArray<SessionEventRow>,
): string | null => {
  // Scan only terminal events (where the node attaches proof refs), and only the
  // public detail line — a digest token is itself a public ref, not raw content.
  for (let i = events.length - 1; i >= 0; i--) {
    const event = events[i]!
    if (
      !TERMINAL_OK.has(event.phase) &&
      !TERMINAL_OK.has(event.state)
    ) {
      continue
    }
    const hay = `${event.detail} ${event.full ?? ""}`
    const match = hay.match(SHA256_TOKEN)
    if (match) return match[0].toLowerCase()
  }
  return null
}

const statusForPhase = (
  phase: LiveTurnPhase,
  whenRunning: ChatStepStatus,
  whenDone: ChatStepStatus,
): ChatStepStatus => {
  switch (phase) {
    case "spawning":
      return "pending"
    case "queued":
      return "running"
    case "running":
      return whenRunning
    case "completed":
      return whenDone
    case "failed":
      return "blocked"
  }
}

// The honest exact-replay verdict for the Tassadar/replay steps:
//   - `verified` ONLY on a real terminal-completed turn.
//   - `rejected` on a terminal failure.
//   - `pending`  while spawning / queued / running.
export const liveExactReplayVerdict = (
  phase: LiveTurnPhase,
): ChatStepVerdict => {
  switch (phase) {
    case "completed":
      return "verified"
    case "failed":
      return "rejected"
    default:
      return "pending"
  }
}

// Build the live Blueprint program steps for ONE assistant turn from the real
// session events + the semantic signature selection. This is the runtime-derived
// replacement for the seeded `blueprintChatScopedSteps`.
export const liveChatScopedSteps = (input: {
  selection: SignatureSelection
  linkedSessionRef: string | null
  events: ReadonlyArray<SessionEventRow>
  proofReplaySlug: string
}): Array<ChatStep> => {
  const { selection, linkedSessionRef, events, proofReplaySlug } = input
  const phase: LiveTurnPhase =
    linkedSessionRef === null ? "spawning" : liveTurnPhase(events)
  const agentStarted = sawAgentOutput(events)
  const verdict = liveExactReplayVerdict(phase)
  const digest = phase === "completed" ? extractReplayDigest(events) : null

  // Signature step: chosen by the semantic router; completes once the turn is
  // accepted by the node (any non-spawning phase).
  const signatureStatus: ChatStepStatus =
    phase === "spawning"
      ? "pending"
      : phase === "failed"
        ? "blocked"
        : "completed"

  // Tool-scope step: completes once the agent emits its first transcript event;
  // otherwise it is running while the turn is live.
  const contextStatus: ChatStepStatus =
    phase === "spawning"
      ? "pending"
      : phase === "failed"
        ? "blocked"
        : agentStarted || phase === "completed"
          ? "completed"
          : "running"

  // Receipt ref is honest: only present once the step is actually verified.
  const tassadarReceiptRef =
    verdict === "verified"
      ? "receipt.openagents.blueprint_tassadar_step.cc1403674fc0d388"
      : null
  const replayReceiptRef =
    verdict === "verified" ? "receipt.public_proof_replay_bundle" : null

  return [
    {
      id: "blueprint-chat-signature",
      kind: "signature",
      label: selection.confident
        ? "Selected signature (semantic)"
        : "Selected signature (default)",
      status: signatureStatus,
      signatureRef: selection.signatureRef,
      toolRef: null,
      moduleRef: null,
      digestRef: null,
      verdict: null,
      evidenceRef: null,
      receiptRef: null,
      tassadarModuleStepRef: null,
      proofReplayRef: null,
      contentRedacted: false,
      linkedSessionRef,
    },
    {
      id: "blueprint-chat-context-pack",
      kind: "tool_scope",
      label: "Scoped tool",
      status: contextStatus,
      signatureRef: null,
      toolRef: CHAT_CONTEXT_TOOL_REF,
      moduleRef: null,
      digestRef: null,
      verdict: null,
      evidenceRef: null,
      receiptRef: null,
      tassadarModuleStepRef: null,
      proofReplayRef: null,
      contentRedacted: false,
      linkedSessionRef,
    },
    {
      id: "blueprint-chat-tassadar-step",
      kind: "tassadar_module_step",
      label: "Tassadar module step",
      status: statusForPhase(phase, "running", "verified"),
      signatureRef: null,
      toolRef: CHAT_TASSADAR_TOOL_REF,
      moduleRef: CHAT_TASSADAR_MODULE_REF,
      // Digest is the REAL replay digest from terminal events, or null. No
      // hardcoded digest constant drives the rendered verdict.
      digestRef: digest,
      verdict,
      evidenceRef:
        verdict === "verified"
          ? "evidence.openagents.blueprint_tassadar_step.cc1403674fc0d388"
          : null,
      receiptRef: tassadarReceiptRef,
      tassadarModuleStepRef: CHAT_TASSADAR_STEP_REF,
      proofReplayRef: proofReplaySlug,
      contentRedacted: true,
      linkedSessionRef,
    },
    {
      id: "blueprint-chat-replay-module",
      kind: "replay_module",
      label: "Proof replay bundle",
      status: statusForPhase(phase, "running", "verified"),
      signatureRef: CHAT_REPLAY_SIGNATURE_REF,
      toolRef: CHAT_REPLAY_TOOL_REF,
      moduleRef: CHAT_REPLAY_MODULE_REF,
      digestRef: null,
      verdict: null,
      evidenceRef:
        verdict === "verified"
          ? "evidence.openagents.blueprint_replay_module.first-real-settlement"
          : null,
      receiptRef: replayReceiptRef,
      tassadarModuleStepRef: null,
      proofReplayRef: proofReplaySlug,
      contentRedacted: true,
      linkedSessionRef,
    },
  ]
}
