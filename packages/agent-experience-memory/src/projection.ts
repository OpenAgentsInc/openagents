import { Schema as S } from "effect";

import { canonicalStringify } from "./internal/canonical.js";
import { sha256Hex } from "./internal/sha256.js";
import { computeEngramEventId, EngramBody, type EngramEvent } from "./engram.js";

/**
 * The derived projection over an engram log (issue #223).
 *
 * The engram stream is the authority; this is a view of it. Nothing here is
 * stored anywhere that survives a process, and nothing here can be repaired in
 * place — a projection that disagrees with the log is discarded and rebuilt.
 * That is the whole design: an append-only signed log is the thing you can
 * trust, and a queryable index over it is a convenience that must never become
 * a second source of truth.
 *
 * Three properties the tests hold this to:
 *
 * 1. **Derived only.** `project` is a pure function of the events it is given.
 *    No clock, no randomness, no ambient state.
 * 2. **Idempotent.** Projecting the same log twice produces an identical
 *    value, digest included, so a cold-start rebuild is indistinguishable from
 *    a warm one.
 * 3. **Order-independent.** Events arriving in any order — a relay replaying
 *    out of sequence, two clients merging — project to the same value, because
 *    order is taken from `created_at` and the supersession chain rather than
 *    from arrival.
 *
 * An event that fails verification is not projected. A chain whose
 * supersession does not resolve is dropped whole rather than partly applied:
 * half a correction is worse than none, because it reads as fact.
 */

export const PROJECTION_SCHEMA_ID = "openagents.engram_projection.v1" as const;

/** One live slug: the surviving value and where it came from. */
export const ProjectedEntry = S.Struct({
  slug: S.String,
  value: S.String,
  entityId: S.String,
  /** How many events stand behind this value, corrections included. */
  revisions: S.Number,
  /** The event id of the newest engram in the chain. */
  head: S.String.check(S.isPattern(/^[0-9a-f]{64}$/)),
  /** Seconds since the epoch, from the surviving engram. */
  updatedAt: S.Number,
  derivedFromSlugs: S.Array(S.String),
});
export type ProjectedEntry = typeof ProjectedEntry.Type;

/** One entity gathered from the slugs that name it. */
export const ProjectedEntity = S.Struct({
  entityId: S.String,
  slugs: S.Array(S.String),
});
export type ProjectedEntity = typeof ProjectedEntity.Type;

/** One derivation edge: a slug and something it was distilled from. */
export const ProjectedRelation = S.Struct({
  from: S.String,
  to: S.String,
  type: S.String,
});
export type ProjectedRelation = typeof ProjectedRelation.Type;

export const Projection = S.Struct({
  schema: S.Literal(PROJECTION_SCHEMA_ID),
  entries: S.Array(ProjectedEntry),
  entities: S.Array(ProjectedEntity),
  relations: S.Array(ProjectedRelation),
  /** Slugs whose surviving engram is a tombstone. Named, not silently gone. */
  tombstoned: S.Array(S.String),
  /** Events the projection refused, by reason, so a gap is countable. */
  rejected: S.Struct({
    unverified: S.Number,
    unresolvedChain: S.Number,
    malformed: S.Number,
  }),
  /** The digest of everything above: two equal logs give one digest. */
  digest: S.String.check(S.isPattern(/^sha256:[0-9a-f]{64}$/)),
});
export type Projection = typeof Projection.Type;

const dTagOf = (event: EngramEvent): string | undefined =>
  event.tags.find((tag) => tag[0] === "d")?.[1];

const bodyOf = (event: EngramEvent): EngramBody | undefined => {
  try {
    return JSON.parse(event.content) as EngramBody;
  } catch {
    return undefined;
  }
};

/**
 * Order one slug's events into a chain, oldest first.
 *
 * The `supersedes` links are the authority — `created_at` only breaks ties
 * among roots, because a clock is a claim and a reference is a fact. A chain
 * that does not resolve into a single line (a fork, a cycle, a missing link)
 * is refused whole.
 */
const resolveChain = (
  events: ReadonlyArray<EngramEvent>,
): ReadonlyArray<EngramEvent> | undefined => {
  if (events.length === 0) return undefined;
  const byId = new Map(events.map((event) => [event.id, event]));
  const supersededBy = new Map<string, EngramEvent>();
  const roots: Array<EngramEvent> = [];

  for (const event of events) {
    const body = bodyOf(event);
    if (body === undefined) return undefined;
    const prior = body.openagents.supersedes;
    if (prior === undefined) {
      roots.push(event);
      continue;
    }
    if (!byId.has(prior)) return undefined;
    // Two events superseding the same prior is a fork, not a chain.
    if (supersededBy.has(prior)) return undefined;
    supersededBy.set(prior, event);
  }

  if (roots.length !== 1) return undefined;
  const chain: Array<EngramEvent> = [];
  const seen = new Set<string>();
  let current: EngramEvent | undefined = roots[0];
  while (current !== undefined) {
    if (seen.has(current.id)) return undefined;
    seen.add(current.id);
    chain.push(current);
    current = supersededBy.get(current.id);
  }
  // Every event must be on the one line; a stray is an unresolved chain.
  return chain.length === events.length ? chain : undefined;
};

/**
 * Build the projection for a log.
 *
 * Pass every engram known for the scope. Order does not matter.
 */
export const project = (events: ReadonlyArray<EngramEvent>): Projection => {
  let unverified = 0;
  let malformed = 0;
  let unresolvedChain = 0;

  const bySlug = new Map<string, Array<EngramEvent>>();
  for (const event of events) {
    if (event.id !== computeEngramEventId(event)) {
      unverified += 1;
      continue;
    }
    const slug = dTagOf(event);
    if (slug === undefined || bodyOf(event) === undefined) {
      malformed += 1;
      continue;
    }
    const chain = bySlug.get(slug) ?? [];
    chain.push(event);
    bySlug.set(slug, chain);
  }

  const entries: Array<ProjectedEntry> = [];
  const tombstoned: Array<string> = [];
  const relations: Array<ProjectedRelation> = [];
  const entityToSlugs = new Map<string, Set<string>>();

  for (const [slug, slugEvents] of bySlug) {
    const ordered = [...slugEvents].sort((left, right) =>
      left.created_at !== right.created_at
        ? left.created_at - right.created_at
        : left.id.localeCompare(right.id),
    );
    const chain = resolveChain(ordered);
    if (chain === undefined) {
      unresolvedChain += slugEvents.length;
      continue;
    }
    const head = chain[chain.length - 1];
    if (head === undefined) continue;
    const body = bodyOf(head);
    if (body === undefined) {
      malformed += 1;
      continue;
    }
    if (body.value === null) {
      tombstoned.push(slug);
      continue;
    }

    entries.push({
      slug,
      value: body.value,
      entityId: body.openagents.entityId,
      revisions: chain.length,
      head: head.id,
      updatedAt: head.created_at,
      derivedFromSlugs: [...body.openagents.derivedFromSlugs].sort(),
    });

    const slugs = entityToSlugs.get(body.openagents.entityId) ?? new Set<string>();
    slugs.add(slug);
    entityToSlugs.set(body.openagents.entityId, slugs);

    for (const source of body.openagents.derivedFromSlugs) {
      relations.push({ from: slug, to: source, type: "derived_from" });
    }
    for (const relation of body.openagents.relations) {
      relations.push({ from: slug, to: relation.targetSlug, type: relation.type });
    }
  }

  entries.sort((left, right) => left.slug.localeCompare(right.slug));
  tombstoned.sort();
  relations.sort(
    (left, right) =>
      left.from.localeCompare(right.from) ||
      left.to.localeCompare(right.to) ||
      left.type.localeCompare(right.type),
  );
  const entities = [...entityToSlugs.entries()]
    .map(([entityId, slugs]) => ({ entityId, slugs: [...slugs].sort() }))
    .sort((left, right) => left.entityId.localeCompare(right.entityId));

  const withoutDigest = {
    schema: PROJECTION_SCHEMA_ID,
    entries,
    entities,
    relations,
    tombstoned,
    rejected: { unverified, unresolvedChain, malformed },
  } as const;

  return {
    ...withoutDigest,
    digest: `sha256:${sha256Hex(canonicalStringify(withoutDigest))}`,
  };
};

/**
 * Whether a projection still describes a log.
 *
 * The check is a rebuild: re-project and compare digests. There is no cheaper
 * honest answer, because a projection carries no authority of its own — if it
 * disagrees with the log, the log is right and the projection is discarded.
 */
export const projectionMatches = (
  projection: Projection,
  events: ReadonlyArray<EngramEvent>,
): boolean => project(events).digest === projection.digest;

/** The live value for one slug, or undefined when absent or tombstoned. */
export const projectedValue = (projection: Projection, slug: string): string | undefined =>
  projection.entries.find((entry) => entry.slug === slug)?.value;
