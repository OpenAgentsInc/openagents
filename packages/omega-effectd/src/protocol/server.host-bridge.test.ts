import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

import { createOmegaEffectdService } from "../service.ts";
import {
  OMEGA_EFFECTD_MAX_FRAME_BYTES,
  OMEGA_EFFECTD_PROTOCOL_SCHEMA,
  type OmegaEffectdHostRequest,
} from "./framed.ts";
import { createOmegaEffectdFramedServer } from "./server.ts";
import { makeOmegaEffectdTestHost } from "./test-host.ts";

const withRoot = async (run: (root: string) => Promise<void>): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-effectd-host-bridge-"));
  try {
    await run(root);
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

const startParams = {
  workspaceRef: "workspace.omega.supervised",
  title: "Host bridge proof",
  objective: "PRIVATE_OBJECTIVE",
  doneCondition: "PRIVATE_DONE_CONDITION",
};

describe("omega-effectd host bridge", () => {
  test("refreshes every host lane before it projects capacity", async () => {
    await withRoot(async (root) => {
      const probedLanes: string[] = [];
      const baseHost = makeOmegaEffectdTestHost();
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          hostRequestHandler: async (frame) => {
            if (frame.method !== "lane_readiness") return baseHost(frame);
            const lane = (frame.params as { lane: string }).lane;
            probedLanes.push(lane);
            return {
              known: true,
              admitted: true,
              fullAuto: true,
              state: lane === "codex-local" ? "available" : "unavailable",
            };
          },
        },
      );
      await server.handleLine(request("init", 0, "initialize", { generation: 1 }));

      const capacity = await server.handleLine(request("capacity", 1, "get_capacity"));

      expect(capacity?.ok).toBe(true);
      expect(probedLanes.sort()).toEqual([
        "acp:cursor-agent",
        "acp:grok-cli",
        "claude-local",
        "codex-local",
        "harness:goose",
        "harness:opencode",
        "harness:pi",
      ]);
      const lanes = (capacity?.result as { lanes: Array<{ lane: string; state: string }> }).lanes;
      expect(lanes.find((lane) => lane.lane === "codex-local")?.state).toBe("available");
      expect(lanes.find((lane) => lane.lane === "claude-local")?.state).toBe("unavailable");
    });
  });

  test("rejects a capacity refresh from an old supervisor generation", async () => {
    await withRoot(async (root) => {
      const emitted: OmegaEffectdHostRequest[] = [];
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          emitHostRequest: (frame) => {
            emitted.push(frame);
          },
        },
      );
      await server.handleLine(request("init-1", 0, "initialize", { generation: 1 }));
      const oldCapacity = server.handleLine(request("capacity-1", 1, "get_capacity"));
      await Promise.resolve();
      expect(emitted.filter((frame) => frame.generation === 1)).toHaveLength(7);

      const initialized = await server.handleLine(
        request("init-2", 1, "initialize", { generation: 2 }),
      );
      expect(initialized?.ok).toBe(true);
      const stale = await oldCapacity;
      expect(stale?.ok).toBe(false);
      expect(stale?.error?.code).toBe("stale_generation");

      const currentCapacity = server.handleLine(request("capacity-2", 2, "get_capacity"));
      await Promise.resolve();
      const currentRequests = emitted.filter((frame) => frame.generation === 2);
      expect(currentRequests).toHaveLength(7);
      for (const frame of currentRequests) {
        await server.handleLine(
          JSON.stringify({
            schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
            kind: "host_response",
            id: frame.id,
            generation: frame.generation,
            ok: true,
            result: { known: true, admitted: true, fullAuto: true, state: "available" },
          }),
        );
      }
      const current = await currentCapacity;
      expect(current?.ok).toBe(true);
      const lanes = (current?.result as { lanes: Array<{ state: string }> }).lanes;
      expect(lanes.every((lane) => lane.state === "available")).toBe(true);
    });
  });

  test("dispatches the exact leased continuation once", async () => {
    await withRoot(async (root) => {
      const dispatched: OmegaEffectdHostRequest[] = [];
      const baseHost = makeOmegaEffectdTestHost();
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          hostRequestHandler: async (frame) => {
            if (frame.method === "dispatch_turn") dispatched.push(frame);
            return baseHost(frame);
          },
        },
      );
      await server.handleLine(request("init", 0, "initialize", { generation: 1 }));
      const started = await server.handleLine(request("start", 1, "start", startParams));

      expect(started?.ok).toBe(true);
      expect(dispatched).toHaveLength(1);
      const params = dispatched[0]!.params as { threadRef: string; turnRef: string };
      expect(params.threadRef).toBe(
        (started!.result as { run: { threadRef: string } }).run.threadRef,
      );
      expect(params.turnRef).toMatch(/^turn\.full-auto\./);
    });
  });

  test("settles pausing to paused after the provider turn completes", async () => {
    await withRoot(async (root) => {
      const baseHost = makeOmegaEffectdTestHost();
      let evidenceRefreshes = 0;
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          hostRequestHandler: async (frame) => {
            if (frame.method !== "refresh_evidence") return baseHost(frame);
            evidenceRefreshes += 1;
            return {
              present: true,
              revision: evidenceRefreshes,
              live:
                evidenceRefreshes === 2
                  ? { state: "turn_running", turnRef: "turn.full-auto.fixture" }
                  : evidenceRefreshes >= 3
                    ? { state: "turn_completed", turnRef: "turn.full-auto.fixture" }
                    : null,
              turns: [],
            };
          },
        },
      );
      await server.handleLine(request("init", 0, "initialize", { generation: 1 }));
      const started = await server.handleLine(request("start", 1, "start", startParams));
      const runRef = (started!.result as { run: { runRef: string } }).run.runRef;

      const pausing = await server.handleLine(request("pause", 1, "pause", { runRef }));
      expect((pausing!.result as { run: { state: string } }).run.state).toBe("pausing");

      const settled = await server.handleLine(request("get", 1, "get_run", { runRef }));
      expect((settled!.result as { run: { state: string } }).run.state).toBe("paused");
      expect(evidenceRefreshes).toBe(3);
    });
  });

  test("fails closed with a typed error when the host is unavailable", async () => {
    await withRoot(async (root) => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(service, { dataRoot: root });
      await server.handleLine(request("init", 0, "initialize", { generation: 1 }));
      const started = await server.handleLine(request("start", 1, "start", startParams));

      expect(started?.ok).toBe(false);
      expect(started?.error?.code).toBe("host_unavailable");
    });
  });

  test("stalls the run when the host cannot find its freshly bound thread", async () => {
    await withRoot(async (root) => {
      const baseHost = makeOmegaEffectdTestHost();
      const dispatched: OmegaEffectdHostRequest[] = [];
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          hostRequestHandler: async (frame) => {
            if (frame.method === "refresh_evidence") {
              return { present: false, revision: 1, live: null, turns: [] };
            }
            if (frame.method === "dispatch_turn") dispatched.push(frame);
            return baseHost(frame);
          },
        },
      );
      await server.handleLine(request("init", 0, "initialize", { generation: 1 }));
      const started = await server.handleLine(request("start", 1, "start", startParams));

      expect(started?.ok).toBe(true);
      const run = (started!.result as { run: { state: string; stallCause: string | null } }).run;
      expect(run.state).toBe("stalled");
      expect(run.stallCause).toBe("host_thread_missing");
      expect(dispatched).toHaveLength(0);
    });
  });

  test("rejects stale host replies without settling a newer generation", async () => {
    await withRoot(async (root) => {
      const emitted: OmegaEffectdHostRequest[] = [];
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          emitHostRequest: (frame) => {
            emitted.push(frame);
          },
        },
      );
      await server.handleLine(request("init-1", 0, "initialize", { generation: 1 }));
      const pendingStart = server.handleLine(request("start", 1, "start", startParams));
      await Promise.resolve();
      expect(emitted).toHaveLength(1);

      const initialized = await server.handleLine(
        request("init-2", 1, "initialize", { generation: 2 }),
      );
      expect(initialized?.ok).toBe(true);
      const stale = emitted[0]!;
      await server.handleLine(
        JSON.stringify({
          schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
          kind: "host_response",
          id: stale.id,
          generation: stale.generation,
          ok: true,
          result: { workspaceRef: startParams.workspaceRef },
        }),
      );
      const result = await pendingStart;
      expect(result?.ok).toBe(false);
      expect(result?.error?.code).toBe("stale_generation");
    });
  });

  test("times out unanswered host calls and rejects their late replies", async () => {
    await withRoot(async (root) => {
      const emitted: OmegaEffectdHostRequest[] = [];
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          emitHostRequest: (frame) => {
            emitted.push(frame);
          },
          hostRequestTimeoutMs: 5,
        },
      );
      await server.handleLine(request("init", 0, "initialize", { generation: 1 }));
      const started = await server.handleLine(request("start", 1, "start", startParams));

      expect(started?.ok).toBe(false);
      expect(started?.error?.code).toBe("host_timeout");
      expect(emitted).toHaveLength(1);
      const timedOut = emitted[0]!;
      const late = await server.handleLine(
        JSON.stringify({
          schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
          kind: "host_response",
          id: timedOut.id,
          generation: timedOut.generation,
          ok: true,
          result: { workspaceRef: startParams.workspaceRef },
        }),
      );
      expect(late).toBeNull();

      const health = await server.handleLine(request("health", 1, "health"));
      expect(health?.ok).toBe(true);
    });
  });

  test("bounds frames and keeps public projections secret-free", async () => {
    await withRoot(async (root) => {
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          hostRequestHandler: makeOmegaEffectdTestHost(),
        },
      );
      await server.handleLine(request("init", 0, "initialize", { generation: 1 }));
      const oversized = await server.handleLine("x".repeat(OMEGA_EFFECTD_MAX_FRAME_BYTES + 1));
      expect(oversized?.error?.code).toBe("frame_too_large");

      const started = await server.handleLine(request("start", 1, "start", startParams));
      expect(started?.ok).toBe(true);
      const listed = await server.handleLine(request("list", 1, "list_runs"));
      const publicJson = JSON.stringify(listed);
      expect(publicJson).not.toContain("PRIVATE_OBJECTIVE");
      expect(publicJson).not.toContain("PRIVATE_DONE_CONDITION");
    });
  });
});
