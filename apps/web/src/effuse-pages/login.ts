import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html, rawHtml } from "@openagentsinc/effuse";

const ENVELOPE_ICON = rawHtml(
  '<svg class="size-12 text-accent" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden"><rect x="2" y="4" width="20" height="16" rx="2"/><path d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7"/></svg>'
);

export function runLoginPage(
  container: Element,
  signInUrl: string
): Effect.Effect<void> {
  const safeUrl = signInUrl.startsWith("http") || signInUrl.startsWith("/") ? signInUrl : "#";
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    const content = html`
      <div class="effuse-login mx-auto flex flex-1 w-full max-w-5xl items-center justify-center">
        <div class="w-full max-w-[400px] rounded-2xl border border-white/10 bg-bg-secondary/95 p-8 shadow-[0_0_0_1px_rgba(255,255,255,0.06),0_8px_32px_rgba(0,0,0,0.4)]">
          <div class="flex flex-col items-center text-center">
            <div class="mb-6 text-accent">${ENVELOPE_ICON}</div>
            <h1 class="text-2xl font-bold tracking-tight text-text-primary">
              Log in to OpenAgents
            </h1>
            <p class="mt-2 text-sm text-text-muted">
              Enter your email and we'll send you a magic link.
            </p>
          </div>
          <div class="mt-8">
            <label for="login-email" class="mb-1.5 block text-sm font-medium text-text-dim">
              Email address
            </label>
            <input
              id="login-email"
              type="email"
              placeholder="you@example.com"
              autocomplete="email"
              class="w-full rounded-lg border border-border-dark bg-surface-primary px-4 py-3 text-text-primary placeholder:text-text-dim outline-none transition-colors focus:border-border-focus focus:ring-2 focus:ring-border-focus/30"
            />
          </div>
          <a
            href="${safeUrl}"
            class="mt-6 flex w-full items-center justify-center gap-2 rounded-xl border border-accent bg-accent px-4 py-3.5 text-base font-semibold text-bg-primary transition-colors hover:bg-accent-muted hover:border-accent-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            Continue with Email
            <span aria-hidden>→</span>
          </a>
        </div>
      </div>
    `;
    yield* dom.render(container, content);
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse login]", err);
      return Effect.void;
    })
  );
}
