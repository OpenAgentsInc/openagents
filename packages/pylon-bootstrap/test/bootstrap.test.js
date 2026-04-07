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
  runProcess,
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
    function latestReleaseFetch(release) {
      return async (url) => {
        const parsed = new URL(url);
        if (
          parsed.pathname === "/repos/OpenAgentsInc/openagents/releases" &&
          parsed.searchParams.get("per_page") === "100"
        ) {
          return Response.json([release]);
        }
        throw new Error(`Unexpected fetch URL: ${url}`);
      };
    }

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
      expect(server.counters().taggedReleaseHits - initialCounters.taggedReleaseHits).toBe(1);
    });

    test("installs a newer tagged pylon release when the cached install is older", async () => {
      const installRoot = await fs.mkdtemp(
        path.join(os.tmpdir(), "pylon-bootstrap-install-upgrade-"),
      );
      const target = resolvePlatformTarget("darwin", "arm64");
      const createArchiveForVersion = async (version) => {
        const { archiveName } = buildAssetNames(version, target);
        const tempDir = await fs.mkdtemp(
          path.join(os.tmpdir(), `pylon-bootstrap-upgrade-${version}-`),
        );
        const archiveRoot = path.join(tempDir, "stage");
        const archiveBase = `pylon-v${version}-darwin-arm64`;
        const extractedDir = path.join(archiveRoot, archiveBase);
        await fs.mkdir(extractedDir, { recursive: true });
        await fs.writeFile(
          path.join(extractedDir, "pylon"),
          `#!/bin/sh\necho ${version}\n`,
        );
        await fs.writeFile(
          path.join(extractedDir, "pylon-tui"),
          `#!/bin/sh\necho tui ${version}\n`,
        );
        await fs.chmod(path.join(extractedDir, "pylon"), 0o755);
        await fs.chmod(path.join(extractedDir, "pylon-tui"), 0o755);

        const archivePath = path.join(tempDir, archiveName);
        await Bun.spawn(
          ["tar", "-czf", archivePath, "-C", archiveRoot, archiveBase],
          {
            stdout: "ignore",
            stderr: "inherit",
          },
        ).exited;

        const archivePayload = await fs.readFile(archivePath);
        return {
          archiveName,
          archivePayload,
          checksumPayload: `${createHash("sha256").update(archivePayload).digest("hex")}  ${archiveName}\n`,
        };
      };

      const assetsByVersion = {
        "1.2.3": await createArchiveForVersion("1.2.3"),
        "1.2.4": await createArchiveForVersion("1.2.4"),
      };
      let latestVersion = "1.2.3";
      let releaseListHits = 0;
      let archiveHits = 0;

      const upgradeServer = Bun.serve({
        port: 0,
        fetch(request) {
          const url = new URL(request.url);
          if (url.pathname === "/repos/OpenAgentsInc/openagents/releases") {
            releaseListHits += 1;
            const latestAssets = assetsByVersion[latestVersion];
            return Response.json([
              {
                tag_name: `pylon-v${latestVersion}`,
                draft: false,
                assets: [
                  {
                    name: latestAssets.archiveName,
                    browser_download_url: `${upgradeServerUrl}/assets/${latestAssets.archiveName}`,
                  },
                  {
                    name: `${latestAssets.archiveName}.sha256`,
                    browser_download_url: `${upgradeServerUrl}/assets/${latestAssets.archiveName}.sha256`,
                  },
                ],
              },
            ]);
          }

          const assetMatch = url.pathname.match(/^\/assets\/(.+)$/);
          if (assetMatch) {
            const requestedName = assetMatch[1];
            const payload = Object.values(assetsByVersion).find(
              (entry) =>
                entry.archiveName === requestedName ||
                `${entry.archiveName}.sha256` === requestedName,
            );
            if (!payload) {
              return new Response("not found", { status: 404 });
            }
            if (requestedName.endsWith(".sha256")) {
              return new Response(payload.checksumPayload);
            }
            archiveHits += 1;
            return new Response(payload.archivePayload);
          }

          return new Response("not found", { status: 404 });
        },
      });
      const upgradeServerUrl = `http://127.0.0.1:${upgradeServer.port}`;

      try {
        const first = await ensureReleaseInstall({
          apiBase: upgradeServerUrl,
          repo: "OpenAgentsInc/openagents",
          installRoot,
          platform: "darwin",
          arch: "arm64",
        });

        latestVersion = "1.2.4";

        const second = await ensureReleaseInstall({
          apiBase: upgradeServerUrl,
          repo: "OpenAgentsInc/openagents",
          installRoot,
          platform: "darwin",
          arch: "arm64",
        });

        expect(first.version).toBe("1.2.3");
        expect(second.version).toBe("1.2.4");
        expect(second.cached).toBe(false);
        expect(await fs.stat(second.pylonPath)).toBeTruthy();
        expect(await fs.stat(second.pylonTuiPath)).toBeTruthy();
        expect(releaseListHits).toBe(2);
        expect(archiveHits).toBe(2);
      } finally {
        upgradeServer.stop(true);
      }
    });

    test("falls back to curl transport when fetch fails during release resolution", async () => {
      const target = resolvePlatformTarget("darwin", "arm64");
      const { archiveName } = buildAssetNames("9.9.9", target);
      const tempDir = await fs.mkdtemp(
        path.join(os.tmpdir(), "pylon-bootstrap-curl-fallback-"),
      );
      const archiveRoot = path.join(tempDir, "stage");
      const extractedDir = path.join(archiveRoot, "pylon-v9.9.9-darwin-arm64");
      await fs.mkdir(extractedDir, { recursive: true });
      await fs.writeFile(path.join(extractedDir, "pylon"), "#!/bin/sh\nexit 0\n");
      await fs.writeFile(path.join(extractedDir, "pylon-tui"), "#!/bin/sh\nexit 0\n");
      await fs.chmod(path.join(extractedDir, "pylon"), 0o755);
      await fs.chmod(path.join(extractedDir, "pylon-tui"), 0o755);

      const archivePath = path.join(tempDir, archiveName);
      await Bun.spawn(["tar", "-czf", archivePath, "-C", archiveRoot, "pylon-v9.9.9-darwin-arm64"], {
        stdout: "ignore",
        stderr: "inherit",
      }).exited;
      const archivePayload = await fs.readFile(archivePath);
      const checksumPayload = `${createHash("sha256").update(archivePayload).digest("hex")}  ${archiveName}\n`;
      const installRoot = await fs.mkdtemp(
        path.join(os.tmpdir(), "pylon-bootstrap-install-curl-"),
      );

      const install = await ensureReleaseInstall(
        {
          apiBase: "https://api.github.invalid",
          repo: "OpenAgentsInc/openagents",
          installRoot,
          platform: "darwin",
          arch: "arm64",
        },
        {
          fetchImpl: async () => {
            throw new TypeError("fetch failed", {
              cause: Object.assign(new Error("connect ENETUNREACH api.github.invalid:443"), {
                code: "ENETUNREACH",
                hostname: "api.github.invalid",
                port: 443,
              }),
            });
          },
          runProcessImpl: async (command, args, options) => {
            if (command !== "curl") {
              return runProcess(command, args, options);
            }
            const url = args.at(-1);
            if (url.endsWith("/releases?per_page=100")) {
              return {
                stdout: JSON.stringify([
                  {
                    tag_name: "pylon-v9.9.9",
                    draft: false,
                    assets: [
                      {
                        name: archiveName,
                        browser_download_url: "https://downloads.example.test/archive",
                      },
                      {
                        name: `${archiveName}.sha256`,
                        browser_download_url: "https://downloads.example.test/archive.sha256",
                      },
                    ],
                  },
                ]),
                stderr: "",
              };
            }
            if (url === "https://downloads.example.test/archive.sha256") {
              return { stdout: checksumPayload, stderr: "" };
            }
            if (url === "https://downloads.example.test/archive") {
              const outputIndex = args.indexOf("--output");
              await fs.writeFile(args[outputIndex + 1], archivePayload);
              return { stdout: "", stderr: "" };
            }
            throw new Error(`Unexpected curl invocation: ${url}`);
          },
        },
      );

      expect(install.version).toBe("9.9.9");
      expect(install.cached).toBe(false);
      expect(await fs.stat(install.pylonPath)).toBeTruthy();
      expect(await fs.stat(install.pylonTuiPath)).toBeTruthy();
    });

    test("reports actionable release lookup diagnostics when fetch and curl both fail", async () => {
      const installRoot = await fs.mkdtemp(
        path.join(os.tmpdir(), "pylon-bootstrap-install-error-"),
      );

      await expect(
        ensureReleaseInstall(
          {
            apiBase: "https://api.github.invalid",
            repo: "OpenAgentsInc/openagents",
            installRoot,
            platform: "darwin",
            arch: "arm64",
          },
          {
            fetchImpl: async () => {
              throw new TypeError("fetch failed", {
                cause: Object.assign(new Error("getaddrinfo EAI_AGAIN api.github.invalid"), {
                  code: "EAI_AGAIN",
                  hostname: "api.github.invalid",
                }),
              });
            },
            runProcessImpl: async () => {
              throw new Error("Failed to start curl: spawn curl ENOENT");
            },
          },
        ),
      ).rejects.toThrow(/classification=dns/);

      await expect(
        ensureReleaseInstall(
          {
            apiBase: "https://api.github.invalid",
            repo: "OpenAgentsInc/openagents",
            installRoot,
            platform: "darwin",
            arch: "arm64",
          },
          {
            fetchImpl: async () => {
              throw new TypeError("fetch failed", {
                cause: Object.assign(new Error("getaddrinfo EAI_AGAIN api.github.invalid"), {
                  code: "EAI_AGAIN",
                  hostname: "api.github.invalid",
                }),
              });
            },
            runProcessImpl: async () => {
              throw new Error("Failed to start curl: spawn curl ENOENT");
            },
          },
        ),
      ).rejects.toThrow(/Retry with verbose diagnostics/);
    });

    test("falls back to a deterministic source build when the release has no asset for the local target", async () => {
      const installRoot = await fs.mkdtemp(
        path.join(os.tmpdir(), "pylon-bootstrap-source-install-"),
      );
      const statuses = [];
      const commands = [];
      const expectedCommit = "1234567890abcdef1234567890abcdef12345678";

      const install = await ensureReleaseInstall(
        {
          apiBase: "https://api.github.com",
          repo: "OpenAgentsInc/openagents",
          installRoot,
          platform: "linux",
          arch: "x64",
          sourceRepoUrl: "/tmp/openagents-source.git",
        },
        {
          fetchImpl: latestReleaseFetch({
            tag_name: "pylon-v1.2.3",
            target_commitish: expectedCommit,
            draft: false,
            assets: [
              {
                name: "pylon-v1.2.3-darwin-arm64.tar.gz",
                browser_download_url: "https://example.com/archive",
              },
              {
                name: "pylon-v1.2.3-darwin-arm64.tar.gz.sha256",
                browser_download_url: "https://example.com/checksum",
              },
            ],
          }),
          commandExistsImpl: async (command) =>
            ["git", "cargo", "rustc"].includes(command),
          runProcessImpl: async (command, args, options = {}) => {
            commands.push({
              command,
              args,
              cwd: options.cwd ?? null,
            });
            const joined = args.join(" ");
            if (command === "git" && joined === "init") {
              return { stdout: "", stderr: "" };
            }
            if (
              command === "git" &&
              joined === "remote add origin /tmp/openagents-source.git"
            ) {
              return { stdout: "", stderr: "" };
            }
            if (
              command === "git" &&
              joined ===
                "fetch --depth 1 origin refs/tags/pylon-v1.2.3:refs/tags/pylon-v1.2.3"
            ) {
              return { stdout: "", stderr: "" };
            }
            if (
              command === "git" &&
              joined === "checkout --detach refs/tags/pylon-v1.2.3"
            ) {
              return { stdout: "", stderr: "" };
            }
            if (command === "git" && joined === "rev-parse HEAD") {
              return { stdout: `${expectedCommit}\n`, stderr: "" };
            }
            if (
              command === "cargo" &&
              joined === "build --release -p pylon -p pylon-tui"
            ) {
              const releaseDir = path.join(options.cwd, "target", "release");
              await fs.mkdir(releaseDir, { recursive: true });
              await fs.writeFile(
                path.join(releaseDir, "pylon"),
                "#!/bin/sh\necho source build\n",
              );
              await fs.writeFile(
                path.join(releaseDir, "pylon-tui"),
                "#!/bin/sh\necho source build tui\n",
              );
              await fs.chmod(path.join(releaseDir, "pylon"), 0o755);
              await fs.chmod(path.join(releaseDir, "pylon-tui"), 0o755);
              return { stdout: "", stderr: "" };
            }
            throw new Error(`Unexpected command: ${command} ${joined}`);
          },
          onStatus: (event) => statuses.push(event),
        },
      );

      expect(install.installMethod).toBe("source_build");
      expect(install.cached).toBe(false);
      expect(install.sourceCommit).toBe(expectedCommit);
      expect(await fs.stat(install.pylonPath)).toBeTruthy();
      expect(await fs.stat(install.pylonTuiPath)).toBeTruthy();
      expect(
        JSON.parse(await fs.readFile(install.manifestPath, "utf8")),
      ).toEqual(
        expect.objectContaining({
          installMethod: "source_build",
          sourceCloneUrl: "/tmp/openagents-source.git",
          sourceCommit: expectedCommit,
          buildCommand: "cargo build --release -p pylon -p pylon-tui",
        }),
      );
      expect(statuses).toContainEqual({
        message: "Prebuilt asset missing; falling back to source build",
        detail: "pylon-v1.2.3 for linux-x86_64",
      });
      expect(
        commands.some(
          (entry) =>
            entry.command === "cargo" &&
            entry.args.join(" ") === "build --release -p pylon -p pylon-tui",
        ),
      ).toBe(true);

      const cached = await ensureReleaseInstall(
        {
          apiBase: "https://api.github.com",
          repo: "OpenAgentsInc/openagents",
          installRoot,
          platform: "linux",
          arch: "x64",
          sourceRepoUrl: "/tmp/openagents-source.git",
        },
        {
          fetchImpl: latestReleaseFetch({
            tag_name: "pylon-v1.2.3",
            target_commitish: expectedCommit,
            draft: false,
            assets: [],
          }),
          commandExistsImpl: async () => true,
          runProcessImpl: async (command, args) => {
            throw new Error(`Cached install should not rebuild: ${command} ${args.join(" ")}`);
          },
        },
      );

      expect(cached.cached).toBe(true);
      expect(cached.installMethod).toBe("source_build");
      expect(cached.sourceCommit).toBe(expectedCommit);
    });

    test("prompts before installing Rust when a source build needs a toolchain", async () => {
      const installRoot = await fs.mkdtemp(
        path.join(os.tmpdir(), "pylon-bootstrap-rustup-install-"),
      );
      const expectedCommit = "abcdefabcdefabcdefabcdefabcdefabcdefabcd";
      const prompts = [];
      let rustInstalled = false;

      const install = await ensureReleaseInstall(
        {
          apiBase: "https://api.github.com",
          repo: "OpenAgentsInc/openagents",
          installRoot,
          platform: "linux",
          arch: "x64",
          sourceRepoUrl: "/tmp/openagents-source.git",
        },
        {
          fetchImpl: async (url) => {
            if (url === "https://sh.rustup.rs") {
              return new Response("#!/bin/sh\nexit 0\n");
            }
            return latestReleaseFetch({
              tag_name: "pylon-v1.2.3",
              target_commitish: expectedCommit,
              draft: false,
              assets: [],
            })(url);
          },
          promptImpl: async (message) => {
            prompts.push(message);
            return true;
          },
          commandExistsImpl: async (command) => {
            if (command === "git") {
              return true;
            }
            if (command === "cargo" || command === "rustc") {
              return rustInstalled;
            }
            return false;
          },
          runProcessImpl: async (command, args, options = {}) => {
            const joined = args.join(" ");
            if (command === "sh") {
              rustInstalled = true;
              return { stdout: "", stderr: "" };
            }
            if (command === "git" && joined === "init") {
              return { stdout: "", stderr: "" };
            }
            if (
              command === "git" &&
              joined === "remote add origin /tmp/openagents-source.git"
            ) {
              return { stdout: "", stderr: "" };
            }
            if (
              command === "git" &&
              joined ===
                "fetch --depth 1 origin refs/tags/pylon-v1.2.3:refs/tags/pylon-v1.2.3"
            ) {
              return { stdout: "", stderr: "" };
            }
            if (
              command === "git" &&
              joined === "checkout --detach refs/tags/pylon-v1.2.3"
            ) {
              return { stdout: "", stderr: "" };
            }
            if (command === "git" && joined === "rev-parse HEAD") {
              return { stdout: `${expectedCommit}\n`, stderr: "" };
            }
            if (
              command === "cargo" &&
              joined === "build --release -p pylon -p pylon-tui"
            ) {
              const releaseDir = path.join(options.cwd, "target", "release");
              await fs.mkdir(releaseDir, { recursive: true });
              await fs.writeFile(path.join(releaseDir, "pylon"), "#!/bin/sh\n");
              await fs.writeFile(path.join(releaseDir, "pylon-tui"), "#!/bin/sh\n");
              await fs.chmod(path.join(releaseDir, "pylon"), 0o755);
              await fs.chmod(path.join(releaseDir, "pylon-tui"), 0o755);
              return { stdout: "", stderr: "" };
            }
            throw new Error(`Unexpected command: ${command} ${joined}`);
          },
        },
      );

      expect(rustInstalled).toBe(true);
      expect(prompts).toHaveLength(1);
      expect(prompts[0]).toContain("Install the official Rust toolchain now via rustup?");
      expect(install.installMethod).toBe("source_build");
    });

    test("prints manual Rust install guidance when the user declines source-build toolchain installation", async () => {
      const installRoot = await fs.mkdtemp(
        path.join(os.tmpdir(), "pylon-bootstrap-rustup-decline-"),
      );

      await expect(
        ensureReleaseInstall(
          {
            apiBase: "https://api.github.com",
            repo: "OpenAgentsInc/openagents",
            installRoot,
            platform: "linux",
            arch: "x64",
            sourceRepoUrl: "/tmp/openagents-source.git",
          },
          {
            fetchImpl: latestReleaseFetch({
              tag_name: "pylon-v1.2.3",
              target_commitish: "abcdefabcdefabcdefabcdefabcdefabcdefabcd",
              draft: false,
              assets: [],
            }),
            promptImpl: async () => false,
            commandExistsImpl: async (command) => command === "git",
            runProcessImpl: async (command, args) => {
              throw new Error(`Unexpected command: ${command} ${args.join(" ")}`);
            },
          },
        ),
      ).rejects.toThrow("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y");
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
        skipModelDownload: false,
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
        skipModelDownload: true,
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
