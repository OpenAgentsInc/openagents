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

function createTelemetryRecorder() {
  const events = [];

  return {
    events,
    client: {
      emit(eventName, properties) {
        events.push({ eventName, properties });
        return Promise.resolve(true);
      },
      flush() {
        return Promise.resolve();
      },
    },
  };
}

async function withCapturedConsole(run) {
  const logs = [];
  const errors = [];
  console.log = (...args) => logs.push(args.join(" "));
  console.error = (...args) => errors.push(args.join(" "));
  const result = await run({ logs, errors });
  return { ...result, logs, errors };
}

test("parseArgs defaults to launching pylon-tui", () => {
  const options = parseArgs([]);

  expect(options.help).toBe(false);
  expect(options.noLaunch).toBe(false);
  expect(options.noUpdates).toBe(false);
  expect(options.skipModelDownload).toBe(true);
  expect(options.skipDiagnostics).toBe(true);
  expect(options.verbose).toBe(false);
  expect(options.pylonArgs).toEqual([]);
});

test("parseArgs supports explicit cache download and verbose network flags", () => {
  const options = parseArgs([
    "--download-curated-cache",
    "--debug-network",
  ]);

  expect(options.skipModelDownload).toBe(false);
  expect(options.verbose).toBe(true);
});

test("parseArgs supports explicit Gemma diagnostics opt-in", () => {
  const options = parseArgs(["--run-diagnostics"]);

  expect(options.skipDiagnostics).toBe(false);
});

test("parseArgs supports disabling release polling", () => {
  const options = parseArgs(["--no-updates"]);

  expect(options.noUpdates).toBe(true);
});

test("parseArgs forwards Pylon CLI commands after launcher options", () => {
  const options = parseArgs([
    "--install-root",
    "/tmp/pylon-cache",
    "status",
    "--json",
  ]);

  expect(options.installRoot).toBe("/tmp/pylon-cache");
  expect(options.json).toBe(false);
  expect(options.pylonArgs).toEqual(["status", "--json"]);
});

test("parseArgs forwards Pylon CLI commands after -- separator", () => {
  const options = parseArgs(["--", "status", "--json"]);

  expect(options.pylonArgs).toEqual(["status", "--json"]);
});

test("main launches pylon-tui by default after bootstrap", async () => {
  const calls = [];
  const telemetry = createTelemetryRecorder();

  await withCapturedConsole(async () =>
    main([], {
      telemetryClient: telemetry.client,
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
      launchInstalledPylonImpl: async (options) => {
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
      pinnedVersion: false,
      pylonTuiPath: "/tmp/pylon-tui",
    }),
  );
  expect(telemetry.events.map((event) => event.eventName)).toEqual([
    "installer_started",
    "installer_finished",
  ]);
});

test("main runs forwarded Pylon CLI commands instead of pylon-tui", async () => {
  const calls = [];
  const telemetry = createTelemetryRecorder();

  await withCapturedConsole(async () =>
    main(["status", "--json"], {
      telemetryClient: telemetry.client,
      ensureReleaseInstallImpl: async () => BASE_INSTALL,
      bootstrapInstalledPylonImpl: async () => BASE_SUMMARY,
      launchInstalledPylonImpl: async () => {
        throw new Error("pylon-tui launch should be skipped");
      },
      runInstalledPylonCliImpl: async (options, args) => {
        calls.push({ options, args });
        return { stdout: "", stderr: "" };
      },
    }),
  );

  expect(calls).toEqual([
    {
      options: expect.objectContaining({
        version: "1.2.3",
        pylonPath: "/tmp/pylon",
      }),
      args: ["status", "--json"],
    },
  ]);
  expect(telemetry.events.map((event) => event.eventName)).toEqual([
    "installer_started",
    "installer_finished",
  ]);
});

test("main prints a warning verdict when bootstrap completes without a usable runtime", async () => {
  const telemetry = createTelemetryRecorder();
  const { logs } = await withCapturedConsole(async () =>
    main(["--no-launch"], {
      telemetryClient: telemetry.client,
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
      launchInstalledPylonImpl: async () => {
        throw new Error("launch should be skipped");
      },
    }),
  );

  expect(logs.some((line) => line.includes("Pylon installed but runtime missing"))).toBe(
    true,
  );
});

test("main skips pylon-tui launch when --no-launch is set", async () => {
  const calls = [];
  const telemetry = createTelemetryRecorder();

  const { logs } = await withCapturedConsole(async () =>
    main(["--no-launch"], {
      telemetryClient: telemetry.client,
      ensureReleaseInstallImpl: async () => BASE_INSTALL,
      bootstrapInstalledPylonImpl: async () => BASE_SUMMARY,
      launchInstalledPylonImpl: async () => {
        calls.push("launch");
        return { stdout: "", stderr: "" };
      },
    }),
  );

  expect(calls).toEqual([]);
  expect(logs.some((line) => line.includes("Skipped Pylon terminal UI launch"))).toBe(true);
});

test("main marks explicit versions as pinned for the launcher", async () => {
  const calls = [];

  await withCapturedConsole(async () =>
    main(["--version", "1.2.3"], {
      telemetryClient: createTelemetryRecorder().client,
      ensureReleaseInstallImpl: async () => BASE_INSTALL,
      bootstrapInstalledPylonImpl: async () => BASE_SUMMARY,
      launchInstalledPylonImpl: async (options) => {
        calls.push(options);
        return { stdout: "", stderr: "" };
      },
    }),
  );

  expect(calls).toHaveLength(1);
  expect(calls[0]).toEqual(
    expect.objectContaining({
      version: "1.2.3",
      pinnedVersion: true,
    }),
  );
});

test("main records a failed installer finish event when bootstrap aborts", async () => {
  const telemetry = createTelemetryRecorder();

  await expect(
    withCapturedConsole(async () =>
      main(["--no-launch"], {
        telemetryClient: telemetry.client,
        ensureReleaseInstallImpl: async () => {
          throw new Error("release resolution exploded");
        },
      }),
    ),
  ).rejects.toThrow("release resolution exploded");

  expect(telemetry.events.map((event) => event.eventName)).toEqual([
    "installer_started",
    "installer_finished",
  ]);
  expect(telemetry.events[1].properties).toEqual(
    expect.objectContaining({
      result: "failed",
      error_stage: "launcher",
    }),
  );
});
