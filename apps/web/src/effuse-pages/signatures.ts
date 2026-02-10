import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";

import type { TemplateResult } from "@openagentsinc/effuse";

export type SignatureItem = {
  readonly signatureId: string;
  readonly promptSummary: string;
  readonly inputSchemaJson: string;
  readonly outputSchemaJson: string;
  readonly promptIrJson: string;
  readonly defaultsJson: string;
};

export type SignaturesPageData = {
  readonly errorText: string | null;
  readonly sorted: ReadonlyArray<SignatureItem> | null;
};

export function signaturesPageTemplate(data: SignaturesPageData): TemplateResult {
  const body =
    data.errorText != null
      ? html`<div class="text-xs text-red-400">Error: ${data.errorText}</div>`
      : data.sorted == null
        ? html`<div class="text-xs text-text-dim">Loading…</div>`
        : data.sorted.length === 0
          ? html`<div class="text-xs text-text-dim">(no signatures)</div>`
          : html`
              <div class="flex flex-col gap-3">
                ${data.sorted.map(
                  (s) => html`
                    <details
                      class="rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset]"
                    >
                      <summary class="cursor-pointer select-none px-3 py-2">
                        <div class="flex items-baseline gap-3 min-w-0">
                          <span class="text-xs font-semibold text-text-primary truncate">${s.signatureId}</span>
                          <span class="text-[10px] font-mono text-text-dim truncate">${s.promptSummary}</span>
                        </div>
                      </summary>
                      <div class="border-t border-border-dark/70 px-3 py-2">
                        <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">Input Schema (JSON Schema)</div>
                        <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${s.inputSchemaJson}</pre>
                        <div class="mt-3 border-t border-border-dark/60 border-dashed pt-2">
                          <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">Output Schema (JSON Schema)</div>
                          <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${s.outputSchemaJson}</pre>
                        </div>
                        <div class="mt-3 border-t border-border-dark/60 border-dashed pt-2">
                          <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">Prompt IR</div>
                          <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${s.promptIrJson}</pre>
                        </div>
                        <div class="mt-3 border-t border-border-dark/60 border-dashed pt-2">
                          <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">Defaults</div>
                          <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${s.defaultsJson}</pre>
                        </div>
                      </div>
                    </details>
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
      <span class="text-xs text-text-dim uppercase tracking-wider">DSE Signatures</span>
    </header>
    <main class="flex-1 min-h-0 w-full p-4 overflow-hidden">
      <div class="mx-auto w-full max-w-5xl h-full min-h-0">
        <div class="h-full min-h-0 rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset] flex flex-col">
          <div class="flex h-full min-h-0 flex-col px-4 py-4 sm:px-6 lg:px-8">
            <div class="flex items-baseline justify-between gap-4">
              <div>
                <div class="text-xs text-text-dim uppercase tracking-wider">Signature Contracts</div>
                <div class="text-[11px] text-text-muted mt-1">
                  Source: Worker \`GET /api/contracts/signatures\`
                </div>
              </div>
              <a href="/modules" class="text-[11px] text-text-muted hover:text-text-primary">View modules →</a>
            </div>
            <div class="mt-4 flex-1 min-h-0 overflow-y-auto overseer-scroll pr-1">
              ${body}
            </div>
          </div>
        </div>
      </div>
    </main>
  `;
}

export function runSignaturesPage(
  container: Element,
  data: SignaturesPageData
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    yield* dom.render(container, signaturesPageTemplate(data));
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse signatures]", err);
      return Effect.void;
    })
  );
}
