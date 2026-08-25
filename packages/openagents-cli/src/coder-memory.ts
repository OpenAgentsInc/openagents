import { createHmac, randomBytes } from "node:crypto";
import {
  appendFileSync,
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import {
  buildEngramBody,
  buildEngramEvent,
  buildSubagentMemoryContext,
  consolidateEpisodes,
  engramContentDigest,
  guardEngramContent,
  harvestSubagentOutcome,
  ledgerEntriesAsHeuristics,
  promoteHeuristicToPattern,
  signSupersedingEngram,
  verifyEngramEventId,
  verifySupersessionChain,
  HarvestedLedgerEntry,
  type EngramEvent,
  type EngramBody,
  type ParentHeuristic,
} from "./memory/index.js";
import { Schema as S } from "effect";

/**
 * The coder's own memory: a local, append-only ledger of signed engrams.
 *
 * This is the live wiring of the agent-continuity program (openagents.com
 * project 15): what a delegated child learns is harvested into the parent's
 * ledger (#227), and what the parent knows seeds the next child's prompt as a
 * bounded advisory block (#226). Between sessions the ledger is the memory —
 * one JSONL file of NIP-AE-shaped engram events under `~/.openagents/memory`,
 * every value through the hard-unsafe redaction gate before it is signed, and
 * every read re-verifying event ids and supersession chains.
 *
 * The signature is a local HMAC over the canonical event id with a key held at
 * `~/.openagents/memory/signing-key` (0600) — integrity against accidental
 * edits and a stable authorship mark for this machine, not a Nostr Schnorr
 * signature. When the relay sync adapter (#222) lands, the same events re-sign
 * under a real Nostr key; the body and chain shapes are already NIP-AE.
 */

const decodeLedgerEntry = S.decodeUnknownSync(HarvestedLedgerEntry);

/** What the delegate fleet asks of memory, kept small so the fleet stays dumb. */
export interface CoderDelegationMemory {
  /** The advisory block to append to a child prompt, or empty when nothing qualifies. */
  inherit(taskText: string): string;
  /** Record a completed child's answer into the ledger. Never throws. */
  harvest(childId: string, answer: string): void;
}

export interface CoderMemoryOptions {
  /** Ledger directory. Default `~/.openagents/memory`. */
  readonly directory?: string;
  /** The project scope written into harvested entries. Default derived from cwd. */
  readonly projectScope?: string;
  /** Epoch-milliseconds clock, injectable for tests. */
  readonly now?: () => number;
}

const OWNER_SCOPE = "owner:local";

const sanitizeScope = (value: string): string => {
  const cleaned = value.replace(/[^A-Za-z0-9._:/-]+/g, "-").replace(/^[^A-Za-z0-9]+/, "");
  return cleaned.length > 0 ? cleaned.slice(0, 200) : "project";
};

export class CoderMemory implements CoderDelegationMemory {
  private readonly directory: string;
  private readonly ledgerPath: string;
  private readonly keyPath: string;
  private readonly projectScope: string;
  private readonly now: () => number;
  private key: Buffer | undefined;

  constructor(options: CoderMemoryOptions = {}) {
    this.directory = options.directory ?? join(homedir(), ".openagents", "memory");
    this.ledgerPath = join(this.directory, "engrams.jsonl");
    this.keyPath = join(this.directory, "signing-key");
    this.projectScope = sanitizeScope(options.projectScope ?? `project:${process.cwd()}`);
    this.now = options.now ?? Date.now;
  }

  /** The local signing key, created on first use. */
  private signingKey(): Buffer {
    if (this.key !== undefined) return this.key;
    mkdirSync(this.directory, { recursive: true, mode: 0o700 });
    if (!existsSync(this.keyPath)) {
      writeFileSync(this.keyPath, randomBytes(32).toString("hex"), { mode: 0o600 });
      chmodSync(this.keyPath, 0o600);
    }
    this.key = Buffer.from(readFileSync(this.keyPath, "utf8").trim(), "hex");
    return this.key;
  }

  private signer(): { pubkey: string; sign: (eventId: string) => string } {
    const key = this.signingKey();
    // A stable 64-hex identity derived from the key, so events from the same
    // machine share an author without the key itself ever leaving the file.
    const pubkey = createHmac("sha256", key).update("openagents.coder-memory.pubkey").digest("hex");
    return {
      pubkey,
      sign: (eventId: string) => createHmac("sha256", key).update(eventId).digest("hex"),
    };
  }

  /**
   * Guard, build, sign, and append one engram. Returns the event, or undefined
   * when the value is hard-unsafe (credential-shaped material never persists).
   */
  record(slug: string, value: string | null, entityId: string): EngramEvent | undefined {
    const verdict = guardEngramContent(value);
    if (!verdict.storable) return undefined;
    const body = buildEngramBody(slug, verdict.redacted, {
      admission: "admitted",
      entityId,
      contentDigest: engramContentDigest(verdict.redacted),
      sourceEventRefs: [],
      relations: [],
      derivedFromSlugs: [],
    });
    const { pubkey, sign } = this.signer();
    const event = buildEngramEvent(
      pubkey,
      Math.floor(this.now() / 1000),
      slug,
      JSON.stringify(body),
      sign,
    );
    appendFileSync(this.ledgerPath, `${JSON.stringify(event)}\n`, { mode: 0o600 });
    return event;
  }

  /**
   * Correct a remembered value: append a superseding engram referencing the
   * prior event. The prior event stays in the ledger; reads resolve the chain.
   * Pass `null` to tombstone the slug.
   */
  correct(slug: string, newValue: string | null): EngramEvent | undefined {
    const chain = this.chains().get(slug);
    const prior = chain?.[chain.length - 1];
    if (prior === undefined) return undefined;
    const verdict = guardEngramContent(newValue);
    if (!verdict.storable) return undefined;
    const { pubkey, sign } = this.signer();
    const event = signSupersedingEngram(
      prior,
      verdict.redacted,
      Math.max(Math.floor(this.now() / 1000), prior.created_at + 1),
      pubkey,
      sign,
    );
    appendFileSync(this.ledgerPath, `${JSON.stringify(event)}\n`, { mode: 0o600 });
    return event;
  }

  /** All ledger events grouped per slug in append order, invalid lines dropped. */
  private chains(): Map<string, Array<EngramEvent>> {
    const chains = new Map<string, Array<EngramEvent>>();
    if (!existsSync(this.ledgerPath)) return chains;
    for (const line of readFileSync(this.ledgerPath, "utf8").split("\n")) {
      if (line.trim().length === 0) continue;
      let event: EngramEvent;
      try {
        event = JSON.parse(line) as EngramEvent;
      } catch {
        continue;
      }
      if (!verifyEngramEventId(event)) continue;
      const dTag = event.tags.find((tag) => tag[0] === "d")?.[1];
      if (dTag === undefined) continue;
      const chain = chains.get(dTag) ?? [];
      chain.push(event);
      chains.set(dTag, chain);
    }
    return chains;
  }

  /**
   * The live value per slug: each chain verified end to end, the newest body
   * winning, tombstoned and broken chains dropped entirely — a chain that does
   * not verify is not memory, it is noise.
   */
  bodies(): ReadonlyArray<EngramBody> {
    return this.living().map((entry) => entry.body);
  }

  private living(): ReadonlyArray<{ body: EngramBody; createdAtMs: number }> {
    const out: Array<{ body: EngramBody; createdAtMs: number }> = [];
    for (const chain of this.chains().values()) {
      if (!verifySupersessionChain(chain)) continue;
      const last = chain[chain.length - 1];
      if (last === undefined) continue;
      let body: EngramBody;
      try {
        body = JSON.parse(last.content) as EngramBody;
      } catch {
        continue;
      }
      if (body.value === null) continue;
      out.push({ body, createdAtMs: last.created_at * 1000 });
    }
    return out;
  }

  /**
   * The harvested ledger entries currently alive in the engram stream.
   *
   * The ledger stores only the redacted finding text — a typed entry carries
   * 64-hex digests, and the redaction gate rightly refuses hex of that shape
   * as key-shaped material — so the typed entry is rebuilt deterministically
   * from the finding, the child id, and the event time on every read.
   */
  entries(): ReadonlyArray<HarvestedLedgerEntry> {
    const entries: Array<HarvestedLedgerEntry> = [];
    for (const { body, createdAtMs } of this.living()) {
      if (!body.slug.startsWith("harvest/") || body.value === null) continue;
      const rebuilt = harvestSubagentOutcome({
        ownerScope: OWNER_SCOPE,
        projectScope: this.projectScope,
        outcome: {
          childId: body.openagents.entityId,
          summary: body.value,
          completedAtMs: createdAtMs,
        },
      });
      for (const entry of rebuilt.entries) {
        entries.push(decodeLedgerEntry(entry));
      }
    }
    return entries;
  }

  /**
   * The parent's heuristics: harvested findings at the harvest confidence
   * floor, plus what one dreaming pass distills from them — clusters of
   * related findings synthesized and, where support is strong enough,
   * promoted through the reviewed pattern layer at higher confidence.
   */
  heuristics(): ReadonlyArray<ParentHeuristic> {
    const entries = this.entries();
    const base = ledgerEntriesAsHeuristics(entries);
    const consolidated = consolidateEpisodes({
      ownerScope: OWNER_SCOPE,
      projectScope: this.projectScope,
      episodes: entries.map((entry) => ({
        ref: entry.entryId,
        text: entry.finding,
        observedAtMs: Date.parse(entry.completedAt),
      })),
      nowMs: this.now(),
    });
    const promoted = consolidated.heuristics.map((heuristic): ParentHeuristic => {
      const pattern = promoteHeuristicToPattern(
        heuristic,
        "delegating a coding task like the ones this heuristic came from",
      );
      return {
        ref: pattern.patternRef,
        text: heuristic.heuristic,
        confidence: heuristic.confidence,
      };
    });
    return [...promoted, ...base];
  }

  inherit(taskText: string): string {
    try {
      const heuristics = this.heuristics();
      if (heuristics.length === 0) return "";
      return buildSubagentMemoryContext({ heuristics, taskText }).block;
    } catch {
      // Memory must never break a delegation.
      return "";
    }
  }

  harvest(childId: string, answer: string): void {
    try {
      const trimmed = answer.trim();
      if (trimmed.length === 0) return;
      const harvested = harvestSubagentOutcome({
        ownerScope: OWNER_SCOPE,
        projectScope: this.projectScope,
        outcome: {
          childId,
          summary: trimmed.slice(0, 1000),
          completedAtMs: this.now(),
        },
      });
      for (const entry of harvested.entries) {
        this.record(`harvest/${entry.digest.slice(7, 19)}`, entry.finding, entry.childId);
      }
    } catch {
      // Memory must never break a delegation.
    }
  }
}
