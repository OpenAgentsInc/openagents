import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html, renderToolPart } from "@openagentsinc/effuse";
import type { ToolPartModel } from "@openagentsinc/effuse";

/** Text part for display (state for streaming done) */
export type RenderTextPart = {
  readonly kind: "text";
  readonly text: string;
  readonly state?: "streaming" | "done";
};

/** Tool part for display */
export type RenderToolPart = {
  readonly kind: "tool";
  readonly model: ToolPartModel;
};

export type RenderPart = RenderTextPart | RenderToolPart;

export type RenderedMessage = {
  readonly id: string;
  readonly role: "user" | "assistant";
  readonly renderParts: ReadonlyArray<RenderPart>;
};

export type AutopilotAuthStep = "closed" | "email" | "code";

export type AutopilotAuthModel = {
  readonly isAuthed: boolean;
  readonly authedEmail: string | null;
  readonly step: AutopilotAuthStep;
  readonly email: string;
  readonly code: string;
  readonly isBusy: boolean;
  readonly errorText: string | null;
};

export type AutopilotChatData = {
  readonly messages: ReadonlyArray<RenderedMessage>;
  readonly isBusy: boolean;
  readonly isAtBottom: boolean;
  readonly inputValue: string;
  readonly errorText: string | null;
  readonly auth: AutopilotAuthModel;
};

export function runAutopilotChat(
  container: Element,
  data: AutopilotChatData
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;

    const messageEls = data.messages.map((m) => {
      const userText =
        m.role === "user"
          ? m.renderParts
              .filter((p): p is RenderTextPart => p.kind === "text")
              .map((p) => p.text)
              .join("")
          : "";
      const messageClass =
        m.role === "user"
          ? "max-w-[90%] px-3 py-2 text-sm leading-relaxed font-mono self-end rounded border bg-accent-subtle text-text-primary border-accent-muted"
          : "max-w-[90%] px-3 py-2 text-sm leading-relaxed font-mono self-start text-text-primary";

      if (m.role === "user") {
        return html`
          <div class="${messageClass}" data-message-id="${m.id}" data-chat-role="user">
            <div class="whitespace-pre-wrap">${userText}</div>
          </div>
        `;
      }

      const partEls = m.renderParts.map((p) => {
        if (p.kind === "text") {
          return html`<div class="whitespace-pre-wrap break-words">${p.text}</div>`;
        }
        // Default tool card rendering: enforces toolCallId visibility + BlobRef view-full affordance.
        // Style is inherited from the surrounding typography.
        return renderToolPart(p.model);
      });

      return html`
        <div class="${messageClass}" data-message-id="${m.id}" data-chat-role="assistant">
          <div class="flex flex-col gap-2">${partEls}</div>
        </div>
      `;
    });

    const scrollButton =
      !data.isAtBottom && data.messages.length > 0
        ? html`
            <button
              type="button"
              data-ez="autopilot.chat.scrollBottom"
              class="absolute -top-12 left-1/2 -translate-x-1/2 inline-flex h-9 items-center justify-center rounded px-3 text-xs font-medium bg-surface-primary text-text-primary border border-border-dark hover:bg-surface-secondary hover:border-border-hover focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus font-mono"
            >
              Scroll to bottom
            </button>
          `
        : null;

    const errorBanner = data.errorText
      ? html`
          <div
            data-autopilot-chat-error="1"
            class="mt-2 rounded border border-status-blocked/40 bg-status-blocked/10 px-3 py-2 text-xs text-status-blocked"
          >
            ${data.errorText}
          </div>
        `
      : null;

    const authPanel = (() => {
      if (data.auth.isAuthed) {
        return html`
          <div data-autopilot-auth="1" data-autopilot-auth-step="authed" class="mt-2 text-xs text-text-dim">
            Signed in as <span class="text-text-primary">${data.auth.authedEmail ?? "user"}</span>
          </div>
        `;
      }

      if (data.auth.step === "closed") {
        return html`
          <div
            data-autopilot-auth="1"
            data-autopilot-auth-step="closed"
            class="mt-2 flex items-center justify-between gap-3 rounded border border-border-dark bg-surface-primary/35 px-3 py-2"
          >
            <div class="text-xs text-text-dim">
              Verify your email to claim this chat and keep your Blueprint.
            </div>
            <button
              type="button"
              data-ez="autopilot.auth.open"
              class="inline-flex h-8 items-center justify-center rounded px-3 text-xs font-medium bg-surface-primary text-text-primary border border-border-dark hover:bg-surface-secondary hover:border-border-hover focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus font-mono shrink-0"
            >
              Verify email
            </button>
          </div>
        `;
      }

      const errorBlock = data.auth.errorText
        ? html`<div class="mt-2 rounded border border-status-blocked/40 bg-status-blocked/10 px-3 py-2 text-xs text-status-blocked">
            ${data.auth.errorText}
          </div>`
        : null;

      if (data.auth.step === "email") {
        return html`
          <form
            data-autopilot-auth="1"
            data-autopilot-auth-step="email"
            data-ez="autopilot.auth.email.submit"
            class="mt-2 rounded border border-border-dark bg-surface-primary/35 px-3 py-3"
          >
            <div class="flex items-center justify-between gap-3">
              <div class="text-xs text-text-dim">Enter your email and we'll send a one-time code.</div>
              <button
                type="button"
                data-ez="autopilot.auth.close"
                class="text-xs text-text-muted hover:text-text-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
              >
                Not now
              </button>
            </div>
            <div class="mt-2 flex items-center gap-2">
              <input
                name="email"
                type="email"
                placeholder="you@example.com"
                autocomplete="email"
                data-ez="autopilot.auth.email.input"
                data-ez-trigger="input"
                value="${data.auth.email}"
                class="h-9 flex-1 rounded border border-border-dark bg-surface-primary px-3 text-sm text-text-primary placeholder:text-text-dim outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
              />
              <button
                type="submit"
                ${data.auth.isBusy ? "disabled" : ""}
                class="inline-flex h-9 items-center justify-center rounded px-3 text-sm font-medium bg-accent text-bg-primary border border-accent hover:bg-accent-muted hover:border-accent-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-accent font-mono shrink-0 disabled:opacity-60"
              >
                ${data.auth.isBusy ? "Sending..." : "Send code"}
              </button>
            </div>
            ${errorBlock}
          </form>
        `;
      }

      // code
      return html`
        <form
          data-autopilot-auth="1"
          data-autopilot-auth-step="code"
          data-ez="autopilot.auth.code.submit"
          class="mt-2 rounded border border-border-dark bg-surface-primary/35 px-3 py-3"
        >
          <div class="text-xs text-text-dim">
            Enter the code sent to <span class="text-text-primary">${data.auth.email || "your email"}</span>.
          </div>
          <div class="mt-2 flex items-center gap-2">
            <input
              name="code"
              inputmode="numeric"
              autocomplete="one-time-code"
              placeholder="123456"
              data-ez="autopilot.auth.code.input"
              data-ez-trigger="input"
              value="${data.auth.code}"
              class="h-9 flex-1 rounded border border-border-dark bg-surface-primary px-3 text-sm text-text-primary placeholder:text-text-dim outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
            />
            <button
              type="submit"
              ${data.auth.isBusy ? "disabled" : ""}
              class="inline-flex h-9 items-center justify-center rounded px-3 text-sm font-medium bg-accent text-bg-primary border border-accent hover:bg-accent-muted hover:border-accent-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-accent font-mono shrink-0 disabled:opacity-60"
            >
              ${data.auth.isBusy ? "Verifying..." : "Verify"}
            </button>
          </div>
          <div class="mt-2 flex items-center justify-between text-xs">
            <button
              type="button"
              data-ez="autopilot.auth.code.back"
              class="text-text-muted hover:text-text-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
            >
              Back
            </button>
            <button
              type="button"
              data-ez="autopilot.auth.code.resend"
              class="text-text-muted hover:text-text-primary focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
            >
              Resend code
            </button>
          </div>
          ${errorBlock}
        </form>
      `;
    })();

    const content = html`
      <div class="flex-1 min-h-0 flex flex-col overflow-hidden" data-autopilot-chat>
        <header class="flex items-center h-12 px-4 gap-3 border-b border-border-dark bg-bg-secondary shrink-0 shadow-[0_1px_0_rgba(255,255,255,0.06)]">
          <span class="text-xs text-text-dim uppercase tracking-wider">Autopilot</span>
        </header>
        <section class="flex-1 min-h-0 flex flex-col p-4">
          <div class="flex-1 flex flex-col min-h-0 mx-auto w-full max-w-4xl">
            <div class="flex-1 min-h-0 rounded border border-border-dark bg-surface-primary/35 shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset] flex flex-col">
              <div class="flex h-full min-h-0 flex-col px-4 py-4 sm:px-6 lg:px-8">
                <div
                  data-scroll-id="autopilot-chat-scroll"
                  class="flex-1 overflow-y-auto overseer-scroll pr-1 scroll-smooth"
                >
                  <div class="flex flex-col gap-3">
                    ${messageEls}
                    <div data-autopilot-bottom></div>
                  </div>
                </div>
                <div class="relative mt-3">
                  ${scrollButton}
                  ${errorBanner}
                  ${authPanel}
                  <form
                    id="chat-form"
                    data-ez="autopilot.chat.send"
                    class="flex items-center gap-2 rounded border border-border-dark bg-bg-secondary p-3 shadow-[0_0_0_1px_rgba(255,255,255,0.04)_inset]"
                  >
                    <input
                      name="message"
                      type="text"
                      placeholder="Message Autopilot…"
                      autocomplete="off"
                      data-ez="autopilot.chat.input"
                      data-ez-trigger="input"
                      data-autopilot-chat-input="1"
                      value="${data.inputValue}"
                      ${data.isBusy ? "disabled" : ""}
                      class="h-9 flex-1 rounded border border-border-dark bg-surface-primary px-3 text-sm text-text-primary placeholder:text-text-dim outline-none focus:border-border-focus focus:ring-1 focus:ring-border-focus font-mono"
                    />
                    ${data.isBusy
                      ? html`
                          <button
                            type="button"
                            data-ez="autopilot.chat.stop"
                            class="inline-flex h-9 items-center justify-center rounded px-3 text-sm font-medium bg-surface-primary text-text-primary border border-border-dark hover:bg-surface-secondary hover:border-border-hover focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus font-mono"
                          >
                            Stop
                          </button>
                        `
                      : html`
                          <button
                            type="submit"
                            data-autopilot-chat-send="1"
                            class="inline-flex h-9 items-center justify-center rounded px-3 text-sm font-medium bg-accent text-bg-primary border border-accent hover:bg-accent-muted hover:border-accent-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-accent font-mono"
                          >
                            Send
                          </button>
                        `}
                  </form>
                </div>
              </div>
            </div>
          </div>
        </section>
      </div>
    `;
    yield* dom.render(container, content);
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse autopilot chat]", err);
      return Effect.void;
    })
  );
}
