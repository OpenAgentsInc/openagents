import {
  DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
  type DesktopSourceSafePointControlRequest,
} from "@openagentsinc/portable-session-contract";
import { afterEach, describe, expect, test } from "vite-plus/test";
import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { makePylonDesktopSourceSafePointClient } from "./desktop-source-safe-point-client.ts";

const roots: string[] = [];
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

const writeRendezvous = async (root: string, desktopInstanceRef = "desktop.instance.1") => {
  const directory = join(root, "desktop-source-safe-point");
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const path = join(directory, "control.json");
  await writeFile(
    path,
    JSON.stringify({
      schema: DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
      desktopInstanceRef,
      pylonRef: request.pylonRef,
      url: "http://127.0.0.1:49123/v1/source-safe-point",
      bearerToken: "a".repeat(64),
      issuedAt: "2026-07-20T12:00:00.000Z",
    }),
    { mode: 0o600 },
  );
  return path;
};

describe("Pylon Desktop source safe-point client", () => {
  test("validates private custody and sends only the exact refs body", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-desktop-safe-point-"));
    roots.push(root);
    await writeRendezvous(root);
    let posted: unknown = null;
    const client = makePylonDesktopSourceSafePointClient({
      pylonHome: root,
      pylonRef: request.pylonRef,
      now: () => new Date("2026-07-20T12:00:00.000Z"),
      fetchImpl: async (_url, init) => {
        posted = JSON.parse(String(init?.body));
        expect(init?.method).toBe("POST");
        expect(new Headers(init?.headers).get("authorization")).toBe(`Bearer ${"a".repeat(64)}`);
        return Response.json({
          schema: DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
          desktopInstanceRef: "desktop.instance.1",
          operationRef: request.operationRef,
          state: "quiescent",
          reasonRef: null,
          evidenceRefs: ["receipt.desktop.safe-point.1"],
          remoteExecution: "not_claimed",
        });
      },
    });
    await expect(client.quiesce(request)).resolves.toMatchObject({ state: "quiescent" });
    expect(posted).toEqual(request);
    expect(JSON.stringify(posted)).not.toMatch(/grantRef|credential|path|graph|process|provider/u);
  });

  test("fails closed for a symlink rendezvous and a changed Desktop instance", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-desktop-safe-point-"));
    roots.push(root);
    const directory = join(root, "desktop-source-safe-point");
    await mkdir(directory, { mode: 0o700 });
    const outside = join(root, "outside.json");
    await writeFile(outside, "{}", { mode: 0o600 });
    await symlink(outside, join(directory, "control.json"));
    const blocked = makePylonDesktopSourceSafePointClient({
      pylonHome: root,
      pylonRef: request.pylonRef,
      now: () => new Date("2026-07-20T12:00:00.000Z"),
    });
    await expect(blocked.quiesce(request)).rejects.toMatchObject({
      reason: "invalid_custody",
    });

    await rm(join(directory, "control.json"));
    await writeRendezvous(root, "desktop.instance.before");
    const restarted = makePylonDesktopSourceSafePointClient({
      pylonHome: root,
      pylonRef: request.pylonRef,
      now: () => new Date("2026-07-20T12:00:00.000Z"),
      fetchImpl: async () =>
        Response.json({
          schema: DESKTOP_SOURCE_SAFE_POINT_CONTROL_SCHEMA_VERSION,
          desktopInstanceRef: "desktop.instance.after",
          operationRef: request.operationRef,
          state: "quiescent",
          reasonRef: null,
          evidenceRefs: [],
          remoteExecution: "not_claimed",
        }),
    });
    await expect(restarted.quiesce(request)).rejects.toMatchObject({
      reason: "desktop_restarted",
    });
  });
});
