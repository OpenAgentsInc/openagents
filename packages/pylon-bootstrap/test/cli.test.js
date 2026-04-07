import { afterEach, expect, test } from "bun:test";

import { main, parseArgs } from "../src/cli.js";

const BASE_INSTALL = {
  version: "1.2.3",
  tagName: "pylon-v1.2.3",
  target: { os: "darwin", arch: "arm64" },
  cached: false,
  pylonPath: "/tmp/pylon",
  pylonTuiPath: "/tmp/pylon-tui",
};

const BASE_SUMMARY = {
  version: "1.2.3",
  tagName: "pylon-v1.2.3",
  target: { os: "darwin", arch: "arm64" },
  cached: false,
  binaries: {
    pylon: "/tmp/pylon",
    pylonTui: "/tmp/pylon-tui",
  },
  configPath: "/tmp/pylon-config.json",
  status: {
    snapshot: {
      runtime: {
        authoritative_status: "offline",
      },
    },
  },
  inventory: {
    rows: [],
  },
  model: "gemma-4-e4b",
  init: {},
  download: null,
  diagnostic: null,
  diagnosticResult: null,
};

const originalConsoleLog = console.log;
const originalConsoleError = console.error;

afterEach(() => {
  console.log = originalConsoleLog;
  console.error = originalConsoleError;
});

async function withCapturedConsole(run) {
  const logs = [];
  const errors = [];
  console.log = (...args) => logs.push(args.join(" "));
  console.error = (...args) => errors.push(args.join(" "));
  const result = await run({ logs, errors });
  return { ...result, logs, errors };
}

test("parseArgs defaults to launching the TUI", () => {
  const options = parseArgs([]);

  expect(options.help).toBe(false);
  expect(options.noLaunch).toBe(false);
  expect(options.skipModelDownload).toBe(true);
  expect(options.skipDiagnostics).toBe(false);
  expect(options.verbose).toBe(false);
});

test("parseArgs supports explicit cache download and verbose network flags", () => {
  const options = parseArgs([
    "--download-curated-cache",
    "--debug-network",
  ]);

  expect(options.skipModelDownload).toBe(false);
  expect(options.verbose).toBe(true);
});

test("main launches pylon-tui by default after bootstrap", async () => {
  const calls = [];

  await withCapturedConsole(async () =>
    main([], {
      ensureReleaseInstallImpl: async (options, dependencies) => {
        calls.push({
          step: "install",
          options,
          hasStatusReporter: typeof dependencies.onStatus === "function",
        });
        dependencies.onStatus?.({
          message: "Checking for newer tagged Pylon releases",
          detail: "default release track",
        });
        return BASE_INSTALL;
      },
      bootstrapInstalledPylonImpl: async (options, dependencies) => {
        calls.push({
          step: "bootstrap",
          options,
          hasStatusReporter: typeof dependencies.onStatus === "function",
        });
        dependencies.onStatus?.({
          message: "Scanning for local models",
          detail: null,
        });
        return BASE_SUMMARY;
      },
      launchInstalledPylonTuiImpl: async (options) => {
        calls.push({
          step: "launch",
          options,
        });
        return { stdout: "", stderr: "" };
      },
    }),
  );

  expect(calls.map((entry) => entry.step)).toEqual([
    "install",
    "bootstrap",
    "launch",
  ]);
  expect(calls[0].hasStatusReporter).toBe(true);
  expect(calls[1].hasStatusReporter).toBe(true);
  expect(calls[2].options).toEqual(
    expect.objectContaining({
      version: "1.2.3",
      pylonTuiPath: "/tmp/pylon-tui",
    }),
  );
});

test("main prints a warning verdict when bootstrap completes without a usable runtime", async () => {
  const { logs } = await withCapturedConsole(async () =>
    main(["--no-launch"], {
      ensureReleaseInstallImpl: async () => BASE_INSTALL,
      bootstrapInstalledPylonImpl: async () => ({
        ...BASE_SUMMARY,
        status: {
          snapshot: {
            runtime: {
              authoritative_status: "degraded",
            },
            availability: {
              local_gemma: {
                last_error:
                  "local Gemma runtime not reachable at http://127.0.0.1:11434/api/tags",
                ready_model: null,
              },
            },
          },
        },
      }),
      launchInstalledPylonTuiImpl: async () => {
        throw new Error("launch should be skipped");
      },
    }),
  );

  expect(logs.some((line) => line.includes("Pylon installed but runtime missing"))).toBe(
    true,
  );
});

test("main skips TUI launch when --no-launch is set", async () => {
  const calls = [];

  const { logs } = await withCapturedConsole(async () =>
    main(["--no-launch"], {
      ensureReleaseInstallImpl: async () => BASE_INSTALL,
      bootstrapInstalledPylonImpl: async () => BASE_SUMMARY,
      launchInstalledPylonTuiImpl: async () => {
        calls.push("launch");
        return { stdout: "", stderr: "" };
      },
    }),
  );

  expect(calls).toEqual([]);
  expect(logs.some((line) => line.includes("Skipped Pylon terminal UI launch"))).toBe(
    true,
  );
});
