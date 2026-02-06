import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html, rawHtml } from "@openagentsinc/effuse";

const X_ICON = rawHtml(
  '<svg class="size-5" viewBox="0 0 24 24" fill="currentColor" aria-hidden><path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/></svg>'
);
const GITHUB_ICON = rawHtml(
  '<svg class="size-5" viewBox="0 0 24 24" fill="currentColor" aria-hidden><path d="M12 2C6.477 2 2 6.477 2 12c0 4.42 2.865 8.166 6.839 9.489.5.092.682-.217.682-.482 0-.237-.008-.866-.013-1.7-2.782.604-3.369-1.34-3.369-1.34-.454-1.156-1.11-1.464-1.11-1.464-.908-.62.069-.608.069-.608 1.003.07 1.531 1.03 1.531 1.03.892 1.529 2.341 1.087 2.91.831.092-.646.35-1.086.636-1.336-2.22-.253-4.555-1.11-4.555-4.943 0-1.091.39-1.984 1.029-2.683-.103-.253-.446-1.27.098-2.647 0 0 .84-.269 2.75 1.025A9.578 9.578 0 0112 6.836c.85.004 1.705.114 2.504.336 1.909-1.294 2.747-1.025 2.747-1.025.546 1.377.203 2.394.1 2.647.64.699 1.028 1.592 1.028 2.683 0 3.842-2.339 4.687-4.566 4.935.359.309.678.919.678 1.852 0 1.336-.012 2.415-.012 2.743 0 .267.18.578.688.48C19.138 20.163 22 16.418 22 12c0-5.523-4.477-10-10-10z"/></svg>'
);

export function runHomePage(container: Element): Effect.Effect<void> {
  const year = new Date().getFullYear();
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    const content = html`
      <div class="effuse-home">
        <main class="mx-auto flex w-full max-w-5xl flex-1 items-center justify-center">
          <div class="w-full max-w-3xl text-center text-white">
            <h1 class="text-balance text-4xl font-semibold tracking-tight sm:text-6xl">
              Introducing Autopilot
            </h1>
            <p class="mx-auto mt-4 max-w-2xl text-pretty text-lg text-white/80 sm:text-xl">
              Your personal agent, no Mac Mini required
            </p>
            <div class="mt-8 flex flex-col items-center justify-center gap-3 sm:flex-row">
              <a href="/login" class="inline-flex h-14 min-h-14 items-center justify-center rounded-lg border border-white/90 bg-transparent px-8 text-base font-semibold uppercase tracking-wide text-white transition-opacity hover:opacity-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-white/50" style="font-family: var(--font-square721); opacity: 0.9;">
                Start for free
              </a>
            </div>
          </div>
        </main>
        <footer class="-mx-4 mt-auto flex w-full items-center justify-between px-6 py-4">
          <span class="text-sm text-white/75">© ${String(year)} OpenAgents, Inc.</span>
          <div class="flex items-center gap-4">
            <a href="https://x.com/OpenAgentsInc" target="_blank" rel="noopener noreferrer" class="text-white/75 transition-colors hover:text-white" aria-label="OpenAgents on X">${X_ICON}</a>
            <a href="https://github.com/OpenAgentsInc/openagents" target="_blank" rel="noopener noreferrer" class="text-white/75 transition-colors hover:text-white" aria-label="OpenAgents on GitHub">${GITHUB_ICON}</a>
          </div>
        </footer>
      </div>
    `;
    yield* dom.render(container, content);
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse home]", err);
      return Effect.void;
    })
  );
}
