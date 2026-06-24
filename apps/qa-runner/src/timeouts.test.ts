// Tests for the per-step timeout + bounded deterministic retry primitives.
//
// Deterministic discipline: every test uses a MANUAL timer (no real wall-clock
// wait, no sleep). A deadline is fired explicitly; a retry delay resolves
// instantly. This proves the policy WITHOUT flakiness — the harness's own test
// setup is itself deterministic, which is the point of #6193.

import { describe, expect, test } from "bun:test";
import {
  StepTimeoutError,
  runStepWithPolicy,
  withDeadline,
  type TimerLike,
} from "./timeouts";

/**
 * A manual timer: `delay(ms)` returns a promise the test resolves by calling
 * `fire(ms)`. No real time passes. `cancel()` marks the handle settled so a
 * fired-after-cancel does nothing (mirrors clearTimeout). Pending delays are
 * inspectable so a test asserts the deadline was actually armed.
 */
function makeManualTimer() {
  interface Pending {
    readonly ms: number;
    resolve: () => void;
    cancelled: boolean;
  }
  const pending: Pending[] = [];
  const timer: TimerLike = {
    delay: (ms) => {
      const entry: Pending = { ms, resolve: () => undefined, cancelled: false };
      const promise = new Promise<void>((resolve) => {
        entry.resolve = resolve;
      });
      pending.push(entry);
      return { promise, cancel: () => (entry.cancelled = true) };
    },
  };
  return {
    timer,
    /** Resolve all live (non-cancelled) pending delays — fires the deadline. */
    fireAll: () => {
      for (const p of pending) if (!p.cancelled) p.resolve();
    },
    pendingCount: () => pending.filter((p) => !p.cancelled).length,
  };
}

const tick = () => new Promise<void>((r) => setTimeout(r, 0));

describe("withDeadline", () => {
  test("returns the value when the step settles before the deadline", async () => {
    const { timer } = makeManualTimer();
    const result = await withDeadline("fast", 1000, async () => 42, timer);
    expect(result).toBe(42);
  });

  test("rejects with StepTimeoutError when the deadline fires first", async () => {
    const { timer, fireAll } = makeManualTimer();
    // a step that never resolves
    const promise = withDeadline("hang", 50, () => new Promise<number>(() => {}), timer);
    await tick();
    fireAll(); // deadline wins
    await expect(promise).rejects.toBeInstanceOf(StepTimeoutError);
  });

  test("no timeout (undefined / <= 0) just runs the step directly", async () => {
    const { timer, pendingCount } = makeManualTimer();
    expect(await withDeadline("none", undefined, async () => "ok", timer)).toBe("ok");
    expect(await withDeadline("zero", 0, async () => "ok", timer)).toBe("ok");
    // never armed a timer
    expect(pendingCount()).toBe(0);
  });

  test("cancels the pending deadline once the step settles (no leak)", async () => {
    const { timer, pendingCount } = makeManualTimer();
    await withDeadline("fast", 1000, async () => 1, timer);
    expect(pendingCount()).toBe(0);
  });
});

describe("runStepWithPolicy — bounded deterministic retry", () => {
  test("no policy -> one attempt, value through", async () => {
    const { timer } = makeManualTimer();
    const out = await runStepWithPolicy("step", undefined, async () => "v", timer);
    expect(out.value).toBe("v");
    expect(out.attempts).toBe(1);
  });

  test("a flaky step that succeeds on the 3rd try is retried and the flake is VISIBLE (attempts=3)", async () => {
    const { timer } = makeManualTimer();
    let calls = 0;
    const out = await runStepWithPolicy(
      "flaky",
      { retry: { maxAttempts: 3 } },
      async () => {
        calls++;
        if (calls < 3) throw new Error("transient");
        return "ok";
      },
      timer,
    );
    expect(out.value).toBe("ok");
    expect(out.attempts).toBe(3); // the retried flake is recorded, not hidden
    expect(calls).toBe(3);
  });

  test("a step that NEVER passes within maxAttempts fails honestly (throws last error)", async () => {
    const { timer } = makeManualTimer();
    let calls = 0;
    await expect(
      runStepWithPolicy(
        "broken",
        { retry: { maxAttempts: 3 } },
        async () => {
          calls++;
          throw new Error(`fail-${calls}`);
        },
        timer,
      ),
    ).rejects.toThrow("fail-3"); // last error surfaced — no fake pass
    expect(calls).toBe(3); // exactly the bound, never more
  });

  test("a NON-retryable error fails immediately without burning attempts", async () => {
    const { timer } = makeManualTimer();
    let calls = 0;
    await expect(
      runStepWithPolicy(
        "fatal",
        { retry: { maxAttempts: 5, retryable: (e) => !(e instanceof TypeError) } },
        async () => {
          calls++;
          throw new TypeError("not retryable");
        },
        timer,
      ),
    ).rejects.toBeInstanceOf(TypeError);
    expect(calls).toBe(1); // stopped at the first non-retryable error
  });

  test("maxAttempts < 1 is clamped to 1 (never zero attempts)", async () => {
    const { timer } = makeManualTimer();
    let calls = 0;
    const out = await runStepWithPolicy(
      "clamp",
      { retry: { maxAttempts: 0 } },
      async () => {
        calls++;
        return "v";
      },
      timer,
    );
    expect(out.attempts).toBe(1);
    expect(calls).toBe(1);
  });

  test("a per-step timeout inside the policy surfaces as a retryable failure", async () => {
    const { timer, fireAll } = makeManualTimer();
    let calls = 0;
    const promise = runStepWithPolicy(
      "slow-then-fast",
      { timeoutMs: 10, retry: { maxAttempts: 2 } },
      async () => {
        calls++;
        if (calls === 1) return new Promise<string>(() => {}); // hangs -> times out
        return "ok"; // 2nd attempt is instant
      },
      timer,
    );
    await tick();
    fireAll(); // fire the 1st-attempt deadline
    await tick();
    fireAll(); // (2nd attempt resolves on its own; harmless extra fire)
    const out = await promise;
    expect(out.value).toBe("ok");
    expect(out.attempts).toBe(2);
  });
});
