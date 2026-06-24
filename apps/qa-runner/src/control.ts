// QA control plane — the in-process engine the HTTP daemon drives (#6196).
//
// "Do everything via API." The qa-runner is a CLI today; this module is the
// transport-agnostic CORE the HTTP daemon (api-server.ts) exposes so the full
// autonomous-QA / eval flow — submit -> run -> fetch artifacts + verdict +
// /pro link — is drivable programmatically. The same engine the CLI uses
// (runQaSession / runEval) runs here in-process, async, with an in-memory job
// store. One core, two faces (CLI + API).
//
// WHY a daemon, not a Cloudflare Worker: the runner drives a REAL Chrome via
// Playwright (localBackend), which cannot run inside a Worker isolate. So the
// control plane runs on a machine WITH Chrome and is reached over HTTP. The
// Worker side (/pro) only DEREFERENCES the public-safe artifacts this produces.
//
// DESIGN BOUNDARIES (honored deliberately):
//   - This module OWNS only orchestration + job state. It does NOT redefine the
//     result schema (result.ts), the runner (runner.ts), evals core (evals.ts),
//     the distiller, /pro web files, or khala-config/driver/openrouter. It reads
//     the additive `verify` (peer lane) and `receipt` (landed) fields off
//     result.json if present, never defining or mutating them.
//   - DETERMINISTIC MOCK PATH: a `scriptedBrain` + a fake-chromium fixture
//     backend runs the whole submit->run->fetch flow with NO network and NO
//     spend — the path tests + a third party's curl quick-start use.
//   - REAL runs are GATED: a real (network-touching) run requires the daemon to
//     be armed (env) AND a per-run token budget; un-armed real submits are
//     refused honestly (no silent fallback to a fake green).
//   - HONEST receipts: each job carries an honest receipt of what it actually
//     did (mode: mock vs real, spend-capable: false for the fixture path).

import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { Effect } from "effect";
import { localBackend } from "./backend";
import { scriptedBrain } from "./brain";
import { type EvalVariant, runEval, type EvalResult } from "./evals";
import { makeFakeChromium } from "./fake-chromium";
import { readRunArtifacts, type RunArtifacts } from "./artifacts";
import { writeReceiptForRun } from "./receipt";
import {
  loginRegressionSteps,
  loginRegressionStepsWrong,
} from "./scenarios";
import { runQaSession } from "./runner";
import { makeTarget, type Target } from "./target";
import { TARGET_REGISTRY, isTargetName } from "./target-registry";

// Resolve a control-API `target` field to a base URL: a registry NAME
// (dev/staging/prod/selfhost) -> its baseUrl; a full http(s) URL -> as-is;
// otherwise default to prod. Without this a named target like "prod" reached
// the runner as a bare baseUrl and produced an invalid relative navigate.
const resolveControlBaseUrl = (target?: string): string => {
  if (!target) return "https://openagents.com";
  if (/^https?:\/\//i.test(target)) return target;
  if (isTargetName(target)) {
    const baseUrl = TARGET_REGISTRY[target].baseUrl;
    if (baseUrl) return baseUrl;
  }
  return "https://openagents.com";
};

// ---------------------------------------------------------------------------
// Public-safe job model (what GET /runs/:id and GET /evals/:id project)
// ---------------------------------------------------------------------------

export type JobKind = "run" | "eval";
export type JobStatus = "queued" | "running" | "succeeded" | "failed";

/** How a job executed: deterministic mock vs a real (spend-capable-class) run. */
export type JobMode = "mock" | "real";

/**
 * The honest, public-safe receipt the control plane attaches to every job. It
 * records WHAT the job actually did — the mode, whether the seam was
 * spend-capable, and the token budget cap applied — so a caller never mistakes a
 * fixture run for a decision-grade one. No secrets: a mode + booleans + a cap.
 */
export interface JobReceipt {
  readonly mode: JobMode;
  /** True only when a real spend-capable seam produced the outcome. Fixture/mock = false. */
  readonly spendCapable: boolean;
  /** The per-job token budget cap honored (0 for the mock path). */
  readonly tokenBudget: number;
  /** Tokens this job is accounted to have spent (0 for the deterministic mock path). */
  readonly tokensSpent: number;
}

export interface Job {
  readonly id: string;
  readonly kind: JobKind;
  readonly status: JobStatus;
  readonly mode: JobMode;
  /** Public-safe scenario id (e.g. "login-regression"). */
  readonly scenario: string;
  /** Public-safe target name (never a secret). */
  readonly targetName: string;
  readonly createdAt: string;
  readonly startedAt?: string;
  readonly endedAt?: string;
  /** Honest one-line failure summary when status is "failed". */
  readonly error?: string;
  readonly receipt: JobReceipt;
  /** Relative artifact dir under the store root (filled once the run starts). */
  readonly artifactDir?: string;
}

// ---------------------------------------------------------------------------
// Submit shapes (what POST /runs and POST /evals accept)
// ---------------------------------------------------------------------------

/** A named scenario the control plane can run. Bounded + public-safe. */
export type ControlScenario = "login-regression" | "login-regression-wrong";

const SCENARIO_STEPS = {
  "login-regression": loginRegressionSteps,
  "login-regression-wrong": loginRegressionStepsWrong,
} as const;

export const isControlScenario = (value: unknown): value is ControlScenario =>
  value === "login-regression" || value === "login-regression-wrong";

export interface SubmitRunInput {
  readonly scenario?: ControlScenario;
  /** Target base URL (e.g. https://openagents.com) — real runs only. */
  readonly target?: string;
  readonly targetName?: string;
  /** brain/model selector. "scripted" runs now; "khala"/OpenRouter is real-gated. */
  readonly brain?: string;
  readonly model?: string;
  /** Public-safe commitments echoed onto the job (e.g. ["own-infra"]). */
  readonly commitments?: ReadonlyArray<string>;
  /** Run against the real Target via real chromium (gated). Default false (mock). */
  readonly real?: boolean;
  /** Per-run token budget cap for a real run. */
  readonly tokenBudget?: number;
}

export interface SubmitEvalInput {
  readonly id?: string;
  readonly title?: string;
  readonly target?: string;
  readonly targetName?: string;
  /** The scenario held fixed across variants. */
  readonly scenario?: ControlScenario;
  /** >= 2 variants to compare. Each names a scenario the variant's brain replays. */
  readonly variants?: ReadonlyArray<SubmitEvalVariant>;
  readonly repetitions?: number;
  readonly real?: boolean;
  readonly tokenBudget?: number;
}

export interface SubmitEvalVariant {
  readonly id: string;
  readonly label?: string;
  readonly note?: string;
  /** Which scenario this variant's scripted brain replays. */
  readonly scenario?: ControlScenario;
}

// ---------------------------------------------------------------------------
// Errors (mapped to HTTP status by the server)
// ---------------------------------------------------------------------------

export class BadRequestError extends Error {
  readonly _tag = "BadRequestError";
}
export class NotArmedError extends Error {
  readonly _tag = "NotArmedError";
}
export class NotFoundError extends Error {
  readonly _tag = "NotFoundError";
}

// ---------------------------------------------------------------------------
// The control plane
// ---------------------------------------------------------------------------

export interface ControlOptions {
  /** Root directory artifacts are written under (one subdir per job). */
  readonly storeDir: string;
  /** Base URL for /pro links (default https://openagents.com). */
  readonly proBaseUrl?: string;
  /**
   * Whether REAL (network-touching) runs are armed. When false, a `real: true`
   * submit is refused honestly. Default false (mock-only). The HTTP daemon sets
   * this from QA_CONTROL_ARM_REAL=1.
   */
  readonly allowReal?: boolean;
  /** Default token budget cap for a real run when the caller omits one. */
  readonly defaultTokenBudget?: number;
  /** Injectable id generator (deterministic in tests). */
  readonly genId?: (kind: JobKind) => string;
  /** Injectable clock (deterministic timestamps in tests). */
  readonly now?: () => Date;
}

let counter = 0;
const defaultGenId = (kind: JobKind): string =>
  `${kind}_${Date.now().toString(36)}_${(counter++).toString(36)}`;

export class QaControl {
  private readonly jobs = new Map<string, Job>();
  private readonly inflight = new Map<string, Promise<void>>();
  private readonly proBaseUrl: string;
  private readonly allowReal: boolean;
  private readonly defaultTokenBudget: number;
  private readonly genId: (kind: JobKind) => string;
  private readonly now: () => Date;

  constructor(private readonly options: ControlOptions) {
    this.proBaseUrl = (options.proBaseUrl ?? "https://openagents.com").replace(/\/$/, "");
    this.allowReal = options.allowReal ?? false;
    this.defaultTokenBudget = options.defaultTokenBudget ?? 0;
    this.genId = options.genId ?? defaultGenId;
    this.now = options.now ?? (() => new Date());
    mkdirSync(options.storeDir, { recursive: true });
  }

  // ── submit a run ────────────────────────────────────────────────────────

  submitRun(input: SubmitRunInput): Job {
    const scenario = input.scenario ?? "login-regression";
    if (!isControlScenario(scenario)) {
      throw new BadRequestError(`unknown scenario "${scenario}"`);
    }
    const real = input.real === true;
    if (real && !this.allowReal) {
      throw new NotArmedError(
        "real runs are not armed on this daemon: set QA_CONTROL_ARM_REAL=1 to enable a " +
          "real (network/spend-capable) run. The deterministic mock path runs without arming.",
      );
    }
    const mode: JobMode = real ? "real" : "mock";
    const id = this.genId("run");
    const createdAt = this.now().toISOString();
    const tokenBudget = real ? (input.tokenBudget ?? this.defaultTokenBudget) : 0;
    const targetName = real
      ? input.targetName ?? input.target ?? "openagents.com-prod"
      : "fixtures";

    const job: Job = {
      id,
      kind: "run",
      status: "queued",
      mode,
      scenario,
      targetName,
      createdAt,
      receipt: { mode, spendCapable: real, tokenBudget, tokensSpent: 0 },
    };
    this.jobs.set(id, job);

    const artifactDir = join(this.options.storeDir, id);
    this.set(id, { artifactDir: id });

    const target: Target = real
      ? makeTarget({
          name: targetName,
          baseUrl: resolveControlBaseUrl(input.target),
        })
      : makeTarget({ name: "fixtures", baseUrl: "https://example.test" });

    const backend = () =>
      real
        ? localBackend()
        : localBackend({ chromium: this.mockChromium(scenario) });

    this.inflight.set(
      id,
      this.execute(id, async () => {
        await Effect.runPromise(
          runQaSession({
            target,
            brain: scriptedBrain(SCENARIO_STEPS[scenario]()),
            backend: backend(),
            artifactDir,
            ...(this.options.now ? { now: this.options.now } : {}),
          }),
        );
        // Attach the additive receipt the post-run helper owns (idempotent).
        writeReceiptForRun(artifactDir);
      }),
    );
    return this.get(id);
  }

  // ── submit an eval (>= 2 variants) ────────────────────────────────────────

  submitEval(input: SubmitEvalInput): Job {
    const variants = input.variants ?? [];
    if (variants.length < 2) {
      throw new BadRequestError(
        `a chill-eval compares variants: got ${variants.length}, need >= 2`,
      );
    }
    for (const v of variants) {
      const s = v.scenario ?? "login-regression";
      if (!isControlScenario(s)) {
        throw new BadRequestError(`variant "${v.id}" names unknown scenario "${s}"`);
      }
    }
    const real = input.real === true;
    if (real && !this.allowReal) {
      throw new NotArmedError(
        "real evals are not armed on this daemon: set QA_CONTROL_ARM_REAL=1. The " +
          "deterministic mock path runs without arming.",
      );
    }
    const mode: JobMode = real ? "real" : "mock";
    const id = this.genId("eval");
    const createdAt = this.now().toISOString();
    const tokenBudget = real ? (input.tokenBudget ?? this.defaultTokenBudget) : 0;
    const scenario = input.scenario ?? "login-regression";
    const targetName = real
      ? input.targetName ?? input.target ?? "openagents.com-prod"
      : "fixtures";

    const job: Job = {
      id,
      kind: "eval",
      status: "queued",
      mode,
      scenario,
      targetName,
      createdAt,
      receipt: { mode, spendCapable: real, tokenBudget, tokensSpent: 0 },
    };
    this.jobs.set(id, job);
    const artifactDir = join(this.options.storeDir, id);
    this.set(id, { artifactDir: id });

    const target: Target = real
      ? makeTarget({ name: targetName, baseUrl: input.target ?? "https://openagents.com" })
      : makeTarget({ name: "fixtures", baseUrl: "https://example.test" });

    const evalVariants: ReadonlyArray<EvalVariant> = variants.map((v) => {
      const vScenario = v.scenario ?? "login-regression";
      const backend = () =>
        real
          ? localBackend()
          : localBackend({ chromium: this.mockChromium(vScenario) });
      return {
        id: v.id,
        label: v.label ?? v.id,
        ...(v.note !== undefined ? { note: v.note } : {}),
        brain: () => scriptedBrain(SCENARIO_STEPS[vScenario]()),
        backend,
      };
    });

    this.inflight.set(
      id,
      this.execute(id, async () => {
        await Effect.runPromise(
          runEval({
            id,
            title: input.title ?? `eval ${id}`,
            target,
            scenario: { id: scenario, label: scenario },
            variants: evalVariants,
            repetitions: input.repetitions ?? 1,
            artifactDir,
            ...(this.options.now ? { now: this.options.now } : {}),
          }),
        );
      }),
    );
    return this.get(id);
  }

  // ── status + artifacts ────────────────────────────────────────────────────

  /** Snapshot a job's status (throws NotFoundError if unknown). */
  status(id: string): Job {
    return this.get(id);
  }

  /**
   * Fetch a finished RUN's artifacts: video ref/url, the committed e2e test ref
   * (if the distiller produced one — read-only), result.json (incl. the additive
   * `verify` verdict + `receipt` fields if present), and the /pro/runs/:id link.
   */
  runArtifacts(id: string): RunArtifactsResponse {
    const job = this.get(id);
    if (job.kind !== "run") throw new BadRequestError(`job ${id} is not a run`);
    const dir = join(this.options.storeDir, id);
    const artifacts = readRunArtifacts(dir);
    return {
      jobId: id,
      status: job.status,
      proUrl: `${this.proBaseUrl}/pro/runs/${id}`,
      jobReceipt: job.receipt,
      ...artifacts,
    };
  }

  /** Fetch a finished EVAL's comparison (the eval.json the engine wrote) + /pro link. */
  evalComparison(id: string): EvalComparisonResponse {
    const job = this.get(id);
    if (job.kind !== "eval") throw new BadRequestError(`job ${id} is not an eval`);
    const dir = join(this.options.storeDir, id);
    const comparison = readEvalResult(dir);
    return {
      jobId: id,
      status: job.status,
      proUrl: `${this.proBaseUrl}/pro/evals/${id}`,
      jobReceipt: job.receipt,
      comparison,
    };
  }

  /** Await a job's completion (test helper; the API polls instead). */
  async wait(id: string): Promise<Job> {
    const p = this.inflight.get(id);
    if (p) await p;
    return this.get(id);
  }

  // ── internals ─────────────────────────────────────────────────────────────

  private mockChromium(scenario: ControlScenario) {
    // A deterministic fixture page so the scripted login scenario passes (and the
    // intentionally-wrong variant fails) with NO network. The "-wrong" scenario
    // reuses the same passing page; it fails on its own bad assertion, honestly.
    void scenario;
    return makeFakeChromium({
      pages: {
        "/login": {
          text: "Log in to OpenAgents",
          html: "<form>Log in to OpenAgents</form>",
        },
      },
    });
  }

  private async execute(id: string, body: () => Promise<void>): Promise<void> {
    this.set(id, { status: "running", startedAt: this.now().toISOString() });
    try {
      await body();
      this.set(id, { status: "succeeded", endedAt: this.now().toISOString() });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.set(id, { status: "failed", endedAt: this.now().toISOString(), error: message });
    } finally {
      this.inflight.delete(id);
    }
  }

  private get(id: string): Job {
    const job = this.jobs.get(id);
    if (!job) throw new NotFoundError(`no job ${id}`);
    return job;
  }

  private set(id: string, patch: Partial<Job>): void {
    const job = this.jobs.get(id);
    if (!job) return;
    this.jobs.set(id, { ...job, ...patch });
  }
}

// ---------------------------------------------------------------------------
// Response shapes
// ---------------------------------------------------------------------------

export interface RunArtifactsResponse extends RunArtifacts {
  readonly jobId: string;
  readonly status: JobStatus;
  /** The shareable /pro/runs/:id link a reviewer opens. */
  readonly proUrl: string;
  /**
   * The control plane's honest job receipt (mode/spend/budget). Distinct from
   * `RunArtifacts.receipt`, which is the ADDITIVE run-result receipt read off
   * result.json (owned by receipt.ts).
   */
  readonly jobReceipt: JobReceipt;
}

export interface EvalComparisonResponse {
  readonly jobId: string;
  readonly status: JobStatus;
  readonly proUrl: string;
  readonly jobReceipt: JobReceipt;
  /** The eval.json comparison the engine wrote (null until it lands). */
  readonly comparison: EvalResult | null;
}

// Read an eval.json the engine wrote (read-only; never defines the schema here).
import { existsSync, readFileSync } from "node:fs";
function readEvalResult(dir: string): EvalResult | null {
  const p = join(dir, "eval.json");
  if (!existsSync(p)) return null;
  return JSON.parse(readFileSync(p, "utf8")) as EvalResult;
}
