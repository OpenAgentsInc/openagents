import { html } from "@openagentsinc/effuse";

import type { TemplateResult } from "@openagentsinc/effuse";

export type DseEvalReportDetail = {
  readonly signatureId: string;
  readonly evalHash: string;
  readonly compiled_id: string;
  readonly datasetId: string;
  readonly datasetHash: string;
  readonly rewardId: string;
  readonly rewardVersion: number;
  readonly split: string | null;
  readonly n: number | null;
  readonly createdAtMs: number;
  readonly jsonPretty: string;
};

export type DseEvalReportPageData = {
  readonly signatureId: string;
  readonly evalHash: string;
  readonly errorText: string | null;
  readonly report: DseEvalReportDetail | null;
};

const formatMs = (ms: number | null): string => {
  if (typeof ms !== "number" || !Number.isFinite(ms) || ms <= 0) return "-";
  try {
    return new Date(ms).toISOString();
  } catch {
    return String(ms);
  }
};

export function dseEvalReportPageTemplate(data: DseEvalReportPageData): TemplateResult {
  const body =
    data.errorText != null
      ? html`<div class="text-xs text-red-400">Error: ${data.errorText}</div>`
      : data.report == null
        ? html`<div class="text-xs text-text-dim">Loading…</div>`
        : html`
            <div class="rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset] px-3 py-3">
              <div class="grid grid-cols-1 sm:grid-cols-2 gap-2 text-[11px] text-text-muted">
                <div class="sm:col-span-2">
                  <span class="text-text-dim uppercase tracking-wider text-[10px]">signature</span>
                  <span class="ml-2 text-text-primary break-all">${data.report.signatureId}</span>
                </div>
                <div class="sm:col-span-2">
                  <span class="text-text-dim uppercase tracking-wider text-[10px]">evalHash</span>
                  <span class="ml-2 text-text-primary break-all">${data.report.evalHash}</span>
                </div>
                <div class="sm:col-span-2">
                  <span class="text-text-dim uppercase tracking-wider text-[10px]">compiled</span>
                  <span class="ml-2 text-text-primary break-all">${data.report.compiled_id}</span>
                </div>
                <div class="sm:col-span-2">
                  <span class="text-text-dim uppercase tracking-wider text-[10px]">datasetId</span>
                  <span class="ml-2 break-all">${data.report.datasetId}</span>
                </div>
                <div class="sm:col-span-2">
                  <span class="text-text-dim uppercase tracking-wider text-[10px]">datasetHash</span>
                  <span class="ml-2 break-all">${data.report.datasetHash}</span>
                </div>
                <div>
                  <span class="text-text-dim uppercase tracking-wider text-[10px]">reward</span>
                  <span class="ml-2 break-all">${data.report.rewardId}@${data.report.rewardVersion}</span>
                </div>
                <div>
                  <span class="text-text-dim uppercase tracking-wider text-[10px]">split</span>
                  <span class="ml-2">${data.report.split ?? "-"}</span>
                </div>
                <div>
                  <span class="text-text-dim uppercase tracking-wider text-[10px]">n</span>
                  <span class="ml-2">${data.report.n ?? "-"}</span>
                </div>
                <div>
                  <span class="text-text-dim uppercase tracking-wider text-[10px]">created</span>
                  <span class="ml-2">${formatMs(data.report.createdAtMs)}</span>
                </div>
              </div>
              <div class="mt-3 border-t border-border-dark/70 pt-3">
                <div class="text-[10px] text-text-dim uppercase tracking-wider mb-2">Report JSON</div>
                <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${data.report.jsonPretty}</pre>
              </div>
            </div>
          `;

  const signatureHref = `/dse/signature/${encodeURIComponent(data.signatureId)}`;

  return html`
    <header class="flex items-center h-12 px-4 gap-3 border-b border-border-dark bg-bg-secondary shrink-0 shadow-[0_1px_0_rgba(255,255,255,0.06)]">
      <a
        href="/autopilot"
        class="text-accent font-mono font-bold text-base tracking-[0.12em] leading-none uppercase hover:opacity-90"
      >
        OpenAgents
      </a>
      <div class="h-6 w-px bg-border-dark/70" aria-hidden="true"></div>
      <span class="text-xs text-text-dim uppercase tracking-wider">DSE Eval Report</span>
    </header>
    <main class="flex-1 min-h-0 w-full p-4 overflow-hidden">
      <div class="mx-auto w-full max-w-5xl h-full min-h-0">
        <div class="h-full min-h-0 rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset] flex flex-col">
          <div class="flex h-full min-h-0 flex-col px-4 py-4 sm:px-6 lg:px-8">
            <div class="flex items-baseline justify-between gap-4">
              <div>
                <div class="text-xs text-text-dim uppercase tracking-wider">Eval Report</div>
                <div class="text-[11px] text-text-muted mt-1">${data.evalHash}</div>
              </div>
              <a href="${signatureHref}" class="text-[11px] text-text-muted hover:text-text-primary">Back to signature →</a>
            </div>
            <div class="mt-4 flex-1 min-h-0 overflow-y-auto overseer-scroll pr-1">${body}</div>
          </div>
        </div>
      </div>
    </main>
  `;
}

