// Khala loop integration — verified-serve (M4) -> Bitcoin/Spark payout (M3).
// (EPIC #6017; M3 #6011 / M4 #6012; Lane B↔Lane E seam; design:
// docs/khala/khala-buildout-roadmap.md "Next-wave delegation" +
// "Payments: Bitcoin-only, and pay the guinea-pig Pylon first".)
//
// PAYMENTS DIRECTION (owner, 2026-06-22): Bitcoin-only, SPARK as the PRIMARY
// payout method (Lightning as the rail). No Stripe, no card funding. This module
// connects the two ALREADY-MERGED, independently-tested halves of the loop:
//
//   M4 (psionic-fabric-serve.ts):  ask-plan -> execute -> EXACT-PARITY receipt
//   M3 (khala-verified-work-settlement.ts):  ARMED decision + receipt -> Spark
//
// Each half is proven on its own; nothing yet proves the WHOLE chain
//
//   serve(parity-verified) -> ServingReceipt -> payout DECISION -> settlement
//   sink -> dereferenceable settled receipt
//
// as one flow against the guinea-pig Pylon. This module owns exactly that seam,
// and keeps it INERT BY DEFAULT.
//
// WHAT THIS MODULE OWNS (and does NOT):
//   - A pluggable PYLON TRANSPORT interface the fabric adapter consumes, so a
//     real Pylon serve transport (HTTP to a live online Pylon) drops in later
//     with no contract change. Today only a local/fake serve is wired (the live
//     fabric is owner-gated / Psionic-planned).
//   - A DRY-RUN settlement dispatch: it records the `realBitcoinMoved`-shaped
//     settlement receipt to an injected ledger so the loop produces a
//     dereferenceable receipt, but NEVER performs a real Spark send.
//   - `makeKhalaLoopSettlementDispatch`: the GATED dispatch SELECTOR that picks
//     between the proven REAL Spark send (`dispatchRealRunSettlementCore`) and the
//     dry-run, per payout. It resolves to the REAL dispatch ONLY when BOTH the
//     loop-arming flag is `armed` AND the M3 owner real-settlement gate authorizes
//     that exact payout (`resolveTassadarSettlementAdapter` -> `realAuthorized`:
//     gate enabled + adapter match + amount <= cap + run allowlisted + within the
//     daily cap). Otherwise it routes to the dry-run path and ZERO sats move. This
//     is the M4 NEEDS-OWNER "swap the dry-run for the real dispatch" step, done
//     fail-closed: the real branch is a SECOND, independent gate evaluation layered
//     on top of the M3 engine's own gate, and is unreachable without the owner's
//     explicit gate JSON + the loop flag. The wiring layer hands the selector a
//     real dispatch (the core, closed over its Spark resolvers) and the dry-run;
//     the gate is CONSUMED here, never changed.
//   - `makeKhalaLoopSettlementSink`: connects the dormant
//     `makeKhalaServingSettlementSink` (M3) to the metering-hook
//     `recordServingPayout` shape, BEHIND A FLAG (`KhalaLoopArming`), default
//     OFF -> the sink is a no-op and nothing settles.
//   - `runKhalaLoopOnce`: the end-to-end integration entrypoint — serve via the
//     fabric dispatch, take the parity receipt, compute the payout decision,
//     and run it through the settlement sink. The whole thing is inert until BOTH
//     the loop-arming flag AND the M3 owner real-settlement gate are armed.
//
// It does NOT edit Lane B's adapter registry (`provider-adapter.ts` / the
// `index.ts` registration) or Lane E's `metering-hook.ts` internals — it builds a
// sink the metering hook can be handed, and a transport the fabric adapter
// consumes, both as new owned surfaces.
//
// SAFETY: real-money discipline is unchanged. The M3 settlement leg keeps its
// own fail-closed gates (parity, asset boundary, owner real-settlement gate,
// per-payout cap, daily ceiling, registered destination). This module adds a
// SECOND, independent default-OFF flag in front of the sink, so arming requires
// (a) this loop flag AND (b) the M3 owner gate. With either OFF the loop is fully
// inert. No raw Spark address / invoice / preimage ever enters this module: the
// guinea-pig Pylon's address is resolved at the wiring/test layer from the
// gitignored `.secrets/khala-test-payout.env`, NEVER hard-coded or committed.

import { Effect } from 'effect'

import type { NexusPaymentAuthorityReceiptRecord } from '../nexus-treasury-payout-ledger'
import { workerLogEntry } from '../observability'
import {
  type ServingReceipt,
  type NetworkServedResult,
} from './openagents-network-adapter'
import {
  type PsionicServeTransport,
  type PsionicFabricServeConfig,
  dispatchPsionicServe,
} from './psionic-fabric-serve'
import {
  type KhalaSettlementDeps,
  type KhalaSettlementOutcome,
  type KhalaSettlementRecords,
  makeKhalaServingSettlementSink,
  settleVerifiedServingPayout,
} from './khala-verified-work-settlement'
import { type InferenceRequest } from './provider-adapter'
import {
  type ServingNodePayoutDecision,
  type ServingRevenueAsset,
  decideServingNodePayout,
  servingContributorCutMsat,
} from './serving-node-payout'
import { type InferenceResaleRefs } from '../inference-resale-authorization'
import { type MdkPayoutModeGateProjection } from '../mdk-payout-mode-gate'
import {
  type PylonAdmissionDecision,
  type PylonServingSnapshot,
  decidePylonAdmission,
} from './khala-pylon-admission'
import {
  type TassadarRealSettlementGate,
  resolveTassadarSettlementAdapter,
} from '../tassadar-run-settlement-gate'

// ----------------------------------------------------------------------------
// (1) Pluggable Pylon transport — the seam a real Pylon drops into
// ----------------------------------------------------------------------------

// The transport the fabric adapter consumes is the M4 `PsionicServeTransport`
// (ask-plan -> execute -> serve response). A "Pylon transport" is just a
// `PsionicServeTransport` produced by some builder: a test passes a LOCAL/FAKE
// serve; a real wiring passes an HTTP client posting to a live online Pylon's
// `psionic-serve` endpoint (the guinea-pig Pylon FIRST). Re-exported under a
// product-named alias so the loop's wiring layer talks "Pylon", while the wire
// contract stays the single M4 type (no parallel transport surface).
export type PylonServeTransport = PsionicServeTransport

// A builder that produces the Pylon transport from its connection inputs. A real
// builder reads the guinea-pig Pylon's endpoint + (out of band) its Spark
// receive address binding; a test builder returns a fixed fake serve. Kept as a
// thin factory type so the live transport is a pure drop-in: the loop only ever
// holds a `PylonServeTransport`, never the connection details.
export type PylonServeTransportBuilder = (
  input: Readonly<{
    // Stable, public-safe ref of the Pylon to serve against (e.g. the guinea-pig
    // node ref). The transport binds this to its endpoint internally; the loop
    // never sees the raw endpoint or address.
    pylonNodeRef: string
  }>,
) => PylonServeTransport

// Build the fabric serve config the M4 dispatch consumes from a Pylon transport.
// This is the one place a Pylon transport becomes a fabric dispatch input, so a
// real Pylon transport drops in with no change to the dispatch or the loop.
export const fabricConfigForPylon = (
  transport: PylonServeTransport,
): PsionicFabricServeConfig => ({ transport })

// ----------------------------------------------------------------------------
// (2) The loop-arming flag — a SECOND default-OFF gate in front of the sink
// ----------------------------------------------------------------------------

// The Worker env key for the loop-arming flag. Default OFF: absent / anything
// other than the explicit on-token keeps the loop fully inert (the settlement
// sink is a no-op). This is independent of, and additional to, the M3 owner
// real-settlement gate (`OPENAGENTS_REAL_SETTLEMENT_GATE`): arming a real payout
// requires BOTH. NEEDS-OWNER to flip on a staging/preview Worker.
export const KhalaLoopArmingEnvKey = 'OPENAGENTS_KHALA_LOOP_ARMED'

// The single on-token. Anything else (including 'true', '1', '') is OFF — the
// flag must be set EXACTLY to this opt-in value, so a stray truthy env never
// arms the loop by accident. Fails closed.
const KHALA_LOOP_ARMED_ON_TOKEN = 'armed'

export type KhalaLoopArming = Readonly<{
  // Whether the loop-arming flag authorizes the settlement sink to run at all.
  // OFF (default) => the sink is a no-op and no decision is forwarded to M3.
  loopArmed: boolean
}>

export const disabledKhalaLoopArming: KhalaLoopArming = { loopArmed: false }

// Resolve the loop-arming flag from the Worker env. Fails CLOSED: absent,
// non-string, or any value other than the exact on-token yields the disabled
// arming. Mirrors the M3 gate's fail-closed env discipline.
export const readKhalaLoopArming = (
  env: Readonly<Record<string, unknown>>,
): KhalaLoopArming => {
  const raw = env[KhalaLoopArmingEnvKey]
  return typeof raw === 'string' && raw.trim() === KHALA_LOOP_ARMED_ON_TOKEN
    ? { loopArmed: true }
    : disabledKhalaLoopArming
}

// ----------------------------------------------------------------------------
// (3) Dry-run settlement dispatch — produces a receipt, NEVER a real Spark send
// ----------------------------------------------------------------------------

// A minimal ledger sink the dry-run dispatch writes the settled-shaped receipt
// to, so the loop produces a DEREFERENCEABLE receipt without a real money send.
// The wiring layer passes a real ledger store (the same `NexusTreasuryPayoutLedgerStore`
// the M3 deps already carry); a test passes an in-memory map. Idempotent: a
// replay with the same receipt ref is a no-op (never double-records).
export type DryRunSettlementLedger = Readonly<{
  readReceiptByRef: (
    ref: string,
  ) => Promise<NexusPaymentAuthorityReceiptRecord | undefined>
  recordReceipt: (record: NexusPaymentAuthorityReceiptRecord) => Promise<void>
}>

// Build the DRY-RUN `dispatchRealSettlement` the M3 settlement leg consumes. It
// is shaped EXACTLY like the real receipt-first idempotent dispatch
// (`dispatchRealRunSettlementCore`): short-circuit if the settlement receipt
// already exists (idempotency), else record the `realBitcoinMoved`-shaped
// settlement receipt. The ONLY difference from the real path is that it performs
// NO Spark send — it records the receipt and stops. A real dispatch is a
// NEEDS-OWNER drop-in here. Fail-soft for the caller (the M3 leg already wraps it
// in `orElseSucceed`).
export const makeDryRunSettlementDispatch =
  (ledger: DryRunSettlementLedger) =>
  (input: {
    contributorRef: string
    settlement: KhalaSettlementRecords
  }): Effect.Effect<void, unknown> =>
    Effect.gen(function* () {
      const existing = yield* Effect.promise(() =>
        ledger.readReceiptByRef(input.settlement.settlementReceiptRef),
      )
      if (existing !== undefined) {
        // Already recorded for this run+node => idempotent no-op (never re-pays).
        return
      }
      // DRY-RUN: record the dereferenceable settled receipt; perform NO real Spark
      // send. Public-safe diagnostic only (run ref + amount sats + contributor;
      // never an address / invoice / preimage).
      yield* Effect.logInfo(
        workerLogEntry('inference.khala_loop.dry_run_settlement', {
          amountSats: input.settlement.amountSats,
          contributorRef: input.contributorRef,
          settlementReceiptRef: input.settlement.settlementReceiptRef,
        }),
      )
      yield* Effect.promise(() =>
        ledger.recordReceipt(input.settlement.settlementReceipt),
      )
    })

// ----------------------------------------------------------------------------
// (3b) Gated dispatch SELECTOR — REAL Spark send when armed + gate authorizes,
//      otherwise the dry-run receipt. The owner's M4 NEEDS_OWNER drop-in.
// ----------------------------------------------------------------------------

// The settlement-dispatch shape the M3 engine consumes (`KhalaSettlementDeps.
// dispatchRealSettlement`): receipt-first, idempotent, fail-soft. Both the
// dry-run dispatch and a real `dispatchRealRunSettlementCore`-backed dispatch
// satisfy this shape, so the selector can pick between them per call without the
// engine knowing which path it got.
export type KhalaSettlementDispatch = (input: {
  contributorRef: string
  settlement: KhalaSettlementRecords
}) => Effect.Effect<void, unknown>

// Inputs to the gated dispatch selector. `realDispatch` is the REAL Spark send —
// at the wiring layer it is `dispatchRealRunSettlementCore` adapted to the
// `KhalaSettlementDispatch` shape (resolvers closed over; settlement passed
// through). `dryRunDispatch` is `makeDryRunSettlementDispatch` (records a receipt,
// NEVER a Spark send). `readGate` reads the owner real-settlement gate
// (`OPENAGENTS_REAL_SETTLEMENT_GATE`, default DISABLED). `settlementRunRef` is the
// run allowlist key the gate checks (the SAME ref the M3 engine threads as
// `KhalaSettlementDeps.settlementRunRef`, so the selector's gate evaluation
// matches the engine's GATE 3 exactly).
export type KhalaLoopDispatchSelectorInput = Readonly<{
  arming: KhalaLoopArming
  dryRunDispatch: KhalaSettlementDispatch
  readGate: () => TassadarRealSettlementGate
  realDispatch: KhalaSettlementDispatch
  settlementRunRef: string
}>

// Build the loop's settlement dispatch. It resolves to the REAL Spark dispatch
// ONLY when BOTH:
//   (a) the loop-arming flag is armed (`arming.loopArmed` — the FIRST owner gate);
//       AND
//   (b) the M3 owner real-settlement gate AUTHORIZES this exact payout
//       (`resolveTassadarSettlementAdapter(...).realAuthorized`: gate enabled +
//       requested adapter == `spark_treasury` + amount <= `maxPayoutSats` + run
//       allowlisted + contributor eligible — the SECOND owner gate).
// Otherwise it resolves to the DRY-RUN dispatch (records a receipt, NO real send).
//
// This is a SECOND, independent fail-closed evaluation of the gate at the dispatch
// boundary, layered on top of the M3 engine's own GATE 3 (`settleVerifiedServing-
// Payout` only calls `dispatchRealSettlement` after its own `resolveTassadar-
// SettlementAdapter` + cap + daily-budget + destination checks pass). So the real
// branch is unreachable unless EVERY gate — loop flag, owner gate, per-payout cap,
// daily cap, run allowlist, adapter match, registered destination — holds. With
// any of them missing/disabled/out-of-bounds the selector routes to the dry-run
// path and ZERO sats move. The gate is CONSUMED here, never mutated.
//
// Receipt-first + idempotent is preserved by construction: both `realDispatch`
// (`dispatchRealRunSettlementCore`) and `dryRunDispatch` short-circuit when the
// deterministic settlement receipt already exists, so a replay with the same
// settlement receipt ref never double-sends regardless of which branch is taken.
//
// NOTE: `amountSats` is read from the built records (`settlement.amountSats`),
// which the engine derived from the per-share floor-to-sat split — the SAME value
// the engine's own gate evaluation used, so the two cap checks agree.
export const makeKhalaLoopSettlementDispatch = (
  input: KhalaLoopDispatchSelectorInput,
): KhalaSettlementDispatch =>
  dispatchInput =>
    Effect.gen(function* () {
      // Evaluate the owner real-settlement gate for THIS payout (pure, fail-closed).
      const gate = input.readGate()
      const decision = resolveTassadarSettlementAdapter({
        amountSats: dispatchInput.settlement.amountSats,
        contributorRef: dispatchInput.contributorRef,
        gate,
        requestedAdapterKind: 'spark_treasury',
        trainingRunRef: input.settlementRunRef,
      })

      const realRoute = input.arming.loopArmed && decision.realAuthorized
      if (!realRoute) {
        // Fail-closed default: loop flag OFF, gate disabled, or any bound failed.
        // Record a dry-run receipt; perform NO real Spark send. Public-safe
        // diagnostic only (refs + neutral blocker reason; never payment material).
        yield* Effect.logInfo(
          workerLogEntry('inference.khala_loop.dispatch_dry_run', {
            blockedReason: decision.blockedReason,
            contributorRef: dispatchInput.contributorRef,
            loopArmed: input.arming.loopArmed,
            settlementReceiptRef:
              dispatchInput.settlement.settlementReceiptRef,
          }),
        )
        yield* input.dryRunDispatch(dispatchInput)
        return
      }

      // ARMED + AUTHORIZED: route to the REAL Spark dispatch (receipt-first +
      // idempotent inside the core). Public-safe diagnostic only.
      yield* Effect.logInfo(
        workerLogEntry('inference.khala_loop.dispatch_real', {
          amountSats: dispatchInput.settlement.amountSats,
          contributorRef: dispatchInput.contributorRef,
          eligibilitySource: decision.eligibilitySource,
          settlementReceiptRef:
            dispatchInput.settlement.settlementReceiptRef,
        }),
      )
      yield* input.realDispatch(dispatchInput)
    })

// ----------------------------------------------------------------------------
// (2 cont.) The flagged settlement sink — connects M3 to the metering path
// ----------------------------------------------------------------------------

// The `recordServingPayout` sink the metering hook (`metering-hook.ts`
// `LedgerMeteringDeps.recordServingPayout`) forwards an ARMED serving-payout
// decision + receipt to. This is the wiring point that turns the dormant M3
// `makeKhalaServingSettlementSink` into the metering-path sink — BEHIND THE LOOP
// FLAG. With the flag OFF (default) the returned sink is a no-op: it logs that the
// loop is disarmed and forwards NOTHING to M3, so the metering hook can carry it
// fire-and-forget with zero live-money risk. With the flag ON it delegates to the
// M3 sink, which independently re-checks its OWN owner real-settlement gate
// (still default OFF) + caps + destination — so arming a real payout requires
// BOTH this loop flag AND the M3 gate.
export const makeKhalaLoopSettlementSink = (
  input: Readonly<{
    arming: KhalaLoopArming
    settlementDeps: KhalaSettlementDeps
  }>,
): ((
  decision: ServingNodePayoutDecision,
  receipt: ServingReceipt,
) => Effect.Effect<void>) => {
  const m3Sink = makeKhalaServingSettlementSink(input.settlementDeps)
  return (decision, receipt) =>
    Effect.gen(function* () {
      if (!input.arming.loopArmed) {
        // Loop flag OFF => inert. Public-safe diagnostic (refs only); forward
        // nothing to M3.
        yield* Effect.logInfo(
          workerLogEntry('inference.khala_loop.disarmed', {
            servingRunRef: decision.servingRunRef,
          }),
        )
        return
      }
      yield* m3Sink(decision, receipt)
    })
}

// ----------------------------------------------------------------------------
// (3 cont.) The end-to-end loop entrypoint
// ----------------------------------------------------------------------------

// How the served request's revenue was sourced for the RL-3 asset-boundary check
// in the payout decision. Bitcoin-only this wave: a Bitcoin-funded request
// produces withdrawable Bitcoin serving share. Defaulted by the caller.
export type KhalaLoopRevenueAsset = ServingRevenueAsset

export type KhalaLoopOutcome = Readonly<{
  // The M4 Pylon admission decision (capability + heartbeat + wallet/payout
  // readiness), when an admission snapshot was supplied. null when the caller did
  // not gate on admission (e.g. the local/fake test path). When present and NOT
  // admitted, the loop refused to route to the Pylon: `served`/`receipt`/
  // `decision` are null and `admittedAndServed` is false (safe fall-back to the
  // existing cloud adapters happens at the caller).
  admission: PylonAdmissionDecision | null
  // Whether the Pylon was admitted AND actually served. False when admission
  // refused (no Pylon routing) — the honest "fell back to cloud" signal.
  admittedAndServed: boolean
  // The parity-verified serving receipt the fabric dispatch produced. null when
  // admission refused (no serve was attempted).
  receipt: ServingReceipt | null
  // The full served completion result (content + receipt-first usage). null when
  // admission refused.
  served: NetworkServedResult | null
  // The computed (possibly unarmed) per-stage payout decision. null when
  // admission refused (nothing to settle).
  decision: ServingNodePayoutDecision | null
  // The settlement outcome from the M3 sink. Present only when the loop flag is
  // armed AND the decision was armed; otherwise null (the loop short-circuited
  // before forwarding to M3). A null outcome with an unarmed decision is the
  // honest inert default.
  settlement: KhalaSettlementOutcome | null
  // Whether the loop forwarded to M3 at all (admitted + served + loop flag armed
  // AND decision armed).
  forwardedToSettlement: boolean
}>

export type KhalaLoopConfig = Readonly<{
  // The Pylon serve transport (local/fake in tests, HTTP to a live Pylon later).
  transport: PylonServeTransport
  // The loop-arming flag (default OFF).
  arming: KhalaLoopArming
  // The M3 settlement deps (ledger, destination resolver, owner gate, dispatch).
  // The wiring layer builds `dispatchRealSettlement` via the GATED selector
  // (`makeKhalaLoopSettlementDispatch`): it routes to the REAL Spark dispatch
  // (`dispatchRealRunSettlementCore`) when armed + the owner gate authorizes,
  // else to the dry-run. A test may pass the dry-run directly to assert the
  // money-free path. Either way the M3 engine independently re-checks its own
  // gate stack before invoking the dispatch.
  settlementDeps: KhalaSettlementDeps
  // The contributor cut to split, in integer msat. Derived upstream from the
  // priced margin (`servingContributorCutMsat`); the test passes a tiny,
  // treasury-bounded value (e.g. 5_000 msat = 5 sats).
  contributorCutMsat: number
  // The revenue asset for the RL-3 boundary (bitcoin this wave).
  revenueAsset: KhalaLoopRevenueAsset
  // The owner-armed MDK payout-mode gate the payout DECISION consults (the FIRST
  // owner gate; the M3 settlement leg consults the SECOND, independent
  // real-settlement gate). Default-disabled keeps the decision unarmed.
  payoutGate: MdkPayoutModeGateProjection
  // RL-3 resale-authorization refs for the api_inference_gateway_resale lane.
  // Required for the decision to ARM; omitted in the honest inert default.
  resaleRefs?: Partial<InferenceResaleRefs> | undefined
  // M4 ADMISSION GATE (the issue's "capability/registration" task). When present,
  // the loop checks the candidate Pylon is ADMITTED (advertises the required
  // capability + has a fresh healthy heartbeat + is wallet/payout ready) BEFORE
  // dispatching to its transport. A non-admitted Pylon is NOT routed to: the loop
  // short-circuits with `admittedAndServed: false` and the caller falls back to
  // the existing cloud adapters — never a serve/pay against an unready node. When
  // omitted (the local/fake test path), no admission gate runs (admission: null).
  admission?:
    | Readonly<{
        snapshot: PylonServingSnapshot
        requiredCapabilityRef: string
        nowMs: number
        heartbeatTtlMs?: number | undefined
      }>
    | undefined
}>

// Run the loop ONCE for a single inference request: serve via the M4 fabric
// dispatch (parity-gated — no parity, no result), compute the per-stage payout
// decision from the parity receipt, and run it through the flagged M3 settlement
// sink. The serve FAILS CLOSED on a malformed/unverified/sharded serve (the M4
// dispatch's typed error propagates). Once a parity-verified receipt is in hand,
// the settlement leg is fail-soft + idempotent and never throws into the caller.
//
// INERT BY DEFAULT: with the loop flag OFF (default) the sink forwards nothing to
// M3 (`settlement: null`, `forwardedToSettlement: false`). Even with the loop
// flag ON, the decision only arms with the MDK gate + RL-3 refs, and the M3 leg
// only settles for real with its OWN owner gate armed + caps + a registered
// destination. No real sats move unless EVERY gate is armed; the dry-run dispatch
// records a receipt without a real Spark send.
export const runKhalaLoopOnce = (
  config: KhalaLoopConfig,
  request: InferenceRequest,
): Effect.Effect<KhalaLoopOutcome, unknown> =>
  Effect.gen(function* () {
    // M4 ADMISSION GATE (capability + heartbeat + wallet/payout readiness). When
    // an admission snapshot is supplied, decide whether the candidate Pylon is
    // admitted BEFORE touching the transport. A non-admitted Pylon is NOT routed
    // to: short-circuit with no serve, no receipt, no decision, no settlement —
    // the caller falls back to the existing cloud adapters. PURE + fail-closed.
    const admission =
      config.admission === undefined
        ? null
        : decidePylonAdmission({
            nowMs: config.admission.nowMs,
            requiredCapabilityRef: config.admission.requiredCapabilityRef,
            snapshot: config.admission.snapshot,
            ...(config.admission.heartbeatTtlMs === undefined
              ? {}
              : { heartbeatTtlMs: config.admission.heartbeatTtlMs }),
          })

    if (admission !== null && !admission.admitted) {
      yield* Effect.logInfo(
        workerLogEntry('inference.khala_loop.pylon_not_admitted', {
          blockerCount: admission.blockerRefs.length,
          pylonRef: admission.pylonRef,
        }),
      )
      return {
        admission,
        admittedAndServed: false,
        decision: null,
        forwardedToSettlement: false,
        receipt: null,
        served: null,
        settlement: null,
      }
    }

    // M4: serve through the parity-gated fabric dispatch. We need the RECEIPT
    // (not just the completion), so we call the dispatch directly rather than
    // through the InferenceProviderAdapter (which surfaces only the result).
    const served = yield* dispatchPsionicServe(
      fabricConfigForPylon(config.transport),
    )(request)
    const receipt = served.receipt

    // Compute the per-stage payout DECISION from the parity receipt. PURE +
    // owner-armed-gated: armed only with the MDK gate AND the RL-3 ref chain.
    const decision = decideServingNodePayout({
      contributorCutMsat: config.contributorCutMsat,
      payoutGate: config.payoutGate,
      receipt,
      revenueAsset: config.revenueAsset,
      ...(config.resaleRefs === undefined
        ? {}
        : { resaleRefs: config.resaleRefs }),
    })

    yield* Effect.logInfo(
      workerLogEntry('inference.khala_loop.served', {
        armed: decision.armed,
        loopArmed: config.arming.loopArmed,
        parityVerified: receipt.parityVerified,
        servingRunRef: decision.servingRunRef,
        stageCount: receipt.stages.length,
      }),
    )

    // Forward to the flagged M3 sink only when the loop flag is armed AND the
    // decision is armed (mirrors the metering hook's own armed-only forwarding).
    const forwardedToSettlement = config.arming.loopArmed && decision.armed
    if (!forwardedToSettlement) {
      return {
        admission,
        admittedAndServed: true,
        decision,
        forwardedToSettlement: false,
        receipt,
        served,
        settlement: null,
      }
    }

    // Settle (dry-run): the M3 leg builds + records the realBitcoinMoved-shaped
    // receipt through the (dry-run) dispatch. We call the M3 entrypoint via the
    // flagged sink wrapper so the contract stays the metering-hook sink shape,
    // then re-derive the outcome by running the same leg directly so the caller
    // sees the structured settlement outcome (the sink itself returns void to fit
    // the fire-and-forget metering contract).
    const sink = makeKhalaLoopSettlementSink({
      arming: config.arming,
      settlementDeps: config.settlementDeps,
    })
    // Run the void sink for its side effects (the actual ledger write/dispatch),
    // proving the metering-hook-shaped path fires; this performs the dry-run
    // record.
    yield* sink(decision, receipt)
    // Then obtain the STRUCTURED outcome for the caller by running the same M3
    // leg again. The settlement leg is idempotent (the receipt now exists), so
    // this second call records nothing new and simply reports the leg outcomes.
    const settlement = yield* settleVerifiedServingPayout(config.settlementDeps, {
      decision,
      parityVerified: receipt.parityVerified,
      servedModel: receipt.servedModel,
    })

    return {
      admission,
      admittedAndServed: true,
      decision,
      forwardedToSettlement: true,
      receipt,
      served,
      settlement,
    }
  })

// Re-export the contributor-cut helper so a wiring layer derives the cut with the
// SAME function the metering path uses (no parallel cut math).
export { servingContributorCutMsat }
