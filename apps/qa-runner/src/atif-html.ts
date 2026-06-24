// Beautiful, self-contained HTML trace viewer for an ATIF-v1.7 `Trajectory`.
//
// Styled to OpenAgents' `apps/openagents.com/DESIGN.md`: dark, pure black,
// warm off-white (#f1efe8), Commit Mono, a command-surface / timeline aesthetic
// (registers, strips, status rows — NOT cards). The render is ONE static HTML
// file with inline CSS + a tiny vanilla-JS enhancement (collapsible reasoning,
// screenshot lightbox). Artifacts (video + screenshots) are referenced by
// RELATIVE path so the file is viewable in place next to the run artifacts.
//
// It shows:
//   - a header strip: agent, model openagents/khala, verdict (PASS/REFUTED/
//     INCONCLUSIVE with the semantic accent), duration, cost $0
//   - a vertical step timeline: the user goal, then each agent step with its
//     narration, collapsible reasoning, the tool call + its args, the
//     observation result, and screenshot thumbnails inline
//   - the embedded, playable video
//   - final metrics
//
// No framework, no network, no build step: open the file and it renders + plays.

import type { AtifStep, AtifToolCall, AtifTrajectory, AtifVerdict, Json } from "./atif";

export interface RenderTraceOptions {
  /** Relative path to the playable video (mp4/webm), e.g. "session.mp4". */
  readonly video?: string;
  readonly videoFormat?: "mp4" | "webm";
  /** Relative paths to per-step screenshots, e.g. ["login-page.png"]. */
  readonly screenshots?: ReadonlyArray<string>;
  /** Page title. */
  readonly title?: string;
}

// --- HTML escaping (the only safety primitive a static renderer needs) -------

function esc(value: unknown): string {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function verdictOf(t: AtifTrajectory): AtifVerdict {
  const v = t.final_metrics?.extra?.verdict;
  if (v === "PASS" || v === "REFUTED" || v === "INCONCLUSIVE") return v;
  return "INCONCLUSIVE";
}

function verdictAccent(verdict: AtifVerdict): string {
  return verdict === "PASS"
    ? "var(--oa-positive)"
    : verdict === "REFUTED"
      ? "var(--oa-negative)"
      : "var(--oa-warning)";
}

function fmtDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return "0s";
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)}s`;
  const m = Math.floor(s / 60);
  return `${m}m ${Math.round(s % 60)}s`;
}

function fmtTime(iso?: string): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return esc(iso);
  return d.toISOString().replace("T", " ").replace(/\.\d+Z$/, "Z");
}

// Glyph for a tool/function name (kept inside the mono palette — no icon font).
function toolGlyph(fn: string): string {
  switch (fn) {
    case "navigate":
      return "→";
    case "click":
      return "✦";
    case "type":
      return "⌨";
    case "readText":
      return "≣";
    case "waitFor":
      return "◷";
    case "screenshot":
      return "▣";
    case "assert":
      return "✓";
    case "done":
      return "■";
    case "error":
      return "✕";
    default:
      return "•";
  }
}

function argsTable(args: Record<string, Json>): string {
  const rows = Object.entries(args)
    .map(([k, v]) => {
      const value = typeof v === "string" ? v : JSON.stringify(v);
      return `<div class="kv"><span class="k">${esc(k)}</span><span class="v">${esc(value)}</span></div>`;
    })
    .join("");
  return `<div class="args">${rows}</div>`;
}

function toolCallBlock(tc: AtifToolCall): string {
  return `
    <div class="toolcall">
      <div class="tool-head">
        <span class="tool-glyph">${esc(toolGlyph(tc.function_name))}</span>
        <span class="tool-name">${esc(tc.function_name)}</span>
        <span class="tool-id">${esc(tc.tool_call_id)}</span>
      </div>
      ${argsTable(tc.arguments)}
    </div>`;
}

function obsBlock(step: AtifStep): string {
  if (!step.observation || step.observation.results.length === 0) return "";
  const rows = step.observation.results
    .map((r) => {
      const content = typeof r.content === "string" ? r.content : JSON.stringify(r.content ?? "");
      const failed = content.startsWith("FAILED");
      return `<div class="obs-row${failed ? " obs-failed" : ""}">
        <span class="obs-arrow">⟵</span>
        <span class="obs-content">${esc(content)}</span>
      </div>`;
    })
    .join("");
  return `<div class="obs">${rows}</div>`;
}

function reasoningBlock(step: AtifStep): string {
  if (!step.reasoning_content) return "";
  return `
    <details class="reasoning">
      <summary>reasoning</summary>
      <pre>${esc(step.reasoning_content)}</pre>
    </details>`;
}

// Match screenshots to a step by the action's target/label (best-effort, neutral).
function screenshotsForStep(step: AtifStep, screenshots: ReadonlyArray<string>): string[] {
  const tc = step.tool_calls?.[0];
  if (!tc || tc.function_name !== "screenshot") return [];
  const label = typeof tc.arguments.target === "string" ? tc.arguments.target : "";
  const hit = screenshots.filter((s) => label && s.includes(label));
  return hit.length > 0 ? hit : [];
}

function thumbBlock(paths: ReadonlyArray<string>): string {
  if (paths.length === 0) return "";
  const thumbs = paths
    .map(
      (p) =>
        `<a class="thumb" href="${esc(p)}" target="_blank" rel="noopener">
           <img src="${esc(p)}" alt="${esc(p)}" loading="lazy" />
           <span class="thumb-label">${esc(p)}</span>
         </a>`,
    )
    .join("");
  return `<div class="thumbs">${thumbs}</div>`;
}

function stepBlock(step: AtifStep, screenshots: ReadonlyArray<string>): string {
  if (step.source === "user") {
    return `
      <li class="step step-user">
        <div class="rail"><span class="dot dot-user"></span></div>
        <div class="step-body">
          <div class="step-head">
            <span class="step-id">#${step.step_id}</span>
            <span class="src src-user">user · goal</span>
            <span class="step-time">${fmtTime(step.timestamp)}</span>
          </div>
          <p class="goal">${esc(typeof step.message === "string" ? step.message : JSON.stringify(step.message))}</p>
        </div>
      </li>`;
  }

  const tc = step.tool_calls?.[0];
  const isDone = tc?.function_name === "done";
  const isError = tc?.function_name === "error";
  const dotClass = isDone ? "dot-done" : isError ? "dot-error" : "dot-agent";
  const shots = screenshotsForStep(step, screenshots);
  const message = typeof step.message === "string" ? step.message : JSON.stringify(step.message);

  return `
    <li class="step step-agent${isDone ? " step-done" : ""}">
      <div class="rail"><span class="dot ${dotClass}"></span></div>
      <div class="step-body">
        <div class="step-head">
          <span class="step-id">#${step.step_id}</span>
          <span class="src src-agent">agent</span>
          ${step.model_name ? `<span class="model">${esc(step.model_name)}</span>` : ""}
          ${tc ? `<span class="action-chip">${esc(toolGlyph(tc.function_name))} ${esc(tc.function_name)}</span>` : ""}
          <span class="step-time">${fmtTime(step.timestamp)}</span>
        </div>
        <p class="narration">${esc(message)}</p>
        ${reasoningBlock(step)}
        ${tc ? toolCallBlock(tc) : ""}
        ${obsBlock(step)}
        ${thumbBlock(shots)}
      </div>
    </li>`;
}

const STYLE = `
:root{
  --oa-bg:#000000; --oa-panel:#010102; --oa-panel-active:#141414; --oa-hover:#080808;
  --oa-border-subtle:#222222; --oa-border-active:#333333;
  --oa-text:#f1efe8; --oa-text-strong:rgba(255,255,255,.9);
  --oa-text-muted:rgba(255,255,255,.6); --oa-text-faint:rgba(255,255,255,.35);
  --oa-highlight:#ffb400; --oa-positive:#00c853; --oa-negative:#d32f2f;
  --oa-warning:#ff6f00; --oa-info:#2979ff;
  --mono:"Commit Mono","Berkeley Mono",ui-monospace,SFMono-Regular,Menlo,monospace;
}
*{box-sizing:border-box}
html,body{margin:0;background:var(--oa-bg);color:var(--oa-text);
  font-family:var(--mono);font-size:14px;line-height:1.55;letter-spacing:.01em;
  -webkit-font-smoothing:antialiased}
a{color:inherit}
.wrap{max-width:1040px;margin:0 auto;padding:0 20px 96px}

/* ---- header strip (command surface, not a card) ---- */
.masthead{position:sticky;top:0;z-index:30;background:linear-gradient(180deg,#000 60%,rgba(0,0,0,.85));
  border-bottom:1px solid var(--oa-border-subtle);backdrop-filter:blur(6px)}
.masthead-inner{max-width:1040px;margin:0 auto;padding:14px 20px}
.title-row{display:flex;align-items:baseline;gap:10px;flex-wrap:wrap}
.title-row h1{font-size:15px;font-weight:600;margin:0;color:var(--oa-text-strong);letter-spacing:.02em}
.title-row .sub{color:var(--oa-text-faint);font-size:12px}
.strip{display:flex;flex-wrap:wrap;gap:0;margin-top:12px;border:1px solid var(--oa-border-subtle)}
.cell{display:flex;flex-direction:column;gap:3px;padding:8px 14px;min-width:120px;
  border-right:1px solid var(--oa-border-subtle)}
.cell:last-child{border-right:0}
.cell .lbl{font-size:10px;text-transform:uppercase;letter-spacing:.12em;color:var(--oa-text-faint)}
.cell .val{font-size:13px;color:var(--oa-text-strong);font-variant-numeric:tabular-nums}
.verdict{font-weight:600;letter-spacing:.04em}
.verdict::before{content:"";display:inline-block;width:7px;height:7px;border-radius:50%;
  margin-right:7px;vertical-align:middle;background:var(--accent);box-shadow:0 0 10px var(--accent)}

/* ---- section labels ---- */
.section-label{display:flex;align-items:center;gap:10px;margin:40px 0 14px;
  font-size:10px;text-transform:uppercase;letter-spacing:.16em;color:var(--oa-text-faint)}
.section-label::after{content:"";flex:1;height:1px;background:var(--oa-border-subtle)}

/* ---- video ---- */
.video-frame{border:1px solid var(--oa-border-subtle);background:#000}
.video-frame video{display:block;width:100%;max-height:560px;background:#000}
.video-empty{padding:18px;color:var(--oa-text-faint);font-size:12px;border:1px dashed var(--oa-border-subtle)}

/* ---- timeline ---- */
ol.timeline{list-style:none;margin:0;padding:0}
.step{display:grid;grid-template-columns:28px 1fr;gap:0}
.rail{position:relative;display:flex;justify-content:center}
.rail::before{content:"";position:absolute;top:0;bottom:0;width:1px;background:var(--oa-border-subtle)}
.step:first-child .rail::before{top:18px}
.step:last-child .rail::before{bottom:auto;height:18px}
.dot{position:relative;margin-top:14px;width:9px;height:9px;border-radius:50%;
  background:var(--oa-panel-active);border:1px solid var(--oa-border-active);z-index:1}
.dot-user{background:var(--oa-info);border-color:var(--oa-info);box-shadow:0 0 8px rgba(41,121,255,.5)}
.dot-agent{background:var(--oa-text-faint);border-color:var(--oa-border-active)}
.dot-done{background:var(--oa-positive);border-color:var(--oa-positive);box-shadow:0 0 8px rgba(0,200,83,.5)}
.dot-error{background:var(--oa-negative);border-color:var(--oa-negative);box-shadow:0 0 8px rgba(211,47,47,.5)}
.step-body{padding:8px 0 16px 14px;min-width:0}
.step-head{display:flex;align-items:center;gap:10px;flex-wrap:wrap;font-size:11px}
.step-id{color:var(--oa-text-faint);font-variant-numeric:tabular-nums}
.src{text-transform:lowercase;letter-spacing:.04em;font-weight:600}
.src-user{color:var(--oa-info)}
.src-agent{color:var(--oa-text-muted)}
.model{color:var(--oa-text-faint)}
.action-chip{margin-left:auto;padding:1px 8px;border:1px solid var(--oa-border-active);
  border-radius:2px;color:var(--oa-text-strong);font-size:11px}
.step-time{color:var(--oa-text-faint);font-variant-numeric:tabular-nums;font-size:10px}
.step-head .step-time{margin-left:8px}
.goal{margin:8px 0 0;color:var(--oa-text);font-size:14px;max-width:75ch}
.narration{margin:8px 0 0;color:var(--oa-text-strong);max-width:78ch}
.step-done .narration{color:var(--oa-positive)}

/* reasoning (collapsible) */
details.reasoning{margin:10px 0 0}
details.reasoning summary{cursor:pointer;color:var(--oa-text-faint);font-size:11px;
  letter-spacing:.08em;text-transform:uppercase;list-style:none;user-select:none}
details.reasoning summary::-webkit-details-marker{display:none}
details.reasoning summary::before{content:"▸ ";color:var(--oa-text-faint)}
details.reasoning[open] summary::before{content:"▾ "}
details.reasoning pre{margin:8px 0 0;padding:10px 12px;background:var(--oa-panel-active);
  border:1px solid var(--oa-border-subtle);white-space:pre-wrap;word-break:break-word;
  color:var(--oa-text-muted);font-size:12.5px;max-width:80ch}

/* tool call */
.toolcall{margin:10px 0 0;border:1px solid var(--oa-border-subtle);background:var(--oa-panel)}
.tool-head{display:flex;align-items:center;gap:10px;padding:7px 12px;border-bottom:1px solid var(--oa-border-subtle);
  background:var(--oa-hover)}
.tool-glyph{color:var(--oa-highlight)}
.tool-name{font-weight:600;color:var(--oa-text-strong)}
.tool-id{margin-left:auto;color:var(--oa-text-faint);font-size:11px}
.args{padding:4px 0}
.kv{display:grid;grid-template-columns:120px 1fr;gap:12px;padding:4px 12px;font-size:12.5px}
.kv .k{color:var(--oa-text-faint)}
.kv .v{color:var(--oa-text);word-break:break-word}

/* observation */
.obs{margin:10px 0 0;display:flex;flex-direction:column;gap:4px}
.obs-row{display:grid;grid-template-columns:18px 1fr;gap:8px;padding:6px 12px;font-size:12.5px;
  border-left:1px solid var(--oa-border-active);color:var(--oa-text-muted)}
.obs-arrow{color:var(--oa-text-faint)}
.obs-failed{color:var(--oa-negative);border-left-color:var(--oa-negative)}
.obs-failed .obs-arrow{color:var(--oa-negative)}

/* screenshots */
.thumbs{display:flex;flex-wrap:wrap;gap:10px;margin:12px 0 0}
.thumb{display:flex;flex-direction:column;gap:4px;text-decoration:none;width:200px}
.thumb img{width:200px;height:auto;border:1px solid var(--oa-border-subtle);background:#000;
  transition:border-color .18s ease,transform .18s ease}
.thumb:hover img{border-color:var(--oa-highlight);transform:translateY(-1px)}
.thumb-label{font-size:10px;color:var(--oa-text-faint);word-break:break-all}

/* final metrics */
.metrics{display:flex;flex-wrap:wrap;gap:0;border:1px solid var(--oa-border-subtle)}
.metrics .cell{flex:1;min-width:140px}
footer{margin-top:48px;padding-top:16px;border-top:1px solid var(--oa-border-subtle);
  color:var(--oa-text-faint);font-size:11px;display:flex;justify-content:space-between;flex-wrap:wrap;gap:8px}

@media (max-width:640px){
  .kv{grid-template-columns:90px 1fr}
  .cell{min-width:96px}
}
@media (prefers-reduced-motion:reduce){
  .thumb img{transition:none}
}
`;

/** Render a complete, self-contained HTML trace viewer from a trajectory. */
export function renderTraceHtml(trajectory: AtifTrajectory, options: RenderTraceOptions = {}): string {
  const verdict = verdictOf(trajectory);
  const accent = verdictAccent(verdict);
  const target = (trajectory.extra?.target ?? {}) as { name?: string; baseUrl?: string };
  const durationMs = Number(trajectory.final_metrics?.extra?.duration_ms ?? 0);
  const screenshots = options.screenshots ?? extractScreenshots(trajectory);
  const video = options.video ?? extractVideo(trajectory);
  const videoFormat = options.videoFormat ?? (video?.endsWith(".webm") ? "webm" : "mp4");
  const title = options.title ?? "OpenAgents · Khala QA Trace";

  const stepsHtml = trajectory.steps.map((s) => stepBlock(s, screenshots)).join("\n");

  const videoHtml = video
    ? `<div class="video-frame"><video controls preload="metadata" playsinline>
         <source src="${esc(video)}" type="video/${esc(videoFormat)}" />
         Your browser cannot play this recording. Open <a href="${esc(video)}">${esc(video)}</a>.
       </video></div>`
    : `<div class="video-empty">No video recorded for this run.</div>`;

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<meta name="color-scheme" content="dark" />
<title>${esc(title)}</title>
<style>${STYLE}</style>
</head>
<body>
<header class="masthead">
  <div class="masthead-inner">
    <div class="title-row">
      <h1>${esc(trajectory.agent.name)}</h1>
      <span class="sub">${esc(trajectory.trajectory_id ?? trajectory.session_id ?? "")}</span>
    </div>
    <div class="strip">
      <div class="cell"><span class="lbl">model</span><span class="val">${esc(trajectory.agent.model_name ?? "openagents/khala")}</span></div>
      <div class="cell"><span class="lbl">target</span><span class="val">${esc(target.name ?? "")}</span></div>
      <div class="cell"><span class="lbl">verdict</span><span class="val verdict" style="--accent:${accent};color:${accent}">${esc(verdict)}</span></div>
      <div class="cell"><span class="lbl">duration</span><span class="val">${esc(fmtDuration(durationMs))}</span></div>
      <div class="cell"><span class="lbl">cost</span><span class="val">$0.00</span></div>
      <div class="cell"><span class="lbl">steps</span><span class="val">${esc(trajectory.steps.length)}</span></div>
    </div>
  </div>
</header>

<main class="wrap">
  <div class="section-label">session recording</div>
  ${videoHtml}

  <div class="section-label">trajectory · ATIF-v1.7</div>
  <ol class="timeline">
${stepsHtml}
  </ol>

  <div class="section-label">final metrics</div>
  <div class="metrics">
    <div class="cell"><span class="lbl">total steps</span><span class="val">${esc(trajectory.final_metrics?.total_steps ?? trajectory.steps.length)}</span></div>
    <div class="cell"><span class="lbl">prompt tokens</span><span class="val">${esc(trajectory.final_metrics?.total_prompt_tokens ?? 0)}</span></div>
    <div class="cell"><span class="lbl">completion tokens</span><span class="val">${esc(trajectory.final_metrics?.total_completion_tokens ?? 0)}</span></div>
    <div class="cell"><span class="lbl">total cost</span><span class="val">$${esc((trajectory.final_metrics?.total_cost_usd ?? 0).toFixed(2))}</span></div>
    <div class="cell"><span class="lbl">trace digest</span><span class="val">${esc(String(trajectory.final_metrics?.extra?.trace_digest ?? "").slice(0, 16))}…</span></div>
  </div>

  <footer>
    <span>${esc(trajectory.schema_version)} · ${esc(trajectory.agent.name)} v${esc(trajectory.agent.version)}</span>
    <span>own-infra · $0 · public-safe</span>
  </footer>
</main>
</body>
</html>
`;
}

function extractScreenshots(trajectory: AtifTrajectory): string[] {
  const arts = trajectory.extra?.artifacts as { screenshots?: unknown } | undefined;
  if (arts && Array.isArray(arts.screenshots)) return arts.screenshots.map(String);
  return [];
}

function extractVideo(trajectory: AtifTrajectory): string | undefined {
  const arts = trajectory.extra?.artifacts as { video?: unknown } | undefined;
  if (arts && typeof arts.video === "string") return arts.video;
  return undefined;
}
