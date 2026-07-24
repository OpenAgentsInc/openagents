/**
 * SARAH-CW-03 — pure tick decomposition into bounded community work units.
 *
 * A community agent never runs a Sarah tick. It runs a work unit that a Sarah
 * tick produced. Each unit carries its own narrow grant: target, allowed
 * actions, budget, expiration, and idempotency identity. No unit may carry a
 * Sarah grant.
 *
 * Spec: docs/omega/2026-07-24-sarah-workroom-mvp-spec.md §33, §38.2
 * Issue: OpenAgentsInc/openagents#9225
 *
 * This module is pure types and templates. It does not publish to a relay,
 * dispatch compute, or wire the autonomous tick runner.
 */
import { Schema as S } from "effect";

// ---------------------------------------------------------------------------
// Packet identity
// ---------------------------------------------------------------------------

export const SARAH_CW_03_PACKET = "SARAH-CW-03" as const;
export const SARAH_CW_03_ISSUE = "OpenAgentsInc/openagents#9225" as const;
export const SARAH_CW_03_SPEC_SECTION =
  "docs/omega/2026-07-24-sarah-workroom-mvp-spec.md#33-ticks-and-the-correction-it-needs" as const;

export const SARAH_COMMUNITY_WORK_UNIT_SCHEMA =
  "openagents.sarah.community_work_unit.v1" as const;
export const SARAH_COMMUNITY_WORK_UNIT_GRANT_SCHEMA =
  "openagents.sarah.community_work_unit_grant.v1" as const;
export const SARAH_COMMUNITY_TICK_DECOMPOSITION_SCHEMA =
  "openagents.sarah.community_tick_decomposition.v1" as const;

/** Explicit authority class on every unit grant. Never a Sarah profile. */
export const SARAH_COMMUNITY_UNIT_AUTHORITY_CLASS =
  "community_unit_narrow" as const;

/**
 * Hard cap on units one tick may publish. Keeps the wake bounded even when
 * many candidates exist. CW-04 may publish a subset; the cap still holds.
 */
export const SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK = 16 as const;

/** Default grant lifetime when a candidate omits expiration (seconds). */
export const SARAH_COMMUNITY_WORK_UNIT_DEFAULT_TTL_SECONDS = 3_600 as const;

/** Maximum grant lifetime from now (seconds). NIP-40 bounds on the wire. */
export const SARAH_COMMUNITY_WORK_UNIT_MAX_TTL_SECONDS = 86_400 as const;

// ---------------------------------------------------------------------------
// Forbidden Sarah authority material
// ---------------------------------------------------------------------------

/**
 * Substrings that mark Sarah-side authority. Any unit grant or payload that
 * contains one is refused. Community compute must not inherit these.
 */
export const FORBIDDEN_SARAH_AUTHORITY_MARKERS: ReadonlyArray<string> = [
  "grant.sarah.",
  "principal.sarah",
  "openagents.sarah-owner-orchestrator",
  "openagents.owner-delegated-autonomy",
  "capability.sarah.",
  "program.sarah_",
  "program.managed_agent_sandboxes",
  "SARAH_RUNTIME_AUTHORITY_PROFILE",
  "authorityMayAmplify",
  "sarah_orchestrator",
  "increase_own_authority",
];

/** Allowed community unit actions (narrow, versioned). Not Sarah tools. */
export const SARAH_COMMUNITY_UNIT_ACTIONS = [
  "quote_work_unit",
  "execute_public_objective",
  "return_evidence",
  "verify_peer_result",
  "review_peer_result",
] as const;

export type SarahCommunityUnitAction =
  (typeof SARAH_COMMUNITY_UNIT_ACTIONS)[number];

// ---------------------------------------------------------------------------
// Schema primitives
// ---------------------------------------------------------------------------

const Ref = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));
const PublicObjective = S.Trim.check(S.isMinLength(3), S.isMaxLength(1_000));
const IdempotencyId = S.Trim.check(
  S.isMinLength(8),
  S.isMaxLength(128),
  S.isPattern(/^[A-Za-z0-9._:-]+$/),
);
const UnixSeconds = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(4_102_444_800), // ~2100-01-01
);
const UnitAction = S.Literals([...SARAH_COMMUNITY_UNIT_ACTIONS]);

export const SarahCommunityWorkUnitBudgetKind = S.Literals([
  "experience_tier",
  "msats",
]);
export type SarahCommunityWorkUnitBudgetKind = S.Schema.Type<
  typeof SarahCommunityWorkUnitBudgetKind
>;

/**
 * Budget on the unit. v1 rewards experience only (`experience_tier` 1|2|3).
 * `msats` is reserved for a future paid version and is accepted in the schema
 * so CW-04/CW-07 can share the grant shape without a break.
 */
export const SarahCommunityWorkUnitBudget = S.Struct({
  kind: SarahCommunityWorkUnitBudgetKind,
  amount: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
});
export type SarahCommunityWorkUnitBudget = S.Schema.Type<
  typeof SarahCommunityWorkUnitBudget
>;

/**
 * Narrow grant. Names target, actions, budget, expiration, and idempotency.
 * `authorityClass` is always `community_unit_narrow` — never a Sarah grant.
 */
export const SarahCommunityWorkUnitGrant = S.Struct({
  schema: S.Literal(SARAH_COMMUNITY_WORK_UNIT_GRANT_SCHEMA),
  authorityClass: S.Literal(SARAH_COMMUNITY_UNIT_AUTHORITY_CLASS),
  targetRef: Ref,
  allowedActions: S.Array(UnitAction).check(S.isMinLength(1), S.isMaxLength(8)),
  budget: SarahCommunityWorkUnitBudget,
  /** Unix seconds. NIP-40 `expiration` tag uses the same bound on the wire. */
  expiresAtUnix: UnixSeconds,
  idempotencyId: IdempotencyId,
});
export type SarahCommunityWorkUnitGrant = S.Schema.Type<
  typeof SarahCommunityWorkUnitGrant
>;

export const SarahCommunityWorkUnit = S.Struct({
  schema: S.Literal(SARAH_COMMUNITY_WORK_UNIT_SCHEMA),
  packet: S.Literal(SARAH_CW_03_PACKET),
  unitRef: Ref,
  tickRef: Ref,
  /** Public-safe objective summary. Never a private prompt or Sarah grant. */
  objective: PublicObjective,
  grant: SarahCommunityWorkUnitGrant,
  /** Experience tier for scoring (CW-06). Independent of paid budget. */
  experienceTier: S.Literals([1, 2, 3]),
  createdAtUnix: UnixSeconds,
});
export type SarahCommunityWorkUnit = S.Schema.Type<
  typeof SarahCommunityWorkUnit
>;

export const SarahCommunityTickDecomposition = S.Struct({
  schema: S.Literal(SARAH_COMMUNITY_TICK_DECOMPOSITION_SCHEMA),
  packet: S.Literal(SARAH_CW_03_PACKET),
  tickRef: Ref,
  decomposedAtUnix: UnixSeconds,
  unitCap: S.Literal(SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK),
  units: S.Array(SarahCommunityWorkUnit).check(
    S.isMaxLength(SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK),
  ),
  truncatedCandidateCount: S.Number.check(
    S.isInt(),
    S.isGreaterThanOrEqualTo(0),
  ),
});
export type SarahCommunityTickDecomposition = S.Schema.Type<
  typeof SarahCommunityTickDecomposition
>;

export const decodeSarahCommunityWorkUnit = S.decodeUnknownSync(
  SarahCommunityWorkUnit,
);
export const decodeSarahCommunityWorkUnitGrant = S.decodeUnknownSync(
  SarahCommunityWorkUnitGrant,
);
export const decodeSarahCommunityTickDecomposition = S.decodeUnknownSync(
  SarahCommunityTickDecomposition,
);

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

export type SarahCommunityWorkUnitErrorCode =
  | "expired_grant"
  | "sarah_grant_forbidden"
  | "unit_cap_exceeded"
  | "invalid_budget"
  | "invalid_expiration"
  | "invalid_candidate"
  | "duplicate_idempotency"
  | "unsafe_material";

export class SarahCommunityWorkUnitError extends Error {
  readonly code: SarahCommunityWorkUnitErrorCode;

  constructor(code: SarahCommunityWorkUnitErrorCode, message: string) {
    super(message);
    this.name = "SarahCommunityWorkUnitError";
    this.code = code;
  }
}

// ---------------------------------------------------------------------------
// Public-safe / Sarah-grant guards
// ---------------------------------------------------------------------------

const UNSAFE_MATERIAL_PATTERN =
  /(ANTHROPIC_API_KEY|OPENAI_API_KEY|MDK_ACCESS_TOKEN|SECRET|TOKEN=|-----BEGIN|mnemonic|payment_hash|payment_preimage|preimage|lnbc|lntb|lno1|file:\/\/|\/Users\/|\/home\/|C:\\|ssh:\/\/|private[_-]?repo|raw prompt|provider payload|nsec1)/iu;

const collectStringLeaves = (value: unknown, out: string[]): void => {
  if (value === null || value === undefined) return;
  if (typeof value === "string") {
    out.push(value);
    return;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    out.push(String(value));
    return;
  }
  if (Array.isArray(value)) {
    for (const item of value) collectStringLeaves(item, out);
    return;
  }
  if (typeof value === "object") {
    for (const [key, child] of Object.entries(
      value as Record<string, unknown>,
    )) {
      out.push(key);
      collectStringLeaves(child, out);
    }
  }
};

/**
 * Refuse if any string leaf or key names Sarah authority material.
 * Decomposition must never copy Sarah grants onto a community unit.
 */
export const assertNoSarahGrant = (value: unknown): void => {
  const leaves: string[] = [];
  collectStringLeaves(value, leaves);
  for (const leaf of leaves) {
    for (const marker of FORBIDDEN_SARAH_AUTHORITY_MARKERS) {
      if (leaf.includes(marker)) {
        throw new SarahCommunityWorkUnitError(
          "sarah_grant_forbidden",
          `community work unit must not carry Sarah authority material: ${marker}`,
        );
      }
    }
    if (UNSAFE_MATERIAL_PATTERN.test(leaf)) {
      throw new SarahCommunityWorkUnitError(
        "unsafe_material",
        "community work unit contains private or payment material",
      );
    }
  }
};

/**
 * A unit whose grant expired is refused, not extended.
 * `nowUnix` is compared with `expiresAtUnix` (inclusive of the boundary:
 * equal means expired / not usable).
 */
export const assertGrantActive = (
  grant: SarahCommunityWorkUnitGrant,
  nowUnix: number,
): void => {
  if (!Number.isInteger(nowUnix) || nowUnix < 0) {
    throw new SarahCommunityWorkUnitError(
      "invalid_expiration",
      "nowUnix must be a non-negative integer unix second",
    );
  }
  if (grant.expiresAtUnix <= nowUnix) {
    throw new SarahCommunityWorkUnitError(
      "expired_grant",
      `work unit grant expired at ${grant.expiresAtUnix} (now ${nowUnix}); refuse, do not extend`,
    );
  }
};

/** True when the grant is still usable at `nowUnix`. */
export const isGrantActive = (
  grant: SarahCommunityWorkUnitGrant,
  nowUnix: number,
): boolean => {
  try {
    assertGrantActive(grant, nowUnix);
    return true;
  } catch (error) {
    if (
      error instanceof SarahCommunityWorkUnitError &&
      error.code === "expired_grant"
    ) {
      return false;
    }
    throw error;
  }
};

// ---------------------------------------------------------------------------
// Budget validation
// ---------------------------------------------------------------------------

const validateBudget = (budget: SarahCommunityWorkUnitBudget): void => {
  if (budget.kind === "experience_tier") {
    if (budget.amount < 1 || budget.amount > 3) {
      throw new SarahCommunityWorkUnitError(
        "invalid_budget",
        "experience_tier amount must be 1, 2, or 3",
      );
    }
    return;
  }
  // msats reserved for paid version; allow zero only as an explicit unpaid
  // placeholder, never negative (schema already enforces non-negative).
  if (budget.amount < 0) {
    throw new SarahCommunityWorkUnitError(
      "invalid_budget",
      "msats amount must be non-negative",
    );
  }
};

// ---------------------------------------------------------------------------
// Builders (templates)
// ---------------------------------------------------------------------------

export type BuildSarahCommunityWorkUnitGrantInput = Readonly<{
  targetRef: string;
  allowedActions: ReadonlyArray<SarahCommunityUnitAction>;
  budget: SarahCommunityWorkUnitBudget;
  expiresAtUnix: number;
  idempotencyId: string;
}>;

export const buildSarahCommunityWorkUnitGrant = (
  input: BuildSarahCommunityWorkUnitGrantInput,
): SarahCommunityWorkUnitGrant => {
  const grant = decodeSarahCommunityWorkUnitGrant({
    schema: SARAH_COMMUNITY_WORK_UNIT_GRANT_SCHEMA,
    authorityClass: SARAH_COMMUNITY_UNIT_AUTHORITY_CLASS,
    targetRef: input.targetRef,
    allowedActions: [...input.allowedActions],
    budget: input.budget,
    expiresAtUnix: input.expiresAtUnix,
    idempotencyId: input.idempotencyId,
  });
  validateBudget(grant.budget);
  assertNoSarahGrant(grant);
  return grant;
};

export type BuildSarahCommunityWorkUnitInput = Readonly<{
  unitRef: string;
  tickRef: string;
  objective: string;
  grant: BuildSarahCommunityWorkUnitGrantInput | SarahCommunityWorkUnitGrant;
  experienceTier: 1 | 2 | 3;
  createdAtUnix: number;
}>;

export const buildSarahCommunityWorkUnit = (
  input: BuildSarahCommunityWorkUnitInput,
): SarahCommunityWorkUnit => {
  const grant =
    "schema" in input.grant &&
    input.grant.schema === SARAH_COMMUNITY_WORK_UNIT_GRANT_SCHEMA
      ? decodeSarahCommunityWorkUnitGrant(input.grant)
      : buildSarahCommunityWorkUnitGrant(
          input.grant as BuildSarahCommunityWorkUnitGrantInput,
        );
  validateBudget(grant.budget);
  if (grant.expiresAtUnix <= input.createdAtUnix) {
    throw new SarahCommunityWorkUnitError(
      "invalid_expiration",
      "grant expiresAtUnix must be strictly after createdAtUnix",
    );
  }
  const unit = decodeSarahCommunityWorkUnit({
    schema: SARAH_COMMUNITY_WORK_UNIT_SCHEMA,
    packet: SARAH_CW_03_PACKET,
    unitRef: input.unitRef,
    tickRef: input.tickRef,
    objective: input.objective,
    grant,
    experienceTier: input.experienceTier,
    createdAtUnix: input.createdAtUnix,
  });
  assertNoSarahGrant(unit);
  return unit;
};

// ---------------------------------------------------------------------------
// Tick decomposition
// ---------------------------------------------------------------------------

/**
 * Candidate unit before validation. The tick produces these; decomposition
 * validates, caps, and freezes them into work units with narrow grants.
 */
export type SarahCommunityWorkUnitCandidate = Readonly<{
  /** Optional; derived from tickRef + index + idempotency when omitted. */
  unitRef?: string;
  objective: string;
  targetRef: string;
  allowedActions: ReadonlyArray<SarahCommunityUnitAction>;
  budget: SarahCommunityWorkUnitBudget;
  /** Unix seconds; default is now + DEFAULT_TTL when omitted. */
  expiresAtUnix?: number;
  idempotencyId: string;
  experienceTier: 1 | 2 | 3;
}>;

export type DecomposeSarahTickInput = Readonly<{
  tickRef: string;
  candidates: ReadonlyArray<SarahCommunityWorkUnitCandidate>;
  /** Unix seconds for createdAt and default expiration. */
  nowUnix: number;
  /**
   * Optional tighter cap (still clamped to MAX_PER_TICK). Defaults to the
   * hard max.
   */
  unitCap?: number;
}>;

/**
 * Decompose one Sarah tick into many bounded community work units.
 *
 * - Caps unit count (hard max {@link SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK}).
 * - Assigns narrow grants only (never Sarah grants).
 * - Refuses candidates whose expiration is already past.
 * - Refuses duplicate idempotency identities within the same tick.
 * - Does not publish, dispatch, or settle.
 */
export const decomposeSarahTickToWorkUnits = (
  input: DecomposeSarahTickInput,
): SarahCommunityTickDecomposition => {
  if (!Number.isInteger(input.nowUnix) || input.nowUnix < 0) {
    throw new SarahCommunityWorkUnitError(
      "invalid_expiration",
      "nowUnix must be a non-negative integer unix second",
    );
  }
  assertNoSarahGrant({ tickRef: input.tickRef });

  const requestedCap =
    input.unitCap === undefined
      ? SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK
      : input.unitCap;
  if (
    !Number.isInteger(requestedCap) ||
    requestedCap < 0 ||
    requestedCap > SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK
  ) {
    throw new SarahCommunityWorkUnitError(
      "unit_cap_exceeded",
      `unitCap must be an integer in 0..${SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK}`,
    );
  }

  const accepted = input.candidates.slice(0, requestedCap);
  const truncatedCandidateCount = Math.max(
    0,
    input.candidates.length - accepted.length,
  );

  const seenIdempotency = new Set<string>();
  const units: SarahCommunityWorkUnit[] = [];

  for (let index = 0; index < accepted.length; index += 1) {
    const candidate = accepted[index]!;
    if (seenIdempotency.has(candidate.idempotencyId)) {
      throw new SarahCommunityWorkUnitError(
        "duplicate_idempotency",
        `duplicate idempotency identity within tick: ${candidate.idempotencyId}`,
      );
    }
    seenIdempotency.add(candidate.idempotencyId);

    const expiresAtUnix =
      candidate.expiresAtUnix ??
      input.nowUnix + SARAH_COMMUNITY_WORK_UNIT_DEFAULT_TTL_SECONDS;

    if (expiresAtUnix <= input.nowUnix) {
      throw new SarahCommunityWorkUnitError(
        "expired_grant",
        `candidate ${candidate.idempotencyId} grant already expired; refuse, do not extend`,
      );
    }
    if (
      expiresAtUnix >
      input.nowUnix + SARAH_COMMUNITY_WORK_UNIT_MAX_TTL_SECONDS
    ) {
      throw new SarahCommunityWorkUnitError(
        "invalid_expiration",
        `candidate ${candidate.idempotencyId} exceeds max grant TTL of ${SARAH_COMMUNITY_WORK_UNIT_MAX_TTL_SECONDS}s`,
      );
    }

    const unitRef =
      candidate.unitRef ??
      `unit.cw.${input.tickRef.replace(/^tick\./, "")}.${index + 1}`;

    try {
      const unit = buildSarahCommunityWorkUnit({
        unitRef,
        tickRef: input.tickRef,
        objective: candidate.objective,
        experienceTier: candidate.experienceTier,
        createdAtUnix: input.nowUnix,
        grant: {
          targetRef: candidate.targetRef,
          allowedActions: candidate.allowedActions,
          budget: candidate.budget,
          expiresAtUnix,
          idempotencyId: candidate.idempotencyId,
        },
      });
      // Fresh units must be active at creation time.
      assertGrantActive(unit.grant, input.nowUnix);
      units.push(unit);
    } catch (error) {
      if (error instanceof SarahCommunityWorkUnitError) throw error;
      throw new SarahCommunityWorkUnitError(
        "invalid_candidate",
        `candidate ${candidate.idempotencyId} failed validation: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }

  const decomposition = decodeSarahCommunityTickDecomposition({
    schema: SARAH_COMMUNITY_TICK_DECOMPOSITION_SCHEMA,
    packet: SARAH_CW_03_PACKET,
    tickRef: input.tickRef,
    decomposedAtUnix: input.nowUnix,
    unitCap: SARAH_COMMUNITY_WORK_UNIT_MAX_PER_TICK,
    units,
    truncatedCandidateCount,
  });
  assertNoSarahGrant(decomposition);
  return decomposition;
};

// ---------------------------------------------------------------------------
// NIP-40 expiration helper (wire template only)
// ---------------------------------------------------------------------------

/**
 * NIP-40 `expiration` tag value for a unit grant (unix seconds as string).
 * Pure helper — does not sign or publish.
 */
export const nip40ExpirationTag = (
  grant: SarahCommunityWorkUnitGrant,
): readonly ["expiration", string] => [
  "expiration",
  String(grant.expiresAtUnix),
];

/**
 * Refuse an expired grant when preparing wire tags. Does not extend the
 * expiration — callers must mint a new unit if work is still needed.
 */
export const wireTagsForActiveGrant = (
  grant: SarahCommunityWorkUnitGrant,
  nowUnix: number,
): ReadonlyArray<readonly [string, string]> => {
  assertGrantActive(grant, nowUnix);
  return [
    nip40ExpirationTag(grant),
    ["i", grant.idempotencyId],
    ["target", grant.targetRef],
    ["authority_class", grant.authorityClass],
  ];
};
