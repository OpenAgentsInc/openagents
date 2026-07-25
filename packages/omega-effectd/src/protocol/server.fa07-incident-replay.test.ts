/**
 * FA-07 gate 1 — the 2026-07-17 thread-pressure / cache-eviction incident replay.
 *
 * The incident (docs/fable/2026-07-17-full-auto-implementation-audit.md §2):
 *
 *   1. Desktop kept only five threads in its mutable local thread cache.
 *   2. Eviction used creation time rather than last access.
 *   3. Concurrent chats displaced the older-created Full Auto thread even
 *      though it was still the active continuation target.
 *   4. Full Auto retained a durable enabled record and attempted the next
 *      turn against that thread ref.
 *   5. The dispatch failed closed with "That conversation no longer exists."
 *   6. The loop recorded a failed dispatch, but the owner-facing surface
 *      exposed only a generic failure banner.
 *
 * The result was a ~6 hour SILENT LOSS OF AUTONOMY.
 *
 * §2.3 of that audit states the requirement this file exists to meet:
 *
 *   "The regression closes the exact cache defect. It does not prove the
 *    surrounding unattended-run contract. A dedicated replay must recreate
 *    thread pressure while a real Full Auto run advances through multiple
 *    turns."
 *
 * That is what this file does, and it is why it does NOT reproduce the
 * incident by editing the registry file on disk. File surgery asserts that
 * the classifier can label an already-broken state. It does not exercise the
 * composition that actually failed: a bounded host thread cache evicting a
 * live continuation target while a run is mid-program.
 *
 * The replay drives the real supervised engine through the real framed
 * protocol against a host that models Omega's bounded mutable thread cache,
 * including its eviction policy. Two arms are run against the same engine:
 *
 *   Arm A — `creation_time` eviction. This is the 2026-07-17 defect itself.
 *           The Full Auto thread is displaced by concurrent chats. The gate
 *           requires a TYPED STALL: `stalled` / `host_thread_missing` /
 *           `stop_only`, surfaced to the owner.
 *
 *   Arm B — `last_access_lru` eviction. This is the fix landed in Desktop
 *           `8cb900bbf9`. The continuation target stays hot because the loop
 *           keeps touching it. The gate requires CONTINUED AUTONOMY: the next
 *           turn is dispatched and the run is still running.
 *
 * Both arms assert the single property gate 1 actually names:
 *
 *   **never silent death**
 *
 * defined here as: the run must never remain `running` while no turn is in
 * flight, no dispatch was accepted, and no failure was recorded. That exact
 * conjunction is what the owner experienced for six hours, and it is the only
 * outcome this gate forbids. A typed stall is a pass. Continued autonomy is a
 * pass. Silence is the failure.
 *
 * FALSIFICATION. A replay that has never been observed to fail proves
 * nothing, so this one was deliberately regressed twice and watched go red
 * (2026-07-25, against this engine tree):
 *
 *   Experiment 1 — delete the `!evidence.present -> recordFailure + stalled`
 *     reconciliation guard in `server.ts`, reinstating the pre-fix behaviour
 *     where an evicted continuation target produced no typed classification.
 *     RESULT: Arm A red, `expected 'retrying' to be 'stalled'`. The dispatch
 *     failure is still recorded by the reconciler's own failure budget, so
 *     the run is not silent -- but the typed `host_thread_missing` cause and
 *     the `stop_only` recovery affordance are both lost, and the replay
 *     catches exactly that regression.
 *
 *   Experiment 2 — additionally swallow host dispatch refusals, so the loop
 *     believes every dispatch succeeded. Nothing fails, nothing is recorded,
 *     and the run keeps claiming to be running: the exact 2026-07-17 shape.
 *     RESULT: Arm A red on `assertNeverSilentDeath` itself, reporting
 *     `observed={"dispatchAccepted":0,"dispatchRefused":1}` against a run
 *     projecting `"running"` with `failedAttempts: 0` and `stallCause: null`.
 *
 * Both regressions were reverted; the engine tree under test is unmodified.
 */

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

import { createOmegaEffectdService } from "../service.ts";
import { createOmegaEffectdFramedServer } from "./server.ts";
import { OMEGA_EFFECTD_PROTOCOL_SCHEMA } from "./framed.ts";
import type { OmegaEffectdHostRequest } from "./framed.ts";
import { LOCAL_TURN_RECORD_SCHEMA } from "../support/local-turn-journal.ts";

/**
 * The incident's exact cache bound: "Desktop kept only five threads in its
 * mutable local thread cache" (audit §2.2 step 1).
 */
const INCIDENT_THREAD_CACHE_CAPACITY = 5;

type EvictionPolicy = "creation_time" | "last_access_lru";

type CachedThread = {
  readonly threadRef: string;
  readonly createdSeq: number;
  lastAccessSeq: number;
  turns: Array<Record<string, unknown>>;
  turnRunning: boolean;
};

/**
 * A host that models Omega's bounded mutable thread cache.
 *
 * This is the piece the original tests did not have. `refresh_evidence` and
 * `dispatch_turn` answer from the cache, so an evicted continuation target
 * produces exactly the host-side shape the incident produced -- the thread ref
 * is durably retained by Full Auto but the host can no longer open it.
 */
const makeBoundedThreadCacheHost = (options: {
  readonly capacity: number;
  readonly evictionPolicy: EvictionPolicy;
}) => {
  const cache = new Map<string, CachedThread>();
  const evicted = new Set<string>();
  let seq = 0;
  let threadCounter = 0;
  const dispatchAccepted = new Map<string, number>();
  const dispatchRefused = new Map<string, number>();
  const bump = (counter: Map<string, number>, threadRef: string): void => {
    counter.set(threadRef, (counter.get(threadRef) ?? 0) + 1);
  };

  const evictIfNeeded = (): void => {
    while (cache.size > options.capacity) {
      // The exact policy difference the incident turned on.
      const victim = [...cache.values()].sort((a, b) =>
        options.evictionPolicy === "creation_time"
          ? a.createdSeq - b.createdSeq
          : a.lastAccessSeq - b.lastAccessSeq,
      )[0];
      if (victim === undefined) return;
      cache.delete(victim.threadRef);
      evicted.add(victim.threadRef);
    }
  };

  const touch = (threadRef: string): CachedThread | null => {
    const entry = cache.get(threadRef);
    if (entry === undefined) return null;
    // Only a last-access policy records the read. Under creation-time
    // eviction, touching a thread does not protect it -- which is precisely
    // why the live continuation target was displaced.
    if (options.evictionPolicy === "last_access_lru") entry.lastAccessSeq = ++seq;
    return entry;
  };

  const open = (threadRef: string): void => {
    const existing = cache.get(threadRef);
    if (existing !== undefined) {
      touch(threadRef);
      return;
    }
    cache.set(threadRef, {
      threadRef,
      createdSeq: ++seq,
      lastAccessSeq: seq,
      turns: [],
      turnRunning: false,
    });
    evicted.delete(threadRef);
    evictIfNeeded();
  };

  const handler = async (request: OmegaEffectdHostRequest): Promise<unknown> => {
    const params = (request.params ?? {}) as Record<string, unknown>;
    switch (request.method) {
      case "resolve_workspace":
        return { workspaceRef: params.expectedWorkspaceRef ?? "workspace.omega.supervised" };
      case "lane_readiness":
        return { known: true, admitted: true, fullAuto: true, state: "available" };
      case "create_thread": {
        const threadRef = `thread.omega.fa07.${++threadCounter}`;
        open(threadRef);
        return { threadRef };
      }
      case "refresh_evidence": {
        const threadRef = String(params.threadRef ?? "");
        const entry = touch(threadRef);
        if (entry === null) {
          // The host can no longer open the ref. This is the observable
          // shape behind Desktop's "That conversation no longer exists."
          return { present: false, revision: 1, live: null, turns: [] };
        }
        return {
          present: true,
          revision: 1,
          live: entry.turnRunning ? { state: "turn_running", turnRef: null } : null,
          turns: entry.turns,
        };
      }
      case "dispatch_turn": {
        const threadRef = String(params.threadRef ?? "");
        const entry = touch(threadRef);
        if (entry === null) {
          bump(dispatchRefused, threadRef);
          return {
            accepted: false,
            reason: "host_thread_missing",
            failureCause: "host_thread_missing",
          };
        }
        bump(dispatchAccepted, threadRef);
        // A real host records the turn under the ref the ENGINE minted --
        // that ref is the FA-H3 dispatch lease identity. Inventing our own
        // ref here would leave the engine's lease permanently unresolved and
        // make the replay measure the wrong thing.
        const turnRef = String(params.turnRef ?? "");
        const now = new Date().toISOString();
        entry.turnRunning = true;
        entry.turns = [
          ...entry.turns,
          {
            schema: LOCAL_TURN_RECORD_SCHEMA,
            threadRef,
            turnRef,
            lane: "codex-local",
            userMessageKey: `${turnRef}.user`,
            assistantMessageKey: `${turnRef}.assistant`,
            accountRef: null,
            providerSessionRef: null,
            model: null,
            phase: "streaming",
            persistedCursor: 1,
            assistantText: "",
            assistantSegments: [],
            recoveryGeneration: 0,
            // In flight. The engine treats a null disposition as nonterminal.
            disposition: null,
            createdAt: now,
            updatedAt: now,
          },
        ];
        return { accepted: true };
      }
      case "interrupt_turn":
        return { interrupted: true };
      case "append_system_note":
        return { appended: true };
    }
    return undefined;
  };

  return {
    handler,
    /**
     * "Concurrent chats displaced the older-created Full Auto thread." The
     * owner opening ordinary conversations is the stressor, so the replay
     * applies it exactly that way rather than by deleting a record.
     */
    openConcurrentChats: (wave: number, count: number): void => {
      for (let index = 0; index < count; index += 1) {
        open(`thread.omega.ordinary-chat.${wave}.${index}`);
      }
    },
    /**
     * Resolve the in-flight provider turn as a real success -- "the first
     * autonomous packet completed successfully after about 14m 40s". The run
     * must have genuine forward progress before the stressor, or the replay
     * is not replaying the incident.
     */
    completeTurn: (threadRef: string): void => {
      // Resolving a turn writes the assistant's final message into the
      // thread, so it is a genuine cache access -- `touch`, not a bare
      // lookup. This is the access the incident turned on: at 12:12 AM the
      // Full Auto thread was the most recently used thread in the cache, and
      // creation-time eviction threw it away anyway.
      const entry = touch(threadRef);
      if (entry === null) return;
      const now = new Date().toISOString();
      entry.turnRunning = false;
      entry.turns = entry.turns.map((turn) =>
        turn.disposition === null
          ? {
              ...turn,
              phase: "completed",
              assistantText: "Bounded packet complete.",
              disposition: "completed",
              updatedAt: now,
            }
          : turn,
      );
    },
    isCached: (threadRef: string): boolean => cache.has(threadRef),
    wasEvicted: (threadRef: string): boolean => evicted.has(threadRef),
    /**
     * Counters are read per thread and relative to a baseline taken at the
     * moment the stressor is applied. The gate asks what happened AFTER the
     * last successful packet -- the first turn always dispatches fine, and
     * counting it would make "something happened" trivially true and the
     * silent-death check vacuous.
     */
    countsSince: (baseline: ReadonlyMap<string, number>, threadRef: string) => ({
      dispatchAccepted: (dispatchAccepted.get(threadRef) ?? 0) - (baseline.get(threadRef) ?? 0),
      dispatchRefused: dispatchRefused.get(threadRef) ?? 0,
    }),
    baseline: (): ReadonlyMap<string, number> => new Map(dispatchAccepted),
  };
};

const request = (id: string, generation: number, method: string, params?: unknown) =>
  JSON.stringify({
    schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
    kind: "request",
    id,
    generation,
    method,
    ...(params === undefined ? {} : { params }),
  });

type RunDetail = {
  runRef: string;
  threadRef: string | null;
  state: string;
  stallCause: string | null;
  recoveryAction: string;
  failedAttempts: number;
};

/**
 * The gate-1 property, stated once so both arms are judged by the same rule.
 *
 * Silent death is the conjunction the owner actually lived through: the run
 * still claims to be running, but nothing is in flight, nothing was accepted,
 * and nothing was recorded as having failed. There is no signal anywhere for
 * the owner to act on.
 */
const assertNeverSilentDeath = (
  run: RunDetail,
  observed: { readonly dispatchAccepted: number; readonly dispatchRefused: number },
): void => {
  const claimsHealthy = run.state === "running" || run.state === "draft";
  const nothingHappened = observed.dispatchAccepted === 0;
  const nothingReported = run.failedAttempts === 0 && run.stallCause === null;
  const silentDeath = claimsHealthy && nothingHappened && nothingReported;

  expect(
    silentDeath,
    `SILENT DEATH: the run projects "${run.state}" with no accepted dispatch, ` +
      `no recorded failure, and no stall cause. This is the 2026-07-17 failure mode: ` +
      `autonomy stopped and nothing told the owner. ` +
      `observed=${JSON.stringify(observed)} run=${JSON.stringify(run)}`,
  ).toBe(false);
};

/**
 * The owner's other conversations. Four are already open alongside the
 * autonomous run (filling the five-slot cache exactly), and one more is
 * opened after the packet completes -- the overflow that forces an eviction
 * and decides the whole incident.
 */
const CHATS_OPEN_DURING_RUN = INCIDENT_THREAD_CACHE_CAPACITY - 1;

const runIncidentReplay = async (
  evictionPolicy: EvictionPolicy,
): Promise<{
  run: RunDetail;
  host: ReturnType<typeof makeBoundedThreadCacheHost>;
  threadRef: string;
  baseline: ReadonlyMap<string, number>;
}> => {
  const root = mkdtempSync(path.join(tmpdir(), `oa-fa07-replay-${evictionPolicy}-`));
  try {
    const host = makeBoundedThreadCacheHost({
      capacity: INCIDENT_THREAD_CACHE_CAPACITY,
      evictionPolicy,
    });
    const service = createOmegaEffectdService({ paths: { dataRoot: root } });
    const server = createOmegaEffectdFramedServer(
      service,
      { dataRoot: root },
      { hostRequestHandler: host.handler },
    );
    await server.handleLine(request("1", 0, "initialize", { generation: 1 }));

    const startRun = async (id: string, suffix: string) =>
      server.handleLine(
        request(id, 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: `FA-07 incident replay ${suffix}`,
          objective: "Advance the bounded program across multiple unattended turns.",
          doneCondition: "The program's remaining work is reported.",
          turnCap: 20,
          projectRef: `project.fa07.replay.${suffix}`,
          worktreeRef: `worktree.fa07.replay.${suffix}`,
          gitHead: "deadbeef",
        }),
      );

    // 1. A real Full Auto run begins, exactly as the overnight run did.
    const started = await startRun("2", "primary");
    expect(started?.ok).toBe(true);
    const run = (started?.result as { run: { runRef: string; threadRef: string } }).run;
    const threadRef = run.threadRef;
    expect(host.isCached(threadRef)).toBe(true);

    // 2. The owner's other conversations are already open alongside the
    //    autonomous run -- "several concurrent chats". The bounded cache is
    //    now exactly full, and every one of these was created AFTER the
    //    Full Auto thread.
    host.openConcurrentChats(0, CHATS_OPEN_DURING_RUN);

    // 3. "The first autonomous packet completed successfully after about
    //    14m 40s." This is the decisive access: at this instant the Full Auto
    //    thread is the MOST recently used thread in the cache, and also the
    //    OLDEST created. The two eviction policies disagree about it from
    //    here on, and that disagreement is the whole incident.
    host.completeTurn(threadRef);

    // Everything counted from here is "what happened after the last good
    // packet" -- the exact window the owner lost.
    const baseline = host.baseline();

    // 4. THE STRESSOR: one more ordinary conversation overflows the cache and
    //    forces an eviction while the run is still the live continuation
    //    target.
    host.openConcurrentChats(1, 1);

    // 5. The next Full Auto reconciliation sweeps every enabled thread --
    //    the exact step that failed at 12:12 AM and then never recovered.
    //    Starting a further run is a real trigger for that global pass.
    await startRun("4", "sweep");

    const detail = await server.handleLine(request("5", 1, "get_run", { runRef: run.runRef }));
    return {
      run: (detail?.result as { run: RunDetail }).run,
      host,
      threadRef,
      baseline,
    };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

describe("FA-07 gate 1 — 2026-07-17 thread-pressure / cache-eviction incident replay", () => {
  test("Arm A — creation-time eviction displaces the live target → TYPED STALL, never silence", async () => {
    const { run, host, threadRef, baseline } = await runIncidentReplay("creation_time");

    // The stressor must actually have reproduced the incident's cause. If the
    // thread was not evicted, this test is not replaying anything and must
    // say so rather than passing vacuously.
    expect(
      host.wasEvicted(threadRef),
      "the replay did not reproduce the incident: the Full Auto thread was never evicted",
    ).toBe(true);

    // The gate: never silent death.
    assertNeverSilentDeath(run, host.countsSince(baseline, threadRef));

    // And specifically, the typed stall the audit demands instead of a
    // generic failure banner and a six-hour silence.
    expect(run.state).toBe("stalled");
    expect(run.stallCause).toBe("host_thread_missing");
    expect(run.recoveryAction).toBe("stop_only");
  });

  test("Arm B — last-access LRU keeps the target hot → CONTINUED AUTONOMY", async () => {
    const { run, host, threadRef, baseline } = await runIncidentReplay("last_access_lru");

    // Desktop 8cb900bbf9's fix: the live continuation target survives the
    // same thread pressure because the loop keeps touching it, so the idle
    // ordinary chats become the eviction victims instead.
    expect(
      host.wasEvicted(threadRef),
      "last-access LRU must not evict the live continuation target",
    ).toBe(false);
    expect(host.isCached(threadRef)).toBe(true);

    // The gate: never silent death.
    assertNeverSilentDeath(run, host.countsSince(baseline, threadRef));

    // Autonomy continued: further turns were actually accepted by the host
    // after the last completed packet, and the run never stalled.
    expect(host.countsSince(baseline, threadRef).dispatchAccepted).toBeGreaterThan(0);
    expect(host.countsSince(baseline, threadRef).dispatchRefused).toBe(0);
    expect(run.stallCause).toBeNull();
    expect(run.state).toBe("running");
  });

  test("the replay discriminates: the two arms differ only in the host's eviction policy", async () => {
    // Guards against the replay silently becoming policy-insensitive. If a
    // future change made both arms behave identically, the replay would stop
    // discriminating and would no longer be evidence for anything, while
    // still reporting green.
    const armA = await runIncidentReplay("creation_time");
    const armB = await runIncidentReplay("last_access_lru");
    expect(armA.run.state).not.toBe(armB.run.state);
    expect(armA.host.wasEvicted(armA.threadRef)).not.toBe(armB.host.wasEvicted(armB.threadRef));
  });
});
