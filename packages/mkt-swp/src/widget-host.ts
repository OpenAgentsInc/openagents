/**
 * The widget host controller (SWAP-0, openagents#9315).
 *
 * One place where events fold into the typed widget state and the engine
 * boundary is spoken to. Renderers subscribe to `state` and dispatch; they
 * never call the engine directly, so the verify-before-fund gate cannot be
 * bypassed by a view.
 *
 * The route mount (SWAP-7, #9322) provides the engine layer — the wasm
 * binding via `engineLayer` in production, `fixtureEngineLayer` in tests and
 * on dev/staging — plus `EntropySource.webCryptoLayer`.
 */
import { Effect, SubscriptionRef } from "effect";
import { fundingGate } from "@openagentsinc/mkt-swp-compare";
import type { FundingGate } from "@openagentsinc/mkt-swp-compare";
import { classifySwpState } from "@openagentsinc/mkt-swp-status";
import type { StatusState } from "@openagentsinc/nip-mkt";
import { generateIdempotencyKey } from "@openagentsinc/nip-mkt";
import type { SwpErrorIdentifier } from "@openagentsinc/swap-i18n";
import { composeWidgetState } from "./compose.js";
import type { WidgetInputs } from "./compose.js";
import { EntropySource } from "./entropy-source.js";
import { AuthorizeFundingRequestSchema, SwapEngine } from "./swap-engine.js";
import type {
  EngineDescription,
  ExitPackageDescriptor,
  FundingAuthorization,
  SessionRecords,
  TransactionTemplate,
} from "./swap-engine.js";
import {
  SwapWidgetEvent,
  initialSwapWidgetState,
  transitionSwapWidgetState,
} from "./widget-state.js";
import type { SessionBaseState, SessionProgressState, SwapWidgetState } from "./widget-state.js";

/**
 * `widget-state.ts` restates the NIP-MKT base `state` vocabulary locally so
 * the presentation graph never imports the relay stack. These two assertions
 * fail to compile if the sets ever diverge in either direction.
 */
type AssertSameStates = [
  StatusState extends SessionBaseState ? true : never,
  SessionBaseState extends StatusState ? true : never,
];
const _sessionBaseStatesMatchNipMkt: AssertSameStates = [true, true];
void _sessionBaseStatesMatchNipMkt;

export type SubmitOrderOutcome =
  | {
      readonly _tag: "FundingAuthorized";
      readonly authorization: FundingAuthorization;
      readonly exitPackage: ExitPackageDescriptor;
      readonly fundingTransaction: TransactionTemplate;
      readonly idempotencyKey: string;
    }
  | { readonly _tag: "Refused"; readonly identifier: SwpErrorIdentifier }
  | { readonly _tag: "NotReady"; readonly state: SwapWidgetState };

export interface SwapWidgetController {
  readonly state: SubscriptionRef.SubscriptionRef<SwapWidgetState>;
  readonly current: Effect.Effect<SwapWidgetState>;
  /** Public-safe engine provenance for the SWAP-7 build-provenance line. */
  readonly engineDescription: Effect.Effect<EngineDescription>;
  /** Re-derive the pre-order state from the sibling packages' verdicts. */
  readonly updateInputs: (inputs: WidgetInputs) => Effect.Effect<SwapWidgetState>;
  /** Run verify-before-fund and fold SWAP-3's gate over the result. */
  readonly verifyTerms: (
    records: SessionRecords,
  ) => Effect.Effect<{ readonly gate: FundingGate | null; readonly state: SwapWidgetState }>;
  /** Fold one profile `swp_state` claim through SWAP-6's §9 classification. */
  readonly applySwpState: (swpState: string) => Effect.Effect<SwapWidgetState>;
  /**
   * Drive order → verification → exit package → funding authorisation. The
   * host passes signed records through opaquely and never computes a
   * profile-level verdict; any engine refusal folds back into the state.
   */
  readonly submitOrder: (records: SessionRecords) => Effect.Effect<SubmitOrderOutcome>;
}

export const makeSwapWidgetController: Effect.Effect<
  SwapWidgetController,
  never,
  SwapEngine.Service | EntropySource.Service
> = Effect.gen(function* () {
  const engine = yield* SwapEngine.Service;
  const entropy = yield* EntropySource.Service;
  const state = yield* SubscriptionRef.make(initialSwapWidgetState);

  const apply = (event: SwapWidgetEvent): Effect.Effect<SwapWidgetState> =>
    SubscriptionRef.updateAndGet(state, (current) => transitionSwapWidgetState(current, event));

  const updateInputs = Effect.fn("MktSwpWidgetHost.updateInputs")(function* (inputs: WidgetInputs) {
    return yield* apply(SwapWidgetEvent.FormRederived({ state: composeWidgetState(inputs) }));
  });

  const applySwpState = Effect.fn("MktSwpWidgetHost.applySwpState")(function* (swpState: string) {
    const classified = classifySwpState(swpState);
    if (!classified.ok) {
      // `swp_status_transition_invalid`, a local-only projection, or an
      // unknown value: retained by SWAP-6, advancing nothing here.
      return yield* SubscriptionRef.get(state);
    }
    const sessionState: SessionProgressState =
      swpState === "unresolved" ? "unresolved" : classified.base;
    return yield* apply(SwapWidgetEvent.SessionAdvanced({ state: sessionState }));
  });

  const verifyTerms = Effect.fn("MktSwpWidgetHost.verifyTerms")(function* (
    records: SessionRecords,
  ) {
    yield* engine.openSession(records).pipe(Effect.option);
    const report = yield* engine
      .verifySession({
        quoteEventId: records.quoteEventId,
        orderEventId: records.orderEventId,
        epoch: records.epoch,
      })
      .pipe(Effect.option);
    const gate = report._tag === "Some" ? fundingGate(report.value, records.epoch) : null;
    const next =
      gate === null
        ? yield* SubscriptionRef.get(state)
        : yield* apply(SwapWidgetEvent.FundingGateChanged({ gate }));
    return { gate, state: next };
  });

  const refuse = Effect.fn("MktSwpWidgetHost.refuse")(function* (identifier: SwpErrorIdentifier) {
    yield* apply(SwapWidgetEvent.EngineRefused({ identifier }));
    return { _tag: "Refused", identifier } as const;
  });

  const submitOrder = Effect.fn("MktSwpWidgetHost.submitOrder")(function* (
    records: SessionRecords,
  ) {
    const before = yield* SubscriptionRef.get(state);
    if (before._tag !== "Ready") return { _tag: "NotReady", state: before } as const;
    yield* apply(SwapWidgetEvent.SubmitPressed());
    const idempotencyKey = generateIdempotencyKey(yield* entropy.bytes(32));

    for (const record of [records.quote, records.order, ...records.swapContracts]) {
      const verdict = yield* engine
        .validateProfileRecord(record)
        .pipe(
          Effect.catchTag("MktSwp.SwapEngineError", (error) =>
            Effect.succeed({ _tag: "Refused", identifier: error.identifier } as const),
          ),
        );
      if (verdict._tag === "Refused") return yield* refuse(verdict.identifier);
    }

    return yield* Effect.gen(function* () {
      yield* engine.openSession(records);
      const exitPackage = yield* engine.buildExitPackage(records.sessionId);
      const authorization = yield* engine.authorizeFunding(
        AuthorizeFundingRequestSchema.make({ sessionId: records.sessionId, exitPackage }),
      );
      const fundingTransaction = yield* engine.constructFundingTransaction(authorization);
      yield* apply(SwapWidgetEvent.FundingAuthorized({ authorization }));
      return {
        _tag: "FundingAuthorized",
        authorization,
        exitPackage,
        fundingTransaction,
        idempotencyKey,
      } as const;
    }).pipe(Effect.catchTag("MktSwp.SwapEngineError", (error) => refuse(error.identifier)));
  });

  return {
    state,
    current: SubscriptionRef.get(state),
    engineDescription: engine.describe(),
    updateInputs,
    verifyTerms,
    applySwpState,
    submitOrder,
  };
});
