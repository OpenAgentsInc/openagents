// Khala Code Desktop backend for qa-runner.
//
// This backend boots the desktop app in preview-server mode, then drives the
// existing Chromium surface against that preview URL. It also exposes the typed
// Khala Code RPC client from `@openagentsinc/khala-qa-harness` with the preview
// access header already configured. Fixture mode is the default and uses the
// fixture Codex app-server so qa-runner smokes do not require live login/spend.

import {
  accessSync,
  constants,
  existsSync,
  mkdirSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:net";
import { basename, dirname, join, resolve } from "node:path";
import { randomBytes } from "node:crypto";
import { Effect } from "effect";
import {
  makeKhalaCodeRpcQaDriver,
  runKhalaCodeQaScenario,
  type KhalaCodeQaScenario,
  type KhalaCodeQaScenarioRunReport,
  type KhalaCodeRpcClient,
  type KhalaCodeRpcFetch,
} from "@openagentsinc/khala-qa-harness";

import {
  localBackend,
  type Backend,
  type BackendSession,
  type LocalBackendOptions,
} from "./backend";
import {
  runNativeDesktopScenario,
  type NativeDesktopBackendOptions,
  type NativeDesktopOutcome,
  type NativeDesktopScenario,
} from "./native-desktop-backend";
import type { Target } from "./target";

const DEFAULT_DESKTOP_CWD = resolve(import.meta.dir, "../../../clients/khala-code-desktop");
const DESKTOP_STDOUT_FILE = "khala-desktop-stdout.jsonl";
const DESKTOP_STDERR_FILE = "khala-desktop-stderr.jsonl";
const HARNESS_REPORT_FILE = "khala-desktop-harness-report.json";
const PACKAGED_NATIVE_SMOKE_REPORT_FILE = "khala-packaged-native-smoke.json";
const KHALA_CODE_APP_NAME = "Khala Code";
const DEFAULT_PACKAGED_NATIVE_PROMPT = "Run the public fixture smoke.";
const ELECTROBUN_LAUNCHER_EXECUTABLE = "launcher";
const CHILD_TERM_GRACE_MS = 1_000;

export type KhalaDesktopBackendTier = "fixture" | "live_codex";

export type KhalaDesktopChildProcess = {
  readonly pid?: number;
  readonly stdout?: ReadableStream<Uint8Array> | null;
  readonly stderr?: ReadableStream<Uint8Array> | null;
  readonly exited: Promise<number>;
  readonly kill: (signal?: number | NodeJS.Signals) => void;
};

export type KhalaDesktopSpawn = (input: {
  readonly cmd: readonly string[];
  readonly cwd: string;
  readonly env: Readonly<Record<string, string | undefined>>;
}) => KhalaDesktopChildProcess;

export interface KhalaDesktopBackendOptions extends LocalBackendOptions {
  readonly backendTier?: KhalaDesktopBackendTier;
  readonly desktopCwd?: string;
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly fetch?: typeof fetch;
  readonly previewAccessToken?: string;
  readonly previewPort?: number;
  readonly spawn?: KhalaDesktopSpawn;
  readonly waitTimeoutMs?: number;
}

export interface KhalaPackagedNativeSmokeSelectors {
  readonly composer?: string;
  readonly hotbar?: string;
  readonly send?: string;
}

export interface KhalaPackagedNativeSmokeOptions {
  readonly appProcessName?: string;
  readonly appPath?: string;
  readonly artifactDir: string;
  readonly desktopCwd?: string;
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly launchWaitMs?: number;
  readonly native?: NativeDesktopBackendOptions;
  readonly promptText?: string;
  readonly selectors?: KhalaPackagedNativeSmokeSelectors;
  readonly spawn?: KhalaDesktopSpawn;
  readonly target: Target;
}

export interface KhalaPackagedNativeSmokeOutcome extends NativeDesktopOutcome {
  readonly appPath: string;
  readonly executablePath: string;
  readonly smokeReportPath: string;
}

export interface KhalaDesktopBackendSession extends BackendSession {
  readonly previewAccessHeader: "x-khala-code-preview-token";
  readonly previewBaseUrl: string;
  readonly rpcClient: KhalaCodeRpcClient;
}

export class KhalaDesktopBackendBootError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "KhalaDesktopBackendBootError";
  }
}

const sleep = (ms: number): Promise<void> => new Promise((resolveSleep) => setTimeout(resolveSleep, ms));

const previewBaseUrl = (port: number): string => `http://127.0.0.1:${port}`;

const makePreviewAccessToken = (): string => `qa_${randomBytes(18).toString("base64url")}`;

const findAvailablePort = (): Promise<number> =>
  new Promise((resolvePort, rejectPort) => {
    const server = createServer();
    server.once("error", rejectPort);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => {
        if (typeof address === "object" && address !== null) resolvePort(address.port);
        else rejectPort(new Error("failed to allocate preview port"));
      });
    });
  });

const defaultSpawn: KhalaDesktopSpawn = ({ cmd, cwd, env }) =>
  Bun.spawn([...cmd], {
    cwd,
    env: Object.fromEntries(
      Object.entries(env).filter((entry): entry is [string, string] => entry[1] !== undefined),
    ),
    stderr: "pipe",
    stdout: "pipe",
  });

const appendStreamToFile = (
  stream: ReadableStream<Uint8Array> | null | undefined,
  path: string,
): Promise<void> => {
  if (stream === null || stream === undefined) return Promise.resolve();
  const decoder = new TextDecoder();
  const reader = stream.getReader();
  return (async () => {
    let buffered = "";
    try {
      for (;;) {
        const chunk = await reader.read();
        if (chunk.done) break;
        buffered += decoder.decode(chunk.value, { stream: true });
      }
      buffered += decoder.decode();
      if (buffered.length > 0) writeFileSync(path, buffered);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      writeFileSync(path, `${JSON.stringify({ level: "error", message })}\n`);
    }
  })();
};

const waitForPreviewHealth = async (input: {
  readonly baseUrl: string;
  readonly childExited: Promise<number>;
  readonly fetch: typeof fetch;
  readonly timeoutMs: number;
}): Promise<void> => {
  const deadline = Date.now() + input.timeoutMs;
  let lastError = "preview health check did not respond";
  const childExited = input.childExited.then((code): never => {
    throw new KhalaDesktopBackendBootError(`Khala desktop preview exited before health check passed (code ${code})`);
  });
  while (Date.now() < deadline) {
    try {
      const response = await Promise.race([
        input.fetch(new URL("/health", input.baseUrl), {
          signal: AbortSignal.timeout(500),
        }),
        childExited,
      ]);
      if (response.ok) return;
      lastError = `preview health returned ${response.status}`;
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error);
    }
    await Promise.race([sleep(100), childExited]);
  }
  throw new KhalaDesktopBackendBootError(`Khala desktop preview failed to boot: ${lastError}`);
};

const shutdownChild = async (
  child: KhalaDesktopChildProcess,
  logFlushes: ReadonlyArray<Promise<void>>,
): Promise<void> => {
  child.kill("SIGTERM");
  const exitedAfterTerm = await Promise.race([
    child.exited.then(() => true, () => true),
    sleep(CHILD_TERM_GRACE_MS).then(() => false),
  ]);
  if (!exitedAfterTerm) child.kill("SIGKILL");
  await child.exited.catch(() => undefined);
  await Promise.allSettled(logFlushes);
};

const discoverChildProcessPid = async (parentPid: number | undefined): Promise<number | undefined> => {
  if (parentPid === undefined || process.platform !== "darwin") return undefined;
  try {
    const child = Bun.spawn(["pgrep", "-P", String(parentPid)], {
      stderr: "ignore",
      stdout: "pipe",
    });
    const output = await new Response(child.stdout).text();
    await child.exited.catch(() => undefined);
    const pid = output
      .split(/\s+/)
      .map((value) => Number.parseInt(value, 10))
      .find((value) => Number.isInteger(value) && value > 0);
    return pid;
  } catch {
    return undefined;
  }
};

const executableFile = (path: string): boolean => {
  try {
    const stat = statSync(path);
    if (!stat.isFile()) return false;
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
};

const walkForKhalaCodeApp = (
  root: string,
  depth: number,
): string | undefined => {
  if (depth < 0 || !existsSync(root)) return undefined;
  const entries = readdirSync(root, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name));
  for (const entry of entries) {
    const path = join(root, entry.name);
    if (
      entry.isDirectory() &&
      entry.name.startsWith(KHALA_CODE_APP_NAME) &&
      entry.name.endsWith(".app")
    ) {
      return path;
    }
  }
  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    const path = join(root, entry.name);
    const found = walkForKhalaCodeApp(path, depth - 1);
    if (found !== undefined) return found;
  }
  return undefined;
};

export const resolveKhalaCodePackagedAppPath = (input: {
  readonly appPath?: string;
  readonly desktopCwd?: string;
  readonly env?: Readonly<Record<string, string | undefined>>;
} = {}): string => {
  const env = input.env ?? process.env;
  const explicit = input.appPath ?? env.QA_KHALA_CODE_APP_PATH;
  if (explicit !== undefined && explicit.trim().length > 0) {
    const resolved = resolve(explicit);
    if (!existsSync(resolved)) throw new KhalaDesktopBackendBootError(`Packaged Khala Code app not found: ${resolved}`);
    return resolved;
  }

  const desktopCwd = input.desktopCwd ?? DEFAULT_DESKTOP_CWD;
  for (const candidateRoot of [
    join(desktopCwd, "build"),
    join(desktopCwd, ".electrobun", "build"),
    join(desktopCwd, "out"),
  ]) {
    const found = walkForKhalaCodeApp(candidateRoot, 6);
    if (found !== undefined) return found;
  }
  throw new KhalaDesktopBackendBootError(
    `Packaged Khala Code app was not found under ${desktopCwd}. Run ` +
      "`bun run --cwd clients/khala-code-desktop build` first, or set QA_KHALA_CODE_APP_PATH.",
  );
};

export const resolveKhalaCodePackagedExecutablePath = (appPath: string): string => {
  const macOsDir = join(appPath, "Contents", "MacOS");
  for (const preferredName of [
    ELECTROBUN_LAUNCHER_EXECUTABLE,
    basename(appPath, ".app"),
    KHALA_CODE_APP_NAME,
  ]) {
    const preferred = join(macOsDir, preferredName);
    if (executableFile(preferred)) return preferred;
  }
  const fallback = existsSync(macOsDir)
    ? readdirSync(macOsDir)
        .map((name) => join(macOsDir, name))
        .find((path) => !basename(path).endsWith(".dylib") && executableFile(path))
    : undefined;
  if (fallback !== undefined) return fallback;
  throw new KhalaDesktopBackendBootError(`No executable was found in ${macOsDir}`);
};

export const resolveKhalaCodePackagedAppProcessName = (appPath: string): string => {
  const appBundleName = basename(appPath);
  return appBundleName.endsWith(".app")
    ? appBundleName.slice(0, -".app".length)
    : appBundleName || KHALA_CODE_APP_NAME;
};

const fixtureDefaultEnv = (
  env: Readonly<Record<string, string | undefined>>,
  artifactDir: string,
  allowOverrides: boolean,
): Record<string, string> => ({
  CODEX_HOME: allowOverrides && env.CODEX_HOME !== undefined ? env.CODEX_HOME : join(artifactDir, "fixture-codex-home"),
  KHALA_CODE_DESKTOP_PREVIEW_READONLY:
    allowOverrides && env.KHALA_CODE_DESKTOP_PREVIEW_READONLY !== undefined
      ? env.KHALA_CODE_DESKTOP_PREVIEW_READONLY
      : "1",
  KHALA_CODE_DESKTOP_WORKSPACE:
    allowOverrides && env.KHALA_CODE_DESKTOP_WORKSPACE !== undefined
      ? env.KHALA_CODE_DESKTOP_WORKSPACE
      : join(artifactDir, "fixture-workspace"),
  KHALA_CODE_CODEX_APP_SERVER_FIXTURE:
    allowOverrides && env.KHALA_CODE_CODEX_APP_SERVER_FIXTURE !== undefined ? env.KHALA_CODE_CODEX_APP_SERVER_FIXTURE : "1",
  KHALA_CODE_TOKEN_USAGE_BACKGROUND_SYNC_DISABLED:
    allowOverrides && env.KHALA_CODE_TOKEN_USAGE_BACKGROUND_SYNC_DISABLED !== undefined
      ? env.KHALA_CODE_TOKEN_USAGE_BACKGROUND_SYNC_DISABLED
      : "1",
  KHALA_CODE_TOKEN_USAGE_DISABLED:
    allowOverrides && env.KHALA_CODE_TOKEN_USAGE_DISABLED !== undefined ? env.KHALA_CODE_TOKEN_USAGE_DISABLED : "1",
});

const selectorFromEnv = (
  env: Readonly<Record<string, string | undefined>>,
  key: string,
): string | undefined => {
  const value = env[key]?.trim();
  return value === undefined || value.length === 0 ? undefined : value;
};

export const khalaCodePackagedFixtureNativeScenario = (input: {
  readonly appPid?: number;
  readonly appProcessName?: string;
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly promptText?: string;
  readonly selectors?: KhalaPackagedNativeSmokeSelectors;
} = {}): NativeDesktopScenario => {
  const env = input.env ?? process.env;
  const appProcessName = input.appProcessName ?? KHALA_CODE_APP_NAME;
  const promptText = input.promptText ?? DEFAULT_PACKAGED_NATIVE_PROMPT;
  const hotbarSelector =
    input.selectors?.hotbar ??
    selectorFromEnv(env, "QA_KHALA_CODE_PACKAGED_HOTBAR_SELECTOR") ??
    "AXButton:Fleet";
  const composerSelector =
    input.selectors?.composer ??
    selectorFromEnv(env, "QA_KHALA_CODE_PACKAGED_COMPOSER_SELECTOR") ??
    "AXTextArea:Message Khala Code";
  const sendSelector =
    input.selectors?.send ??
    selectorFromEnv(env, "QA_KHALA_CODE_PACKAGED_SEND_SELECTOR") ??
    "AXButton:Send message";

  return {
    app: appProcessName,
    ...(input.appPid === undefined ? {} : { appPid: input.appPid }),
    name: "khala-code-packaged-fixture-native-smoke",
    steps: [
      { kind: "focus", label: "focus packaged Khala Code" },
      { durationMs: 1_500, kind: "wait", label: "wait for packaged window boot" },
      { kind: "ax-snapshot", label: "read boot AX tree" },
      { kind: "assert-ax-contains", value: "AXWindow", label: "packaged app exposes a window" },
      { kind: "screenshot", label: "packaged boot screenshot" },
      { kind: "click", label: "open Fleet from hotbar", selector: hotbarSelector },
      { durationMs: 350, kind: "wait", label: "wait for Fleet panel paint" },
      { kind: "ax-snapshot", label: "read Fleet AX tree" },
      { kind: "screenshot", label: "Fleet hotbar screenshot" },
      { kind: "click", label: "focus composer", selector: composerSelector },
      { kind: "type", label: "type fixture prompt into composer", text: promptText },
      { durationMs: 150, kind: "wait", label: "wait for composer echo" },
      { kind: "click", label: "submit fixture prompt", selector: sendSelector },
      { durationMs: 1_500, kind: "wait", label: "wait for fixture turn render" },
      { kind: "ax-snapshot", label: "read submitted turn AX tree" },
      { kind: "screenshot", label: "submitted fixture turn screenshot" },
    ],
  };
};

export function khalaDesktopBackend(options: KhalaDesktopBackendOptions = {}): Backend {
  const env = options.env ?? process.env;
  const accessToken = options.previewAccessToken ?? makePreviewAccessToken();
  const fetchImpl = options.fetch ?? globalThis.fetch.bind(globalThis);
  const spawn = options.spawn ?? defaultSpawn;
  const desktopCwd = options.desktopCwd ?? DEFAULT_DESKTOP_CWD;
  const backendTier = options.backendTier ?? "fixture";
  const waitTimeoutMs = options.waitTimeoutMs ?? 10_000;

  return {
    name: "khala-desktop",
    provision: async ({ target, artifactDir, headed }): Promise<KhalaDesktopBackendSession> => {
      mkdirSync(artifactDir, { recursive: true });
      const port = options.previewPort ?? (await findAvailablePort());
      const baseUrl = previewBaseUrl(port);
      const fixtureEnv = backendTier === "fixture" ? fixtureDefaultEnv(env, artifactDir, options.env !== undefined) : {};
      const childEnv: Record<string, string | undefined> = {
        ...env,
        ...fixtureEnv,
        KHALA_CODE_DESKTOP_OPEN_WINDOW: headed ? "1" : "0",
        KHALA_CODE_DESKTOP_PREVIEW_PORT: String(port),
        KHALA_CODE_DESKTOP_PREVIEW_RPC_TOKEN: accessToken,
        KHALA_CODE_DESKTOP_PREVIEW_READONLY:
          backendTier === "fixture" ? fixtureEnv.KHALA_CODE_DESKTOP_PREVIEW_READONLY : env.KHALA_CODE_DESKTOP_PREVIEW_READONLY,
        KHALA_CODE_CODEX_APP_SERVER_FIXTURE:
          backendTier === "fixture" ? fixtureEnv.KHALA_CODE_CODEX_APP_SERVER_FIXTURE : env.KHALA_CODE_CODEX_APP_SERVER_FIXTURE,
      };
      const child = spawn({
        cmd: [process.execPath, "src/bun/index.ts"],
        cwd: desktopCwd,
        env: childEnv,
      });
      const logFlushes = [
        appendStreamToFile(child.stdout, join(artifactDir, DESKTOP_STDOUT_FILE)),
        appendStreamToFile(child.stderr, join(artifactDir, DESKTOP_STDERR_FILE)),
      ];

      try {
        await waitForPreviewHealth({ baseUrl, childExited: child.exited, fetch: fetchImpl, timeoutMs: waitTimeoutMs });
      } catch (error) {
        await shutdownChild(child, logFlushes);
        throw error;
      }

      const previewTarget: Target = { ...target, baseUrl };
      const browserSession = await localBackend({
        ...(options.chromium !== undefined ? { chromium: options.chromium } : {}),
      }).provision({ target: previewTarget, artifactDir, ...(headed !== undefined ? { headed } : {}) });
      const driver = makeKhalaCodeRpcQaDriver({
        accessToken,
        baseUrl,
        fetch: fetchImpl as KhalaCodeRpcFetch,
      });

      return {
        acquireBrowser: () => browserSession.acquireBrowser(),
        previewAccessHeader: "x-khala-code-preview-token",
        previewBaseUrl: baseUrl,
        rpcClient: driver.client,
        teardown: async () => {
          await browserSession.teardown();
          await shutdownChild(child, logFlushes);
        },
      };
    },
  };
}

export const isKhalaDesktopBackendSession = (
  session: BackendSession,
): session is KhalaDesktopBackendSession =>
  typeof (session as Partial<KhalaDesktopBackendSession>).previewBaseUrl === "string" &&
  (session as Partial<KhalaDesktopBackendSession>).rpcClient !== undefined;

export async function runKhalaDesktopHarnessScenario(input: {
  readonly artifactDir: string;
  readonly backend?: Backend;
  readonly scenario: KhalaCodeQaScenario;
  readonly target: Target;
}): Promise<KhalaCodeQaScenarioRunReport> {
  const backend = input.backend ?? khalaDesktopBackend();
  const session = await backend.provision({
    artifactDir: input.artifactDir,
    target: input.target,
  });
  if (!isKhalaDesktopBackendSession(session)) {
    await session.teardown();
    throw new KhalaDesktopBackendBootError("Backend did not return a Khala desktop session.");
  }
  const driver = makeKhalaCodeRpcQaDriver({
    baseUrl: session.previewBaseUrl,
    fetch: session.rpcClient.fetch,
    ...(session.rpcClient.accessToken === undefined
      ? {}
      : { accessToken: session.rpcClient.accessToken }),
  });
  try {
    const report = await Effect.runPromise(runKhalaCodeQaScenario({ driver, scenario: input.scenario }));
    writeFileSync(join(input.artifactDir, HARNESS_REPORT_FILE), `${JSON.stringify(report, null, 2)}\n`);
    return report;
  } finally {
    await session.teardown();
  }
}

export async function runKhalaDesktopHeadedNativeSmoke(
  input: KhalaPackagedNativeSmokeOptions,
): Promise<KhalaPackagedNativeSmokeOutcome> {
  const env = input.env ?? process.env;
  const desktopCwd = input.desktopCwd ?? DEFAULT_DESKTOP_CWD;
  const appPathSource = input.appPath !== undefined || env.QA_KHALA_CODE_APP_PATH !== undefined ? "explicit" : "auto";
  const appPath = resolveKhalaCodePackagedAppPath({
    ...(input.appPath === undefined ? {} : { appPath: input.appPath }),
    desktopCwd,
    env,
  });
  const executablePath = resolveKhalaCodePackagedExecutablePath(appPath);
  const appProcessName =
    input.appProcessName ??
    selectorFromEnv(env, "QA_KHALA_CODE_APP_PROCESS_NAME") ??
    resolveKhalaCodePackagedAppProcessName(appPath);
  const fixtureEnv = fixtureDefaultEnv(env, input.artifactDir, input.env !== undefined);
  const childEnv: Record<string, string | undefined> = {
    ...env,
    ...fixtureEnv,
    KHALA_CODE_CODEX_APP_SERVER_FIXTURE: fixtureEnv.KHALA_CODE_CODEX_APP_SERVER_FIXTURE,
    KHALA_CODE_DESKTOP_OPEN_WINDOW: "1",
    KHALA_CODE_TOKEN_USAGE_BACKGROUND_SYNC_DISABLED: fixtureEnv.KHALA_CODE_TOKEN_USAGE_BACKGROUND_SYNC_DISABLED,
    KHALA_CODE_TOKEN_USAGE_DISABLED: fixtureEnv.KHALA_CODE_TOKEN_USAGE_DISABLED,
  };
  const spawn = input.spawn ?? defaultSpawn;
  mkdirSync(input.artifactDir, { recursive: true });
  const child = spawn({
    cmd: [executablePath],
    cwd: dirname(executablePath),
    env: childEnv,
  });
  const logFlushes = [
    appendStreamToFile(child.stdout, join(input.artifactDir, DESKTOP_STDOUT_FILE)),
    appendStreamToFile(child.stderr, join(input.artifactDir, DESKTOP_STDERR_FILE)),
  ];

  try {
    await sleep(input.launchWaitMs ?? 2_500);
    const appPid = await discoverChildProcessPid(child.pid);
    const scenario = khalaCodePackagedFixtureNativeScenario({
      ...(appPid === undefined ? {} : { appPid }),
      appProcessName,
      env,
      ...(input.promptText === undefined ? {} : { promptText: input.promptText }),
      ...(input.selectors === undefined ? {} : { selectors: input.selectors }),
    });
    const outcome = await runNativeDesktopScenario(
      {
        artifactDir: input.artifactDir,
        scenario,
        target: input.target,
      },
      input.native,
    );
    const smokeReportPath = join(input.artifactDir, PACKAGED_NATIVE_SMOKE_REPORT_FILE);
    writeFileSync(
      smokeReportPath,
      `${JSON.stringify({
        schemaVersion: "openagents.qa_runner.khala_packaged_native_smoke.v1",
        appBundle: basename(appPath),
        appProcessName,
        appPathSource,
        executable: basename(executablePath),
        result: "result.json",
        scenario: scenario.name,
        screenshots: outcome.result.artifacts.screenshots,
        status: outcome.result.status,
        targetProcess: appPid === undefined ? "app-name" : "spawned-child",
      }, null, 2)}\n`,
    );
    return { ...outcome, appPath, executablePath, smokeReportPath };
  } finally {
    await shutdownChild(child, logFlushes);
  }
}
