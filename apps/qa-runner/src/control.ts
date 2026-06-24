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
import {
  type FetchLike,
  type PublishTraceConfig,
  type PublishTraceResult,
  publishRunDir,
} from "./publish-trace";
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
  /**
   * The shareable `https://openagents.com/trace/{uuid}` URL, set once the run's
   * trace is PUBLISHED to the ingest API (#6210). Absent when trace publishing is
   * not armed (honest no-op — no fabricated uuid). This supersedes the old
   * `/pro/runs|evals/<id>` link as the shareable artifact.
   */
  readonly traceUrl?: string;
  /** Why a trace was not published (honest no-op / failure reason), when absent. */
  readonly traceNote?: string;
  /**
   * For an EVAL: the published `/trace/{uuid}` per variant id (the comparison is
   * a view over these trace uuids — the sibling web lane owns that view). Absent
   * when publishing is not armed.
   */
  readonly variantTraceUrls?: Readonly<Record<string, string>>;
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
  /**
   * Trace publishing (#6210). When set, a completed job's ATIF trace is REDACTED
   * and published to the ingest API, and the resulting `/trace/{uuid}` becomes
   * the job's shareable link. When omitted, publishing falls back to the
   * environment (`QA_TRACE_PUBLISH_URL` + an agent token); honest no-op when
   * unarmed (no fabricated uuid). Injectable for deterministic, no-network tests.
   */
  readonly publishTrace?: PublishTraceConfig;
  /** Injectable fetch for the trace publish call (deterministic fake in tests). */
  readonly publishFetch?: FetchLike;
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
        // Publish the run's trace -> /trace/{uuid} (env-armed; honest no-op).
        await this.publishTraceForJob(id, artifactDir);
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
        // Publish each variant's representative trace -> /trace/{uuid}. The
        // comparison is a view over these uuids (env-armed; honest no-op).
        await this.publishTracesForEvalJob(
          id,
          artifactDir,
          variants.map((v) => v.id),
          variants[0]!.id,
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
   * `verify` verdict + `receipt` fields if present), and the SHAREABLE
   * `/trace/{uuid}` link (#6210). The `/pro/runs/:id` operator-console deep link
   * is retained for the logged-in console, but the link a reviewer SHARES is now
   * the published `/trace/{uuid}` (`traceUrl`); it is `null` with a `traceNote`
   * when trace publishing is not armed (honest no-op — no fabricated uuid).
   */
  runArtifacts(id: string): RunArtifactsResponse {
    const job = this.get(id);
    if (job.kind !== "run") throw new BadRequestError(`job ${id} is not a run`);
    const dir = join(this.options.storeDir, id);
    const artifacts = readRunArtifacts(dir);
    return {
      jobId: id,
      status: job.status,
      // The SHAREABLE link is the published /trace/{uuid} (null when unarmed).
      traceUrl: job.traceUrl ?? null,
      ...(job.traceNote !== undefined ? { traceNote: job.traceNote } : {}),
      // Retained operator-console deep link (/pro stays the logged-in console).
      proUrl: `${this.proBaseUrl}/pro/runs/${id}`,
      jobReceipt: job.receipt,
      ...artifacts,
    };
  }

  /**
   * Fetch a finished EVAL's comparison (the eval.json the engine wrote) + the
   * SHAREABLE `/trace/{uuid}` links (#6210). The comparison is a view over the
   * per-variant trace uuids (`variantTraceUrls`); `traceUrl` is the baseline
   * variant's trace. Both are absent/`null` with a `traceNote` when publishing is
   * not armed. The `/pro/evals/:id` operator-console deep link is retained.
   */
  evalComparison(id: string): EvalComparisonResponse {
    const job = this.get(id);
    if (job.kind !== "eval") throw new BadRequestError(`job ${id} is not an eval`);
    const dir = join(this.options.storeDir, id);
    const comparison = readEvalResult(dir);
    return {
      jobId: id,
      status: job.status,
      traceUrl: job.traceUrl ?? null,
      ...(job.variantTraceUrls !== undefined ? { variantTraceUrls: job.variantTraceUrls } : {}),
      ...(job.traceNote !== undefined ? { traceNote: job.traceNote } : {}),
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

  /** Common publish input wiring (env/config + injectable fetch). */
  private publishCommon(): {
    config?: PublishTraceConfig;
    fetch?: FetchLike;
    shareBaseUrl: string;
  } {
    return {
      ...(this.options.publishTrace ? { config: this.options.publishTrace } : {}),
      ...(this.options.publishFetch ? { fetch: this.options.publishFetch } : {}),
      shareBaseUrl: this.proBaseUrl,
    };
  }

  /**
   * Publish a finished RUN's trace -> /trace/{uuid} and record it on the job. The
   * publish module is env-armed; an unarmed/failed publish is an HONEST NO-OP:
   * `traceUrl` stays unset and the job carries a `traceNote` (never a fake uuid).
   */
  private async publishTraceForJob(id: string, runDir: string): Promise<void> {
    const result = await Effect.runPromise(
      publishRunDir({ runDir, sessionId: id, ...this.publishCommon() }),
    );
    this.recordTraceResult(id, result);
    // UPGRADE the run receipt's execution-trace evidence (#6216): on a published
    // trace, point `traceRef` at the published `/trace/{uuid}` uuid. When the
    // publish was an honest no-op, the receipt keeps its honest local
    // trajectory_id fallback (written by writeReceiptForRun above). Idempotent.
    if (result.published) {
      writeReceiptForRun(runDir, { traceRef: result.uuid });
    }
  }

  /**
   * Publish each EVAL variant's representative run (`<variantId>.0`) as a trace.
   * The comparison view (sibling web lane) reads these uuids. The baseline
   * variant's trace becomes the eval's primary `traceUrl`. Honest no-op when
   * unarmed.
   */
  private async publishTracesForEvalJob(
    id: string,
    evalDir: string,
    variantIds: ReadonlyArray<string>,
    baselineId: string,
  ): Promise<void> {
    const variantTraceUrls: Record<string, string> = {};
    let baselineResult: PublishTraceResult | undefined;
    let lastNote: string | undefined;

    for (const variantId of variantIds) {
      const runDir = join(evalDir, `${variantId}.0`);
      const result = await Effect.runPromise(
        publishRunDir({
          runDir,
          sessionId: `${id}-${variantId}`,
          ...this.publishCommon(),
        }),
      );
      if (result.published) {
        variantTraceUrls[variantId] = result.url;
      } else {
        lastNote = result.reason;
      }
      if (variantId === baselineId) baselineResult = result;
    }

    if (Object.keys(variantTraceUrls).length > 0) {
      this.set(id, { variantTraceUrls });
    }
    if (baselineResult !== undefined) {
      this.recordTraceResult(id, baselineResult);
    } else if (lastNote !== undefined) {
      this.set(id, { traceNote: lastNote });
    }
  }

  /** Record a publish result onto a job: the trace URL on success, else a note. */
  private recordTraceResult(id: string, result: PublishTraceResult): void {
    if (result.published) {
      this.set(id, { traceUrl: result.url });
    } else {
      this.set(id, { traceNote: result.reason });
    }
  }

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
  /**
   * The SHAREABLE link a reviewer opens: the published `/trace/{uuid}` (#6210).
   * `null` when trace publishing is not armed (see `traceNote`) — never a fake
   * uuid. This supersedes `/pro/runs/:id` as the shareable artifact.
   */
  readonly traceUrl: string | null;
  /** Why a trace was not published (honest no-op / failure reason), when `traceUrl` is null. */
  readonly traceNote?: string;
  /** The operator-console deep link (the logged-in `/pro` console; not shared). */
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
  /**
   * The SHAREABLE link: the baseline variant's published `/trace/{uuid}` (#6210).
   * `null` when publishing is not armed. The comparison is a view over
   * `variantTraceUrls`. Supersedes `/pro/evals/:id` as the shareable artifact.
   */
  readonly traceUrl: string | null;
  /** Per-variant published `/trace/{uuid}` links (the comparison's trace set). */
  readonly variantTraceUrls?: Readonly<Record<string, string>>;
  /** Why traces were not published (honest no-op / failure reason), when unarmed. */
  readonly traceNote?: string;
  /** The operator-console deep link (the logged-in `/pro` console; not shared). */
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
