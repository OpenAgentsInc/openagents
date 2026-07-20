/**
 * IDR-02 fail-closed identity open, plus the SEPARATE explicit create operation.
 *
 * `openIdentity` resolves an EXISTING identity from existence-only discovery, or
 * it fails closed. It drives the IDR-01 recovery state machine from `Idle`:
 *
 * - No candidate  -> `NoCandidateFound` (terminal). It NEVER mints a root.
 * - A symbolic link candidate only -> `Blocked("link_check_failed")`.
 * - A weak-permission candidate only -> `Blocked("permission_denied")`.
 * - One or more admissible candidates -> `CandidateDiscovered`, ready for the
 *   authorized read and reconciliation of a LATER packet (IDR-03/IDR-04).
 *
 * `openIdentity` has NO reference to the creator port. By construction it cannot
 * create a root, and the state machine rejects a custody import from any state a
 * "no candidate" open can reach.
 *
 * `createIdentityRoot` is a DIFFERENT, explicit operation. A caller must
 * intentionally acknowledge that it creates a NEW root, and it delegates to an
 * injected `IdentityRootCreator`. Open never calls it.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Context, Effect, Schema as S } from "effect";
import type { IdentityRef } from "../contract/index.ts";
import {
  type CandidateDiagnostic,
  type IdentityCandidateSource,
  summarizeDiagnostics,
} from "./discovery.ts";
import {
  RecoveryEvent,
  type RecoveryState,
  applyRecoveryEvent,
  initialRecoveryState,
} from "./recovery-state.ts";

/**
 * The outcome of a fail-closed open. It carries the final recovery state, the
 * public-safe diagnostics, and the admissible-candidate count. The state is
 * never `CustodyImported`: open resolves an existing candidate or stops.
 */
export interface OpenIdentityOutcome {
  readonly state: RecoveryState;
  readonly diagnostics: ReadonlyArray<CandidateDiagnostic>;
  readonly admissibleCount: number;
}

/** Fold a legal event into the state, or throw a defect if the machine rejects it. */
const advance = (state: RecoveryState, event: RecoveryEvent): RecoveryState => {
  const outcome = applyRecoveryEvent(state, event);
  if (!outcome.ok) {
    // This is unreachable: `openIdentity` only issues the discovery events the
    // machine accepts from `Idle`/`Discovering`. A rejection here is a defect.
    throw new Error(`recovery state machine rejected an open transition: ${outcome.reason}`);
  }
  return outcome.state;
};

/**
 * Open an EXISTING identity, or fail closed. It runs existence-only discovery
 * and folds the result into the recovery state machine. It reads no secret bytes
 * and creates nothing.
 */
export const openIdentity = Effect.fn("SovereignIdentity.openIdentity")(function* (
  source: IdentityCandidateSource,
) {
  const diagnostics = yield* source.discover();
  const { admissible, blockers } = summarizeDiagnostics(diagnostics);

  let state = advance(initialRecoveryState, RecoveryEvent.BeginDiscovery());

  if (admissible.length > 0) {
    state = advance(state, RecoveryEvent.CandidatesFound({ count: admissible.length }));
  } else if (blockers.includes("link_refused")) {
    state = advance(state, RecoveryEvent.EncounteredBlocker({ blocker: "link_check_failed" }));
  } else if (blockers.includes("weak_permissions")) {
    state = advance(state, RecoveryEvent.EncounteredBlocker({ blocker: "permission_denied" }));
  } else {
    state = advance(state, RecoveryEvent.NoCandidates());
  }

  return {
    state,
    diagnostics,
    admissibleCount: admissible.length,
  } satisfies OpenIdentityOutcome;
});

// ---------------------------------------------------------------------------
// The SEPARATE explicit create operation
// ---------------------------------------------------------------------------

/** A typed create failure. It never carries secret material. */
export class CreateRootError extends S.TaggedErrorClass<CreateRootError>()(
  "sovereign-identity.CreateRootError",
  {
    reason: S.Literals(["intent_not_acknowledged", "creator_unavailable", "create_failed"]),
  },
) {}

/**
 * The explicit intent to create a NEW root. A caller MUST set
 * `acknowledgeCreatesNewRoot` to `true` on purpose. The field is required, so a
 * create can never happen as a side effect of an open path; an unacknowledged
 * intent is refused with `intent_not_acknowledged`.
 */
export interface CreateIdentityRootIntent {
  readonly acknowledgeCreatesNewRoot: boolean;
}

/** The public result of a successful create. It carries the new identity reference only. */
export interface CreatedIdentityRoot {
  readonly identityRef: IdentityRef;
}

/**
 * The create port. A custody/composition root (IDR-05) implements it and holds
 * the only authority that writes a new root secret. `openIdentity` never
 * resolves this tag.
 */
export interface IdentityRootCreatorInterface {
  readonly create: (
    intent: CreateIdentityRootIntent,
  ) => Effect.Effect<CreatedIdentityRoot, CreateRootError>;
}

/** The `IdentityRootCreator` service tag. A normal open path never resolves it. */
export class IdentityRootCreator extends Context.Service<
  IdentityRootCreator,
  IdentityRootCreatorInterface
>()("sovereign-identity.IdentityRootCreator") {}

/**
 * Create a NEW identity root. This is the SEPARATE explicit operation. It refuses
 * an unacknowledged intent and otherwise delegates to the injected creator. It
 * shares no code path with `openIdentity`, so opening can never create.
 */
export const createIdentityRoot = Effect.fn("SovereignIdentity.createIdentityRoot")(function* (
  intent: CreateIdentityRootIntent,
) {
  if (intent.acknowledgeCreatesNewRoot !== true) {
    return yield* new CreateRootError({ reason: "intent_not_acknowledged" });
  }
  const creator = yield* IdentityRootCreator;
  return yield* creator.create(intent);
});
