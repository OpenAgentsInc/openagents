import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vite-plus/test";

describe("remaining Sarah LiveKit drill CLI", () => {
  test("dry-run performs no authority read, network request, or receipt write", () => {
    const script = fileURLToPath(new URL("./remaining-drill-cli.ts", import.meta.url));
    const result = spawnSync(process.execPath, ["--import", "tsx", script], {
      cwd: fileURLToPath(new URL("..", import.meta.url)),
      encoding: "utf8",
      env: {
        PATH: process.env.PATH,
        NODE_PATH: process.env.NODE_PATH,
      },
    });

    expect(result.status).toBe(0);
    expect(result.stderr).toBe("");
    expect(JSON.parse(result.stdout)).toMatchObject({
      schema: "openagents.sarah.livekit-remaining-drill-dry-run.v1",
      liveSessionOpened: false,
      providerDisconnectRequested: false,
      creditMutated: false,
      receiptWritten: false,
    });
  });

  test("binds live collection to read-only authority and exclusive private/public files", () => {
    const source = readFileSync(new URL("./remaining-drill-cli.ts", import.meta.url), "utf8");

    expect(source).toContain("I_ACCEPT_EP263_SARAH_REMAINING_DRILLS");
    expect(source).toContain("BEGIN TRANSACTION READ ONLY");
    expect(source).toContain('spawn("psql", ["-X", "-q", "-A", "-t", "-v", "ON_ERROR_STOP=1"]');
    expect(source).toContain("sarah_livekit_provider_disconnect_faults");
    expect(source).toContain("activityAfterTerminalCount");
    expect(source).toContain("chargedMsat");
    expect(source).toContain("private remaining-drill observation must stay outside");
    expect(source).toContain("remaining-drill public receipt must stay under");
    expect(source.match(/flag: "wx"/gu)).toHaveLength(2);
    expect(source.match(/mode: 0o600/gu)).toHaveLength(2);
    expect(source).not.toContain("secretmanager");
    expect(source).not.toContain("gcloud secrets");
    expect(source).not.toMatch(/console\.log\([^)]*(?:BEARER|authorization)/u);
  });
});
