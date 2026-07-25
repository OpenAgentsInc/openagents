/**
 * FA-07 gate 3 — restart reconciliation with durable reports intact.
 *
 * The 2026-07-17 audit's §3.2 defect 2 is that there was "no durable run
 * report": nothing summarised objective, turns attempted, outcomes, or stop
 * reason, so an owner returning to a run had nothing to read. The run report
 * now exists, which makes the restart question sharper than "does the run
 * survive": the REPORT must survive too, without dropping previously captured
 * turns and without duplicating them, because a report that silently loses or
 * doubles turns is worse than no report at all.
 *
 * This drives a real supervisor restart -- a second service and framed server
 * over the same data root, initialised at a new generation, exactly as the
 * Rust supervisor relaunches the engine -- rather than asserting over an
 * in-memory registry.
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
 * A host that keeps its thread/turn evidence across supervisor restarts --
 * which is the real situation, since the host outlives the engine process.
 */
const makeDurableHost = () => {
  const turns = new Map<string, Array<Record<string, unknown>>>();
  let counter = 0;
  const dispatched: string[] = [];

  const handler = async (request: OmegaEffectdHostRequest): Promise<unknown> => {
    const params = (request.params ?? {}) as Record<string, unknown>;
    switch (request.method) {
      case "resolve_workspace":
        return { workspaceRef: params.expectedWorkspaceRef ?? "workspace.omega.supervised" };
      case "lane_readiness":
        return { known: true, admitted: true, fullAuto: true, state: "available" };
      case "create_thread": {
        const threadRef = `thread.omega.restart.${++counter}`;
        turns.set(threadRef, []);
        return { threadRef };
      }
      case "refresh_evidence":
        return {
          present: true,
          revision: 1,
          live: null,
          turns: turns.get(String(params.threadRef)) ?? [],
        };
      case "dispatch_turn": {
        const threadRef = String(params.threadRef);
        const turnRef = String(params.turnRef);
        dispatched.push(turnRef);
        const now = new Date().toISOString();
        turns.get(threadRef)?.push({
          schema: LOCAL_TURN_RECORD_SCHEMA,
          threadRef,
          turnRef,
          lane: "codex-local",
          userMessageKey: `${turnRef}.user`,
          assistantMessageKey: `${turnRef}.assistant`,
          accountRef: null,
          providerSessionRef: null,
          model: null,
          phase: "completed",
          persistedCursor: 1,
          assistantText: "Bounded packet complete.",
          assistantSegments: [],
          recoveryGeneration: 0,
          disposition: "completed",
          createdAt: now,
          updatedAt: now,
        });
        return { accepted: true };
      }
      case "interrupt_turn":
        return { interrupted: true };
      case "append_system_note":
        return { appended: true };
    }
    return undefined;
  };

  return { handler, dispatched, turnCount: (threadRef: string) => turns.get(threadRef)?.length ?? 0 };
};

describe("FA-07 gate 3 — restart reconciliation with durable reports intact", () => {
  test("run, report, and native binding survive a supervisor restart without loss or duplication", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa07-restart-"));
    try {
      const host = makeDurableHost();

      // --- generation 1: the run does real work -------------------------
      const serviceA = createOmegaEffectdService({ paths: { dataRoot: root } });
      const serverA = createOmegaEffectdFramedServer(
        serviceA,
        { dataRoot: root },
        { hostRequestHandler: host.handler },
      );
      await serverA.handleLine(request("1", 0, "initialize", { generation: 1 }));
      const started = await serverA.handleLine(
        request("2", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "FA-07 restart",
          objective: "Prove durable truth survives a supervisor restart.",
          doneCondition: "The report still reads correctly afterwards.",
          turnCap: 20,
          projectRef: "project.fa07.restart",
          worktreeRef: "worktree.fa07.restart",
          gitHead: "cafebabe",
        }),
      );
      expect(started?.ok).toBe(true);
      const run = (started?.result as { run: { runRef: string; threadRef: string } }).run;

      // The report syncs from the engine's host-evidence cache, which is
      // refreshed at the START of a reconciliation pass -- so the turn just
      // dispatched is not visible until the NEXT pass re-reads the thread.
      // Drive one more sweep so there is real captured turn history to lose.
      await serverA.handleLine(
        request("2b", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "FA-07 restart pre-sweep",
          objective: "Drive one reconciliation sweep before the restart.",
          doneCondition: "done",
          turnCap: 20,
          projectRef: "project.fa07.restart.pre",
          worktreeRef: "worktree.fa07.restart.pre",
        }),
      );

      const beforeReport = await serverA.handleLine(
        request("3", 1, "get_report", { runRef: run.runRef }),
      );
      expect(beforeReport?.ok).toBe(true);
      const beforeTurns = (
        beforeReport?.result as { report: { turns: ReadonlyArray<{ turnRef: string }> } }
      ).report.turns;
      const beforeDetail = (
        (await serverA.handleLine(request("4", 1, "get_run", { runRef: run.runRef })))
          ?.result as { run: Record<string, unknown> }
      ).run;

      // The run must have actually done something before the restart, or the
      // test proves only that an empty record round-trips.
      expect(host.dispatched.length).toBeGreaterThan(0);

      // --- the restart --------------------------------------------------
      // A brand new service and framed server over the same data root, at a
      // new generation: exactly what the Rust supervisor does on relaunch.
      const serviceB = createOmegaEffectdService({ paths: { dataRoot: root } });
      const serverB = createOmegaEffectdFramedServer(
        serviceB,
        { dataRoot: root },
        { hostRequestHandler: host.handler },
      );
      await serverB.handleLine(request("10", 0, "initialize", { generation: 2 }));

      // --- generation 2: durable truth is intact ------------------------
      const afterDetail = (
        (await serverB.handleLine(request("11", 2, "get_run", { runRef: run.runRef })))
          ?.result as { run: Record<string, unknown> }
      ).run;

      expect(afterDetail.runRef).toBe(beforeDetail.runRef);
      expect(afterDetail.threadRef).toBe(beforeDetail.threadRef);
      expect(afterDetail.objective).toBe(beforeDetail.objective);
      expect(afterDetail.doneCondition).toBe(beforeDetail.doneCondition);
      expect(afterDetail.title).toBe(beforeDetail.title);
      expect(afterDetail.workspaceRef).toBe(beforeDetail.workspaceRef);
      expect(afterDetail.turnCap).toBe(beforeDetail.turnCap);

      // The native workspace binding is what makes a resumed run safe to
      // dispatch into; losing it silently would let a run continue against
      // the wrong tree.
      expect(afterDetail.nativeEvidence).toEqual(beforeDetail.nativeEvidence);

      const afterReport = await serverB.handleLine(
        request("12", 2, "get_report", { runRef: run.runRef }),
      );
      expect(afterReport?.ok).toBe(true);
      const afterTurns = (
        afterReport?.result as { report: { turns: ReadonlyArray<{ turnRef: string }> } }
      ).report.turns;

      // The pre-restart report must actually carry turns, or the
      // "nothing dropped" loop below would pass over an empty set and prove
      // nothing at all.
      expect(
        beforeTurns.length,
        "the run report captured no turns before the restart, so this test is vacuous",
      ).toBeGreaterThan(0);

      // Nothing dropped.
      for (const turn of beforeTurns) {
        expect(
          afterTurns.some((candidate) => candidate.turnRef === turn.turnRef),
          `the run report lost turn ${turn.turnRef} across the restart`,
        ).toBe(true);
      }
      // Nothing duplicated -- the restart re-reads the same host evidence, so
      // a naive re-sync would double every turn.
      const refs = afterTurns.map((turn) => turn.turnRef);
      expect(new Set(refs).size).toBe(refs.length);

      // --- reconciliation resumes --------------------------------------
      // A restart that preserves the record but never dispatches again is the
      // 2026-07-17 failure in a different costume.
      const dispatchedBefore = host.dispatched.length;
      await serverB.handleLine(
        request("13", 2, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "FA-07 restart sweep",
          objective: "Drive one reconciliation sweep after the restart.",
          doneCondition: "done",
          turnCap: 20,
          projectRef: "project.fa07.restart.sweep",
          worktreeRef: "worktree.fa07.restart.sweep",
        }),
      );
      expect(
        host.dispatched.length,
        "no continuation was dispatched for the restored run after restart",
      ).toBeGreaterThan(dispatchedBefore);

      const resumed = (
        (await serverB.handleLine(request("14", 2, "get_run", { runRef: run.runRef })))
          ?.result as { run: { state: string; stallCause: string | null } }
      ).run;
      expect(resumed.state).toBe("running");
      expect(resumed.stallCause).toBeNull();
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  test("a stale generation cannot address the restored run", async () => {
    // FA-07's restart story depends on generation fencing: after a restart,
    // frames minted by the previous generation must not mutate the new one.
    const root = mkdtempSync(path.join(tmpdir(), "oa-fa07-restart-gen-"));
    try {
      const host = makeDurableHost();
      const serviceA = createOmegaEffectdService({ paths: { dataRoot: root } });
      const serverA = createOmegaEffectdFramedServer(
        serviceA,
        { dataRoot: root },
        { hostRequestHandler: host.handler },
      );
      await serverA.handleLine(request("1", 0, "initialize", { generation: 1 }));
      const started = await serverA.handleLine(
        request("2", 1, "start", {
          workspaceRef: "workspace.omega.supervised",
          title: "generation fence",
          objective: "Prove stale frames cannot mutate a restored run.",
          doneCondition: "done",
          turnCap: 6,
          projectRef: "project.fence",
          worktreeRef: "worktree.fence",
        }),
      );
      const runRef = (started?.result as { run: { runRef: string } }).run.runRef;

      const serviceB = createOmegaEffectdService({ paths: { dataRoot: root } });
      const serverB = createOmegaEffectdFramedServer(
        serviceB,
        { dataRoot: root },
        { hostRequestHandler: host.handler },
      );
      await serverB.handleLine(request("10", 0, "initialize", { generation: 2 }));

      // A stop frame from the OLD generation must be refused.
      const stale = await serverB.handleLine(request("11", 1, "stop", { runRef }));
      expect(stale?.ok).toBe(false);

      const state = (
        (await serverB.handleLine(request("12", 2, "get_run", { runRef })))?.result as {
          run: { state: string };
        }
      ).run.state;
      expect(state).not.toBe("stopped");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
