// Publish a completed QA run as a real shareable `/trace/{uuid}` (#6210, epic
// #6206).
//
// The QA flow already emits an ATIF-v1.7 trajectory from a run (atif.ts). This
// module closes the loop: take a completed run -> ATIF trajectory -> REDACT it
// (TraceRedactor, #6219) -> POST it to the trace ingest API (#6208, agent-bearer
// auth + Idempotency-Key) -> return the `{ uuid, url }` of the stored, public-
// safe `/trace/{uuid}`. The control API (control.ts) and the PR-comment composer
// (pr-comment.ts) then emit that `/trace/{uuid}` URL as the shareable link in
// place of the old `/pro/runs|evals/<id>` links.
//
// DESIGN BOUNDARIES (honored deliberately):
//   - READ-ONLY reuse of atif.ts (the emitter) and redaction.ts (the redactor).
//     This module never redefines the ATIF shape, the result schema, or the
//     redaction rules.
//   - ENV-ARMED, HONEST NO-OP. Publishing touches the network, so it is gated on
//     env (a publish URL + an agent bearer token). When UNCONFIGURED it is a
//     no-op: it logs that it is unarmed and returns `{ published: false }` with a
//     reason — it NEVER fabricates a uuid or a `/trace/...` URL.
//   - REDACT BEFORE PUBLISH. The trajectory is deep-redacted (the TraceRedactor
//     engine, #6219) before it is serialized into the POST body. atif.ts output
//     is already public-safe; this is belt-and-suspenders so no secret can ever
//     reach the wire. A test asserts the posted body carries no secret.
//   - DETERMINISTIC, INJECTABLE TRANSPORT. The HTTP call goes through an
//     injectable `fetch` (defaults to global fetch). Tests pass a FAKE local
//     ingest that records the request and returns a deterministic uuid — no
//     network, no spend.
//   - IDEMPOTENT. The ingest API requires an `Idempotency-Key`; we derive a
//     stable one from the trajectory (or accept an explicit override) so a retry
//     of the same run does not create a duplicate trace.

import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync } from "node:fs";
import { basename, join } from "node:path";
import { Effect } from "effect";

import { type AtifTrajectory, mapKhalaRunToAtif, serializeTrajectory } from "./atif";
import { assertValidAtif } from "./atif-validate";
import { redactValue, type RedactionReport } from "./redaction";
import { decodeQaRunResult, type QaRunResult } from "./result";
import {
  computeDigest,
  decodeSessionTrace,
  type KhalaSessionTrace,
  type SessionBeat,
  SESSION_TRACE_SCHEMA_VERSION,
} from "./session-trace";

// ---------------------------------------------------------------------------
// Transport contract (matches the ingest API, #6208)
// ---------------------------------------------------------------------------

/** A blob reference uploaded to R2 and referenced from the stored trace. */
export interface TraceBlobRef {
  readonly kind: "video" | "screenshot" | "image";
  readonly r2Key: string;
  readonly contentType?: string;
  readonly caption?: string;
}

/** The trace visibility the ingest API accepts (default unlisted). */
export type TraceVisibility = "public" | "unlisted" | "owner_only";

/** A minimal fetch shape so tests can inject a fake ingest with no network. */
export type FetchLike = (
  url: string,
  init: {
    readonly method: string;
    readonly headers: Record<string, string>;
    readonly body: string;
  },
) => Promise<{
  readonly ok: boolean;
  readonly status: number;
  readonly text: () => Promise<string>;
}>;

/**
 * A minimal binary-upload fetch shape for the media-blob bytes (#6223). Separate
 * from `FetchLike` (which posts a JSON string) so tests can inject a fake R2-
 * backed receiver with no network. The body is the raw bytes.
 */
export type BlobFetchLike = (
  url: string,
  init: {
    readonly method: string;
    readonly headers: Record<string, string>;
    readonly body: Uint8Array;
  },
) => Promise<{
  readonly ok: boolean;
  readonly status: number;
  readonly text: () => Promise<string>;
}>;

/**
 * Resolve the raw bytes for one blobRef's `r2Key`. Returns `undefined` when the
 * file is absent (an honest skip — we never claim an upload that did not happen).
 */
export type BlobByteSource = (
  r2Key: string,
) => { readonly bytes: Uint8Array; readonly contentType?: string } | undefined;

// Per-blob upload cap mirrors the worker's R2-backed media cap (#6223).
const MAX_BLOB_UPLOAD_BYTES = 32 * 1024 * 1024;

/** Outcome of uploading the media blob bytes after a trace is stored (#6223). */
export interface BlobUploadReport {
  /** r2Keys whose bytes were uploaded successfully. */
  readonly uploaded: ReadonlyArray<string>;
  /** r2Keys skipped because the source file was absent/too large/unreadable. */
  readonly skipped: ReadonlyArray<{ readonly r2Key: string; readonly reason: string }>;
}

// ---------------------------------------------------------------------------
// Config (env-armed)
// ---------------------------------------------------------------------------

export interface PublishTraceConfig {
  /** Full ingest endpoint, e.g. "https://openagents.com/api/traces". */
  readonly url: string;
  /** The Khala/OpenAgents agent bearer token (`oa_agent_...`). */
  readonly token: string;
  /** Visibility for the stored trace. Default "unlisted". */
  readonly visibility?: TraceVisibility;
}

/**
 * Resolve the publish config from the environment. ARMED only when BOTH a publish
 * URL and an agent token are present. Returns `undefined` (honest no-op) when
 * unconfigured — never a partial/fake config.
 *
 * Env:
 *   QA_TRACE_PUBLISH_URL  — full ingest endpoint (or a base; "/api/traces" is
 *                           appended when the value has no `/api/traces` path).
 *   QA_TRACE_PUBLISH_TOKEN / OPENAGENTS_AGENT_TOKEN / OPENAGENTS_AGENT_PENDING_TOKEN
 *                         — the agent bearer token.
 *   QA_TRACE_PUBLISH_VISIBILITY — "public" | "unlisted" | "owner_only".
 */
export function resolvePublishConfig(
  env: Record<string, string | undefined> = process.env,
): PublishTraceConfig | undefined {
  const rawUrl = env.QA_TRACE_PUBLISH_URL?.trim();
  const token = (
    env.QA_TRACE_PUBLISH_TOKEN ??
    env.OPENAGENTS_AGENT_TOKEN ??
    env.OPENAGENTS_AGENT_PENDING_TOKEN
  )?.trim();
  if (!rawUrl || !token) return undefined;

  const url = normalizeIngestUrl(rawUrl);
  const visibility = normalizeVisibility(env.QA_TRACE_PUBLISH_VISIBILITY);
  return visibility ? { url, token, visibility } : { url, token };
}

/** Append `/api/traces` to a bare base URL; leave a full ingest URL untouched. */
function normalizeIngestUrl(raw: string): string {
  const trimmed = raw.replace(/\/+$/, "");
  if (/\/api\/traces$/.test(trimmed)) return trimmed;
  return `${trimmed}/api/traces`;
}

function normalizeVisibility(value: string | undefined): TraceVisibility | undefined {
  const v = value?.trim();
  return v === "public" || v === "unlisted" || v === "owner_only" ? v : undefined;
}

// ---------------------------------------------------------------------------
// Build a trajectory from a finished run directory
// ---------------------------------------------------------------------------

/**
 * Build an ATIF trajectory from a run dir. `result.json` is REQUIRED (every run
 * writes it). `session-trace.json` is OPTIONAL: a Khala run writes it (and we
 * use it for the structured browser-beat correlation), but a fixed-step run does
 * not — in that case we SYNTHESIZE a minimal, deterministic public-safe trace
 * from the result so the same emitter produces a valid trajectory. The emitted
 * trajectory is validated; an invalid trajectory is never returned.
 */
export function buildTrajectoryFromRunDir(
  runDir: string,
  options: { readonly sessionId?: string } = {},
): AtifTrajectory {
  const resultPath = join(runDir, "result.json");
  if (!existsSync(resultPath)) {
    throw new Error(`missing result.json at ${resultPath}`);
  }
  const result = decodeQaRunResult(JSON.parse(readFileSync(resultPath, "utf8")));

  const tracePath = join(runDir, "session-trace.json");
  const trace = existsSync(tracePath)
    ? decodeSessionTrace(JSON.parse(readFileSync(tracePath, "utf8")))
    : synthesizeTraceFromResult(result);

  const sessionId = options.sessionId ?? basename(runDir);
  const trajectory = mapKhalaRunToAtif({ result, trace, sessionId });
  assertValidAtif(trajectory);
  return trajectory;
}

// The trace's browser action enum (from session-trace.ts).
type BrowserAction = Extract<SessionBeat, { kind: "browser" }>["action"];
const BROWSER_ACTIONS: ReadonlySet<string> = new Set<BrowserAction>([
  "navigate",
  "click",
  "type",
  "wait",
  "screenshot",
  "assert",
  "readText",
]);

/** Map a result step `kind` onto the trace's browser action enum. */
function normalizeBrowserAction(kind: string): BrowserAction {
  if (kind === "waitFor") return "wait";
  return (BROWSER_ACTIONS.has(kind) ? kind : "assert") as BrowserAction;
}

/**
 * Synthesize a minimal, deterministic, public-safe `KhalaSessionTrace` from a
 * result that has no session-trace.json (the fixed-step runner path). Each
 * executed browser-style step becomes one `browser` beat correlated by ORDER —
 * exactly what `mapKhalaRunToAtif` expects. Public-safe by construction: it only
 * copies the public-safe action `kind`/`label`/`status` already on the result.
 */
function synthesizeTraceFromResult(result: QaRunResult): KhalaSessionTrace {
  const beats: SessionBeat[] = result.steps
    .filter((step) => step.kind !== "khala") // the synthetic inference-error step has no beat
    .map((step) => ({
      kind: "browser" as const,
      action: normalizeBrowserAction(step.kind),
      // The result step label IS the public-safe narration/target hint.
      targetHint: step.label,
      status: step.status,
    }));

  return {
    schemaVersion: SESSION_TRACE_SCHEMA_VERSION,
    goal: `Verify ${result.target.name}`,
    target: { name: result.target.name, baseUrl: result.target.baseUrl },
    // One model: own infra. Public-safe (`openagents/...`-class) id.
    model: "openagents/khala",
    beats,
    inputs: [],
    outputs: [],
    receipts: [],
    digest: computeDigest(beats),
  };
}

// ---------------------------------------------------------------------------
// Blob refs from the run artifacts (#6210: video/screenshots via ingest refs)
// ---------------------------------------------------------------------------

/**
 * Derive ingest blob refs from a trajectory's recorded artifacts. The video and
 * screenshots are referenced by their RELATIVE artifact paths as R2 keys (the
 * worker/uploader owns the actual R2 upload; here we only carry the references so
 * the stored trace can point at them). Public-safe: paths are the same relative
 * refs already on the public-safe result.
 */
export function blobRefsFromTrajectory(trajectory: AtifTrajectory): TraceBlobRef[] {
  const artifacts = (trajectory.extra?.["artifacts"] ?? null) as
    | { video?: unknown; screenshots?: unknown }
    | null;
  if (artifacts === null || typeof artifacts !== "object") return [];

  const refs: TraceBlobRef[] = [];
  if (typeof artifacts.video === "string" && artifacts.video.length > 0) {
    refs.push({
      kind: "video",
      r2Key: artifacts.video,
      contentType: artifacts.video.endsWith(".mp4") ? "video/mp4" : "video/webm",
    });
  }
  if (Array.isArray(artifacts.screenshots)) {
    for (const shot of artifacts.screenshots) {
      if (typeof shot === "string" && shot.length > 0) {
        refs.push({ kind: "screenshot", r2Key: shot, contentType: "image/png" });
      }
    }
  }
  return refs;
}

// ---------------------------------------------------------------------------
// Publish
// ---------------------------------------------------------------------------

export interface PublishTraceInput {
  /** The trajectory to publish (already mapped via atif.ts). */
  readonly trajectory: AtifTrajectory;
  /** Resolved publish config. When omitted, resolved from the environment. */
  readonly config?: PublishTraceConfig;
  /** Blob refs (video/screenshots) to associate. Defaults to those on the trajectory. */
  readonly blobRefs?: ReadonlyArray<TraceBlobRef>;
  /** Explicit idempotency key. Defaults to a stable digest of the trajectory. */
  readonly idempotencyKey?: string;
  /** Injectable fetch (defaults to global fetch). Tests pass a fake local ingest. */
  readonly fetch?: FetchLike;
  /**
   * Resolve the raw bytes for a blobRef's `r2Key` (#6223). When provided AND the
   * publish succeeds, each blobRef's bytes are uploaded to
   * `POST /api/traces/{uuid}/blob/{r2Key}` (agent-bearer auth) so `/trace/{uuid}`
   * serves its own media. Honest: a blobRef whose file is absent is SKIPPED, not
   * faked. When omitted, no bytes are uploaded (only the refs are recorded).
   */
  readonly blobSource?: BlobByteSource;
  /** Injectable binary upload fetch (defaults to global fetch). Tests inject a fake. */
  readonly blobFetch?: BlobFetchLike;
  /** Base URL used to render the absolute shareable URL. Default https://openagents.com. */
  readonly shareBaseUrl?: string;
  /** Logger sink (defaults to console.log) for the honest no-op / status line. */
  readonly log?: (message: string) => void;
}

/** A successful publish: the stored trace's uuid + the absolute shareable URL. */
export interface PublishTraceSuccess {
  readonly published: true;
  /** The stored trace uuid. */
  readonly uuid: string;
  /** The absolute shareable URL, e.g. https://openagents.com/trace/{uuid}. */
  readonly url: string;
  /** The ingest visibility of the stored trace. */
  readonly visibility: TraceVisibility;
  /** True when the ingest replayed an idempotent prior store (HTTP 200, not 201). */
  readonly replay: boolean;
  /** The redaction report for the posted body (proof redaction ran). */
  readonly redaction: RedactionReport;
  /**
   * Media-blob upload outcome (#6223). Present only when a `blobSource` was
   * supplied. Honest: lists exactly which blobRefs' bytes were uploaded and
   * which were skipped (and why). Absent => no upload was attempted.
   */
  readonly blobUpload?: BlobUploadReport;
}

/** An honest no-op or failure: never carries a fabricated uuid/url. */
export interface PublishTraceNoop {
  readonly published: false;
  /** Why publishing did not happen / did not succeed. */
  readonly reason: string;
  /** "unconfigured" when env-unarmed; "error" when the ingest call failed. */
  readonly kind: "unconfigured" | "error";
}

export type PublishTraceResult = PublishTraceSuccess | PublishTraceNoop;

/** The response body shape the ingest API returns on a stored trace. */
interface IngestResponseBody {
  readonly uuid?: string;
  readonly url?: string;
  readonly visibility?: string;
  readonly replay?: boolean;
}

/** Derive a stable idempotency key from the (redacted) trajectory content. */
export function idempotencyKeyForTrajectory(trajectory: AtifTrajectory): string {
  const digest = createHash("sha256")
    .update(serializeTrajectory(trajectory))
    .digest("hex")
    .slice(0, 32);
  return `qa-trace-${digest}`;
}

/**
 * Publish a trajectory as a shareable `/trace/{uuid}`. Effect-returning; never
 * throws on a transport failure (returns an honest `{ published: false }`). It:
 *   1. resolves config (env-armed) — honest no-op when unconfigured;
 *   2. DEEP-REDACTS the trajectory (the TraceRedactor engine) before
 *      serialization;
 *   3. POSTs it to the ingest API with the agent bearer token + Idempotency-Key
 *      (+ blob refs);
 *   4. returns the stored `{ uuid, url }`.
 */
export function publishTrace(
  input: PublishTraceInput,
): Effect.Effect<PublishTraceResult> {
  return Effect.suspend(() => {
    const log = input.log ?? ((m: string) => console.log(m));
    const config = input.config ?? resolvePublishConfig();

    if (config === undefined) {
      const reason =
        "trace publishing is not armed: set QA_TRACE_PUBLISH_URL + an agent token " +
        "(QA_TRACE_PUBLISH_TOKEN / OPENAGENTS_AGENT_TOKEN) to publish to /api/traces. " +
        "No trace was published (no uuid fabricated).";
      log(`[publish-trace] NO-OP — ${reason}`);
      return Effect.succeed({
        published: false,
        reason,
        kind: "unconfigured",
      } satisfies PublishTraceNoop);
    }

    return doPublish(input, config, log);
  });
}

function doPublish(
  input: PublishTraceInput,
  config: PublishTraceConfig,
  log: (message: string) => void,
): Effect.Effect<PublishTraceResult> {
  return Effect.gen(function* () {
    // 2. REDACT before publish (belt-and-suspenders over the already-public-safe
    //    emitter output). Deep-walks the whole trajectory; numerics pass through.
    const { value: redacted, report } = redactValue(input.trajectory);

    const idempotencyKey =
      input.idempotencyKey ?? idempotencyKeyForTrajectory(redacted);
    const blobRefs = input.blobRefs ?? blobRefsFromTrajectory(redacted);
    const shareBaseUrl = (input.shareBaseUrl ?? "https://openagents.com").replace(/\/$/, "");

    const body = JSON.stringify({
      trajectory: redacted,
      ...(config.visibility ? { visibility: config.visibility } : {}),
      ...(blobRefs.length > 0 ? { blobRefs } : {}),
    });

    const doFetch = input.fetch ?? (globalThis.fetch as unknown as FetchLike);

    const response = yield* Effect.tryPromise({
      try: () =>
        doFetch(config.url, {
          method: "POST",
          headers: {
            "content-type": "application/json",
            authorization: `Bearer ${config.token}`,
            "idempotency-key": idempotencyKey,
          },
          body,
        }),
      catch: (error) =>
        new PublishTransportError(
          error instanceof Error ? error.message : String(error),
        ),
    });

    const text = yield* Effect.tryPromise({
      try: () => response.text(),
      catch: () => new PublishTransportError("could not read the ingest response body"),
    });

    if (!response.ok) {
      // Honest: surface the status + a bounded body excerpt (the ingest API
      // returns finding CODES only, never echoed secrets).
      return yield* Effect.fail(
        new PublishTransportError(
          `ingest returned HTTP ${response.status}: ${text.slice(0, 300)}`,
        ),
      );
    }

    const parsed = yield* Effect.try({
      try: () => JSON.parse(text) as IngestResponseBody,
      catch: () => new PublishTransportError("ingest response was not valid JSON"),
    });

    const uuid = parsed.uuid;
    if (typeof uuid !== "string" || uuid.length === 0) {
      return yield* Effect.fail(
        new PublishTransportError("ingest response carried no uuid"),
      );
    }

    const visibility =
      normalizeVisibility(parsed.visibility) ?? config.visibility ?? "unlisted";
    const url = `${shareBaseUrl}/trace/${uuid}`;

    // 5. UPLOAD the media-blob bytes (#6223) so `/trace/{uuid}` serves its own
    //    recording + screenshots (never a GitHub attachment). Only when a
    //    blobSource was supplied. Honest: a blobRef whose file is absent is
    //    skipped, not faked. Never throws the publish away on a blob failure —
    //    the trace itself is already stored.
    const blobUpload =
      input.blobSource !== undefined
        ? yield* uploadBlobBytes({
            uuid,
            ingestUrl: config.url,
            token: config.token,
            blobRefs,
            blobSource: input.blobSource,
            ...(input.blobFetch ? { blobFetch: input.blobFetch } : {}),
            log,
          })
        : undefined;

    log(
      `[publish-trace] published ${url} (visibility=${visibility}, ` +
        `redactions=${report.total}, replay=${parsed.replay === true}` +
        (blobUpload !== undefined
          ? `, blobs=${blobUpload.uploaded.length}/${blobUpload.uploaded.length + blobUpload.skipped.length}`
          : "") +
        `)`,
    );
    return {
      published: true,
      uuid,
      url,
      visibility,
      replay: parsed.replay === true,
      redaction: report,
      ...(blobUpload !== undefined ? { blobUpload } : {}),
    } satisfies PublishTraceSuccess;
  }).pipe(
    Effect.catch((error: unknown) => {
      const reason = error instanceof Error ? error.message : String(error);
      log(`[publish-trace] FAILED — ${reason} (no uuid fabricated)`);
      return Effect.succeed({
        published: false,
        reason,
        kind: "error" as const,
      } satisfies PublishTraceNoop);
    }),
  );
}

/**
 * Build the visibility-gated blob URL for one r2Key. `ingestUrl` is the
 * `.../api/traces` endpoint; the blob route is `.../api/traces/{uuid}/blob/{r2Key}`
 * with each r2Key path segment URI-encoded (matches the worker's route + the
 * page's `traceBlobUrl`).
 */
function blobUploadUrl(ingestUrl: string, uuid: string, r2Key: string): string {
  const base = ingestUrl.replace(/\/+$/, "");
  const encodedKey = r2Key.split("/").map(encodeURIComponent).join("/");
  return `${base}/${encodeURIComponent(uuid)}/blob/${encodedKey}`;
}

/**
 * Upload each blobRef's actual bytes to the trace's media blob route (#6223).
 * Honest + bounded: a blobRef whose source file is absent or too large is
 * SKIPPED (recorded in the report), never faked; a per-blob HTTP failure is
 * recorded as a skip and does not fail the whole publish (the trace is already
 * stored). Effect-returning; never throws.
 */
function uploadBlobBytes(args: {
  readonly uuid: string;
  readonly ingestUrl: string;
  readonly token: string;
  readonly blobRefs: ReadonlyArray<TraceBlobRef>;
  readonly blobSource: BlobByteSource;
  readonly blobFetch?: BlobFetchLike;
  readonly log: (message: string) => void;
}): Effect.Effect<BlobUploadReport> {
  return Effect.gen(function* () {
    const doFetch =
      args.blobFetch ?? (globalThis.fetch as unknown as BlobFetchLike);
    const uploaded: string[] = [];
    const skipped: { r2Key: string; reason: string }[] = [];

    for (const ref of args.blobRefs) {
      const resolved = args.blobSource(ref.r2Key);
      if (resolved === undefined) {
        skipped.push({ r2Key: ref.r2Key, reason: "source file absent" });
        continue;
      }
      if (resolved.bytes.byteLength === 0) {
        skipped.push({ r2Key: ref.r2Key, reason: "source file empty" });
        continue;
      }
      if (resolved.bytes.byteLength > MAX_BLOB_UPLOAD_BYTES) {
        skipped.push({
          r2Key: ref.r2Key,
          reason: `exceeds ${MAX_BLOB_UPLOAD_BYTES}-byte cap (${resolved.bytes.byteLength} bytes)`,
        });
        continue;
      }

      const contentType = ref.contentType ?? resolved.contentType;
      const response = yield* Effect.tryPromise({
        try: () =>
          doFetch(blobUploadUrl(args.ingestUrl, args.uuid, ref.r2Key), {
            method: "POST",
            headers: {
              authorization: `Bearer ${args.token}`,
              ...(contentType ? { "content-type": contentType } : {}),
            },
            body: resolved.bytes,
          }),
        catch: (error) =>
          new PublishTransportError(
            error instanceof Error ? error.message : String(error),
          ),
      }).pipe(Effect.catch(() => Effect.succeed(undefined)));

      if (response === undefined) {
        skipped.push({ r2Key: ref.r2Key, reason: "upload transport error" });
        continue;
      }
      if (!response.ok) {
        skipped.push({
          r2Key: ref.r2Key,
          reason: `upload returned HTTP ${response.status}`,
        });
        continue;
      }
      uploaded.push(ref.r2Key);
    }

    if (skipped.length > 0) {
      args.log(
        `[publish-trace] blob upload: ${uploaded.length} uploaded, ${skipped.length} skipped ` +
          `(${skipped.map((s) => `${s.r2Key}: ${s.reason}`).join("; ")})`,
      );
    }
    return { uploaded, skipped };
  });
}

/** A transport/HTTP failure during publish. Mapped to an honest no-op. */
export class PublishTransportError extends Error {
  readonly _tag = "PublishTransportError";
  constructor(reason: string) {
    super(`publish_transport_error: ${reason}`);
    this.name = "PublishTransportError";
  }
}

// ---------------------------------------------------------------------------
// Convenience: publish straight from a run directory
// ---------------------------------------------------------------------------

export interface PublishRunDirInput
  extends Omit<PublishTraceInput, "trajectory" | "blobRefs"> {
  readonly runDir: string;
  readonly sessionId?: string;
}

/** Best-effort content type for a media artifact path by extension. */
function contentTypeForArtifact(r2Key: string): string | undefined {
  if (r2Key.endsWith(".mp4")) return "video/mp4";
  if (r2Key.endsWith(".webm")) return "video/webm";
  if (r2Key.endsWith(".png")) return "image/png";
  if (r2Key.endsWith(".jpg") || r2Key.endsWith(".jpeg")) return "image/jpeg";
  if (r2Key.endsWith(".webp")) return "image/webp";
  if (r2Key.endsWith(".gif")) return "image/gif";
  return undefined;
}

/**
 * A `BlobByteSource` that reads a blobRef's bytes from a run directory (#6223).
 * The r2Key is the public-safe RELATIVE artifact path (e.g. `session.mp4`,
 * `shots/00-login.png`) — resolved against the run dir. Honest: a missing or
 * unreadable file resolves to `undefined` (a skip), never fabricated bytes.
 * Path-safe: a key that escapes the run dir (absolute or `..`) is refused.
 */
export function runDirBlobSource(runDir: string): BlobByteSource {
  return (r2Key) => {
    if (r2Key.startsWith("/") || r2Key.split("/").includes("..")) {
      return undefined;
    }
    const filePath = join(runDir, r2Key);
    try {
      if (!existsSync(filePath) || !statSync(filePath).isFile()) {
        return undefined;
      }
      const bytes = new Uint8Array(readFileSync(filePath));
      const contentType = contentTypeForArtifact(r2Key);
      return contentType === undefined ? { bytes } : { bytes, contentType };
    } catch {
      return undefined;
    }
  };
}

/**
 * Build a trajectory from a run dir and publish it. The same env-armed / honest
 * no-op rules apply. By default the media-blob bytes are read from the run dir
 * and uploaded after publish (#6223) so `/trace/{uuid}` serves its own
 * recording + screenshots; pass an explicit `blobSource` to override.
 */
export function publishRunDir(
  input: PublishRunDirInput,
): Effect.Effect<PublishTraceResult> {
  return Effect.gen(function* () {
    const trajectory = buildTrajectoryFromRunDir(input.runDir, {
      ...(input.sessionId ? { sessionId: input.sessionId } : {}),
    });
    const blobSource = input.blobSource ?? runDirBlobSource(input.runDir);
    return yield* publishTrace({
      trajectory,
      blobSource,
      ...(input.config ? { config: input.config } : {}),
      ...(input.idempotencyKey ? { idempotencyKey: input.idempotencyKey } : {}),
      ...(input.fetch ? { fetch: input.fetch } : {}),
      ...(input.blobFetch ? { blobFetch: input.blobFetch } : {}),
      ...(input.shareBaseUrl ? { shareBaseUrl: input.shareBaseUrl } : {}),
      ...(input.log ? { log: input.log } : {}),
    });
  });
}
