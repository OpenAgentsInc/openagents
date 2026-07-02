// Khala Code Desktop backend for qa-runner.
//
// This backend boots the desktop app in preview-server mode, then drives the
// existing Chromium surface against that preview URL. It also exposes the typed
// Khala Code RPC client from `@openagentsinc/khala-qa-harness` with the preview
// access header already configured. Fixture mode is the default and uses the
// fixture Codex app-server so qa-runner smokes do not require live login/spend.

import { mkdirSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { join, resolve } from "node:path";
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
  nativeDesktopExample,
  runNativeDesktopScenario,
  type NativeDesktopBackendOptions,
  type NativeDesktopOutcome,
} from "./native-desktop-backend";
import type { Target } from "./target";

const DEFAULT_DESKTOP_CWD = resolve(import.meta.dir, "../../../clients/khala-code-desktop");
const DESKTOP_STDOUT_FILE = "khala-desktop-stdout.jsonl";
const DESKTOP_STDERR_FILE = "khala-desktop-stderr.jsonl";
const HARNESS_REPORT_FILE = "khala-desktop-harness-report.json";
const CHILD_TERM_GRACE_MS = 1_000;

export type KhalaDesktopBackendTier = "fixture" | "live_codex";

export type KhalaDesktopChildProcess = {
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

export async function runKhalaDesktopHeadedNativeSmoke(input: {
  readonly artifactDir: string;
  readonly native?: NativeDesktopBackendOptions;
  readonly target: Target;
}): Promise<NativeDesktopOutcome> {
  return runNativeDesktopScenario(
    {
      artifactDir: input.artifactDir,
      scenario: nativeDesktopExample("Khala Code"),
      target: input.target,
    },
    input.native,
  );
}
