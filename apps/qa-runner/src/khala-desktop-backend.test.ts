import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { Effect } from "effect";

import { scriptedBrain } from "./brain";
import { makeFakeChromium } from "./fake-chromium";
import {
  KhalaDesktopBackendBootError,
  khalaDesktopBackend,
  runKhalaDesktopHarnessScenario,
  runKhalaDesktopHeadedNativeSmoke,
  type KhalaDesktopSpawn,
} from "./khala-desktop-backend";
import { decodeQaRunResult } from "./result";
import { runQaSession } from "./runner";
import { makeTarget } from "./target";
import { NativeDesktopNotArmedError } from "./native-desktop-backend";

let dir: string;

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-khala-desktop-test-"));
});

afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const target = makeTarget({ name: "khala-code-desktop", baseUrl: "khala-desktop://local" });

const jsonResponse = (payload: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(payload), {
    headers: { "content-type": "application/json" },
    status: init?.status ?? 200,
  });

const fakeSpawn = (seen: Array<{ cmd: readonly string[]; env: Readonly<Record<string, string | undefined>> }>): KhalaDesktopSpawn =>
  (input) => {
    seen.push({ cmd: input.cmd, env: input.env });
    let resolveExited!: (code: number) => void;
    const exited = new Promise<number>((resolve) => {
      resolveExited = resolve;
    });
    return {
      exited,
      kill: () => resolveExited(0),
      stderr: new ReadableStream({
        start(controller) {
          controller.enqueue(new TextEncoder().encode("{\"event\":\"fixture-stderr\"}\n"));
          controller.close();
        },
      }),
      stdout: new ReadableStream({
        start(controller) {
          controller.enqueue(new TextEncoder().encode("{\"event\":\"fixture-stdout\"}\n"));
          controller.close();
        },
      }),
    };
  };

describe("khalaDesktopBackend", () => {
  test("boots fixture desktop headless and drives a scripted browser scenario with flushed artifacts", async () => {
    const spawns: Array<{ cmd: readonly string[]; env: Readonly<Record<string, string | undefined>> }> = [];
    const backend = khalaDesktopBackend({
      chromium: makeFakeChromium({
        pages: {
          "/health": { text: "{\"ok\":true,\"app\":\"Khala Code Desktop\"}" },
        },
      }),
      fetch: ((() =>
        Promise.resolve(jsonResponse({
          app: "Khala Code Desktop",
          ok: true,
          observedAt: "2026-07-01T00:00:00.000Z",
        }))) as unknown) as typeof fetch,
      previewAccessToken: "qa-preview-test",
      previewPort: 50131,
      spawn: fakeSpawn(spawns),
    });

    const outcome = await Effect.runPromise(
      runQaSession({
        artifactDir: dir,
        backend,
        brain: scriptedBrain([
          { kind: "navigate", url: "/health", label: "open preview health" },
          {
            check: { kind: "text-contains", selector: "body", value: "Khala Code Desktop" },
            kind: "assert",
            label: "preview health names the desktop app",
          },
          { kind: "screenshot", label: "preview-health" },
        ]),
        target,
      }),
    );

    expect(outcome.result.status).toBe("pass");
    expect(outcome.result.backend).toBe("khala-desktop");
    expect(outcome.result.target.baseUrl).toBe("khala-desktop://local");
    expect(spawns[0]?.cmd).toEqual([process.execPath, "src/bun/index.ts"]);
    expect(spawns[0]?.env.KHALA_CODE_DESKTOP_OPEN_WINDOW).toBe("0");
    expect(spawns[0]?.env.KHALA_CODE_CODEX_APP_SERVER_FIXTURE).toBe("1");
    expect(spawns[0]?.env.CODEX_HOME).toBe(join(dir, "fixture-codex-home"));
    expect(spawns[0]?.env.KHALA_CODE_DESKTOP_WORKSPACE).toBe(join(dir, "fixture-workspace"));
    expect(spawns[0]?.env.KHALA_CODE_TOKEN_USAGE_DISABLED).toBe("1");
    expect(spawns[0]?.env.KHALA_CODE_TOKEN_USAGE_BACKGROUND_SYNC_DISABLED).toBe("1");
    expect(spawns[0]?.env.KHALA_CODE_DESKTOP_PREVIEW_RPC_TOKEN).toBe("qa-preview-test");

    const onDisk = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    expect(onDisk.artifacts.screenshots).toContain("00-preview-health.png");
    expect(existsSync(join(dir, "result.json"))).toBe(true);
    expect(existsSync(join(dir, "trace.zip"))).toBe(true);
    expect(existsSync(join(dir, "khala-desktop-stdout.jsonl"))).toBe(true);
    expect(existsSync(join(dir, "khala-desktop-stderr.jsonl"))).toBe(true);
  });

  test("fixture hermetic defaults are overridable", async () => {
    const spawns: Array<{ cmd: readonly string[]; env: Readonly<Record<string, string | undefined>> }> = [];
    const backend = khalaDesktopBackend({
      chromium: makeFakeChromium(),
      env: {
        CODEX_HOME: "/tmp/custom-codex-home",
        KHALA_CODE_DESKTOP_WORKSPACE: "/tmp/custom-workspace",
        KHALA_CODE_TOKEN_USAGE_BACKGROUND_SYNC_DISABLED: "0",
        KHALA_CODE_TOKEN_USAGE_DISABLED: "0",
      },
      fetch: (() => Promise.resolve(jsonResponse({ ok: true }))) as unknown as typeof fetch,
      spawn: fakeSpawn(spawns),
    });

    const session = await backend.provision({ artifactDir: dir, target });
    await session.teardown();

    expect(spawns[0]?.env.CODEX_HOME).toBe("/tmp/custom-codex-home");
    expect(spawns[0]?.env.KHALA_CODE_DESKTOP_WORKSPACE).toBe("/tmp/custom-workspace");
    expect(spawns[0]?.env.KHALA_CODE_TOKEN_USAGE_DISABLED).toBe("0");
    expect(spawns[0]?.env.KHALA_CODE_TOKEN_USAGE_BACKGROUND_SYNC_DISABLED).toBe("0");
  });

  test("boot failure reaps child, escalates to SIGKILL after TERM grace, and flushes logs", async () => {
    const signals: Array<number | NodeJS.Signals | undefined> = [];
    let resolveExited!: (code: number) => void;
    const backend = khalaDesktopBackend({
      chromium: makeFakeChromium(),
      fetch: (() => Promise.resolve(jsonResponse({ ok: false }, { status: 503 }))) as unknown as typeof fetch,
      previewPort: 50133,
      spawn: () => ({
        exited: new Promise<number>((resolve) => {
          resolveExited = resolve;
        }),
        kill: (signal) => {
          signals.push(signal);
          if (signal === "SIGKILL") resolveExited(137);
        },
        stderr: new ReadableStream({
          start(controller) {
            controller.enqueue(new TextEncoder().encode("{\"event\":\"boot-stderr\"}\n"));
            controller.close();
          },
        }),
        stdout: new ReadableStream({
          start(controller) {
            controller.enqueue(new TextEncoder().encode("{\"event\":\"boot-stdout\"}\n"));
            controller.close();
          },
        }),
      }),
      waitTimeoutMs: 1,
    });

    await expect(backend.provision({ artifactDir: dir, target })).rejects.toBeInstanceOf(KhalaDesktopBackendBootError);

    expect(signals).toEqual(["SIGTERM", "SIGKILL"]);
    expect(readFileSync(join(dir, "khala-desktop-stdout.jsonl"), "utf8")).toContain("boot-stdout");
    expect(readFileSync(join(dir, "khala-desktop-stderr.jsonl"), "utf8")).toContain("boot-stderr");
  });

  test("health wait fails fast when the child exits before preview health is ready", async () => {
    const backend = khalaDesktopBackend({
      chromium: makeFakeChromium(),
      fetch: (() => new Promise<Response>(() => undefined)) as unknown as typeof fetch,
      previewPort: 50134,
      spawn: () => ({
        exited: Promise.resolve(42),
        kill: () => undefined,
        stderr: null,
        stdout: null,
      }),
      waitTimeoutMs: 10_000,
    });

    await expect(backend.provision({ artifactDir: dir, target })).rejects.toThrow(/exited before health check/);
  });

  test("runs a fixture-tier RPC harness scenario through the preview access header", async () => {
    const headers: Record<string, string>[] = [];
    const backend = khalaDesktopBackend({
      chromium: makeFakeChromium(),
      fetch: ((input, init) => {
        if (String(input).endsWith("/health")) {
          return Promise.resolve(jsonResponse({ ok: true }));
        }
        headers.push(Object.fromEntries(new Headers(init?.headers)));
        return Promise.resolve(jsonResponse({
          app: "Khala Code Desktop",
          ok: true,
          observedAt: "2026-07-01T00:00:00.000Z",
        }));
      }) as typeof fetch,
      previewAccessToken: "qa-preview-test",
      previewPort: 50132,
      spawn: fakeSpawn([]),
    });

    const report = await runKhalaDesktopHarnessScenario({
      artifactDir: dir,
      backend,
      scenario: {
        backend: "fixture",
        commitments: [{ claim: "fixture appInfo passes", evidence: "run-pass", id: "run.pass" }],
        id: "scenario.khala_code.qa_runner_desktop_fixture_app_info.v1",
        modes: ["rpc"],
        phases: [
          {
            act: [{ kind: "rpc_call", method: "appInfo" }],
            expect: [{ oracle: "schema", query: "appInfo" }, { oracle: "crash" }],
            name: "boot-rpc",
          },
        ],
      },
      target,
    });

    expect(report.status).toBe("pass");
    expect(headers[0]?.["x-khala-code-preview-token"]).toBe("qa-preview-test");
    expect(existsSync(join(dir, "khala-desktop-harness-report.json"))).toBe(true);
  });

  test("headed native smoke is skip-safe unless QA_NATIVE_DESKTOP is armed", async () => {
    await expect(
      runKhalaDesktopHeadedNativeSmoke({
        artifactDir: dir,
        native: { env: {} },
        target,
      }),
    ).rejects.toBeInstanceOf(NativeDesktopNotArmedError);
  });
});
