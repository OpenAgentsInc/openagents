/**
 * FA-07 gate 2 (omega#26) — an unattended run must reach a SECOND turn.
 *
 * Gate 2 is the only gate whose subject is a person's absence: "owner-real
 * multi-turn unattended run". Everything else in the packet asks what happens
 * when something occurs. This asks what happens when nothing does.
 *
 * On this transport, nothing did.
 *
 * Reconciliation is the only thing that dispatches a continuation, and on the
 * framed path every trigger for it was a control MUTATION — `start`, `pause`,
 * `resume`, `stop`, `retry`, `handoff`, `apply_control_intent`. The Electron
 * control-API host additionally reconciles when a turn completes; this
 * transport has no turn-completion signal at all, because the host answers
 * `dispatch_turn` with `{accepted: true}` and is never asked again. And the
 * three methods the Omega run monitor polls every three seconds — `list_runs`,
 * `get_run`, `decide_attention` — deliberately do not mutate, which is right of
 * them and is exactly why none of them reconciles.
 *
 * So a Full Auto run started on Omega dispatched turn one and then waited
 * forever. Driven against the engine inside installed `0.2.0-rc13`, a real run
 * sat at `successfulAttempts: 1` with `lastProgressAt` frozen at the start
 * instant for over ten minutes with no input.
 *
 * That was never a silent death — the detail projected `stalled` /
 * `dispatch_overdue` with a `retry_now` affordance, which is gate 1's property
 * holding underneath. It simply was not autonomous. The gap was even visible in
 * this suite: `server.fa07-incident-replay.test.ts` has to START A SECOND RUN
 * to make a reconciliation sweep happen, and says so in a comment. It read as a
 * harness detail. It was the product.
 *
 * The tests below assert the property over WALL-CLOCK TIME with no control call
 * in between, because that is the only formulation gate 2 accepts. The second
 * test is the same run with the clock turned off, watched failing.
 */

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

import { createOmegaEffectdService } from "../service.ts";
import { createOmegaEffectdFramedServer } from "./server.ts";
import { OMEGA_EFFECTD_PROTOCOL_SCHEMA, type OmegaEffectdHostRequest } from "./framed.ts";

const LANE = "codex-local";
const LOCAL_TURN_RECORD_SCHEMA = "openagents.desktop.local_turn_record.v1";

const withRoot = async (fn: (root: string) => Promise<void>): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-effectd-fa07-unattended-"));
  try {
    await fn(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
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

/**
 * A host whose turns actually finish.
 *
 * `makeOmegaEffectdTestHost` reports `turns: []` forever, so every
 * reconciliation sees a thread with no turn in flight and dispatches again.
 * A test built on it would report autonomy for an engine that had none — it
 * would be measuring the stub. This host records each dispatched turn and marks
 * it `completed`, so a second dispatch can only happen because the engine
 * observed the first one end and decided to continue.
 */
const makeTurnCompletingHost = () => {
  const turnsByThread = new Map<string, Array<Record<string, unknown>>>();
  const dispatched: Array<{ threadRef: string; turnRef: string }> = [];
  const hostMethods: Array<string> = [];
  let threadCounter = 0;
  const handler = async (hostRequest: OmegaEffectdHostRequest): Promise<unknown> => {
    const params = (hostRequest.params ?? {}) as Record<string, unknown>;
    hostMethods.push(hostRequest.method);
    switch (hostRequest.method) {
      case "resolve_workspace":
        return { workspaceRef: params.expectedWorkspaceRef ?? "workspace.omega.supervised" };
      case "resolve_sync_session":
        return { available: false };
      case "lane_readiness":
        return { known: true, admitted: true, fullAuto: true, state: "available" };
      case "create_thread": {
        const threadRef = `thread.unattended.${(threadCounter += 1)}`;
        turnsByThread.set(threadRef, []);
        return { threadRef };
      }
      case "refresh_evidence": {
        const threadRef = String(params.threadRef);
        return {
          present: turnsByThread.has(threadRef),
          revision: (turnsByThread.get(threadRef) ?? []).length,
          live: null,
          turns: turnsByThread.get(threadRef) ?? [],
        };
      }
      case "dispatch_turn": {
        const threadRef = String(params.threadRef);
        const turnRef = String(params.turnRef);
        const now = new Date().toISOString();
        const turns = turnsByThread.get(threadRef) ?? [];
        // The FULL record shape. A partial one decodes to nothing, the engine
        // never sees its own dispatched turn settle, and its FA-H3 lease stays
        // held forever — which looks exactly like "the clock does not work" and
        // is not. Cost an hour; worth the comment.
        turns.push({
          schema: LOCAL_TURN_RECORD_SCHEMA,
          threadRef,
          turnRef,
          lane: LANE,
          userMessageKey: `msg.user.${turnRef}`,
          assistantMessageKey: `msg.assistant.${turnRef}`,
          accountRef: null,
          providerSessionRef: null,
          model: null,
          phase: "completed",
          persistedCursor: turns.length,
          assistantText: "the turn produced its answer",
          assistantSegments: [],
          recoveryGeneration: 0,
          disposition: "completed",
          createdAt: now,
          updatedAt: now,
        });
        turnsByThread.set(threadRef, turns);
        dispatched.push({ threadRef, turnRef });
        return { accepted: true };
      }
      case "interrupt_turn":
        return { interrupted: true };
      case "append_system_note":
        return { appended: true };
      default:
        return undefined;
    }
  };
  return { handler, dispatched, hostMethods };
};

const startServer = async (root: string, autonomyPollIntervalMs: number) => {
  const host = makeTurnCompletingHost();
  const service = createOmegaEffectdService({ paths: { dataRoot: root } });
  const server = createOmegaEffectdFramedServer(
    service,
    { dataRoot: root },
    { hostRequestHandler: host.handler, autonomyPollIntervalMs },
  );
  await server.handleLine(request("init", 0, "initialize", { generation: 1 }));
  return { server, host };
};

const startRun = async (
  server: Awaited<ReturnType<typeof startServer>>["server"],
  turnCap: number,
) => {
  const started = await server.handleLine(
    request("start", 1, "start", {
      workspaceRef: "workspace.omega.supervised",
      title: "FA-07 gate 2 unattended",
      objective: "FA07_UNATTENDED_OBJECTIVE",
      doneCondition: "FA07_UNATTENDED_DONE",
      lane: LANE,
      turnCap,
      projectRef: "project.fa07.unattended",
      worktreeRef: "worktree.fa07.unattended",
    }),
  );
  expect(started?.ok).toBe(true);
  return String(
    ((started?.result as { run: { runRef: string } }).run satisfies { runRef: string }).runRef,
  );
};

/** Wait on a condition without ever touching the run. */
const waitFor = async (predicate: () => boolean, budgetMs: number): Promise<boolean> => {
  const deadline = Date.now() + budgetMs;
  while (Date.now() < deadline) {
    if (predicate()) return true;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  return predicate();
};

describe("FA-07 gate 2: a run left alone advances by itself", () => {
  test("a second turn is dispatched with no control call after the start", async () => {
    await withRoot(async (root) => {
      const { server, host } = await startServer(root, 20);
      const runRef = await startRun(server, 3);

      const dispatchesAfterStart = host.dispatched.length;
      expect(dispatchesAfterStart).toBe(1);

      // From here to the assertion, nothing is sent to the server. No pause, no
      // resume, no retry, no `apply_control_intent`, and no `get_run` — even a
      // read would leave room to argue that observing the run is what moved it.
      const advanced = await waitFor(() => host.dispatched.length > dispatchesAfterStart, 4_000);
      expect(
        advanced,
        "a run nobody touched must still reach turn two; this is the whole of gate 2",
      ).toBe(true);

      // Only now is the run read, and it must agree with what the host saw.
      const detail = await server.handleLine(request("detail", 1, "get_run", { runRef }));
      const run = (detail?.result as { run: Record<string, unknown> }).run;
      expect(Number(run.successfulAttempts)).toBeGreaterThan(1);
      expect(run.objective).toBe("FA07_UNATTENDED_OBJECTIVE");
    });
  });

  test("the turn cap still bounds an unattended run, so autonomy is not unbounded", async () => {
    await withRoot(async (root) => {
      const { server, host } = await startServer(root, 20);
      const runRef = await startRun(server, 2);

      await waitFor(() => host.dispatched.length >= 2, 4_000);
      // Past the cap the clock keeps ticking; the cap is what must stop it.
      await new Promise((resolve) => setTimeout(resolve, 500));
      expect(
        host.dispatched.length,
        "a timer that ignored the turn cap would spend the owner's budget forever",
      ).toBe(2);

      const detail = await server.handleLine(request("detail", 1, "get_run", { runRef }));
      const run = (detail?.result as { run: Record<string, unknown> }).run;
      expect(Number(run.successfulAttempts)).toBeLessThanOrEqual(2);
    });
  });

  test("an Omega with no run does no work on the clock", async () => {
    await withRoot(async (root) => {
      const { host } = await startServer(root, 20);
      // Initialization asks the host nothing about workspaces or evidence, and
      // a clock that reconciled unconditionally would ask every 20 ms forever
      // on a laptop with Full Auto merely installed.
      const asked = host.hostMethods.length;
      await new Promise((resolve) => setTimeout(resolve, 500));
      expect(
        host.hostMethods.length,
        "an idle Omega must not wake the host on a timer",
      ).toBe(asked);
    });
  });

  /**
   * The watched failure, kept as a test rather than reported as an anecdote.
   *
   * This is the identical run with the continuation clock pushed out of reach —
   * which is exactly the engine that shipped in `0.2.0-rc13` and every candidate
   * before it. If the first test ever passes while this one does too, the first
   * test is measuring something other than the clock.
   */
  test("without the continuation clock the same run never reaches turn two", async () => {
    await withRoot(async (root) => {
      const { server, host } = await startServer(root, 3_600_000);
      await startRun(server, 3);

      expect(host.dispatched.length).toBe(1);
      const advanced = await waitFor(() => host.dispatched.length > 1, 1_000);
      expect(
        advanced,
        "pre-fix behaviour: turn one dispatches and the run then waits for a person",
      ).toBe(false);
    });
  });
});
