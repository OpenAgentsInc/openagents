import { html } from "@openagentsinc/effuse";

import type { TemplateResult } from "@openagentsinc/effuse";

import type { DseSignaturePageData } from "../lib/pageData/dse";

export type {
  DseActiveHistoryItem,
  DseActivePointer,
  DseCanaryConfig,
  DseCanaryHistoryItem,
  DseCompileReportListItem,
  DseEvalReportListItem,
  DseExampleListItem,
  DseReceiptListItem,
  DseSignaturePageData,
} from "../lib/pageData/dse";

const formatMs = (ms: number | null): string => {
  if (typeof ms !== "number" || !Number.isFinite(ms) || ms <= 0) return "-";
  try {
    return new Date(ms).toISOString();
  } catch {
    return String(ms);
  }
};

const sectionShell = (title: string, subtitle: string | null, body: TemplateResult): TemplateResult => html`
  <div class="rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset] px-3 py-3">
    <div class="flex items-baseline justify-between gap-3">
      <div>
        <div class="text-xs text-text-dim uppercase tracking-wider">${title}</div>
        ${subtitle ? html`<div class="text-[11px] text-text-muted mt-1">${subtitle}</div>` : null}
      </div>
    </div>
    <div class="mt-3">${body}</div>
  </div>
`;

export function dseSignaturePageTemplate(data: DseSignaturePageData): TemplateResult {
  const body =
    data.errorText != null
      ? html`<div class="text-xs text-red-400">Error: ${data.errorText}</div>`
      : data.active == null ||
          data.activeHistory == null ||
          data.canaryHistory == null ||
          data.compileReports == null ||
          data.evalReports == null ||
          data.examples == null ||
          data.receipts == null
        ? html`<div class="text-xs text-text-dim">Loading…</div>`
        : html`
            <div class="flex flex-col gap-4">
              ${sectionShell(
                "Active Pointer",
                "Source: Convex `dseActiveArtifacts` + history",
                html`
                  <div class="text-[11px] text-text-muted">
                    <span class="text-text-dim uppercase tracking-wider text-[10px]">compiled_id</span>
                    <span class="ml-2 text-text-primary">${data.active.compiled_id ?? "(none)"}</span>
                    <span class="ml-4 text-text-dim uppercase tracking-wider text-[10px]">updated</span>
                    <span class="ml-2">${formatMs(data.active.updatedAtMs)}</span>
                  </div>
                  <div class="mt-3">
                    <div class="text-[10px] text-text-dim uppercase tracking-wider mb-2">History</div>
                    ${data.activeHistory.length === 0
                      ? html`<div class="text-xs text-text-dim">(no history)</div>`
                      : html`
                          <div class="flex flex-col gap-2">
                            ${data.activeHistory.map(
                              (h) => html`
                                <details class="rounded border border-border-dark/70 bg-bg-secondary/30 px-3 py-2">
                                  <summary class="cursor-pointer select-none">
                                    <div class="flex items-baseline justify-between gap-3">
                                      <div class="min-w-0">
                                        <span class="text-[10px] text-text-dim uppercase tracking-wider">${h.action}</span>
                                        <span class="ml-2 text-[11px] text-text-primary">${h.toCompiledId ?? "(cleared)"}</span>
                                      </div>
                                      <span class="text-[10px] text-text-dim">${formatMs(h.createdAtMs)}</span>
                                    </div>
                                  </summary>
                                  <div class="mt-2 text-[11px] text-text-muted">
                                    <div><span class="text-text-dim">from</span> ${h.fromCompiledId ?? "-"}</div>
                                    <div><span class="text-text-dim">to</span> ${h.toCompiledId ?? "-"}</div>
                                    ${h.actorUserId ? html`<div><span class="text-text-dim">actor</span> ${h.actorUserId}</div>` : null}
                                    ${h.reason ? html`<div class="mt-2 text-text-primary whitespace-pre-wrap break-words">${h.reason}</div>` : null}
                                  </div>
                                </details>
                              `,
                            )}
                          </div>
                        `}
                  </div>
                `,
              )}

              ${sectionShell(
                "Canary",
                "Source: Convex `dseCanaries` + history",
                html`
                  ${data.canary
                    ? html`
                        <div class="grid grid-cols-1 sm:grid-cols-2 gap-2 text-[11px] text-text-muted">
                          <div><span class="text-text-dim uppercase tracking-wider text-[10px]">enabled</span> ${String(data.canary.enabled)}</div>
                          <div><span class="text-text-dim uppercase tracking-wider text-[10px]">rollout</span> ${data.canary.rolloutPct}%</div>
                          <div class="sm:col-span-2">
                            <span class="text-text-dim uppercase tracking-wider text-[10px]">control</span>
                            <span class="ml-2 text-text-primary">${data.canary.control_compiled_id}</span>
                          </div>
                          <div class="sm:col-span-2">
                            <span class="text-text-dim uppercase tracking-wider text-[10px]">canary</span>
                            <span class="ml-2 text-text-primary">${data.canary.canary_compiled_id}</span>
                          </div>
                          <div>
                            <span class="text-text-dim uppercase tracking-wider text-[10px]">counts</span>
                            <span class="ml-2">${data.canary.okCount} ok / ${data.canary.errorCount} err</span>
                          </div>
                          <div>
                            <span class="text-text-dim uppercase tracking-wider text-[10px]">threshold</span>
                            <span class="ml-2">min=${data.canary.minSamples} maxErr=${data.canary.maxErrorRate}</span>
                          </div>
                          <div class="sm:col-span-2">
                            <span class="text-text-dim uppercase tracking-wider text-[10px]">updated</span>
                            <span class="ml-2">${formatMs(data.canary.updatedAtMs)}</span>
                          </div>
                        </div>
                      `
                    : html`<div class="text-xs text-text-dim">(no active canary)</div>`}

                  <div class="mt-3">
                    <div class="text-[10px] text-text-dim uppercase tracking-wider mb-2">History</div>
                    ${data.canaryHistory.length === 0
                      ? html`<div class="text-xs text-text-dim">(no history)</div>`
                      : html`
                          <div class="flex flex-col gap-2">
                            ${data.canaryHistory.map(
                              (h) => html`
                                <details class="rounded border border-border-dark/70 bg-bg-secondary/30 px-3 py-2">
                                  <summary class="cursor-pointer select-none">
                                    <div class="flex items-baseline justify-between gap-3">
                                      <div class="min-w-0">
                                        <span class="text-[10px] text-text-dim uppercase tracking-wider">${h.action}</span>
                                        <span class="ml-2 text-[11px] text-text-primary">${h.canary_compiled_id ?? "-"}</span>
                                      </div>
                                      <span class="text-[10px] text-text-dim">${formatMs(h.createdAtMs)}</span>
                                    </div>
                                  </summary>
                                  <div class="mt-2 text-[11px] text-text-muted">
                                    ${h.rolloutPct != null ? html`<div><span class="text-text-dim">rollout</span> ${h.rolloutPct}%</div>` : null}
                                    ${h.okCount != null || h.errorCount != null
                                      ? html`<div><span class="text-text-dim">counts</span> ${h.okCount ?? 0} ok / ${h.errorCount ?? 0} err</div>`
                                      : null}
                                    ${h.actorUserId ? html`<div><span class="text-text-dim">actor</span> ${h.actorUserId}</div>` : null}
                                    ${h.reason ? html`<div class="mt-2 text-text-primary whitespace-pre-wrap break-words">${h.reason}</div>` : null}
                                  </div>
                                </details>
                              `,
                            )}
                          </div>
                        `}
                  </div>
                `,
              )}

              ${sectionShell(
                "Compile Reports",
                "Source: Convex `dseCompileReports`",
                data.compileReports.length === 0
                  ? html`<div class="text-xs text-text-dim">(no reports)</div>`
                  : html`
                      <div class="flex flex-col gap-2">
                        ${data.compileReports.map((r) => {
                          const href = `/dse/compile-report/${encodeURIComponent(r.jobHash)}/${encodeURIComponent(
                            r.datasetHash,
                          )}/${encodeURIComponent(data.signatureId)}`;
                          return html`
                            <div class="rounded border border-border-dark/70 bg-bg-secondary/30 px-3 py-2">
                              <div class="flex items-baseline justify-between gap-3">
                                <a href="${href}" class="text-[11px] text-text-primary hover:underline break-all">
                                  ${r.jobHash}
                                </a>
                                <span class="text-[10px] text-text-dim">${formatMs(r.createdAtMs)}</span>
                              </div>
                              <div class="mt-1 text-[11px] text-text-muted">
                                <span class="text-text-dim uppercase tracking-wider text-[10px]">compiled</span>
                                <span class="ml-2 text-text-primary break-all">${r.compiled_id}</span>
                              </div>
                              <div class="mt-1 text-[11px] text-text-muted">
                                <span class="text-text-dim uppercase tracking-wider text-[10px]">dataset</span>
                                <span class="ml-2 break-all">${r.datasetHash}</span>
                              </div>
                            </div>
                          `;
                        })}
                      </div>
                    `,
              )}

              ${sectionShell(
                "Eval Reports",
                "Source: Convex `dseEvalReports`",
                data.evalReports.length === 0
                  ? html`<div class="text-xs text-text-dim">(no reports)</div>`
                  : html`
                      <div class="flex flex-col gap-2">
                        ${data.evalReports.map((r) => {
                          const href = `/dse/eval-report/${encodeURIComponent(r.evalHash)}/${encodeURIComponent(
                            data.signatureId,
                          )}`;
                          return html`
                            <div class="rounded border border-border-dark/70 bg-bg-secondary/30 px-3 py-2">
                              <div class="flex items-baseline justify-between gap-3">
                                <a href="${href}" class="text-[11px] text-text-primary hover:underline break-all">
                                  ${r.evalHash}
                                </a>
                                <span class="text-[10px] text-text-dim">${formatMs(r.createdAtMs)}</span>
                              </div>
                              <div class="mt-1 text-[11px] text-text-muted">
                                <span class="text-text-dim uppercase tracking-wider text-[10px]">compiled</span>
                                <span class="ml-2 text-text-primary break-all">${r.compiled_id}</span>
                              </div>
                              <div class="mt-1 text-[11px] text-text-muted">
                                <span class="text-text-dim uppercase tracking-wider text-[10px]">dataset</span>
                                <span class="ml-2 break-all">${r.datasetHash}</span>
                              </div>
                              <div class="mt-1 text-[11px] text-text-muted">
                                <span class="text-text-dim uppercase tracking-wider text-[10px]">reward</span>
                                <span class="ml-2 break-all">${r.rewardId}</span>
                                <span class="ml-4 text-text-dim uppercase tracking-wider text-[10px]">split</span>
                                <span class="ml-2">${r.split ?? "-"}</span>
                                <span class="ml-4 text-text-dim uppercase tracking-wider text-[10px]">n</span>
                                <span class="ml-2">${r.n ?? "-"}</span>
                              </div>
                            </div>
                          `;
                        })}
                      </div>
                    `,
              )}

              ${sectionShell(
                "Dataset Examples",
                "Source: Convex `dseExamples` (bounded list)",
                data.examples.length === 0
                  ? html`<div class="text-xs text-text-dim">(no examples)</div>`
                  : html`
                      <div class="flex flex-col gap-2">
                        ${data.examples.map(
                          (ex) => html`
                            <details class="rounded border border-border-dark/70 bg-bg-secondary/30 px-3 py-2">
                              <summary class="cursor-pointer select-none">
                                <div class="flex items-baseline justify-between gap-3">
                                  <div class="min-w-0">
                                    <span class="text-[11px] text-text-primary break-all">${ex.exampleId}</span>
                                    ${ex.split
                                      ? html`<span class="ml-2 text-[10px] text-text-dim uppercase tracking-wider">${ex.split}</span>`
                                      : null}
                                  </div>
                                  <span class="text-[10px] text-text-dim">${ex.tags ? ex.tags.slice(0, 4).join(", ") : ""}</span>
                                </div>
                              </summary>
                              <div class="mt-2 border-t border-border-dark/60 border-dashed pt-2">
                                <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">Input</div>
                                <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${ex.inputJson}</pre>
                                <div class="mt-3 border-t border-border-dark/60 border-dashed pt-2">
                                  <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">Expected</div>
                                  <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${ex.expectedJson}</pre>
                                </div>
                              </div>
                            </details>
                          `,
                        )}
                      </div>
                    `,
              )}

              ${sectionShell(
                "Predict Receipts",
                "Source: Convex `receipts` (admin list) + /api/dse/receipt + /api/dse/blob",
                data.receipts.length === 0
                  ? html`<div class="text-xs text-text-dim">(no receipts)</div>`
                  : html`
                      <div class="flex flex-col gap-2">
                        ${data.receipts.map((r) => {
                          const receiptHref = `/api/dse/receipt/${encodeURIComponent(r.receiptId)}`;
                          const blobHref =
                            r.rlmTraceBlobId != null
                              ? `/api/dse/blob/${encodeURIComponent(r.receiptId)}/${encodeURIComponent(r.rlmTraceBlobId)}`
                              : null;
                          return html`
                            <div class="rounded border border-border-dark/70 bg-bg-secondary/30 px-3 py-2">
                              <div class="flex items-baseline justify-between gap-3">
                                <a href="${receiptHref}" class="text-[11px] text-text-primary hover:underline break-all">
                                  ${r.receiptId}
                                </a>
                                <span class="text-[10px] text-text-dim">${formatMs(r.createdAtMs)}</span>
                              </div>
                              <div class="mt-1 text-[11px] text-text-muted">
                                <span class="text-text-dim uppercase tracking-wider text-[10px]">compiled</span>
                                <span class="ml-2 text-text-primary break-all">${r.compiled_id}</span>
                                ${r.strategyId ? html`<span class="ml-3 text-text-dim">strategy</span> <span class="ml-1">${r.strategyId}</span>` : null}
                                ${r.resultTag ? html`<span class="ml-3 text-text-dim">result</span> <span class="ml-1">${r.resultTag}</span>` : null}
                              </div>
                              ${blobHref
                                ? html`
                                    <div class="mt-2 text-[11px] text-text-muted">
                                      <a href="${blobHref}" class="hover:underline text-text-primary">Open RLM trace blob</a>
                                      ${r.rlmTraceEventCount != null
                                        ? html`<span class="ml-2 text-text-dim">events=${r.rlmTraceEventCount}</span>`
                                        : null}
                                    </div>
                                  `
                                : null}
                            </div>
                          `;
                        })}
                      </div>
                    `,
              )}
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
      <span class="text-xs text-text-dim uppercase tracking-wider">DSE Signature</span>
    </header>
    <main class="flex-1 min-h-0 w-full p-4 overflow-hidden">
      <div class="mx-auto w-full max-w-5xl h-full min-h-0">
        <div class="h-full min-h-0 rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset] flex flex-col">
          <div class="flex h-full min-h-0 flex-col px-4 py-4 sm:px-6 lg:px-8">
            <div class="flex items-baseline justify-between gap-4">
              <div>
                <div class="text-xs text-text-dim uppercase tracking-wider">Signature Detail</div>
                <div class="text-[11px] text-text-muted mt-1">${data.signatureId}</div>
              </div>
              <a href="/dse" class="text-[11px] text-text-muted hover:text-text-primary">View ops runs →</a>
            </div>
            <div class="mt-4 flex-1 min-h-0 overflow-y-auto overseer-scroll pr-1">${body}</div>
          </div>
        </div>
      </div>
    </main>
  `;
}
