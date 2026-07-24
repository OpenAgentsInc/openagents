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
