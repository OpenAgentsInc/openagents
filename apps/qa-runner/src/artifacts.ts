// Read a finished run's public-safe artifacts for the control API (#6196).
//
// This is a READ-ONLY view over what the runner + post-run helpers already
// wrote to a run's artifact dir. It NEVER defines or mutates a schema:
//   - it reads result.json as written by runner.ts;
//   - it surfaces the ADDITIVE `verify` verdict (a peer lane owns it) and the
//     ADDITIVE `receipt` (already landed, owned by receipt.ts) IF PRESENT,
//     treating both as optional, read-only passthrough fields;
//   - it points at the playable video, the Playwright trace, the per-step
//     screenshots, and the committed e2e test the distiller emitted (if any).
//
// Honesty: a field that is not on disk is `null`/absent, never fabricated. The
// committed-test ref is the relative path under `generated/` the distiller
// writes; we only report it when the file actually exists.

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

/** The additive verify verdict a peer lane attaches to result.json (#6192). */
export type VerifyVerdict = "CONFIRMED" | "REFUTED" | "INCONCLUSIVE";

/**
 * A read-only projection of a run's artifacts. All paths are RELATIVE to the
 * run's artifact dir (so the /pro side resolves them); `result` is the parsed
 * result.json passed through untouched (so additive fields a peer lane adds
 * flow through without this module knowing their full shape).
 */
export interface RunArtifacts {
  /** Relative path to the playable video (mp4/webm), if captured. */
  readonly video: string | null;
  readonly videoFormat: "mp4" | "webm" | null;
  /** Relative path to the Playwright trace zip, if captured. */
  readonly trace: string | null;
  /** Relative paths to per-step screenshots. */
  readonly screenshots: ReadonlyArray<string>;
  /** Relative path to the committed e2e test the distiller emitted, if any. */
  readonly committedTest: string | null;
  /** The parsed result.json (public-safe), or null if not yet written. */
  readonly result: Record<string, unknown> | null;
  /**
   * The additive `verify` verdict read off result.json if a peer lane wrote it.
   * READ-ONLY passthrough; this module does not define or compute it.
   */
  readonly verify: VerifyVerdict | null;
  /** The additive `receipt` read off result.json if present (landed). */
  readonly receipt: Record<string, unknown> | null;
}

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;

/**
 * Read the additive `verify` verdict off a parsed result, defensively. A peer
 * lane owns the field's exact shape; we accept either a bare string verdict or
 * an object carrying a `verdict`/`status` string, and only return a known
 * verdict literal. Anything else is `null` (honest: we did not read a verdict).
 */
function readVerifyVerdict(result: Record<string, unknown> | null): VerifyVerdict | null {
  if (!result) return null;
  const verify = result["verify"];
  const candidate =
    typeof verify === "string"
      ? verify
      : asRecord(verify)
        ? ((asRecord(verify)!["verdict"] ?? asRecord(verify)!["status"]) as unknown)
        : null;
  if (candidate === "CONFIRMED" || candidate === "REFUTED" || candidate === "INCONCLUSIVE") {
    return candidate;
  }
  return null;
}

/** The relative committed-test path under `generated/`, if exactly one exists. */
function findCommittedTest(dir: string): string | null {
  // The distiller writes generated/<slug>.e2e.test.ts at the repo's app root,
  // not per-run; a run's artifact dir may also carry a committed-test ref via a
  // sidecar. Report a per-run `committed-test.ts`/`.e2e.test.ts` if present.
  if (!existsSync(dir)) return null;
  const candidates = readdirSync(dir).filter(
    (f) => f.endsWith(".e2e.test.ts") || f === "committed-test.ts",
  );
  const found = candidates.sort()[0];
  return found ? found : null;
}

/** Read a run's public-safe artifacts from its artifact dir. Read-only. */
export function readRunArtifacts(dir: string): RunArtifacts {
  const resultPath = join(dir, "result.json");
  const result = existsSync(resultPath)
    ? asRecord(JSON.parse(readFileSync(resultPath, "utf8")))
    : null;

  const artifacts = result ? asRecord(result["artifacts"]) : null;
  const video = artifacts && typeof artifacts["video"] === "string" ? (artifacts["video"] as string) : null;
  const videoFormat =
    artifacts && (artifacts["videoFormat"] === "mp4" || artifacts["videoFormat"] === "webm")
      ? (artifacts["videoFormat"] as "mp4" | "webm")
      : null;
  const trace = artifacts && typeof artifacts["trace"] === "string" ? (artifacts["trace"] as string) : null;
  const screenshots =
    artifacts && Array.isArray(artifacts["screenshots"])
      ? (artifacts["screenshots"] as string[]).filter((s): s is string => typeof s === "string")
      : [];

  return {
    video,
    videoFormat,
    trace,
    screenshots,
    committedTest: findCommittedTest(dir),
    result,
    verify: readVerifyVerdict(result),
    receipt: result ? asRecord(result["receipt"]) : null,
  };
}
