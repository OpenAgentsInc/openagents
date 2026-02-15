import { html } from "@openagentsinc/effuse";

import type { TemplateResult } from "@openagentsinc/effuse";

import type { DseOpsRunDetailPageData, DseOpsRunEventItem } from "../lib/pageData/dse";

export type { DseOpsRunDetail, DseOpsRunDetailPageData, DseOpsRunEventItem } from "../lib/pageData/dse";

const formatMs = (ms: number | null): string => {
  if (typeof ms !== "number" || !Number.isFinite(ms) || ms <= 0) return "-";
  try {
    return new Date(ms).toISOString();
  } catch {
    return String(ms);
  }
};

const levelBadge = (level: DseOpsRunEventItem["level"]): TemplateResult => {
  const cls =
    level === "info"
      ? "bg-blue-500/15 text-blue-200 border-blue-500/30"
      : level === "warn"
        ? "bg-amber-500/15 text-amber-200 border-amber-500/30"
        : "bg-red-500/15 text-red-200 border-red-500/30";
  return html`<span class="inline-flex items-center px-2 py-[2px] rounded border text-[10px] uppercase tracking-wider ${cls}"
    >${level}</span
  >`;
};

export function dseOpsRunDetailPageTemplate(data: DseOpsRunDetailPageData): TemplateResult {
  const body =
    data.errorText != null
      ? html`<div class="text-xs text-red-400">Error: ${data.errorText}</div>`
      : data.run == null || data.events == null
        ? html`<div class="text-xs text-text-dim">Loading…</div>`
        : html`
            <div class="flex flex-col gap-4">
              <div class="rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset] px-3 py-3">
                <div class="flex items-center justify-between gap-3 min-w-0">
                  <div class="min-w-0">
                    <div class="text-xs text-text-dim uppercase tracking-wider">Run</div>
                    <div class="text-xs font-semibold text-text-primary break-words mt-1">${data.run.runId}</div>
                  </div>
                  <div class="text-[11px] text-text-muted shrink-0">
                    <span class="text-text-dim uppercase tracking-wider text-[10px]">status</span>
                    <span class="ml-2">${data.run.status}</span>
                  </div>
                </div>

                <div class="mt-3 grid grid-cols-1 sm:grid-cols-2 gap-2 text-[11px] text-text-muted">
                  <div class="flex gap-2 min-w-0">
                    <span class="text-text-dim uppercase tracking-wider text-[10px]">started</span>
                    <span class="truncate">${formatMs(data.run.startedAtMs)}</span>
                  </div>
                  <div class="flex gap-2 min-w-0">
                    <span class="text-text-dim uppercase tracking-wider text-[10px]">ended</span>
                    <span class="truncate">${formatMs(data.run.endedAtMs)}</span>
                  </div>
                  <div class="flex gap-2 min-w-0">
                    <span class="text-text-dim uppercase tracking-wider text-[10px]">commit</span>
                    <span class="truncate">${data.run.commitSha ?? "-"}</span>
                  </div>
                  <div class="flex gap-2 min-w-0">
                    <span class="text-text-dim uppercase tracking-wider text-[10px]">base</span>
                    <span class="truncate">${data.run.baseUrl ?? "-"}</span>
                  </div>
                  <div class="flex gap-2 min-w-0">
                    <span class="text-text-dim uppercase tracking-wider text-[10px]">actor</span>
                    <span class="truncate">${data.run.actorUserId ?? "-"}</span>
                  </div>
                  <div class="flex gap-2 min-w-0">
                    <span class="text-text-dim uppercase tracking-wider text-[10px]">updated</span>
                    <span class="truncate">${formatMs(data.run.updatedAtMs)}</span>
                  </div>
                </div>

                ${data.run.signatureIds && data.run.signatureIds.length
                  ? html`
                      <div class="mt-3">
                        <div class="text-[10px] text-text-dim uppercase tracking-wider mb-2">Signatures</div>
                        <div class="flex flex-wrap gap-2">
                          ${data.run.signatureIds.slice(0, 20).map((sig) => {
                            const href = `/dse/signature/${encodeURIComponent(sig)}`;
                            return html`<a
                              href="${href}"
                              class="text-[10px] px-2 py-[2px] rounded border border-border-dark/70 bg-bg-secondary/40 text-text-muted hover:text-text-primary hover:border-border-dark"
                              >${sig}</a
                            >`;
                          })}
                        </div>
                      </div>
                    `
                  : null}

                ${data.run.notes
                  ? html`
                      <div class="mt-3">
                        <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">Notes</div>
                        <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${data.run.notes}</pre>
                      </div>
                    `
                  : null}

                ${data.run.linksJson
                  ? html`
                      <div class="mt-3">
                        <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">Links</div>
                        <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${data.run.linksJson}</pre>
                      </div>
                    `
                  : null}

                ${data.run.summaryJson
                  ? html`
                      <div class="mt-3">
                        <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">Summary</div>
                        <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${data.run.summaryJson}</pre>
                      </div>
                    `
                  : null}
              </div>

              <div>
                <div class="flex items-baseline justify-between gap-4">
                  <div>
                    <div class="text-xs text-text-dim uppercase tracking-wider">Events</div>
                    <div class="text-[11px] text-text-muted mt-1">Source: Convex \`dseOpsRunEvents\`</div>
                  </div>
                  <a href="/dse" class="text-[11px] text-text-muted hover:text-text-primary">Back to runs →</a>
                </div>

                <div class="mt-3 flex flex-col gap-2">
                  ${data.events.length === 0
                    ? html`<div class="text-xs text-text-dim">(no events)</div>`
                    : data.events.map((ev) => {
                        const header = `${formatMs(ev.tsMs)} ${ev.phase ? ` ${ev.phase}` : ""}`.trim();
                        return html`
                          <details
                            class="rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset]"
                          >
                            <summary class="cursor-pointer select-none px-3 py-2">
                              <div class="flex items-center justify-between gap-3">
                                <div class="min-w-0 flex items-baseline gap-3">
                                  <span class="text-[10px] text-text-dim truncate">${header}</span>
                                  ${levelBadge(ev.level)}
                                </div>
                                <span class="text-[11px] text-text-muted truncate">${ev.message}</span>
                              </div>
                            </summary>
                            <div class="border-t border-border-dark/70 px-3 py-2">
                              <div class="text-[11px] text-text-primary whitespace-pre-wrap break-words">${ev.message}</div>
                              ${ev.jsonPreview
                                ? html`
                                    <div class="mt-2 border-t border-border-dark/60 border-dashed pt-2">
                                      <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">JSON</div>
                                      <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${ev.jsonPreview}</pre>
                                    </div>
                                  `
                                : null}
                            </div>
                          </details>
                        `;
                      })}
                </div>
              </div>
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
      <span class="text-xs text-text-dim uppercase tracking-wider">DSE Ops Run</span>
    </header>
    <main class="flex-1 min-h-0 w-full p-4 overflow-hidden">
      <div class="mx-auto w-full max-w-5xl h-full min-h-0">
        <div class="h-full min-h-0 rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset] flex flex-col">
          <div class="flex h-full min-h-0 flex-col px-4 py-4 sm:px-6 lg:px-8">
            <div class="flex items-baseline justify-between gap-4">
              <div>
                <div class="text-xs text-text-dim uppercase tracking-wider">Ops Run Detail</div>
                <div class="text-[11px] text-text-muted mt-1">runId=${data.runId}</div>
              </div>
              <a href="/dse" class="text-[11px] text-text-muted hover:text-text-primary">View all runs →</a>
            </div>
            <div class="mt-4 flex-1 min-h-0 overflow-y-auto overseer-scroll pr-1">${body}</div>
          </div>
        </div>
      </div>
    </main>
  `;
}
