/**
 * #9161 criterion 8: derive PUBLIC-SAFE and PRIVATE evidence receipts from a
 * headless host turn, and apply the deterministic layer of the
 * conversation-coherence rubric
 * (`docs/analysis/conversation-thread-coherence-rubric.md`).
 *
 * The public receipt carries only bounded, non-sensitive facts (refs, frame
 * kind counts, disposition, token totals) — never the raw answer text, the
 * user message, local paths, or provider payloads. The private receipt keeps
 * the full ordered frames and the answer for owner-local inspection.
 */

import type { ClaudeLocalEvent } from "./claude-local-contract";
import type { HeadlessTurnResult } from "./desktop-headless-host";

export type HeadlessCoherenceDisposition = "pass" | "needs_review" | "fail";

export interface HeadlessCoherenceScreen {
  readonly disposition: HeadlessCoherenceDisposition;
  /** Deterministic tripwires that fired (rubric §"Deterministic tripwires"). */
  readonly tripwires: ReadonlyArray<string>;
}

export interface HeadlessPublicReceipt {
  readonly turnRef: string;
  readonly threadRef: string;
  readonly dispatchOk: boolean;
  readonly finishReason: "completed" | "failed" | "none";
  /** Ordered frame kinds (kinds only — never text). */
  readonly frameKinds: ReadonlyArray<string>;
  readonly frameKindCounts: Readonly<Record<string, number>>;
  readonly totalTokens: number | null;
  readonly fullAutoRecordCount: number;
  readonly coherence: HeadlessCoherenceScreen;
}

export interface HeadlessPrivateReceipt {
  readonly public: HeadlessPublicReceipt;
  /** Full ordered frames, including text — owner-local only. */
  readonly frames: ReadonlyArray<{ readonly turnRef: string; readonly event: ClaudeLocalEvent }>;
  /** The concatenated answer text — owner-local only. */
  readonly answer: string;
}

const finishReasonOf = (kinds: ReadonlyArray<string>): "completed" | "failed" | "none" => {
  if (kinds.includes("turn_failed")) return "failed";
  if (kinds.includes("turn_completed")) return "completed";
  return "none";
};

const answerTextOf = (frames: ReadonlyArray<{ readonly event: ClaudeLocalEvent }>): string => {
  let text = "";
  for (const frame of frames) {
    if (frame.event.kind === "text_delta") text += frame.event.text;
  }
  return text;
};

/**
 * The deterministic coherence layer for an ORDINARY host turn. An ordinary
 * turn must: complete with a visible answer, keep its frames ordered
 * (start before completion), and — the load-bearing #9161 invariant —
 * create NO Full Auto record.
 */
export const screenHeadlessTurn = (result: HeadlessTurnResult): HeadlessCoherenceScreen => {
  const kinds = result.frames.map((frame) => frame.event.kind);
  const tripwires: string[] = [];

  // An ordinary turn must never create Full Auto authority.
  if (result.fullAutoRecordCount > 0) tripwires.push("ordinary_turn_created_full_auto_record");

  const startIndex = kinds.indexOf("turn_started");
  const completedIndex = kinds.indexOf("turn_completed");
  const failedIndex = kinds.indexOf("turn_failed");

  if (failedIndex !== -1) {
    // A failed turn is a valid terminal; the receipt records it honestly.
    return { disposition: "fail", tripwires };
  }
  if (startIndex === -1) tripwires.push("no_turn_started_frame");
  if (completedIndex === -1) tripwires.push("no_terminal_frame");
  if (startIndex !== -1 && completedIndex !== -1 && startIndex > completedIndex) {
    tripwires.push("result_before_cause");
  }
  if (completedIndex !== -1 && answerTextOf(result.frames).trim() === "") {
    tripwires.push("completed_turn_has_no_answer");
  }
  if (!result.dispatch.ok) tripwires.push("dispatch_not_ok");

  return {
    disposition: tripwires.length === 0 ? "pass" : "needs_review",
    tripwires,
  };
};

/** Derive the public-safe receipt (no raw text, no paths). */
export const derivePublicReceipt = (
  turnRef: string,
  threadRef: string,
  result: HeadlessTurnResult,
): HeadlessPublicReceipt => {
  const frameKinds = result.frames.map((frame) => frame.event.kind);
  const frameKindCounts: Record<string, number> = {};
  for (const kind of frameKinds) frameKindCounts[kind] = (frameKindCounts[kind] ?? 0) + 1;
  const totalTokens = result.frames.reduce<number | null>((acc, frame) => {
    if (frame.event.kind === "turn_completed" && frame.event.totalTokens !== null) {
      return (acc ?? 0) + frame.event.totalTokens;
    }
    return acc;
  }, null);
  return {
    turnRef,
    threadRef,
    dispatchOk: result.dispatch.ok,
    finishReason: finishReasonOf(frameKinds),
    frameKinds,
    frameKindCounts,
    totalTokens,
    fullAutoRecordCount: result.fullAutoRecordCount,
    coherence: screenHeadlessTurn(result),
  };
};

/** Derive both receipts. The private receipt embeds the public one. */
export const deriveHeadlessReceipts = (
  turnRef: string,
  threadRef: string,
  result: HeadlessTurnResult,
): {
  readonly publicReceipt: HeadlessPublicReceipt;
  readonly privateReceipt: HeadlessPrivateReceipt;
} => {
  const publicReceipt = derivePublicReceipt(turnRef, threadRef, result);
  return {
    publicReceipt,
    privateReceipt: {
      public: publicReceipt,
      frames: result.frames,
      answer: answerTextOf(result.frames),
    },
  };
};
