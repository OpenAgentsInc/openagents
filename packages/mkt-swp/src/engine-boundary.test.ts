// Oracles for the behaviour contract
// `openagents_web.swap_widget.funding_gate.v1`
// (packages/behavior-contracts/src/market-swap-widget.ts): funding stays
// unreachable while any verify-before-fund row is unresolved or failed.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, test } from "vite-plus/test";
import { Effect, Layer } from "effect";
import { fundingGate } from "@openagentsinc/mkt-swp-compare";
import { initialEntryState } from "@openagentsinc/mkt-swp-destination";
import type { DestinationEntryState } from "@openagentsinc/mkt-swp-destination";
import { encodeTestInvoice } from "@openagentsinc/mkt-swp-destination/testkit";
import type { WidgetInputs } from "./compose.js";
import { EntropySource } from "./entropy-source.js";
import { fixtureEngineLayer, fixtureSwapSession } from "./fixture-engine.js";
import { ENGINE_CONTRACT_VERSION, SwapEngine } from "./swap-engine.js";
import { makeSwapWidgetController } from "./widget-host.js";

/**
 * Everything SWAP-1/2/3 answer for a session that is ready except for the
 * verify-before-fund result, which the engine has not produced yet. This is
 * the real pre-verification composition, so the suite drives the same path
 * the mounted widget does rather than a shortcut into the state machine.
 */
const boundDestination: DestinationEntryState = {
  ...initialEntryState("lightning", "regtest"),
  epoch: 1,
  text: encodeTestInvoice(),
  bound: {
    kind: "bolt11_invoice",
    rail: "lightning",
    invoice: encodeTestInvoice(),
  } as DestinationEntryState["bound"],
  verification: { status: "verified", epoch: 1 },
};

const preVerificationInputs: WidgetInputs = {
  online: true,
  engine: { status: "ready" },
  pairsLoaded: true,
  pairGate: { enabled: true, amountSats: 50_000n },
  outputAmountSats: 49_000n,
  quote: { status: "selected" },
  destination: boundDestination,
  fundingGate: null,
};

const hostLayer = (engine: ReturnType<typeof fixtureEngineLayer>) =>
  Layer.merge(engine, EntropySource.webCryptoLayer);

const run = <A, E>(
  engine: ReturnType<typeof fixtureEngineLayer>,
  program: Effect.Effect<A, E, SwapEngine.Service | EntropySource.Service>,
): Promise<A> => Effect.runPromise(program.pipe(Effect.provide(hostLayer(engine))));

describe("engine boundary round trip", () => {
  test("a fixture session drives verification -> funding-authorised without the host computing a profile-level verdict", async () => {
    await run(
      fixtureEngineLayer(),
      Effect.gen(function* () {
        const controller = yield* makeSwapWidgetController;

        // SWAP-1/2/3 compose to "everything but the engine verdict".
        const composed = yield* controller.updateInputs(preVerificationInputs);
        expect(composed._tag).toBe("VerificationPending");

        const verified = yield* controller.verifyTerms(fixtureSwapSession);
        expect(verified.gate?.enabled).toBe(true);
        expect(verified.state._tag).toBe("Ready");

        const outcome = yield* controller.submitOrder(fixtureSwapSession);
        expect(outcome._tag).toBe("FundingAuthorized");
        if (outcome._tag !== "FundingAuthorized") return;
        expect(outcome.authorization.sessionId).toBe(fixtureSwapSession.sessionId);
        expect(outcome.authorization.reportEpoch).toBe(fixtureSwapSession.epoch);
        expect(outcome.exitPackage.contractDigest).toBe(outcome.authorization.contractDigest);
        expect(outcome.exitPackage.packageDigest).toBe(outcome.authorization.exitPackageDigest);
        expect(outcome.fundingTransaction.template.startsWith("fixture-unsigned-tx:")).toBe(true);
        expect(outcome.idempotencyKey).toMatch(/^[0-9a-f]{64}$/);

        expect((yield* controller.current)._tag).toBe("AwaitingFunding");

        const description = yield* controller.engineDescription;
        expect(description.contractVersion).toBe(ENGINE_CONTRACT_VERSION);
        expect(description.profile).toBe("mkt-swp");
      }),
    );
  });

  test("the engine satisfies SWAP-3's FundVerifier port, so one report drives both surfaces", async () => {
    await run(
      fixtureEngineLayer(),
      Effect.gen(function* () {
        const engine = yield* SwapEngine.Service;
        yield* engine.openSession(fixtureSwapSession);
        // Called exactly as `@openagentsinc/mkt-swp-compare` calls a
        // FundVerifier, with no adapter in between.
        const report = yield* engine.verifySession({
          quoteEventId: fixtureSwapSession.quoteEventId,
          orderEventId: fixtureSwapSession.orderEventId,
          epoch: fixtureSwapSession.epoch,
        });
        expect(report.verdict).toBe("verification_passed");
        expect(fundingGate(report, fixtureSwapSession.epoch).enabled).toBe(true);
        // A superseded epoch is refused by SWAP-3's own gate.
        expect(fundingGate(report, fixtureSwapSession.epoch + 1).enabled).toBe(false);
      }),
    );
  });

  test("the widget host passes signed records through opaquely", () => {
    // The oracle for "the host never computes a profile-level verdict": the
    // host module contains no record parsing at all. Every verdict — profile
    // validation, verification, exit-package binding, funding authorisation —
    // arrives typed from behind the engine boundary.
    const source = readFileSync(join(import.meta.dirname, "widget-host.ts"), "utf8");
    expect(source.includes("JSON.parse")).toBe(false);
    expect(source.includes("swapTree")).toBe(false);
    expect(source.includes("scriptPubKey")).toBe(false);
  });

  test("a malformed record is refused by the engine, not interpreted by the host", async () => {
    await run(
      fixtureEngineLayer(),
      Effect.gen(function* () {
        const controller = yield* makeSwapWidgetController;
        yield* controller.updateInputs(preVerificationInputs);
        yield* controller.verifyTerms(fixtureSwapSession);
        const outcome = yield* controller.submitOrder({
          ...fixtureSwapSession,
          quote: "not canonical json",
        });
        expect(outcome).toEqual({ _tag: "Refused", identifier: "swp_unsupported_profile" });
        expect((yield* controller.current)._tag).toBe("VerificationFailed");
      }),
    );
  });

  test("an unavailable engine cannot masquerade as a pass", async () => {
    await run(
      fixtureEngineLayer(),
      Effect.gen(function* () {
        const controller = yield* makeSwapWidgetController;
        yield* controller.updateInputs(preVerificationInputs);
        // A session the engine never opened: SWAP-3's port fails with
        // FundVerifierUnavailable, so there is no report and no gate.
        const result = yield* controller.verifyTerms({
          ...fixtureSwapSession,
          quoteEventId: "9e".repeat(32),
          sessionId: "8d".repeat(32),
          swapContracts: [fixtureSwapSession.swapContracts[0]],
        });
        expect(result.gate).toBe(null);
        expect(result.state._tag).not.toBe("Ready");
      }),
    );
  });
});

describe("the funding gate (swp_funding_not_authorized)", () => {
  test("a failed verify-before-fund row keeps funding unreachable end to end", async () => {
    await run(
      fixtureEngineLayer({ failingCheck: "script_tree_parsed" }),
      Effect.gen(function* () {
        const controller = yield* makeSwapWidgetController;
        yield* controller.updateInputs(preVerificationInputs);
        const verified = yield* controller.verifyTerms(fixtureSwapSession);
        expect(verified.gate?.enabled).toBe(false);
        expect(verified.state._tag).toBe("VerificationFailed");
        // Ready is unreachable, so ordering — and with it funding — is too.
        const outcome = yield* controller.submitOrder(fixtureSwapSession);
        expect(outcome._tag).toBe("NotReady");
      }),
    );
  });

  test("authorizeFunding refuses a failed row, an unresolved row, and a blocked verdict", async () => {
    for (const options of [
      { failingCheck: "script_tree_parsed" as const },
      { unresolvedCheck: "timeout_ladder" as const },
      { blockVerdict: true },
    ]) {
      await run(
        fixtureEngineLayer(options),
        Effect.gen(function* () {
          const engine = yield* SwapEngine.Service;
          yield* engine.openSession(fixtureSwapSession);
          yield* engine.verifySession({
            quoteEventId: fixtureSwapSession.quoteEventId,
            orderEventId: fixtureSwapSession.orderEventId,
            epoch: fixtureSwapSession.epoch,
          });
          const exitPackage = yield* engine.buildExitPackage(fixtureSwapSession.sessionId);
          const error = yield* engine
            .authorizeFunding({ sessionId: fixtureSwapSession.sessionId, exitPackage })
            .pipe(Effect.flip);
          expect(error.identifier).toBe("swp_funding_not_authorized");
        }),
      );
    }
  });

  test("funding cannot be authorised before verification runs at all", async () => {
    await run(
      fixtureEngineLayer(),
      Effect.gen(function* () {
        const engine = yield* SwapEngine.Service;
        yield* engine.openSession(fixtureSwapSession);
        const exitPackage = yield* engine.buildExitPackage(fixtureSwapSession.sessionId);
        const error = yield* engine
          .authorizeFunding({ sessionId: fixtureSwapSession.sessionId, exitPackage })
          .pipe(Effect.flip);
        expect(error.identifier).toBe("swp_funding_not_authorized");
      }),
    );
  });

  test("an exit package not digest-bound to the verified contract pair is refused", async () => {
    await run(
      fixtureEngineLayer(),
      Effect.gen(function* () {
        const engine = yield* SwapEngine.Service;
        yield* engine.openSession(fixtureSwapSession);
        yield* engine.verifySession({
          quoteEventId: fixtureSwapSession.quoteEventId,
          orderEventId: fixtureSwapSession.orderEventId,
          epoch: fixtureSwapSession.epoch,
        });
        const exitPackage = yield* engine.buildExitPackage(fixtureSwapSession.sessionId);
        const error = yield* engine
          .authorizeFunding({
            sessionId: fixtureSwapSession.sessionId,
            exitPackage: { ...exitPackage, contractDigest: "ff".repeat(32) },
          })
          .pipe(Effect.flip);
        expect(error.identifier).toBe("swp_exit_package_mismatch");
      }),
    );
  });

  test("a session claim cannot substitute for the authorization", async () => {
    await run(
      fixtureEngineLayer({ failingCheck: "exit_package" }),
      Effect.gen(function* () {
        const controller = yield* makeSwapWidgetController;
        yield* controller.updateInputs(preVerificationInputs);
        yield* controller.verifyTerms(fixtureSwapSession);
        // A provider Status claiming funding is required, then a local-only
        // projection, then an unknown value: none advances the widget.
        expect((yield* controller.applySwpState("funding_required"))._tag).toBe(
          "VerificationFailed",
        );
        expect((yield* controller.applySwpState("contract_pending"))._tag).toBe(
          "VerificationFailed",
        );
        expect((yield* controller.applySwpState("brand_new_state"))._tag).toBe(
          "VerificationFailed",
        );
      }),
    );
  });

  test("raw unresolved remains visible after its required failed-base normalization", async () => {
    await run(
      fixtureEngineLayer(),
      Effect.gen(function* () {
        const controller = yield* makeSwapWidgetController;
        yield* controller.updateInputs(preVerificationInputs);
        yield* controller.verifyTerms(fixtureSwapSession);
        const outcome = yield* controller.submitOrder(fixtureSwapSession);
        expect(outcome._tag).toBe("FundingAuthorized");
        expect((yield* controller.applySwpState("funding_observed"))._tag).toBe("FundingObserved");
        expect((yield* controller.applySwpState("unresolved"))._tag).toBe("Unresolved");
      }),
    );
  });
});
