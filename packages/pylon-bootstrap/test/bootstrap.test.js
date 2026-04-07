import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import {
  bootstrapInstalledPylon,
  buildAssetNames,
  ensureReleaseInstall,
  launchInstalledPylonTui,
  parseSha256File,
  resolvePlatformTarget,
  selectLatestPylonRelease,
  selectReleaseAssets,
} from "../src/index.js";

describe("@openagentsinc/pylon bootstrap", () => {
  test("resolvePlatformTarget maps supported hosts", () => {
    expect(resolvePlatformTarget("darwin", "arm64")).toEqual({
      os: "darwin",
      arch: "arm64",
    });
    expect(resolvePlatformTarget("linux", "x64")).toEqual({
      os: "linux",
      arch: "x86_64",
    });
    expect(() => resolvePlatformTarget("win32", "x64")).toThrow(
      "Unsupported platform",
    );
  });

  test("selectReleaseAssets matches the expected GitHub asset names", () => {
    const target = resolvePlatformTarget("darwin", "arm64");
    const names = buildAssetNames("1.2.3", target);
    const selection = selectReleaseAssets(
      {
        tag_name: "pylon-v1.2.3",
        assets: [
          {
            name: names.archiveName,
            browser_download_url: "https://example.com/archive",
          },
          {
            name: names.checksumName,
            browser_download_url: "https://example.com/checksum",
          },
        ],
      },
      target,
    );

    expect(selection.version).toBe("1.2.3");
    expect(selection.archiveAsset.name).toBe(names.archiveName);
    expect(selection.checksumAsset.name).toBe(names.checksumName);
  });

  test("parseSha256File keeps the published checksum format honest", () => {
    const payload =
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  pylon-v1.2.3-darwin-arm64.tar.gz\n";
    expect(
      parseSha256File(payload, "pylon-v1.2.3-darwin-arm64.tar.gz"),
    ).toBe(
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    );
  });

  test("selectLatestPylonRelease ignores non-pylon repo releases", () => {
    const release = selectLatestPylonRelease([
      {
        tag_name: "autopilot-v0.1.1",
        draft: false,
      },
      {
        tag_name: "pylon-v0.0.1-rc3",
        draft: false,
      },
      {
        tag_name: "pylon-v0.0.1-rc2",
        draft: false,
      },
    ]);

    expect(release.tag_name).toBe("pylon-v0.0.1-rc3");
  });

  describe("ensureReleaseInstall", () => {
    let server;
    let serverUrl;

    beforeAll(() => {
      let releaseListHits = 0;
      let taggedReleaseHits = 0;
      let archiveHits = 0;
      let checksumHits = 0;

      server = {
        counters: () => ({
          releaseListHits,
          taggedReleaseHits,
          archiveHits,
          checksumHits,
        }),
      };

      let archivePayload;
      let checksumPayload;
      let archiveName;

      server.prepare = async () => {
        const tempDir = await fs.mkdtemp(
          path.join(os.tmpdir(), "pylon-bootstrap-archive-"),
        );
        const archiveRoot = path.join(tempDir, "stage");
        const archiveBase = "pylon-v1.2.3-darwin-arm64";
        archiveName = `${archiveBase}.tar.gz`;
        const extractedDir = path.join(archiveRoot, archiveBase);
        await fs.mkdir(extractedDir, { recursive: true });
        await fs.writeFile(path.join(extractedDir, "pylon"), "#!/bin/sh\nexit 0\n");
        await fs.writeFile(path.join(extractedDir, "pylon-tui"), "#!/bin/sh\nexit 0\n");
        await fs.chmod(path.join(extractedDir, "pylon"), 0o755);
        await fs.chmod(path.join(extractedDir, "pylon-tui"), 0o755);

        const archivePath = path.join(tempDir, archiveName);
        await Bun.spawn(["tar", "-czf", archivePath, "-C", archiveRoot, archiveBase], {
          stdout: "ignore",
          stderr: "inherit",
        }).exited;

        archivePayload = await fs.readFile(archivePath);
        const sha256 = createHash("sha256")
          .update(archivePayload)
          .digest("hex");
        checksumPayload = `${sha256}  ${archiveName}\n`;
      };

      return server.prepare().then(() => {
        const bunServer = Bun.serve({
          port: 0,
          fetch(request) {
            const url = new URL(request.url);
            if (url.pathname === "/repos/OpenAgentsInc/openagents/releases") {
              releaseListHits += 1;
              return Response.json([
                {
                  tag_name: "autopilot-v0.1.1",
                  draft: false,
                  assets: [],
                },
                {
                  tag_name: "pylon-v1.2.3",
                  draft: false,
                  assets: [
                    {
                      name: archiveName,
                      browser_download_url: `${serverUrl}/assets/${archiveName}`,
                    },
                    {
                      name: `${archiveName}.sha256`,
                      browser_download_url: `${serverUrl}/assets/${archiveName}.sha256`,
                    },
                  ],
                },
              ]);
            }
            if (url.pathname === "/repos/OpenAgentsInc/openagents/releases/tags/pylon-v1.2.3") {
              taggedReleaseHits += 1;
              return Response.json({
                tag_name: "pylon-v1.2.3",
                assets: [
                  {
                    name: archiveName,
                    browser_download_url: `${serverUrl}/assets/${archiveName}`,
                  },
                  {
                    name: `${archiveName}.sha256`,
                    browser_download_url: `${serverUrl}/assets/${archiveName}.sha256`,
                  },
                ],
              });
            }
            if (url.pathname === `/assets/${archiveName}`) {
              archiveHits += 1;
              return new Response(archivePayload);
            }
            if (url.pathname === `/assets/${archiveName}.sha256`) {
              checksumHits += 1;
              return new Response(checksumPayload);
            }
            return new Response("not found", { status: 404 });
          },
        });
        server.stop = () => bunServer.stop(true);
        serverUrl = `http://127.0.0.1:${bunServer.port}`;
      });
    });

    afterAll(() => {
      server?.stop?.();
    });

    test("downloads once, verifies the checksum, and reuses the cached install", async () => {
      const installRoot = await fs.mkdtemp(
        path.join(os.tmpdir(), "pylon-bootstrap-install-"),
      );
      const initialCounters = server.counters();

      const first = await ensureReleaseInstall({
        apiBase: serverUrl,
        repo: "OpenAgentsInc/openagents",
        installRoot,
        platform: "darwin",
        arch: "arm64",
      });
      const second = await ensureReleaseInstall({
        apiBase: serverUrl,
        repo: "OpenAgentsInc/openagents",
        installRoot,
        platform: "darwin",
        arch: "arm64",
      });

      expect(await fs.stat(first.pylonPath)).toBeTruthy();
      expect(await fs.stat(first.pylonTuiPath)).toBeTruthy();
      expect(first.cached).toBe(false);
      expect(second.cached).toBe(true);
      expect(server.counters().archiveHits - initialCounters.archiveHits).toBe(1);
      expect(server.counters().releaseListHits - initialCounters.releaseListHits).toBe(2);
      expect(server.counters().taggedReleaseHits - initialCounters.taggedReleaseHits).toBe(0);
    });

    test("downloads an explicitly requested pylon release tag", async () => {
      const installRoot = await fs.mkdtemp(
        path.join(os.tmpdir(), "pylon-bootstrap-install-versioned-"),
      );
      const initialCounters = server.counters();

      const first = await ensureReleaseInstall({
        apiBase: serverUrl,
        repo: "OpenAgentsInc/openagents",
        version: "1.2.3",
        installRoot,
        platform: "darwin",
        arch: "arm64",
      });
      const second = await ensureReleaseInstall({
        apiBase: serverUrl,
        repo: "OpenAgentsInc/openagents",
        version: "1.2.3",
        installRoot,
        platform: "darwin",
        arch: "arm64",
      });

      expect(await fs.stat(first.pylonPath)).toBeTruthy();
      expect(await fs.stat(first.pylonTuiPath)).toBeTruthy();
      expect(first.cached).toBe(false);
      expect(second.cached).toBe(true);
      expect(server.counters().archiveHits - initialCounters.archiveHits).toBe(1);
      expect(server.counters().taggedReleaseHits - initialCounters.taggedReleaseHits).toBe(2);
    });
  });

  test("bootstrapInstalledPylon runs the smoke path in order", async () => {
    const calls = [];
    const statuses = [];
    const summary = await bootstrapInstalledPylon(
      {
        version: "1.2.3",
        tagName: "pylon-v1.2.3",
        target: { os: "darwin", arch: "arm64" },
        cached: false,
        pylonPath: "/tmp/pylon",
        pylonTuiPath: "/tmp/pylon-tui",
        model: "gemma-4-e4b",
        configPath: "/tmp/pylon-config.json",
        diagnosticRepeats: 2,
        diagnosticMaxOutputTokens: 24,
      },
      {
        runProcessImpl: async (command, args) => {
          calls.push([command, ...args]);
          const joined = args.join(" ");
          if (joined === "--help") {
            return { stdout: "usage", stderr: "" };
          }
          if (joined === "init") {
            return {
              stdout: JSON.stringify({
                config_path: "/tmp/pylon-config.json",
                ledger_path: "/tmp/ledger.json",
              }),
              stderr: "",
            };
          }
          if (joined === "status --json") {
            return {
              stdout: JSON.stringify({
                snapshot: {
                  runtime: {
                    authoritative_status: "offline",
                  },
                },
              }),
              stderr: "",
            };
          }
          if (joined === "inventory --json") {
            return {
              stdout: JSON.stringify({
                rows: [{ id: "row-1" }, { id: "row-2" }],
              }),
              stderr: "",
            };
          }
          if (joined === "gemma download gemma-4-e4b --json") {
            return {
              stdout: JSON.stringify({
                results: [{ model_id: "gemma-4-e4b", status: "downloaded" }],
              }),
              stderr: "",
            };
          }
          if (
            joined ===
            "gemma diagnose gemma-4-e4b --max-output-tokens 24 --repeats 2 --json"
          ) {
            return {
              stdout: JSON.stringify({
                report_path: "/tmp/diagnostics/gemma/latest.json",
                results: [
                  {
                    model_id: "gemma-4-e4b",
                    status: "completed",
                    receipt: {
                      mean_total_s: 1.5,
                      mean_ttft_s: 0.2,
                      mean_decode_tok_s: 12.5,
                    },
                  },
                ],
              }),
              stderr: "",
            };
          }
          throw new Error(`Unexpected command: ${command} ${joined}`);
        },
        onStatus: (event) => statuses.push(event.message),
      },
    );

    expect(calls).toEqual([
      ["/tmp/pylon", "--help"],
      ["/tmp/pylon", "init"],
      ["/tmp/pylon", "status", "--json"],
      ["/tmp/pylon", "inventory", "--json"],
      ["/tmp/pylon", "gemma", "download", "gemma-4-e4b", "--json"],
      [
        "/tmp/pylon",
        "gemma",
        "diagnose",
        "gemma-4-e4b",
        "--max-output-tokens",
        "24",
        "--repeats",
        "2",
        "--json",
      ],
    ]);
    expect(statuses).toEqual([
      "Verifying Pylon binary",
      "Bootstrapping local Pylon identity",
      "Checking runtime health",
      "Scanning for local models",
      "Downloading curated model bundle",
      "Running first-run diagnostic",
      "Bootstrap complete",
    ]);
    expect(summary.configPath).toBe("/tmp/pylon-config.json");
    expect(summary.diagnosticResult?.status).toBe("completed");
  });

  test("launchInstalledPylonTui inherits stdio for the interactive shell", async () => {
    const calls = [];

    await launchInstalledPylonTui(
      {
        pylonTuiPath: "/tmp/pylon-tui",
        pylonHome: "/tmp/pylon-home",
        configPath: "/tmp/pylon-config.json",
      },
      {
        runProcessImpl: async (command, args, options) => {
          calls.push({ command, args, options });
          return { stdout: "", stderr: "" };
        },
      },
    );

    expect(calls).toHaveLength(1);
    expect(calls[0]).toEqual({
      command: "/tmp/pylon-tui",
      args: [],
      options: {
        env: expect.objectContaining({
          OPENAGENTS_PYLON_CONFIG_PATH: "/tmp/pylon-config.json",
          OPENAGENTS_PYLON_HOME: "/tmp/pylon-home",
        }),
        stdio: "inherit",
      },
    });
  });

  test("bootstrapInstalledPylon skips gemma diagnose when the release does not support it", async () => {
    const statuses = [];

    const summary = await bootstrapInstalledPylon(
      {
        version: "0.0.1-rc3",
        tagName: "pylon-v0.0.1-rc3",
        target: { os: "darwin", arch: "arm64" },
        cached: false,
        pylonPath: "/tmp/pylon",
        pylonTuiPath: "/tmp/pylon-tui",
        model: "gemma-4-e4b",
      },
      {
        runProcessImpl: async (_command, args) => {
          const joined = args.join(" ");
          if (joined === "--help") {
            return { stdout: "usage", stderr: "" };
          }
          if (joined === "init") {
            return {
              stdout: JSON.stringify({
                config_path: "/tmp/pylon-config.json",
              }),
              stderr: "",
            };
          }
          if (joined === "status --json") {
            return {
              stdout: JSON.stringify({
                snapshot: {
                  runtime: {
                    authoritative_status: "online",
                  },
                },
              }),
              stderr: "",
            };
          }
          if (joined === "inventory --json") {
            return {
              stdout: JSON.stringify({
                rows: [{ id: "row-1" }],
              }),
              stderr: "",
            };
          }
          if (joined === "gemma download gemma-4-e4b --json") {
            return {
              stdout: JSON.stringify({
                results: [{ model_id: "gemma-4-e4b", status: "installed" }],
              }),
              stderr: "",
            };
          }
          if (
            joined ===
            "gemma diagnose gemma-4-e4b --max-output-tokens 96 --repeats 3 --json"
          ) {
            throw new Error(
              "/tmp/pylon gemma diagnose gemma-4-e4b --max-output-tokens 96 --repeats 3 --json exited with code 1: unknown gemma command: diagnose",
            );
          }
          throw new Error(`Unexpected command: ${joined}`);
        },
        onStatus: (event) => statuses.push(event),
      },
    );

    expect(summary.diagnostic).toBeNull();
    expect(summary.diagnosticResult).toBeNull();
    expect(statuses).toContainEqual({
      message: "Skipping first-run diagnostic",
      detail: "installed Pylon release does not expose gemma diagnose",
    });
    expect(statuses.at(-1)).toEqual({
      message: "Bootstrap complete",
      detail: "smoke path complete",
    });
  });
});
