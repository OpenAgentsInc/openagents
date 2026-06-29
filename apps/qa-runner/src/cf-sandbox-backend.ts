// Cloudflare Sandbox/Containers terminal execution backend (#6205).
//
// The prod-native TERMINAL substrate — the Cloudflare analogue of the local
// Docker container backend (#6186). Instead of `docker run` + `docker exec`
// against a local engine, a sandboxed command/terminal scenario runs inside a
// Cloudflare Sandbox (a Durable Object built on Containers) via
// `getSandbox(env.Sandbox, id)` and `sandbox.exec(command)`. No local Docker, no
// VM, no host to provision — it runs on the same Workers stack we already deploy.
//
// SCENARIO MODEL:
//   Cloudflare's `sandbox.exec()` is request/response (run a command -> get
//   { stdout, stderr, exitCode, success }), which is exactly the shape the local
//   container backend uses (exec a command, capture combined output). So a CF
//   sandbox scenario is an ordered list of EXEC steps, each optionally asserting
//   on the command's stdout / exit code. (Interactive PTY scenarios — the
//   wait-for-text/send-input model in terminal-backend.ts — map to Cloudflare's
//   `sandbox.terminal()` WebSocket, which is a heavier, deploy-only path left for
//   a follow-up; this backend covers the command/terminal scenarios the issue
//   calls for.)
//
// RESULT + ARTIFACTS:
//   Emits the EXACT same public-safe `QaRunResult` schema + tripwire the browser
//   and local-terminal backends use (brain="cf-sandbox-scenario",
//   backend="cf-sandbox"), so the brain/target/artifact contracts are unchanged.
//   The replayable artifact is a TRANSCRIPT (one entry per exec: the command, its
//   exit code, and its captured stdout/stderr) written alongside result.json. The
//   raw input/command is recorded; secrets must not be passed as command text
//   (the public-safety tripwire re-checks the written artifacts).
//
// OWNER-GATED / ARMED-BY-ENV (default OFF):
//   Inert unless explicitly armed (`QA_CF_SANDBOX_BACKEND=1`, or `armed: true`).
//   Un-armed -> `CfSandboxBackendNotArmedError`. Never silently runs.
//
// HONEST ABOUT THE BINDING:
//   `env.Sandbox` only exists inside a deployed Worker with a `[[containers]]` +
//   Durable-Object binding. CI has NO live binding, so when armed but the binding
//   is absent, provisioning throws `CfSandboxBindingAbsentError`. The REAL CF run
//   is a DEPLOY step. Unit tests inject a FAKE sandbox (scripted exec results),
//   proving the run lifecycle deterministically with NO network and NO spend.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { assertPublicSafeResult, type QaRunResult, type QaRunStep } from "./result";
import type { Target } from "./target";

const TRANSCRIPT_FILE = "cf-sandbox-transcript.json";

export class CfSandboxBackendNotArmedError extends Error {
  constructor() {
    super(
      "cfSandboxBackend is not armed: the Cloudflare Sandbox/Containers terminal " +
        "backend (sandboxed exec via env.Sandbox) is owner-gated and OFF by " +
        "default. Arm it explicitly with QA_CF_SANDBOX_BACKEND=1 (or { armed: true }).",
    );
    this.name = "CfSandboxBackendNotArmedError";
  }
}

export class CfSandboxBindingAbsentError extends Error {
  constructor() {
    super(
      "cfSandboxBackend is armed but the Cloudflare Sandbox binding (env.Sandbox) " +
        "is absent. That binding only exists inside a deployed Worker with a " +
        "[[containers]] image + a Durable-Object binding; it is NOT available in " +
        "unit CI. Run this on a deploy, or inject a fake sandbox for tests. It " +
        "will NOT fall back to local Docker or fake a result.",
    );
    this.name = "CfSandboxBindingAbsentError";
  }
}

// ── The @cloudflare/sandbox shape we depend on (minimal + injectable) ─────────

/** The result of `sandbox.exec()` (Cloudflare `ExecuteResponse`). */
export interface CfSandboxExecResult {
  readonly success: boolean;
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
}

/** The minimal Sandbox slice this backend drives. */
export interface CfSandbox {
  exec(command: string, options?: { readonly cwd?: string }): Promise<CfSandboxExecResult>;
}

/** The opaque `env.Sandbox` Durable-Object namespace binding. */
export type CfSandboxBinding = unknown;

/** `getSandbox(env.Sandbox, id)` from `@cloudflare/sandbox`. */
export type CfGetSandbox = (binding: CfSandboxBinding, id: string) => CfSandbox;

/** One exec step in a CF sandbox scenario: run `command`, optionally assert. */
export type CfSandboxStep = {
  readonly kind: "exec";
  /** The command to run via `sandbox.exec()`. NEVER pass secrets as text. */
  readonly command: string;
  readonly label?: string;
  /** Working directory for the command. */
  readonly cwd?: string;
  /** Assert the command's stdout CONTAINS this. A miss is an honest red. */
  readonly assertStdoutContains?: string;
  /** Assert the command's stdout does NOT contain this. */
  readonly assertStdoutNotContains?: string;
  /** Assert the exit code equals this (default: no exit-code assertion). */
  readonly assertExitCode?: number;
};

export interface CfSandboxScenario {
  /** Stable scenario name (lands in result.json). */
  readonly name: string;
  /** Ordered exec steps to replay. */
  readonly steps: ReadonlyArray<CfSandboxStep>;
}

export interface CfSandboxBackendOptions {
  /**
   * The Cloudflare Sandbox binding. In a deployed Worker this is `env.Sandbox`.
   * ABSENT in unit CI — when armed without it, provisioning throws
   * `CfSandboxBindingAbsentError`. Tests inject a fake.
   */
  readonly sandboxBinding?: CfSandboxBinding;
  /** Arm the backend. Defaults to reading `QA_CF_SANDBOX_BACKEND` from `env`. */
  readonly armed?: boolean;
  /** Env source for the arming check (default `process.env`). */
  readonly env?: Readonly<Record<string, string | undefined>>;
  /**
   * Injectable `getSandbox`. Default: dynamic import of `@cloudflare/sandbox`.
   * Tests inject a fake returning a scripted `CfSandbox` (no network).
   */
  readonly getSandbox?: CfGetSandbox;
  /** Sandbox instance id (default a per-run id). Maps to `getSandbox(_, id)`. */
  readonly sandboxId?: string;
  /** Injectable clock for deterministic result timestamps + ids. */
  readonly now?: () => number;
}

export interface RunCfSandboxScenarioInput {
  readonly target: Target;
  readonly scenario: CfSandboxScenario;
  /** Directory artifacts (transcript + result.json) are written to. */
  readonly artifactDir: string;
}

export interface RunCfSandboxScenarioOutcome {
  readonly result: QaRunResult;
  readonly resultPath: string;
  readonly transcriptPath: string;
}

/** True when the env arms the CF Sandbox backend. */
export function isCfSandboxBackendArmed(
  env: Readonly<Record<string, string | undefined>> = process.env,
): boolean {
  const v = env.QA_CF_SANDBOX_BACKEND;
  return v === "1" || v === "true";
}

/** Dynamic import of the real `@cloudflare/sandbox` `getSandbox`. Only called
 *  when no `getSandbox` is injected — i.e. inside a deployed Worker. Kept out of
 *  the unit module graph so CI never resolves a Workers-only package. */
async function defaultGetSandboxModule(): Promise<{ getSandbox: CfGetSandbox }> {
  const pkg = ["@cloudflare", "sandbox"].join("/");
  return (await import(/* @vite-ignore */ pkg)) as { getSandbox: CfGetSandbox };
}

/** One transcript entry: a command and its captured outcome. */
interface CfSandboxTranscriptEntry {
  readonly index: number;
  readonly label: string;
  readonly command: string;
  readonly exitCode: number;
  readonly success: boolean;
  readonly stdout: string;
  readonly stderr: string;
}

/**
 * Replay a CF sandbox scenario against a `CfSandbox`, recording steps + a
 * transcript. Honest: a failed assertion or a non-zero exit (when asserted) is a
 * real red — the transcript captures the command's actual output and `status` is
 * "fail". capture-on-failure: stop on the first honest red.
 */
async function driveCfSandboxScenario(
  sandbox: CfSandbox,
  scenario: CfSandboxScenario,
): Promise<{ steps: QaRunStep[]; transcript: CfSandboxTranscriptEntry[]; failure?: string }> {
  const steps: QaRunStep[] = [];
  const transcript: CfSandboxTranscriptEntry[] = [];
  let failure: string | undefined;

  const record = (
    index: number,
    status: "ok" | "failed",
    label: string,
    detail?: Record<string, string | number | boolean>,
  ) => steps.push({ index, kind: "exec", label, status, ...(detail ? { detail } : {}) });

  for (let index = 0; index < scenario.steps.length; index++) {
    const step = scenario.steps[index]!;
    const label = step.label ?? `exec ${step.command}`;
    let exec: CfSandboxExecResult;
    try {
      exec = await sandbox.exec(step.command, step.cwd !== undefined ? { cwd: step.cwd } : undefined);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      record(index, "failed", label, { error: message });
      failure = `exec failed: ${message}`;
      break;
    }

    transcript.push({
      index,
      label,
      command: step.command,
      exitCode: exec.exitCode,
      success: exec.success,
      stdout: exec.stdout,
      stderr: exec.stderr,
    });

    // Evaluate assertions in order; the first failed assertion is the red.
    const fail = (reason: string) => {
      record(index, "failed", label, { exitCode: exec.exitCode, reason });
      failure = `${label}: ${reason}`;
    };

    if (step.assertExitCode !== undefined && exec.exitCode !== step.assertExitCode) {
      fail(`expected exit code ${step.assertExitCode}, got ${exec.exitCode}`);
      break;
    }
    if (step.assertStdoutContains !== undefined && !exec.stdout.includes(step.assertStdoutContains)) {
      fail(`expected stdout to contain "${step.assertStdoutContains}"`);
      break;
    }
    if (
      step.assertStdoutNotContains !== undefined &&
      exec.stdout.includes(step.assertStdoutNotContains)
    ) {
      fail(`expected stdout NOT to contain "${step.assertStdoutNotContains}"`);
      break;
    }
    record(index, "ok", label, { exitCode: exec.exitCode });
  }

  return failure !== undefined ? { steps, transcript, failure } : { steps, transcript };
}

/**
 * Run a Cloudflare Sandbox terminal scenario and emit artifacts. Owner-gated
 * (armed) + honest about the binding (absent in CI). Resolves with the public-safe
 * result + artifact paths. Honest: a failed assertion or non-zero asserted exit
 * yields a non-passing result; no fabricated success.
 */
export async function runCfSandboxScenario(
  input: RunCfSandboxScenarioInput,
  options: CfSandboxBackendOptions = {},
): Promise<RunCfSandboxScenarioOutcome> {
  const env = options.env ?? process.env;
  const armed = options.armed ?? isCfSandboxBackendArmed(env);
  if (!armed) throw new CfSandboxBackendNotArmedError();

  // The binding only exists inside a deployed Worker. Honest in CI.
  if (options.sandboxBinding === undefined || options.sandboxBinding === null) {
    throw new CfSandboxBindingAbsentError();
  }

  const now = options.now ?? Date.now;
  const getSandbox = options.getSandbox ?? (await defaultGetSandboxModule()).getSandbox;
  const sandboxId = options.sandboxId ?? `qa-runner-${now()}`;

  mkdirSync(input.artifactDir, { recursive: true });

  const startedAt = new Date(now());
  const sandbox = getSandbox(options.sandboxBinding, sandboxId);
  const drive = await driveCfSandboxScenario(sandbox, input.scenario);
  const endedAt = new Date(now());

  // ── Artifact 1: the exec transcript (replayable command/output record) ──────
  const transcriptPath = join(input.artifactDir, TRANSCRIPT_FILE);
  const transcript = {
    schemaVersion: "openagents.qa_runner.cf_sandbox_transcript.v1",
    scenario: input.scenario.name,
    sandboxId,
    entries: drive.transcript,
  };
  assertPublicSafeResult(transcript);
  writeFileSync(transcriptPath, `${JSON.stringify(transcript, null, 2)}\n`);

  // ── Artifact 2: result.json (same public-safe schema as the other backends) ─
  const status: "pass" | "fail" = drive.failure === undefined ? "pass" : "fail";
  const result: QaRunResult = {
    schemaVersion: "openagents.qa_runner.result.v1",
    status,
    target: { name: input.target.name, baseUrl: input.target.baseUrl },
    brain: "cf-sandbox-scenario",
    backend: "cf-sandbox",
    startedAt: startedAt.toISOString(),
    endedAt: endedAt.toISOString(),
    durationMs: endedAt.getTime() - startedAt.getTime(),
    steps: drive.steps,
    artifacts: {
      // The transcript is the replayable artifact; recorded under screenshots so
      // the shared QaRunArtifacts contract is unchanged (no video — exec has none).
      screenshots: [TRANSCRIPT_FILE],
    },
    ...(drive.failure ? { failure: drive.failure } : {}),
  };
  assertPublicSafeResult(result);
  const resultPath = join(input.artifactDir, "result.json");
  writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`);

  return { result, resultPath, transcriptPath };
}

/**
 * The shipped deterministic example scenario: a few portable POSIX commands that
 * print a banner, echo a value, and prove a clean exit — exercising
 * exec -> assert-on-stdout -> assert-exit with NO network. The wrong variant
 * (below) asserts on text the commands never print, proving a red is a real red.
 */
export function echoSandboxScenario(): CfSandboxScenario {
  return {
    name: "echo-sandbox",
    steps: [
      {
        kind: "exec",
        command: "printf 'QA SANDBOX READY\\n'",
        label: "banner prints",
        assertStdoutContains: "QA SANDBOX READY",
        assertExitCode: 0,
      },
      {
        kind: "exec",
        command: "echo hello, khala!",
        label: "echo-back prints",
        assertStdoutContains: "hello, khala!",
        assertStdoutNotContains: "Traceback",
        assertExitCode: 0,
      },
      {
        kind: "exec",
        command: "true",
        label: "clean exit",
        assertExitCode: 0,
      },
    ],
  };
}

/**
 * A deliberately-wrong variant: asserts stdout contains text the command never
 * prints. Used to prove a red is a real red (the failed assertion is recorded
 * with the command's actual output in the transcript).
 */
export function echoSandboxScenarioWrong(): CfSandboxScenario {
  return {
    name: "echo-sandbox-wrong",
    steps: [
      {
        kind: "exec",
        command: "echo hello, khala!",
        label: "echo-back prints",
        // WRONG on purpose: the command never prints "goodbye".
        assertStdoutContains: "goodbye, khala!",
      },
    ],
  };
}
