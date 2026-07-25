/**
 * FA-07 gate 8 — no ordinary chat path can start Full Auto authority.
 *
 * This is a security property, not a UX property. The claim is that a user
 * (or a model) inside an ordinary conversation cannot escalate into starting
 * an unattended autonomous run. Only the dedicated Full Auto launcher may do
 * that.
 *
 * The engine's intent layer is the load-bearing boundary for every remote and
 * chat-adjacent surface: mobile controls, the Sync/relay poll loop, the cloud
 * control route, and Sarah's model-facing `full_auto_control` tool all funnel
 * into `apply_control_intent`. Each of those layers independently restricts
 * itself to pause/resume/stop, but the engine must refuse an escalating
 * action on its own authority rather than trusting its callers.
 *
 * Until now that allowlist was only exercised by its three happy paths
 * (`server.test.ts` pause/resume/stop). Nothing asserted that `start` is
 * refused, so a widened union or a new branch could quietly grant a phone or
 * a chat tool the ability to mint autonomy, and the whole existing suite
 * would stay green. These tests pin the refusal itself.
 *
 * Scope note, stated honestly: this file covers the ENGINE boundary reached
 * by remote/chat surfaces. It does not, and cannot, cover the host UI
 * inventory (which GPUI actions exist) or other hosts that embed this same
 * engine. See the FA-07 gate 8 entry-point inventory for those.
 */

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

import { createOmegaEffectdService } from "../service.ts";
import { createOmegaEffectdFramedServer } from "./server.ts";
import { OMEGA_EFFECTD_PROTOCOL_SCHEMA } from "./framed.ts";
import { makeOmegaEffectdTestHost } from "./test-host.ts";

const request = (id: string, generation: number, method: string, params?: unknown) =>
  JSON.stringify({
    schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
    kind: "request",
    id,
    generation,
    method,
    ...(params === undefined ? {} : { params }),
  });

const withServer = async (
  fn: (
    server: ReturnType<typeof createOmegaEffectdFramedServer>,
    runRef: string,
  ) => Promise<void>,
): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-fa07-authority-"));
  try {
    const service = createOmegaEffectdService({ paths: { dataRoot: root } });
    const server = createOmegaEffectdFramedServer(
      service,
      { dataRoot: root },
      { hostRequestHandler: makeOmegaEffectdTestHost() },
    );
    await server.handleLine(request("1", 0, "initialize", { generation: 1 }));
    const started = await server.handleLine(
      request("2", 1, "start", {
        workspaceRef: "workspace.omega.supervised",
        title: "authority fixture",
        objective: "An existing run for the control-intent surface to address.",
        doneCondition: "done",
        turnCap: 6,
        projectRef: "project.authority",
        worktreeRef: "worktree.authority",
      }),
    );
    await fn(server, (started?.result as { run: { runRef: string } }).run.runRef);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

/**
 * Every action a caller might use to try to mint autonomy through the control
 * intent surface. `pause`, `resume`, and `stop` are the entire legitimate
 * vocabulary; anything else must be refused, including near-misses and
 * casing/whitespace variants that a permissive comparison would let through.
 */
const ESCALATING_ACTIONS = [
  "start",
  "create",
  "enable",
  "run",
  "launch",
  "begin",
  "retry",
  "continue",
  "continue_now",
  "Start",
  "START",
  " start",
  "start ",
  "start\n",
  "pause;start",
  "full_auto_start",
] as const;

describe("FA-07 gate 8 — chat and remote surfaces cannot start Full Auto", () => {
  test("apply_control_intent refuses every escalating action on an existing run", async () => {
    await withServer(async (server, runRef) => {
      for (const [index, action] of ESCALATING_ACTIONS.entries()) {
        const response = await server.handleLine(
          request(`esc.${index}`, 1, "apply_control_intent", {
            intentId: `intent.escalate.${index}`,
            runRef,
            action,
            actor: "mobile",
          }),
        );
        expect(
          response?.ok,
          `apply_control_intent accepted the escalating action ${JSON.stringify(action)}. ` +
            `A phone, a relay message, or a model-facing control tool could then mint ` +
            `Full Auto authority from an ordinary conversation.`,
        ).toBe(false);
        expect((response?.error as { code: string } | undefined)?.code).toBe("invalid_request");
      }
    });
  });

  test("apply_control_intent cannot bring a run into existence", async () => {
    await withServer(async (server) => {
      // Not merely "start is not in the enum" -- the surface must have no way
      // to address a run that does not exist yet, whatever verb is used.
      //
      // A refusal here is a typed outcome inside a successful envelope (the
      // engine's "never silent" contract), so the property under test is the
      // OUTCOME, not the transport status: nothing may come back `applied`,
      // and no run may appear.
      for (const action of ["pause", "resume", "stop", "start"] as const) {
        const response = await server.handleLine(
          request(`ghost.${action}`, 1, "apply_control_intent", {
            intentId: `intent.ghost.${action}`,
            runRef: "run.full-auto.does-not-exist.0000",
            action,
            actor: "mobile",
          }),
        );
        const status =
          response?.ok === true
            ? (response.result as { outcome: { status: string } }).outcome.status
            : "transport_error";
        expect(
          status,
          `apply_control_intent(${action}) applied against a run that never existed`,
        ).not.toBe("applied");
      }

      // The ghost run must not have been brought into existence by any of
      // those attempts.
      const listed = await server.handleLine(request("ghost.list", 1, "list_runs"));
      expect(JSON.stringify(listed)).not.toContain("does-not-exist");
    });
  });

  test("the three legitimate controls still work, so the refusals above are not vacuous", async () => {
    await withServer(async (server, runRef) => {
      for (const [index, action] of (["pause", "resume", "stop"] as const).entries()) {
        const response = await server.handleLine(
          request(`ok.${index}`, 1, "apply_control_intent", {
            intentId: `intent.ok.${index}`,
            runRef,
            action,
            actor: "mobile",
          }),
        );
        expect(response?.ok, `the legitimate control ${action} was refused`).toBe(true);
        expect((response?.result as { outcome: { status: string } }).outcome.status).toBe(
          "applied",
        );
      }
    });
  });

  test("a mobile actor cannot reach the start method itself", async () => {
    // `start` exists on the framed protocol because the launcher needs it.
    // The property that matters is that the control-intent surface -- the one
    // every remote and chat-adjacent caller actually reaches -- is a strictly
    // smaller vocabulary. Assert the two surfaces are not interchangeable.
    await withServer(async (server) => {
      const response = await server.handleLine(
        request("m1", 1, "apply_control_intent", {
          intentId: "intent.method.start",
          runRef: "run.full-auto.whatever.0000",
          action: "start",
          actor: "mobile",
          // A caller trying to smuggle launcher arguments through the control
          // surface must not find them honoured.
          workspaceRef: "workspace.omega.supervised",
          objective: "smuggled objective",
          doneCondition: "smuggled done condition",
          projectRef: "project.smuggled",
          worktreeRef: "worktree.smuggled",
        }),
      );
      expect(response?.ok).toBe(false);

      // And nothing was created as a side effect.
      const listed = await server.handleLine(request("m2", 1, "list_runs"));
      const runs = (listed?.result as { runs: ReadonlyArray<{ title: string }> }).runs;
      expect(runs.every((run) => run.title !== "smuggled objective")).toBe(true);
      expect(JSON.stringify(runs)).not.toContain("smuggled");
    });
  });
});
