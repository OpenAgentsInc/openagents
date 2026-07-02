import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { Effect } from "effect";

import { scriptedBrain } from "./brain";
import { makeFakeChromium } from "./fake-chromium";
import {
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
    return {
      exited: Promise.resolve(0),
      kill: () => undefined,
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
    expect(spawns[0]?.cmd).toEqual(["bun", "src/bun/index.ts"]);
    expect(spawns[0]?.env.KHALA_CODE_DESKTOP_OPEN_WINDOW).toBe("0");
    expect(spawns[0]?.env.KHALA_CODE_CODEX_APP_SERVER_FIXTURE).toBe("1");
    expect(spawns[0]?.env.KHALA_CODE_DESKTOP_PREVIEW_RPC_TOKEN).toBe("qa-preview-test");

    const onDisk = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    expect(onDisk.artifacts.screenshots).toContain("00-preview-health.png");
    expect(existsSync(join(dir, "result.json"))).toBe(true);
    expect(existsSync(join(dir, "trace.zip"))).toBe(true);
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
