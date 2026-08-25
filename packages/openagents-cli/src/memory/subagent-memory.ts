// Vendored from packages/agent-experience-memory/src/subagent-memory.ts by scripts/vendor-memory.mjs — do not edit here.
// The drift guard (test/vendored-memory-drift.test.ts) fails when this copy
// no longer matches the canonical source.
import { Schema as S } from "effect";

import type { GlobalPattern } from "./pattern.js";
import { factRef, OwnerScopeId, PatternRef, ProjectScopeId } from "./refs.js";
import { canonicalStringify } from "./canonical.js";
import { sha256Hex } from "./sha256.js";
import { estimateTokens, packWithinBudget, topK } from "./ranking.js";
import { guardEngramContent } from "./engram.js";

/**
 * Scoped memory inheritance for subagent delegation (issue #226) and child
 * engram harvest with parent ledger re-integration (issue #227).
 *
 * Two pure helpers, no store, no host:
 *
 * - `buildSubagentMemoryContext` packages the parent's relevant heuristics and
 *   patterns into a bounded, redacted advisory block for one child prompt. The
 *   child inherits a read-only slice of the parent's distilled memory — never
 *   the parent's private per-case bank — so a delegated child can use what the
 *   parent knows without gaining access to what the parent owns.
 * - `harvestSubagentOutcome` turns a completed child output into parent-side
 *   engrams (a ledger of harvest records), re-guarding every value through the
 *   engram redaction boundary. A child's raw output never enters the parent
 *   ledger unredacted, and nothing harvested is trusted above its source.
 *
 * Both helpers are deterministic and provider-free: packing order, digests, and
 * ledger ids derive only from inputs.
 */

/** One distilled heuristic offered to a delegated child. */
export interface ParentHeuristic {
  readonly ref: string;
  readonly text: string;
  readonly confidence?: number;
  readonly embedding?: ReadonlyArray<number>;
}

export const SUBAGENT_MEMORY_HEADER = "[inherited parent memory — advisory only]" as const;

/** Defaults for the inherited-memory block. */
export const DEFAULT_SUBAGENT_MEMORY_BUDGET_TOKENS = 512 as const;
export const DEFAULT_SUBAGENT_MEMORY_MAX_ITEMS = 8 as const;

export type SubagentMemoryContext = Readonly<{
  /** The formatted advisory block; empty string when nothing qualified. */
  readonly block: string;
  readonly includedRefs: ReadonlyArray<string>;
  readonly droppedRefs: ReadonlyArray<string>;
  readonly usedTokens: number;
  readonly budgetTokens: number;
}>;

/**
 * Package the parent's heuristics into a bounded advisory block for one child
 * prompt.
 *
 * Selection: when a `taskEmbedding` and heuristic embeddings are supplied,
 * candidates are ranked by cosine similarity to the task (`topK`); otherwise
 * all candidates are eligible. Packing: pinned `alwaysInclude` refs fit first,
 * then confidence order, inside the token budget and item cap.
 *
 * The block carries no owner scope, no project scope, no raw episode text, and
 * no reference the child could resolve back into the parent's private bank:
 * refs are shortened to an opaque `h:<digest12>` form derived from content.
 */
export const buildSubagentMemoryContext = (
  input: Readonly<{
    heuristics: ReadonlyArray<ParentHeuristic>;
    taskText?: string;
    taskEmbedding?: ReadonlyArray<number>;
    budgetTokens?: number;
    maxItems?: number;
    alwaysInclude?: ReadonlyArray<string>;
  }>,
): SubagentMemoryContext => {
  const budgetTokens = input.budgetTokens ?? DEFAULT_SUBAGENT_MEMORY_BUDGET_TOKENS;
  const maxItems = input.maxItems ?? DEFAULT_SUBAGENT_MEMORY_MAX_ITEMS;
  const always = new Set(input.alwaysInclude ?? []);

  const opaqueRef = (text: string): string => {
    const digest = sha256Hex(canonicalStringify({ text }));
    return `h:${digest.slice(0, 12)}`;
  };

  let ranked: ReadonlyArray<ParentHeuristic> = input.heuristics;
  if (input.taskEmbedding !== undefined) {
    const withEmbedding = input.heuristics.filter((h) => h.embedding !== undefined);
    const k = Math.trunc(maxItems);
    if (k > 0 && withEmbedding.length > 0) {
      const top = topK(
        input.taskEmbedding,
        withEmbedding.map((h) => ({ ref: opaqueRef(h.text), embedding: h.embedding! })),
        Math.max(withEmbedding.length, k),
      );
      const rank = new Map(top.map((entry, index) => [entry.ref, index]));
      ranked = [...withEmbedding].sort(
        (left, right) => rank.get(opaqueRef(left.text))! - rank.get(opaqueRef(right.text))!,
      );
    }
  }

  const packItems = ranked.map((heuristic) => ({
    ref: opaqueRef(heuristic.text),
    priority: heuristic.confidence ?? 0.5,
    tokens: estimateTokens(`- ${opaqueRef(heuristic.text)}: ${heuristic.text}\n`),
    pinned: always.has(heuristic.ref),
  }));
  const packed = packWithinBudget(packItems, budgetTokens);

  // The packer enforces the token budget; the item cap is enforced here:
  // pinned entries first, then priority, ties keeping the ranked order
  // (Array.prototype.sort is stable), sliced to maxItems.
  const budgetSet = new Set(packed.included);
  const cappedSet = new Set(
    packItems
      .filter((item) => budgetSet.has(item.ref))
      .sort(
        (left, right) =>
          Number(right.pinned) - Number(left.pinned) || right.priority - left.priority,
      )
      .slice(0, Math.max(0, Math.trunc(maxItems)))
      .map((item) => item.ref),
  );

  // Keep input order for readability; membership is decided above.
  const included = input.heuristics.filter((h) => cappedSet.has(opaqueRef(h.text)));
  const dropped = input.heuristics
    .filter((h) => !cappedSet.has(opaqueRef(h.text)))
    .map((h) => h.ref);

  if (included.length === 0) {
    return {
      block: "",
      includedRefs: [],
      droppedRefs: [...new Set(dropped)],
      usedTokens: 0,
      budgetTokens,
    };
  }

  const lines = included.map((h) => `- ${opaqueRef(h.text)}: ${h.text}`);
  const block = [SUBAGENT_MEMORY_HEADER, ...lines].join("\n");
  return {
    block,
    includedRefs: included.map((h) => h.ref),
    droppedRefs: [...new Set(dropped)],
    usedTokens: estimateTokens(`${block}\n`),
    budgetTokens,
  };
};

/** A completed child outcome offered for harvest. */
export interface SubagentOutcome {
  readonly childId: string;
  readonly summary: string;
  /** Structured findings worth keeping as individual parent engrams. */
  readonly findings?: ReadonlyArray<string>;
  readonly completedAtMs: number;
}

/** The schema id written into every harvested ledger entry body. */
export const HARVEST_LEDGER_SCHEMA_ID = "openagents.subagent_harvest.v1" as const;

export const HarvestedLedgerEntry = S.Struct({
  schema: S.Literal("openagents.subagent_harvest.v1"),
  entryId: S.String.check(S.isPattern(/^harvest:[a-f0-9]{64}$/)),
  ownerScope: OwnerScopeId,
  projectScope: ProjectScopeId,
  childId: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  /** The redacted finding text kept in the parent ledger. */
  finding: S.String.check(S.isMinLength(1), S.isMaxLength(1000)),
  /** Provenance: the parent pattern or heuristic refs that seeded the child. */
  inheritedRefs: S.Array(PatternRef),
  completedAt: S.String.check(S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/)),
  digest: S.String.check(S.isPattern(/^sha256:[a-f0-9]{64}$/)),
});
export type HarvestedLedgerEntry = typeof HarvestedLedgerEntry.Type;

const decodeEntry = S.decodeUnknownSync(HarvestedLedgerEntry);
const decodeInheritedRef = S.decodeUnknownSync(PatternRef);

/**
 * Harvest a completed child outcome into parent-ledger entries (issue #227).
 *
 * Every value passes the strict engram guard before it is kept: a finding
 * carrying credential-shaped material is rejected outright (counted in
 * `rejectedUnsafe`), soft PII is redacted but kept, and empty-after-redaction
 * findings are skipped. The summary itself is not stored verbatim; it seeds the
 * per-finding split so each ledger entry stays one bounded fact.
 */
export const harvestSubagentOutcome = (
  input: Readonly<{
    ownerScope: string;
    projectScope: string;
    outcome: SubagentOutcome;
    /** The refs of parent memories that were injected into this child. */
    inheritedRefs?: ReadonlyArray<GlobalPattern | { readonly patternRef: string }>;
  }>,
): Readonly<{
  entries: ReadonlyArray<HarvestedLedgerEntry>;
  rejectedUnsafe: number;
  skippedEmpty: number;
}> => {
  const entries: Array<HarvestedLedgerEntry> = [];
  let rejectedUnsafe = 0;
  let skippedEmpty = 0;
  const inheritedRefs = (input.inheritedRefs ?? []).map((ref) =>
    decodeInheritedRef(ref.patternRef),
  );
  const completedAt = new Date(input.outcome.completedAtMs).toISOString();
  for (const candidate of [
    ...input.outcome.findings ?? [],
    ...(input.outcome.findings === undefined ? [input.outcome.summary] : []),
  ]) {
    const verdict = guardEngramContent(candidate);
    if (!verdict.storable) {
      rejectedUnsafe += 1;
      continue;
    }
    const redacted = verdict.redacted?.trim() ?? "";
    if (redacted.length === 0) {
      skippedEmpty += 1;
      continue;
    }
    const digestValue = sha256Hex(canonicalStringify({ finding: redacted }));
    entries.push(
      decodeEntry({
        schema: HARVEST_LEDGER_SCHEMA_ID,
        entryId: `harvest:${sha256Hex(canonicalStringify({ childId: input.outcome.childId, finding: redacted }))}`,
        ownerScope: input.ownerScope,
        projectScope: input.projectScope,
        childId: input.outcome.childId,
        finding: redacted,
        inheritedRefs,
        completedAt,
        digest: `sha256:${digestValue}`,
      }),
    );
  }
  return { entries, rejectedUnsafe, skippedEmpty } as const;
};

/**
 * Re-integrate harvested entries into the parent's recall path: fold them into
 * the same shape as `ParentHeuristic` so a later delegation inherits what its
 * siblings learned. Confidence starts at the harvest floor — unproven until a
 * consolidation cycle lifts it.
 */
export const HARVEST_CONFIDENCE_FLOOR = 0.35 as const;

export const ledgerEntriesAsHeuristics = (
  entries: ReadonlyArray<HarvestedLedgerEntry>,
): ReadonlyArray<ParentHeuristic> =>
  entries.map((entry) => ({
    ref: factRef(entry.entryId),
    text: entry.finding,
    confidence: HARVEST_CONFIDENCE_FLOOR,
  }));
