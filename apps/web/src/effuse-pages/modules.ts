import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";

import type { TemplateResult } from "@openagentsinc/effuse";

export type ModuleItem = {
  readonly moduleId: string;
  readonly description: string;
  readonly signatureIdsJson: string;
};

export type ModulesPageData = {
  readonly errorText: string | null;
  readonly sorted: ReadonlyArray<ModuleItem> | null;
};

export function modulesPageTemplate(data: ModulesPageData): TemplateResult {
  const body =
    data.errorText != null
      ? html`<div class="text-xs text-red-400">Error: ${data.errorText}</div>`
      : data.sorted == null
        ? html`<div class="text-xs text-text-dim">Loading…</div>`
        : data.sorted.length === 0
          ? html`<div class="text-xs text-text-dim">(no modules)</div>`
          : html`
              <div class="flex flex-col gap-3">
                ${data.sorted.map(
                  (m) => html`
                    <details
                      class="rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset]"
                    >
                      <summary class="cursor-pointer select-none px-3 py-2">
                        <div class="text-xs font-semibold text-text-primary">
                          ${m.moduleId}
                        </div>
                        <div class="text-[11px] text-text-muted mt-1 whitespace-pre-wrap break-words">
                          ${m.description}
                        </div>
                      </summary>
                      <div class="border-t border-border-dark/70 px-3 py-2">
                        <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">
                          Signature IDs
                        </div>
                        <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${m.signatureIdsJson}</pre>
                      </div>
                    </details>
                  `,
                )}
              </div>
            `;

  return html`
    <header class="flex items-center h-12 px-4 gap-3 border-b border-border-dark bg-bg-secondary shrink-0 shadow-[0_1px_0_rgba(255,255,255,0.06)]">
      <a
        href="/autopilot"
        class="text-accent font-mono font-bold text-base tracking-[0.12em] leading-none uppercase hover:opacity-90"
      >
        OpenAgents
      </a>
      <div class="h-6 w-px bg-border-dark/70" aria-hidden="true"></div>
      <span class="text-xs text-text-dim uppercase tracking-wider">DSE Modules</span>
    </header>
    <main class="flex-1 min-h-0 w-full p-4 overflow-hidden">
      <div class="mx-auto w-full max-w-5xl h-full min-h-0">
        <div class="h-full min-h-0 rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset] flex flex-col">
          <div class="flex h-full min-h-0 flex-col px-4 py-4 sm:px-6 lg:px-8">
            <div class="flex items-baseline justify-between gap-4">
              <div>
                <div class="text-xs text-text-dim uppercase tracking-wider">Module Contracts</div>
                <div class="text-[11px] text-text-muted mt-1">
                  Source: Worker \`GET /api/contracts/modules\`
                </div>
              </div>
              <a href="/tools" class="text-[11px] text-text-muted hover:text-text-primary">View tools →</a>
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

export function runModulesPage(
  container: Element,
  data: ModulesPageData
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    yield* dom.render(container, modulesPageTemplate(data));
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse modules]", err);
      return Effect.void;
    })
  );
}
