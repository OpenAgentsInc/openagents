// Eval comparison RENDERERS (#6183): turn an `EvalResult` into a human report.
//
// Two pure renderers (no IO), so the same comparison can be printed to a console
// AND posted into a PR comment (the gh-attach/CI loop, #6185):
//   - `renderEvalConsole`: a compact aligned table for `evals` CLI output.
//   - `renderEvalMarkdown`: the GitHub-flavored markdown the CI loop posts as a
//     PR comment, including the `/pro/evals/<id>` link and slots for the
//     gh-attach'd per-variant videos.
//
// PUBLIC-SAFE + HONEST: both renderers only read the already-public-safe
// `EvalResult`. `not_measured` renders as the literal "not_measured" (never a
// fabricated 0). An illustrative (non-decision-grade) eval is labelled as such.

import type {
  EvalResult,
  EvalVariantDelta,
  EvalVariantMetrics,
  MeasuredNumber,
} from "./evals";
import { isMeasured } from "./evals";

const pct = (rate: number): string => `${Math.round(rate * 100)}%`;

const ms = (value: MeasuredNumber): string =>
  isMeasured(value) ? `${Math.round(value)}ms` : "not_measured";

const signedPct = (delta: number): string => {
  if (delta === 0) return "0%";
  const sign = delta > 0 ? "+" : "";
  return `${sign}${Math.round(delta * 100)}%`;
};

const signedMs = (value: MeasuredNumber): string => {
  if (!isMeasured(value)) return "not_measured";
  if (value === 0) return "0ms";
  const sign = value > 0 ? "+" : "";
  return `${sign}${Math.round(value)}ms`;
};

const deltaFor = (
  deltas: ReadonlyArray<EvalVariantDelta>,
  variantId: string,
): EvalVariantDelta | undefined => deltas.find((d) => d.variantId === variantId);

// ---------------------------------------------------------------------------
// Console table
// ---------------------------------------------------------------------------

const padEnd = (s: string, width: number): string =>
  s.length >= width ? s : s + " ".repeat(width - s.length);

export const renderEvalConsole = (result: EvalResult): string => {
  const lines: string[] = [];
  lines.push(`eval: ${result.title} (${result.id})`);
  lines.push(`scenario: ${result.scenario.label} [${result.scenario.id}]`);
  lines.push(`target: ${result.target.name}  reps: ${result.repetitions}`);
  lines.push(
    `grade: ${result.decisionGrade ? "decision-grade" : "ILLUSTRATIVE (local/fixture no-spend — proves the harness, not lanes)"}`,
  );
  lines.push("");

  const header = [
    padEnd("variant", 22),
    padEnd("axis", 24),
    padEnd("pass", 8),
    padEnd("p50", 12),
    padEnd("p90", 12),
    padEnd("Δpass", 8),
    padEnd("Δp50", 12),
  ].join(" ");
  lines.push(header);
  lines.push("-".repeat(header.length));

  for (const v of result.variants) {
    const d = deltaFor(result.deltas, v.variantId);
    const baseline = v.variantId === result.baselineVariantId;
    lines.push(
      [
        padEnd(`${baseline ? "* " : "  "}${v.label}`, 22),
        padEnd(`${v.axis.kind}:${v.axis.value}`, 24),
        padEnd(`${pct(v.passRate)} (${v.passCount}/${v.runCount})`, 8),
        padEnd(ms(v.latencyP50Ms), 12),
        padEnd(ms(v.latencyP90Ms), 12),
        padEnd(d ? signedPct(d.passRateDelta) : "", 8),
        padEnd(d ? signedMs(d.latencyP50DeltaMs) : "", 12),
      ].join(" "),
    );
  }
  lines.push("");
  lines.push("* = baseline (deltas relative to it)");
  return lines.join("\n");
};

// ---------------------------------------------------------------------------
// Markdown (PR comment body) — #6185 composes the gh-attach video lines + link.
// ---------------------------------------------------------------------------

export interface EvalMarkdownOptions {
  /** Base URL for the operator-console deep link, e.g. "https://openagents.com". */
  readonly proBaseUrl: string;
  /** Optional gh-attach'd video markdown per variant id (uploaded video URL
   *  rendered as an embeddable link/img). When absent, the row shows the
   *  in-eval relative video path so the artifact is still dereferenceable. */
  readonly variantVideoMarkdown?: Readonly<Record<string, string>>;
  /**
   * The SHAREABLE published `/trace/{uuid}` URL (#6210). When present, it is
   * rendered as the headline "Live comparison" link IN PLACE of the old
   * `/pro/evals/<id>` link. Absent when trace publishing is not armed (honest
   * no-op): the comment then falls back to the operator-console deep link.
   */
  readonly traceUrl?: string;
}

const variantRow = (
  v: EvalVariantMetrics,
  d: EvalVariantDelta | undefined,
  baseline: boolean,
): string => {
  const axis = `${v.axis.kind}:${v.axis.value}`;
  const cells = [
    `${baseline ? "**" : ""}${v.label}${baseline ? "** (baseline)" : ""}`,
    axis,
    `${pct(v.passRate)} (${v.passCount}/${v.runCount})`,
    ms(v.latencyP50Ms),
    ms(v.latencyP90Ms),
    d ? signedPct(d.passRateDelta) : "—",
    d ? signedMs(d.latencyP50DeltaMs) : "—",
  ];
  return `| ${cells.join(" | ")} |`;
};

export const renderEvalMarkdown = (
  result: EvalResult,
  options: EvalMarkdownOptions,
): string => {
  const lines: string[] = [];
  // The SHAREABLE link is the published /trace/{uuid} when armed (#6210); else
  // fall back to the operator-console deep link.
  const shareLink =
    options.traceUrl ?? `${options.proBaseUrl.replace(/\/$/, "")}/pro/evals/${result.id}`;

  // Headline: an honest pass/fail summary across variants (no fake green).
  const allPass = result.variants.every((v) => v.passRate === 1);
  const anyFail = result.variants.some((v) => v.passRate < 1);
  const headline = allPass
    ? "✅ all variants passed"
    : anyFail
      ? "⚠️ some variants failed — see deltas"
      : "results below";

  lines.push(`### Chill-eval: ${result.title}`);
  lines.push("");
  lines.push(`${headline} · scenario \`${result.scenario.id}\` · ${result.repetitions} rep(s)`);
  lines.push("");
  if (!result.decisionGrade) {
    lines.push(
      "> _Illustrative run (local/fixture no-spend). Numbers prove the harness, not the lanes._",
    );
    lines.push("");
  }
  lines.push("| variant | axis | pass-rate | p50 | p90 | Δpass | Δp50 |");
  lines.push("| --- | --- | --- | --- | --- | --- | --- |");
  for (const v of result.variants) {
    lines.push(
      variantRow(
        v,
        deltaFor(result.deltas, v.variantId),
        v.variantId === result.baselineVariantId,
      ),
    );
  }
  lines.push("");

  // Per-variant video (gh-attach'd embed when available, else the relative ref).
  const videoMd = options.variantVideoMarkdown ?? {};
  const videoLines: string[] = [];
  for (const v of result.variants) {
    const embedded = videoMd[v.variantId];
    if (embedded !== undefined) {
      videoLines.push(`- **${v.label}**: ${embedded}`);
    } else {
      const firstRunVideo = v.runs.find((r) => r.video !== undefined)?.video;
      if (firstRunVideo !== undefined) {
        videoLines.push(`- **${v.label}**: \`${firstRunVideo}\``);
      }
    }
  }
  if (videoLines.length > 0) {
    lines.push("<details><summary>Videos</summary>");
    lines.push("");
    lines.push(...videoLines);
    lines.push("");
    lines.push("</details>");
    lines.push("");
  }

  lines.push(`▶ **Live comparison:** ${shareLink}`);
  return lines.join("\n");
};
