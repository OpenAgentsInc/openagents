import {
  DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
  DesktopSourceSafePointRendezvousSchema,
  type DesktopSourceSafePointControlRequest,
} from "@openagentsinc/portable-session-contract";
import { afterEach, describe, expect, test } from "vite-plus/test";
import { access, lstat, mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Schema } from "effect";

import { makeDesktopSourceSafePoint } from "./desktop-source-safe-point.ts";
import { startDesktopSourceSafePointControlServer } from "./desktop-source-safe-point-control-server.ts";

const roots: string[] = [];
const decodeRendezvous = Schema.decodeUnknownSync(DesktopSourceSafePointRendezvousSchema);
afterEach(async () =>
  Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))),
);

const request: DesktopSourceSafePointControlRequest = {
  schema: DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
  operationRef: "operation.move.source.quiesce",
  commandRef: "command.move.1",
  commandExecutionClaimRef: "claim.command.move.1",
  ownerRef: "owner.1",
  pylonRef: "pylon.1",
  targetRef: "target.owner.local.1",
  sessionRef: "session.1",
  attachmentRef: "attachment.1",
  attachmentGeneration: 1,
  expiresAt: "2026-07-20T12:05:00.000Z",
};

describe("Desktop source safe-point control server", () => {
  test("publishes private loopback custody, authenticates, and replays one instance result", async () => {
    const root = await mkdtemp(join(tmpdir(), "desktop-safe-point-control-"));
    roots.push(root);
    let calls = 0;
    const safePoint = makeDesktopSourceSafePoint({
      currentBinding: () => ({
        sessionRef: request.sessionRef,
        attachmentRef: request.attachmentRef,
        grantRef: "grant.private.1",
        generation: request.attachmentGeneration,
      }),
      subsystems: [
        {
          subsystem: "workspace",
          quiesce: async () => {
            calls += 1;
            return { state: "quiesced" };
          },
        },
      ],
    });
    const server = await startDesktopSourceSafePointControlServer({
      pylonHome: root,
      pylonRef: request.pylonRef,
      safePoint,
      currentAuthority: () => ({
        ownerRef: request.ownerRef,
        pylonRef: request.pylonRef,
        targetRef: request.targetRef,
      }),
      now: () => new Date("2026-07-20T12:00:00.000Z"),
    });
    const rendezvous = decodeRendezvous(JSON.parse(await readFile(server.rendezvousPath, "utf8")));
    expect((await lstat(join(root, "desktop-source-safe-point"))).mode & 0o077).toBe(0);
    expect((await lstat(server.rendezvousPath)).mode & 0o077).toBe(0);
    expect(rendezvous.url).toMatch(/^http:\/\/127\.0\.0\.1:/u);

    const invoke = () =>
      fetch(rendezvous.url, {
        method: "POST",
        headers: {
          authorization: `Bearer ${rendezvous.bearerToken}`,
          "content-type": "application/json",
        },
        body: JSON.stringify(request),
      });
    const first = await invoke();
    const second = await invoke();
    expect(first.status).toBe(200);
    expect(await first.json()).toMatchObject({
      desktopInstanceRef: server.desktopInstanceRef,
      operationRef: request.operationRef,
      state: "quiescent",
      remoteExecution: "not_claimed",
    });
    expect(await second.json()).toMatchObject({ state: "quiescent" });
    expect(calls).toBe(1);

    const unauthorized = await fetch(rendezvous.url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(request),
    });
    expect(unauthorized.status).toBe(401);
    const excess = await fetch(rendezvous.url, {
      method: "POST",
      headers: {
        authorization: `Bearer ${rendezvous.bearerToken}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({ ...request, grantRef: "grant.must-not-cross" }),
    });
    expect(excess.status).toBe(400);

    await server.stop();
    await expect(access(server.rendezvousPath)).rejects.toThrow();
  });

  test("refuses expired and authority-mismatched requests without entering the safe point", async () => {
    const root = await mkdtemp(join(tmpdir(), "desktop-safe-point-control-"));
    roots.push(root);
    let calls = 0;
    const safePoint = makeDesktopSourceSafePoint({
      currentBinding: () => ({
        sessionRef: request.sessionRef,
        attachmentRef: request.attachmentRef,
        grantRef: "grant.private.1",
        generation: request.attachmentGeneration,
      }),
      subsystems: [
        {
          subsystem: "workspace",
          quiesce: async () => {
            calls += 1;
            return { state: "quiesced" };
          },
        },
      ],
    });
    const server = await startDesktopSourceSafePointControlServer({
      pylonHome: root,
      pylonRef: request.pylonRef,
      safePoint,
      currentAuthority: () => ({
        ownerRef: request.ownerRef,
        pylonRef: request.pylonRef,
        targetRef: request.targetRef,
      }),
      now: () => new Date("2026-07-20T12:10:00.000Z"),
    });
    const rendezvous = decodeRendezvous(JSON.parse(await readFile(server.rendezvousPath, "utf8")));
    const response = await fetch(rendezvous.url, {
      method: "POST",
      headers: {
        authorization: `Bearer ${rendezvous.bearerToken}`,
        "content-type": "application/json",
      },
      body: JSON.stringify(request),
    });
    expect(await response.json()).toMatchObject({
      state: "refused",
      reasonRef: "desktop.safe-point.request-expired",
    });
    expect(calls).toBe(0);
    await server.stop();
  });
});
