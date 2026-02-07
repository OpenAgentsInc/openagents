import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html, rawHtml } from "@openagentsinc/effuse";

import type { TemplateResult } from "@openagentsinc/effuse";

const ENVELOPE_ICON = rawHtml(
  '<svg class="size-12 text-accent" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden"><rect x="2" y="4" width="20" height="16" rx="2"/><path d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7"/></svg>'
);

export type LoginStep = "email" | "code";

export type LoginPageModel = {
  readonly step: LoginStep;
  readonly email: string;
  readonly code: string;
  readonly isBusy: boolean;
  readonly errorText: string | null;
};

export function loginPageTemplate(model: LoginPageModel): TemplateResult {
  const errorBlock = model.errorText
    ? html`<div class="mt-4 rounded-lg border border-status-blocked/40 bg-status-blocked/10 px-4 py-3 text-sm text-status-blocked">
        ${model.errorText}
      </div>`
    : "";

  return html`
    <div class="effuse-login mx-auto flex flex-1 w-full max-w-5xl items-center justify-center">
      <div class="w-full max-w-[400px] rounded-2xl border border-white/10 bg-bg-secondary/95 p-8 shadow-[0_0_0_1px_rgba(255,255,255,0.06),0_8px_32px_rgba(0,0,0,0.4)]">
        <div class="flex flex-col items-center text-center">
          <div class="mb-6 text-accent">${ENVELOPE_ICON}</div>
          <h1 class="text-2xl font-bold tracking-tight text-text-primary">Log in to OpenAgents</h1>
          ${model.step === "email"
            ? html`<p class="mt-2 text-sm text-text-muted">Enter your email and we'll send you a one-time code.</p>`
            : html`<p class="mt-2 text-sm text-text-muted">
                Enter the 6-digit code sent to <span class="font-medium text-text-primary">${model.email}</span>.
              </p>`}
        </div>

        ${model.step === "email"
          ? html`
              <form id="login-email-form" data-ez="login.email.submit" class="mt-8">
                <label for="login-email" class="mb-1.5 block text-sm font-medium text-text-dim">
                  Email address
                </label>
                <input
                  id="login-email"
                  name="email"
                  type="email"
                  placeholder="you@example.com"
                  autocomplete="email"
                  data-ez="login.email.input"
                  data-ez-trigger="input"
                  autofocus
                  value="${model.email}"
                  class="w-full rounded-lg border border-border-dark bg-surface-primary px-4 py-3 text-text-primary placeholder:text-text-dim outline-none transition-colors focus:border-border-focus focus:ring-2 focus:ring-border-focus/30"
                />
                <button
                  type="submit"
                  class="mt-6 flex w-full items-center justify-center gap-2 rounded-xl border border-accent bg-accent px-4 py-3.5 text-base font-semibold text-bg-primary transition-colors hover:bg-accent-muted hover:border-accent-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-accent"
                  aria-busy="${model.isBusy ? "true" : "false"}"
                >
                  ${model.isBusy ? "Sending..." : "Continue with Email"}
                  <span aria-hidden>→</span>
                </button>
              </form>
            `
          : html`
              <form id="login-code-form" data-ez="login.code.submit" class="mt-8">
                <label for="login-code" class="mb-1.5 block text-sm font-medium text-text-dim">
                  Verification code
                </label>
                <input
                  id="login-code"
                  name="code"
                  inputmode="numeric"
                  autocomplete="one-time-code"
                  placeholder="123456"
                  data-ez="login.code.input"
                  data-ez-trigger="input"
                  autofocus
                  value="${model.code}"
                  class="w-full rounded-lg border border-border-dark bg-surface-primary px-4 py-3 text-text-primary placeholder:text-text-dim outline-none transition-colors focus:border-border-focus focus:ring-2 focus:ring-border-focus/30"
                />
                <button
                  type="submit"
                  class="mt-6 flex w-full items-center justify-center gap-2 rounded-xl border border-accent bg-accent px-4 py-3.5 text-base font-semibold text-bg-primary transition-colors hover:bg-accent-muted hover:border-accent-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-accent"
                  aria-busy="${model.isBusy ? "true" : "false"}"
                >
                  ${model.isBusy ? "Verifying..." : "Verify code"}
                  <span aria-hidden>→</span>
                </button>
                <div class="mt-4 flex items-center justify-between text-sm">
                  <button
                    type="button"
                    data-ez="login.code.back"
                    class="text-text-muted hover:text-text-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
                  >
                    Back
                  </button>
                  <button
                    type="button"
                    data-ez="login.code.resend"
                    class="text-text-muted hover:text-text-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
                  >
                    Resend code
                  </button>
                </div>
              </form>
            `}

        ${errorBlock}
      </div>
    </div>
  `;
}

export function runLoginPage(
  container: Element,
  model: LoginPageModel
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    yield* dom.render(container, loginPageTemplate(model));
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse login]", err);
      return Effect.void;
    })
  );
}
