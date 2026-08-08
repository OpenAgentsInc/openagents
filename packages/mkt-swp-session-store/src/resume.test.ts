/**
 * Behaviour-contract oracle for the resume guarantee (issue #9320).
 *
 * Enforced contract (registry: `@openagentsinc/behavior-contracts`,
 * market-swap-session-store):
 * - openagents_web.swap_history.resume_after_reload.v1
 *
 * A session interrupted by a reload resumes without user action and without
 * duplicating an external effect: resume planning is app-wide (every
 * non-terminal session plus terminal-but-unclaimed ones), each task carries
 * a snapshot-before-live catch-up spec, and the persisted effect ledger
 * suppresses a replayed external operation.
 */
import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { EffectBindingConflictError } from "./errors.js";
import { memoryStringKv } from "./kv.js";
import { planResume, reloadGuard, resumeReasonOf } from "./resume.js";
import { openSessionStore } from "./store.js";
import { sampleSession, TEST_PROVIDER_PUBKEY, TEST_REQUESTER_PUBKEY } from "./testkit.js";

const inFlight = sampleSession("in-flight");
const terminalUnclaimed = sampleSession("terminal-unclaimed", {
  projection: {
    state: "completed",
    terminal: true,
    outcome: "completed",
    rung: "measured",
    unclaimedFunds: true,
  },
});
const terminalSettled = sampleSession("terminal-settled", {
  projection: {
    state: "completed",
    terminal: true,
    outcome: "completed",
    rung: "settled",
    unclaimedFunds: false,
  },
});

describe("resume planning is app-wide", () => {
  test("non-terminal and terminal-but-unclaimed sessions resume; settled ones do not", () => {
    const tasks = planResume([inFlight, terminalUnclaimed, terminalSettled]);
    expect(tasks.map((task) => [task.sessionId, task.reason])).toEqual([
      ["in-flight", "in_flight"],
      ["terminal-unclaimed", "terminal_unclaimed"],
    ]);
    expect(resumeReasonOf(terminalSettled)).toBeNull();
  });

  test("every task carries a snapshot-before-live catch-up spec from the last persisted record", () => {
    const [task] = planResume([inFlight]);
    expect(task?.catchUp.snapshotBeforeLive).toBe(true);
    expect(task?.catchUp.since).toBe(1_754_000_000);
    expect(task?.catchUp.authors).toEqual([TEST_REQUESTER_PUBKEY, TEST_PROVIDER_PUBKEY]);
    expect(task?.relayUrl).toBe("wss://relay.example");
  });
});

describe("resume after a reload (openagents_web.swap_history.resume_after_reload.v1)", () => {
  test("a persisted mid-flight session resumes from storage without user action", async () => {
    const kv = memoryStringKv();
    const fundingRequest = {
      operation: "funding_broadcast",
      templateDigest: "ab".repeat(32),
    };

    // Before the "reload": a session mid-swap durably records its funding
    // effect request, the wallet runs it, and the result is persisted.
    await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        yield* store.create(sampleSession("interrupted"));
        yield* store.recordEffectRequest("interrupted", "fund-1", fundingRequest);
        yield* store.recordEffectResult(
          "interrupted",
          "fund-1",
          { txid: "cd".repeat(32) },
          "cd".repeat(32),
        );
      }),
    );

    // The "reload": a brand-new store instance over the same storage.
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        const sessions = yield* store.list();
        const tasks = planResume(sessions);
        const prior = yield* store.priorEffectResult("interrupted", "fund-1", fundingRequest);
        const bound = yield* Effect.flip(
          store.priorEffectResult("interrupted", "fund-1", { operation: "different" }),
        );
        return { tasks, prior, bound };
      }),
    );

    // The session re-attaches without user action…
    expect(outcome.tasks.map((task) => task.sessionId)).toEqual(["interrupted"]);
    // …and the external effect is NOT duplicated: the persisted result is
    // returned for the same request, so the callback is suppressed…
    expect(outcome.prior?.result?.externalId).toBe("cd".repeat(32));
    // …while binding the same effect id to a different request fails closed.
    expect(outcome.bound).toBeInstanceOf(EffectBindingConflictError);
  });

  test("an unresulted effect request survives the reload for the crash-window replay", async () => {
    const kv = memoryStringKv();
    const request = { operation: "reverse_invoice_payment", invoiceDigest: "ef".repeat(32) };
    await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        yield* store.create(sampleSession("crash-window"));
        yield* store.recordEffectRequest("crash-window", "pay-1", request);
      }),
    );
    const prior = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        return yield* store.priorEffectResult("crash-window", "pay-1", request);
      }),
    );
    // No result yet: the operation must be re-driven, from the exact
    // persisted request — never re-invented.
    expect(prior).toBeNull();
  });
});

describe("reload guard", () => {
  test("blocks while an irreversible effect is pending and releases after its result", async () => {
    const kv = memoryStringKv();
    const request = { operation: "funding_broadcast", templateDigest: "aa".repeat(32) };
    const verdicts = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        yield* store.create(sampleSession("guarded"));
        const before = reloadGuard(yield* store.list());
        yield* store.recordEffectRequest("guarded", "fund-1", request);
        const during = reloadGuard(yield* store.list());
        yield* store.recordEffectResult("guarded", "fund-1", { txid: "bb".repeat(32) }, null);
        const after = reloadGuard(yield* store.list());
        return { before, during, after };
      }),
    );
    expect(verdicts.before.blocked).toBe(false);
    expect(verdicts.during).toEqual({ blocked: true, pendingSessionIds: ["guarded"] });
    expect(verdicts.after.blocked).toBe(false);
  });

  test("a definitive failure releases the guard without suppressing the retry", async () => {
    // The wallet call the user cancels must not leave navigation guarded
    // forever — and recording that failure must not hand the failed attempt
    // back as a prior result, which would suppress a legitimate retry.
    const kv = memoryStringKv();
    const request = { operation: "funding_broadcast", templateDigest: "cc".repeat(32) };
    const outcome = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* openSessionStore({ kv });
        yield* store.create(sampleSession("cancelled"));
        yield* store.recordEffectRequest("cancelled", "fund-1", request);
        const during = reloadGuard(yield* store.list());
        yield* store.recordEffectFailure("cancelled", "fund-1", "cancelled", "user dismissed the wallet prompt");
        const after = reloadGuard(yield* store.list());
        // Reopen (the reload) and check the retry window is open.
        const reopened = yield* openSessionStore({ kv });
        const prior = yield* reopened.priorEffectResult("cancelled", "fund-1", request);
        return { during, after, prior };
      }),
    );
    expect(outcome.during.blocked).toBe(true);
    expect(outcome.after.blocked).toBe(false);
    expect(outcome.prior).toBeNull();
  });
});
