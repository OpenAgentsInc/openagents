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
  EngramSyncQueue,
  MemoryTransport,
  project,
  projectedValue,
  projectionMatches,
  verifyEngramEventId,
  COMPANION_SCHEMA_ID,
  HarvestedLedgerEntry,
  type EngramEvent,
  type EngramBody,
  type EngramTransport,
  type ParentHeuristic,
  type Projection,
  type SyncStatus,
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
  /**
   * Where engrams are mirrored. Defaults to an in-process transport, which
   * keeps the seam exercised without a relay; a real Nostr transport drops in
   * here without any caller changing (#222).
   */
  readonly transport?: EngramTransport;
  /**
   * New harvests needed before the harvest path schedules a dream.
   * `Infinity` turns the automatic pass off, leaving `dream` explicit.
   */
  readonly dreamThreshold?: number;
}

const OWNER_SCOPE = "owner:local";

/** New harvests needed before a dream is worth running. */
const DREAM_THRESHOLD = 2;
/** How many episodes one standing heuristic is assumed to account for. */
const DREAM_EPISODES_PER_HEURISTIC = 2;

/**
 * A cluster's identity: its supporting episode refs, order-independent.
 * Two syntheses over the same episodes are the same claim, however the
 * clusterer happened to order them.
 */
const clusterKey = (refs: ReadonlyArray<string>): string => [...refs].sort().join("|");

/**
 * The confidence stamped into a heuristic engram's entity id as
 * `<synthId>#<confidence>`. The companion schema has no numeric field, and the
 * value itself is the heuristic sentence, so the figure rides the identity
 * rather than being recomputed on read.
 */
const confidenceOf = (entityId: string): number => {
  const marked = entityId.lastIndexOf("#");
  if (marked < 0) return 0.5;
  const parsed = Number(entityId.slice(marked + 1));
  return Number.isFinite(parsed) ? parsed : 0.5;
};

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
  private readonly dreamThreshold: number;
  private key: Buffer | undefined;
  private cachedProjection: Projection | undefined;
  private readonly sync: EngramSyncQueue;

  constructor(options: CoderMemoryOptions = {}) {
    this.directory = options.directory ?? join(homedir(), ".openagents", "memory");
    this.ledgerPath = join(this.directory, "engrams.jsonl");
    this.keyPath = join(this.directory, "signing-key");
    this.projectScope = sanitizeScope(options.projectScope ?? `project:${process.cwd()}`);
    this.now = options.now ?? Date.now;
    this.dreamThreshold = options.dreamThreshold ?? DREAM_THRESHOLD;
    this.sync = new EngramSyncQueue(options.transport ?? new MemoryTransport());
  }

  /**
   * What sync is holding. A session can say it is behind rather than leaving
   * a reader to infer it from silence.
   */
  syncStatus(): SyncStatus {
    return this.sync.status();
  }

  /**
   * Hand everything queued to the transport.
   *
   * Called between turns, never inside one: the local ledger is already
   * authoritative by the time this runs, so a transport that is down costs
   * latency and nothing else.
   */
  async flush(): Promise<number> {
    return this.sync.drain();
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
  record(
    slug: string,
    value: string | null,
    entityId: string,
    derivedFromSlugs: ReadonlyArray<string> = [],
  ): EngramEvent | undefined {
    const verdict = guardEngramContent(value);
    if (!verdict.storable) return undefined;
    const body = buildEngramBody(slug, verdict.redacted, {
      admission: "admitted",
      entityId,
      contentDigest: engramContentDigest(verdict.redacted),
      sourceEventRefs: [],
      relations: [],
      derivedFromSlugs: [...derivedFromSlugs],
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
    this.cachedProjection = undefined;
    // Local-first: the engram is on disk and readable now. Sync catches up.
    this.sync.publish(event);
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
    this.cachedProjection = undefined;
    this.sync.publish(event);
    return event;
  }

  /** Every engram on disk, malformed lines dropped. The projection judges them. */
  private events(): ReadonlyArray<EngramEvent> {
    if (!existsSync(this.ledgerPath)) return [];
    const events: Array<EngramEvent> = [];
    for (const line of readFileSync(this.ledgerPath, "utf8").split("\n")) {
      if (line.trim().length === 0) continue;
      try {
        events.push(JSON.parse(line) as EngramEvent);
      } catch {
        continue;
      }
    }
    return events;
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
    const projection = this.projection();
    return projection.entries.map((entry) => ({
      body: {
        slug: entry.slug,
        value: entry.value,
        openagents: {
          schema: COMPANION_SCHEMA_ID,
          admission: "admitted",
          entityId: entry.entityId,
          contentDigest: engramContentDigest(entry.value),
          sourceEventRefs: [],
          relations: [],
          derivedFromSlugs: [...entry.derivedFromSlugs],
        },
      },
      createdAtMs: entry.updatedAt * 1000,
    }));
  }

  /**
   * The queryable view over the ledger (issue #223).
   *
   * Derived, never stored: the projection resolves each slug's supersession
   * chain, names tombstones rather than dropping them, refuses a forked or
   * broken chain whole, and counts what it refused. It is cached against the
   * log it was built from and rebuilt the moment they disagree, so a cold
   * start and a warm one are the same value.
   */
  projection(): Projection {
    const events = this.events();
    if (this.cachedProjection !== undefined && projectionMatches(this.cachedProjection, events)) {
      return this.cachedProjection;
    }
    this.cachedProjection = project(events);
    return this.cachedProjection;
  }

  /** The live value for one slug, or undefined when absent or tombstoned. */
  recall(slug: string): string | undefined {
    return projectedValue(this.projection(), slug);
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
   * The parent's heuristics: what dreaming has already distilled, read back
   * from the ledger, plus the raw harvested findings underneath them.
   *
   * Recall does not consolidate. A dream is a background pass whose output is
   * itself an engram (see `dream`), so a recall reads what was distilled
   * rather than re-deriving it on every delegation.
   */
  heuristics(): ReadonlyArray<ParentHeuristic> {
    const distilled: Array<ParentHeuristic> = [];
    for (const { body } of this.living()) {
      if (!body.slug.startsWith("heuristic/") || body.value === null) continue;
      distilled.push({
        ref: body.slug,
        text: body.value,
        confidence: confidenceOf(body.openagents.entityId),
      });
    }
    return [...distilled, ...ledgerEntriesAsHeuristics(this.entries())];
  }

  /**
   * One dream cycle: cluster the harvested episodes, synthesize a heuristic
   * per cluster, and write each one back as its own engram.
   *
   * This is the offline half of the memory (issue #224). It runs between
   * turns rather than inside one, and it is idempotent: a synthesis whose
   * cluster and wording are unchanged rewrites nothing. Where a cluster
   * reaches a *different* conclusion than the heuristic already standing over
   * it, the old engram is superseded rather than edited — the correction
   * references what it replaces, and both survive in the ledger.
   */
  dream(): Readonly<{ written: number; superseded: number; unchanged: number }> {
    const entries = this.entries();
    if (entries.length === 0) return { written: 0, superseded: 0, unchanged: 0 };

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

    // What already stands, by the cluster it was distilled from.
    const standing = new Map<string, { slug: string; text: string }>();
    for (const { body } of this.living()) {
      if (!body.slug.startsWith("heuristic/") || body.value === null) continue;
      standing.set(clusterKey(body.openagents.derivedFromSlugs), {
        slug: body.slug,
        text: body.value,
      });
    }

    let written = 0;
    let superseded = 0;
    let unchanged = 0;
    for (const heuristic of consolidated.heuristics) {
      const pattern = promoteHeuristicToPattern(
        heuristic,
        "delegating a coding task like the ones this heuristic came from",
      );
      const support = [...heuristic.sourceRefs];
      const key = clusterKey(support);
      const prior = standing.get(key);
      if (prior !== undefined) {
        if (prior.text === heuristic.heuristic) {
          unchanged += 1;
          continue;
        }
        // The same episodes now say something else. Supersede, do not edit.
        if (this.correct(prior.slug, heuristic.heuristic) !== undefined) superseded += 1;
        continue;
      }
      const slug = `heuristic/${pattern.patternRef.slice("pattern:".length, "pattern:".length + 12)}`;
      const entityId = `${heuristic.synthId}#${heuristic.confidence.toFixed(3)}`;
      if (this.record(slug, heuristic.heuristic, entityId, support) !== undefined) written += 1;
    }
    return { written, superseded, unchanged };
  }

  /**
   * Dream when enough has been learned since the last one to be worth it.
   *
   * Called after a harvest, so the pass runs while the session is between
   * delegations rather than on a clock. Cheap when there is nothing new: the
   * count comes from the ledger that was just read.
   */
  private dreamIfDue(): void {
    const harvested = this.entries().length;
    if (harvested === 0) return;
    const distilled = this.living().filter(({ body }) => body.slug.startsWith("heuristic/")).length;
    if (harvested - distilled * DREAM_EPISODES_PER_HEURISTIC < this.dreamThreshold) return;
    this.dream();
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
      this.dreamIfDue();
    } catch {
      // Memory must never break a delegation.
    }
  }
}
