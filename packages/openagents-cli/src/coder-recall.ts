/**
 * The memory rail (OpenAgentsInc/openagents#51): the harness recalls from the
 * local engram ledger on every incoming message and attaches what qualifies to
 * the outgoing turn as a bounded note. The model never calls a tool to
 * remember — by the time it writes, what this agent already holds is in front
 * of it, labelled by bucket, kind, and age, the same way the knowledge base
 * (#49) and the capability catalog (#42) already ride this rail.
 *
 * Two buckets, kept distinct in the note because their authority differs:
 *
 * - `user`: facts the reader explicitly asked to have remembered. An explicit
 *   request to remember is standing consent to be reminded, so these attach
 *   whenever they exist — the acceptance case is precisely a preference
 *   ("I use pnpm, not npm") that shares no vocabulary with the message that
 *   needs it ("install the deps").
 * - `learned`: what consolidation distilled from past sessions. A distilled
 *   heuristic is a guess about the reader, not a promise from them, so it must
 *   at least share vocabulary with the message before it interrupts, and it is
 *   surfaced with provenance — its ledger slug and the episode count under
 *   it — so a wrong learning can be traced and superseded through the existing
 *   superseding-event path rather than argued with.
 *
 * Ranking and packing reuse the owned ranking module (salience recall,
 * token-budget packing); nothing here re-derives an ordering of its own. The
 * note builder is pure, and the rail around it degrades to silence, never to
 * a broken turn.
 */

import { estimateTokens, packWithinBudget, recallOrderBySalience } from "./memory/ranking.js";
import type { CoderTool } from "./coder-tools.js";

/** One live ledger memory as the store offers it to the rail. */
export interface RecallableMemory {
  readonly bucket: "user" | "learned";
  /**
   * The ledger slug — the handle a correction is filed under. For a learned
   * memory this is what the note surfaces so a wrong heuristic can be
   * superseded by name.
   */
  readonly ref: string;
  readonly text: string;
  /** When the live revision of this memory was recorded, epoch milliseconds. */
  readonly recordedAtMs: number;
  /** Consolidation's confidence in a learned memory; absent for user notes. */
  readonly confidence?: number;
  /** The episode slugs a learned memory was distilled from. */
  readonly provenance?: ReadonlyArray<string>;
}

/**
 * The salience a memory needs before it is worth attaching. A user note always
 * clears it; a learned heuristic clears it only with lexical overlap lifting
 * it past its confidence base.
 */
export const MEMORY_ATTACH_FLOOR = 0.5;

/** Most memories one note carries; more is noise, not context. */
export const MEMORY_NOTE_LIMIT = 5;

/** The note's token budget, enforced by the owned packer. */
export const MEMORY_NOTE_BUDGET_TOKENS = 320;

/** A memory's share of one line; a runaway value is clipped, not dropped. */
const TEXT_BOUND = 300;

const clipped = (text: string): string =>
  text.length <= TEXT_BOUND ? text : `${text.slice(0, TEXT_BOUND - 1).trimEnd()}…`;

/** Lowercased words of three letters or more; the rest is noise. */
const tokensOf = (text: string): string[] =>
  text
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter((word) => word.length >= 3);

/**
 * The salience fed to the owned recall ordering. A user note starts at 1 —
 * explicitly asked for, always above the floor — and overlap only sharpens its
 * rank. A learned heuristic with no overlap scores 0 and stays silent; with
 * overlap it starts from consolidation's confidence.
 */
const salienceOf = (memory: RecallableMemory, hits: number): number =>
  memory.bucket === "user"
    ? 1 + hits * 0.25
    : hits === 0
      ? 0
      : (memory.confidence ?? 0.5) + hits * 0.25;

/** A coarse human age: "new" under a minute, then minutes, hours, days. */
const agePhrase = (ageMs: number): string => {
  const ms = Math.max(0, ageMs);
  if (ms < 60_000) return "new";
  const minutes = Math.floor(ms / 60_000);
  if (minutes < 60) return `${minutes}m old`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h old`;
  return `${Math.floor(hours / 24)}d old`;
};

const line = (memory: RecallableMemory, nowMs: number): string => {
  const age = agePhrase(nowMs - memory.recordedAtMs);
  if (memory.bucket === "user") {
    return `- (user note, ${age}) ${clipped(memory.text)}`;
  }
  const episodes = memory.provenance?.length ?? 0;
  const support = episodes === 0 ? "" : `, from ${episodes} episode${episodes === 1 ? "" : "s"}`;
  return `- (learned heuristic ${memory.ref}, ${age}${support}) ${clipped(memory.text)}`;
};

/**
 * The note for one message, or nothing when no memory clears the floor.
 *
 * Selection is the owned ranking module end to end: the floor filters, salience
 * recall orders (salience plus recency), the limit is the top-K, and the token
 * budget is enforced by the owned packer over the rendered lines. Pure and
 * deterministic: equal inputs give an equal note.
 */
export const memoryRecallNote = (
  memories: ReadonlyArray<RecallableMemory>,
  prompt: string,
  nowMs: number,
): string | undefined => {
  if (memories.length === 0) return undefined;
  const terms = tokensOf(prompt);
  const scored = memories
    .map((memory, index) => {
      const haystack = new Set(tokensOf(memory.text));
      const hits = terms.filter((term) => haystack.has(term)).length;
      // The ref fed to the ranking module is positional, not the ledger slug:
      // two memories may legitimately share a slug prefix, and the ranking
      // contract wants refs unique within one call.
      return { memory, ref: `m${index}`, salience: salienceOf(memory, hits) };
    })
    .filter((candidate) => candidate.salience >= MEMORY_ATTACH_FLOOR);
  if (scored.length === 0) return undefined;

  const byRef = new Map(scored.map((candidate) => [candidate.ref, candidate]));
  const ordered = recallOrderBySalience(
    scored.map((candidate) => ({
      ref: candidate.ref,
      salience: candidate.salience,
      lastUsedAt: candidate.memory.recordedAtMs,
    })),
    nowMs,
  ).slice(0, MEMORY_NOTE_LIMIT);

  const lines = new Map(ordered.map((ref) => [ref, line(byRef.get(ref)!.memory, nowMs)] as const));
  const packed = packWithinBudget(
    ordered.map((ref) => ({
      ref,
      priority: byRef.get(ref)!.salience,
      tokens: estimateTokens(`${lines.get(ref) ?? ""}\n`),
    })),
    MEMORY_NOTE_BUDGET_TOKENS,
  );
  const included = new Set(packed.included);
  const kept = ordered.filter((ref) => included.has(ref));
  if (kept.length === 0) return undefined;

  return (
    "[From memory — what this agent already holds about this reader and this work. " +
    "`user` entries were explicitly asked for; `learned` entries were distilled from " +
    "earlier sessions and can be wrong — a wrong one is superseded under its named slug, " +
    "never argued with:\n" +
    kept.map((ref) => lines.get(ref) ?? "").join("\n") +
    "]"
  );
};

/**
 * The explicit write path for the user bucket. The tool exists only so the
 * reader's "remember that …" lands in the ledger; recall never goes through
 * it — the rail attaches memories without a call. Explicit only, never
 * inferred: the description tells the model to use it solely on a direct
 * request, and everything it stores is labelled `user` on the way back out.
 */
export const rememberTool = (memory: { remember(fact: string): unknown }): CoderTool => ({
  name: "remember",
  description:
    "Store one fact the reader explicitly asked to have remembered, such as " +
    '"remember that I use pnpm, not npm". Call this only when the reader directly asks ' +
    "for something to be remembered — never to record your own inferences or ambient " +
    "observations. Stored facts are recalled automatically on later messages; no tool " +
    "call is needed to read them back.",
  parameters: {
    type: "object",
    properties: {
      fact: {
        type: "string",
        description: "The fact to remember, in one bounded sentence, as the reader stated it.",
      },
    },
    required: ["fact"],
  },
  run: async (args: Record<string, unknown>): Promise<string> => {
    const fact = typeof args["fact"] === "string" ? args["fact"].trim() : "";
    if (fact.length === 0) {
      return "Nothing to remember: `fact` must be a non-empty sentence.";
    }
    try {
      const stored = memory.remember(fact);
      if (stored === undefined) {
        return (
          "That was not stored. The memory guard refuses credential-shaped material " +
          "outright; rephrase the fact without the secret if the reader still wants it kept."
        );
      }
      return "Remembered. It will be recalled automatically when a later message needs it.";
    } catch {
      // A tool reports a refusal as text rather than by throwing: the model
      // can act on words and cannot act on a turn that died.
      return "That was not stored: the memory ledger is not writable right now.";
    }
  },
});
