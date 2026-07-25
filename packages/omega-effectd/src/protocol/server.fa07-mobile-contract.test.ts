/**
 * FA-07 gate 7 (omega#26) — the engine must supply the two fields the mobile
 * Full Auto projection refuses to invent.
 *
 * omega#47 made the phone's honesty non-negotiable in two places, and both
 * cost the engine a field:
 *
 *   - `startedAtMs`. The Rust projector
 *     (`crates/full_auto_ui/src/issue31_adjunct.rs`, `started_at_ms`) accepts
 *     only a NUMBER the host recorded. It will not re-derive a start by parsing
 *     the formatted `updatedAt` string, because that would make the owner's
 *     unattended duration a parse of Omega's UI rather than a measurement. A
 *     run without one is refused with `UnattendedDurationUnknown`, since
 *     `unattendedMs: 0` reads on a phone as "just started" — a claim nothing
 *     supports.
 *   - `terminalReasonRef`. A terminal run must name WHY it ended as a bounded
 *     ref. The projector will not classify an ending by reading the free-text
 *     `terminalReason`, so a terminal run without the ref is refused rather
 *     than shown with an invented cause.
 *
 * The consequence is total: if the engine omits either field, the mobile Full
 * Auto adjunct projects NOTHING for that run, and gate 7 cannot be satisfied by
 * any amount of phone-side work. That is not hypothetical. The engine inside
 * the 0.2.0-rc11 candidate is `omega-effectd-v0.1.0-rc.8`
 * (openagents 509ae747f00f6f7ebb413809ff5bd6ea123e1c1c), which predates both
 * fields — `grep` finds neither string in its shipped bundle. Driving that
 * installed engine over the framed protocol returns `startedAtMs: null` and
 * `terminalReasonRef: null` for a real run, so every run it can produce would
 * be refused by the phone.
 *
 * Nothing pinned that, which is why a candidate could be built, signed,
 * notarized, and installed with an engine too old for the gate it was built to
 * prove. This test is the pin. It is a CONTRACT test, deliberately asserted on
 * the framed detail an Omega host actually receives, not on an internal type.
 */

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

import { createOmegaEffectdService } from "../service.ts";
import { createOmegaEffectdFramedServer } from "./server.ts";
import { OMEGA_EFFECTD_PROTOCOL_SCHEMA } from "./framed.ts";
import { makeOmegaEffectdTestHost } from "./test-host.ts";

const withRoot = async (fn: (root: string) => Promise<void>): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-effectd-fa07-mobile-"));
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

const startServer = async (root: string) => {
  const service = createOmegaEffectdService({ paths: { dataRoot: root } });
  const server = createOmegaEffectdFramedServer(
    service,
    { dataRoot: root },
    { hostRequestHandler: makeOmegaEffectdTestHost() },
  );
  await server.handleLine(request("init", 0, "initialize", { generation: 1 }));
  return server;
};

const runOf = (frame: { result?: unknown } | null): Record<string, unknown> =>
  ((frame?.result as Record<string, unknown>)?.run ?? {}) as Record<string, unknown>;

const startRun = (server: Awaited<ReturnType<typeof startServer>>, id: string) =>
  server.handleLine(
    request(id, 1, "start", {
      workspaceRef: "workspace.omega.supervised",
      title: "FA-07 mobile contract",
      objective: "FA07_MOBILE_CONTRACT_OBJECTIVE",
      doneCondition: "FA07_MOBILE_CONTRACT_DONE",
      turnCap: 4,
      projectRef: "project.fa07.mobile",
      worktreeRef: "worktree.fa07.mobile",
    }),
  );

describe("FA-07 gate 7: the engine supplies what the mobile projection refuses to invent", () => {
  test("a live run reports a numeric startedAtMs, so the phone can measure the unattended duration", async () => {
    await withRoot(async (root) => {
      const server = await startServer(root);
      const run = runOf(await startRun(server, "start"));

      expect(typeof run.startedAtMs).toBe("number");
      expect(run.startedAtMs).toBeGreaterThan(0);
      // A measurement, not a parse. `updatedAt` stays display-formatted text
      // precisely so nothing downstream is tempted to re-derive a duration
      // from it, and the numeric field must not simply echo it.
      expect(run.startedAtMs).not.toBe(run.updatedAt);

      const listed = (
        (await server.handleLine(request("list", 1, "list_runs")))?.result as {
          runs: ReadonlyArray<Record<string, unknown>>;
        }
      ).runs;
      // The monitor list is what a phone syncs, so the field has to survive the
      // redacted projection too, not only the owner-local detail.
      expect(typeof listed[0]?.startedAtMs).toBe("number");
    });
  });

  test("a live run has no terminalReasonRef, because it has not ended", async () => {
    await withRoot(async (root) => {
      const server = await startServer(root);
      const run = runOf(await startRun(server, "start"));
      // The inverse matters as much: a ref present on a running run would let a
      // phone render an ending that has not happened.
      expect(run.terminalReasonRef).toBeNull();
    });
  });

  test("a stopped run names why it ended as a bounded ref, not as prose", async () => {
    await withRoot(async (root) => {
      const server = await startServer(root);
      const runRef = String(runOf(await startRun(server, "start")).runRef);

      const stopped = runOf(await server.handleLine(request("stop", 1, "stop", { runRef })));
      expect(stopped.state).toBe("stopped");

      const reasonRef = stopped.terminalReasonRef;
      expect(typeof reasonRef).toBe("string");
      expect(String(reasonRef).startsWith("terminal.full_auto.")).toBe(true);
      // The ref carries the terminal state so the phone can classify the ending
      // without reading `terminalReason`, which stays free text for a human.
      expect(String(reasonRef)).toContain("stopped");
      expect(String(reasonRef).length).toBeLessThanOrEqual(180);
      expect(String(reasonRef)).not.toContain(" ");

      // And the ending must still be measurable: a terminal run the phone
      // cannot date is refused exactly like a live one.
      expect(typeof stopped.startedAtMs).toBe("number");
    });
  });

  test("a run the phone controlled to a stop is still fully projectable", async () => {
    await withRoot(async (root) => {
      const server = await startServer(root);
      const runRef = String(runOf(await startRun(server, "start")).runRef);

      const outcome = (
        (
          await server.handleLine(
            request("intent", 1, "apply_control_intent", {
              intentId: "intent.fa07.mobile.stop",
              runRef,
              action: "stop",
            }),
          )
        )?.result as { outcome: Record<string, unknown> }
      ).outcome;
      expect(outcome.status).toBe("applied");
      expect(outcome.resultLifecycleState).toBe("stopped");

      // The whole point of the packet: a control the owner pressed on a phone
      // must settle into a run the phone can then render honestly. Missing
      // either field here would leave the owner holding a button that appeared
      // to work against a run their phone refuses to show.
      const detail = runOf(await server.handleLine(request("detail", 1, "get_run", { runRef })));
      expect(typeof detail.startedAtMs).toBe("number");
      expect(typeof detail.terminalReasonRef).toBe("string");
    });
  });
});
