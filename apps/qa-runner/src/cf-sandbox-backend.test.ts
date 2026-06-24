// CF Sandbox/Containers terminal backend tests.
//
// FAKE-TESTED (deterministic, no network, no spend): the backend replays a
// terminal scenario against a FAKE sandbox (scripted `exec` results via an
// injected `getSandbox`), producing result.json + a transcript. armed/unarmed +
// binding-absent honest-error are all covered, plus a deliberately-wrong scenario
// proving a red is a real red.
//
// NOT tested here (needs a live CF deploy): the actual `@cloudflare/sandbox`
// Durable-Object container. The `env.Sandbox` binding only exists inside a
// deployed Worker with a `[[containers]]` image — there is no live binding in CI
// — so the real Cloudflare run is a DEPLOY step.

import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";

import {
  CfSandboxBackendNotArmedError,
  CfSandboxBindingAbsentError,
  echoSandboxScenario,
  echoSandboxScenarioWrong,
  isCfSandboxBackendArmed,
  runCfSandboxScenario,
  type CfGetSandbox,
  type CfSandbox,
  type CfSandboxExecResult,
} from "./cf-sandbox-backend";
import { decodeQaRunResult } from "./result";
import { makeTarget } from "./target";

let dir: string;
beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "qa-cf-sandbox-test-"));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

const target = makeTarget({ name: "cf-sandbox-fake", baseUrl: "https://example.test" });

/**
 * A fake `getSandbox` returning a scripted `CfSandbox`. Each command resolves
 * deterministically from `responses` (keyed by exact command); an unknown command
 * resolves to a clean `true`-style success so the simple example scenario passes.
 * Records the commands run so the lifecycle can be asserted.
 */
function makeFakeGetSandbox(config: {
  readonly responses?: Record<string, Partial<CfSandboxExecResult>>;
  readonly recordCommands?: string[];
  readonly onGetSandbox?: (binding: unknown, id: string) => void;
}): CfGetSandbox {
  return (binding, id) => {
    config.onGetSandbox?.(binding, id);
    const sandbox: CfSandbox = {
      exec: async (command) => {
        config.recordCommands?.push(command);
        const r = config.responses?.[command];
        // Default: simulate the POSIX commands the example uses (printf/echo/true).
        const out =
          r?.stdout ??
          (command.startsWith("printf ") || command.startsWith("echo ")
            ? command.replace(/^printf '|^echo /, "").replace(/'$|\\n$/g, "").replace(/\\n/g, "\n")
            : "");
        return {
          success: r?.success ?? true,
          stdout: r?.stdout ?? out,
          stderr: r?.stderr ?? "",
          exitCode: r?.exitCode ?? 0,
        };
      },
    };
    return sandbox;
  };
}

describe("cfSandboxBackend arming", () => {
  test("isCfSandboxBackendArmed reads the env flag", () => {
    expect(isCfSandboxBackendArmed({})).toBe(false);
    expect(isCfSandboxBackendArmed({ QA_CF_SANDBOX_BACKEND: "1" })).toBe(true);
    expect(isCfSandboxBackendArmed({ QA_CF_SANDBOX_BACKEND: "true" })).toBe(true);
    expect(isCfSandboxBackendArmed({ QA_CF_SANDBOX_BACKEND: "0" })).toBe(false);
  });

  test("un-armed run throws CfSandboxBackendNotArmedError (no fake green)", async () => {
    await expect(
      runCfSandboxScenario(
        { target, scenario: echoSandboxScenario(), artifactDir: dir },
        { env: {}, sandboxBinding: {} },
      ),
    ).rejects.toBeInstanceOf(CfSandboxBackendNotArmedError);
  });

  test("armed but binding ABSENT throws CfSandboxBindingAbsentError (honest CI error)", async () => {
    await expect(
      runCfSandboxScenario(
        { target, scenario: echoSandboxScenario(), artifactDir: dir },
        { armed: true },
      ),
    ).rejects.toBeInstanceOf(CfSandboxBindingAbsentError);
  });
});

describe("cfSandboxBackend run (fake sandbox, deterministic)", () => {
  test("replays the echo scenario -> result.json (pass) + transcript", async () => {
    const commands: string[] = [];
    let gotBinding: unknown;
    let gotId: string | undefined;
    const fakeBinding = { __brand: "fake-env-Sandbox" };

    const outcome = await runCfSandboxScenario(
      { target, scenario: echoSandboxScenario(), artifactDir: dir },
      {
        armed: true,
        sandboxBinding: fakeBinding,
        sandboxId: "test-sandbox-1",
        now: () => 1_750_000_000_000,
        getSandbox: makeFakeGetSandbox({
          responses: {
            "printf 'QA SANDBOX READY\\n'": { stdout: "QA SANDBOX READY\n", exitCode: 0, success: true },
            "echo hello, khala!": { stdout: "hello, khala!\n", exitCode: 0, success: true },
            true: { stdout: "", exitCode: 0, success: true },
          },
          recordCommands: commands,
          onGetSandbox: (b, id) => {
            gotBinding = b;
            gotId = id;
          },
        }),
      },
    );

    // The injected binding + id reached getSandbox.
    expect(gotBinding).toBe(fakeBinding);
    expect(gotId).toBe("test-sandbox-1");
    // All three example commands were executed in order.
    expect(commands).toEqual([
      "printf 'QA SANDBOX READY\\n'",
      "echo hello, khala!",
      "true",
    ]);

    // result.json: passing, on the cf-sandbox backend, schema-valid + public-safe.
    expect(outcome.result.status).toBe("pass");
    expect(outcome.result.backend).toBe("cf-sandbox");
    expect(outcome.result.brain).toBe("cf-sandbox-scenario");
    const onDisk = decodeQaRunResult(JSON.parse(readFileSync(outcome.resultPath, "utf8")));
    expect(onDisk.status).toBe("pass");
    expect(onDisk.backend).toBe("cf-sandbox");

    // The transcript is the replayable artifact, captured with real outputs.
    const transcript = JSON.parse(readFileSync(outcome.transcriptPath, "utf8"));
    expect(transcript.schemaVersion).toBe("openagents.qa_runner.cf_sandbox_transcript.v1");
    expect(transcript.sandboxId).toBe("test-sandbox-1");
    expect(transcript.entries).toHaveLength(3);
    expect(transcript.entries[0].command).toBe("printf 'QA SANDBOX READY\\n'");
    expect(transcript.entries[1].stdout).toContain("hello, khala!");
  });

  test("a real red is a real red: a missing-output assertion -> status fail", async () => {
    const outcome = await runCfSandboxScenario(
      { target, scenario: echoSandboxScenarioWrong(), artifactDir: dir },
      {
        armed: true,
        sandboxBinding: {},
        now: () => 1_750_000_000_000,
        getSandbox: makeFakeGetSandbox({
          responses: { "echo hello, khala!": { stdout: "hello, khala!\n", exitCode: 0, success: true } },
        }),
      },
    );
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toContain("goodbye, khala!");
  });

  test("a non-zero exit on an asserted command -> status fail", async () => {
    const outcome = await runCfSandboxScenario(
      {
        target,
        scenario: {
          name: "boom",
          steps: [{ kind: "exec", command: "exit 1", label: "boom", assertExitCode: 0 }],
        },
        artifactDir: dir,
      },
      {
        armed: true,
        sandboxBinding: {},
        now: () => 1_750_000_000_000,
        getSandbox: makeFakeGetSandbox({
          responses: { "exit 1": { stdout: "", stderr: "", exitCode: 1, success: false } },
        }),
      },
    );
    expect(outcome.result.status).toBe("fail");
    expect(outcome.result.failure).toContain("exit code 0");
  });
});
