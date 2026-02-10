import { html } from "@openagentsinc/effuse";

import type { TemplateResult } from "@openagentsinc/effuse";

export type DseOpsRunItem = {
  readonly runId: string;
  readonly status: "running" | "finished" | "failed";
  readonly startedAtMs: number;
  readonly endedAtMs: number | null;
  readonly commitSha: string | null;
  readonly baseUrl: string | null;
  readonly actorUserId: string | null;
  readonly signatureIds: ReadonlyArray<string> | null;
  readonly updatedAtMs: number;
  readonly createdAtMs: number;
};

export type DseOpsRunsPageData = {
  readonly errorText: string | null;
  readonly runs: ReadonlyArray<DseOpsRunItem> | null;
};

const formatMs = (ms: number | null): string => {
  if (typeof ms !== "number" || !Number.isFinite(ms) || ms <= 0) return "-";
  try {
    return new Date(ms).toISOString();
  } catch {
    return String(ms);
  }
};

const statusBadge = (status: DseOpsRunItem["status"]): TemplateResult => {
  const cls =
    status === "running"
      ? "bg-blue-500/15 text-blue-200 border-blue-500/30"
      : status === "finished"
        ? "bg-emerald-500/15 text-emerald-200 border-emerald-500/30"
        : "bg-red-500/15 text-red-200 border-red-500/30";
  return html`<span class="inline-flex items-center px-2 py-[2px] rounded border text-[10px] uppercase tracking-wider ${cls}"
    >${status}</span
  >`;
};

export function dseOpsRunsPageTemplate(data: DseOpsRunsPageData): TemplateResult {
  const body =
    data.errorText != null
      ? html`<div class="text-xs text-red-400">Error: ${data.errorText}</div>`
      : data.runs == null
        ? html`<div class="text-xs text-text-dim">Loading…</div>`
        : data.runs.length === 0
          ? html`<div class="text-xs text-text-dim">(no ops runs)</div>`
          : html`
              <div class="flex flex-col gap-2">
                ${data.runs.map((r) => {
                  const runHref = `/dse/ops/${encodeURIComponent(r.runId)}`;
                  return html`
                    <div
                      class="rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset] px-3 py-2"
                    >
                      <div class="flex items-center justify-between gap-3 min-w-0">
                        <div class="min-w-0 flex items-baseline gap-3">
                          <a href="${runHref}" class="text-xs font-semibold text-text-primary hover:underline truncate">
                            ${r.runId}
                          </a>
                          ${statusBadge(r.status)}
                        </div>
                        <div class="text-[10px] text-text-dim shrink-0">
                          ${formatMs(r.startedAtMs)}
                        </div>
                      </div>
                      <div class="mt-2 grid grid-cols-1 sm:grid-cols-2 gap-2 text-[11px] text-text-muted">
                        <div class="flex gap-2 min-w-0">
                          <span class="text-text-dim uppercase tracking-wider text-[10px]">commit</span>
                          <span class="truncate">${r.commitSha ?? "-"}</span>
                        </div>
                        <div class="flex gap-2 min-w-0">
                          <span class="text-text-dim uppercase tracking-wider text-[10px]">base</span>
                          <span class="truncate">${r.baseUrl ?? "-"}</span>
                        </div>
                        <div class="flex gap-2 min-w-0">
                          <span class="text-text-dim uppercase tracking-wider text-[10px]">actor</span>
                          <span class="truncate">${r.actorUserId ?? "-"}</span>
                        </div>
                        <div class="flex gap-2 min-w-0">
                          <span class="text-text-dim uppercase tracking-wider text-[10px]">updated</span>
                          <span class="truncate">${formatMs(r.updatedAtMs)}</span>
                        </div>
                      </div>
                      ${r.signatureIds && r.signatureIds.length
                        ? html`
                            <div class="mt-2 flex flex-wrap gap-2">
                              ${r.signatureIds.slice(0, 12).map((sig) => {
                                const href = `/dse/signature/${encodeURIComponent(sig)}`;
                                return html`<a
                                  href="${href}"
                                  class="text-[10px] px-2 py-[2px] rounded border border-border-dark/70 bg-bg-secondary/40 text-text-muted hover:text-text-primary hover:border-border-dark"
                                  >${sig}</a
                                >`;
                              })}
                            </div>
                          `
                        : null}
                    </div>
                  `;
                })}
              </div>
            `;

  return html`
    <header class="flex items-center h-12 px-4 gap-3 border-b border-border-dark bg-bg-secondary shrink-0 shadow-[0_1px_0_rgba(255,255,255,0.06)]">
      <a
        href="/"
        class="text-accent font-mono font-bold text-base tracking-[0.12em] leading-none uppercase hover:opacity-90"
      >
        OpenAgents
      </a>
      <div class="h-6 w-px bg-border-dark/70" aria-hidden="true"></div>
      <span class="text-xs text-text-dim uppercase tracking-wider">DSE Ops Runs</span>
    </header>
    <main class="flex-1 min-h-0 w-full p-4 overflow-hidden">
      <div class="mx-auto w-full max-w-5xl h-full min-h-0">
        <div class="h-full min-h-0 rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset] flex flex-col">
          <div class="flex h-full min-h-0 flex-col px-4 py-4 sm:px-6 lg:px-8">
            <div class="flex items-baseline justify-between gap-4">
              <div>
                <div class="text-xs text-text-dim uppercase tracking-wider">Overnight Ops Runs</div>
                <div class="text-[11px] text-text-muted mt-1">Source: Convex \`dseOpsRuns\`</div>
              </div>
              <a href="/signatures" class="text-[11px] text-text-muted hover:text-text-primary">View signatures →</a>
            </div>
            <div class="mt-4 flex-1 min-h-0 overflow-y-auto overseer-scroll pr-1">${body}</div>
          </div>
        </div>
      </div>
    </main>
  `;
}
