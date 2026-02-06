import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html } from "@openagentsinc/effuse";

/** Text part for display (state for streaming done) */
export type RenderTextPart = {
  readonly kind: "text";
  readonly text: string;
  readonly state?: "streaming" | "done";
};

/** Tool part for display */
export type RenderToolPart = {
  readonly kind: "tool";
  readonly toolName: string;
  readonly toolCallId: string;
  readonly state: string;
  readonly inputJson: string;
  readonly outputJson?: string;
  readonly errorText?: string;
  readonly preliminary?: boolean;
  readonly usage?: string | null;
  readonly description?: string | null;
};

export type RenderPart = RenderTextPart | RenderToolPart;

export type RenderedMessage = {
  readonly id: string;
  readonly role: "user" | "assistant";
  readonly renderParts: ReadonlyArray<RenderPart>;
};

export type AutopilotChatData = {
  readonly messages: ReadonlyArray<RenderedMessage>;
  readonly isBusy: boolean;
  readonly isAtBottom: boolean;
  readonly inputValue: string;
};

function toolStateBadge(state: string): string {
  switch (state) {
    case "output-available":
      return "OK";
    case "output-error":
      return "ERR";
    case "output-denied":
      return "DENY";
    case "approval-requested":
      return "ASK";
    case "approval-responded":
      return "ACK";
    case "input-streaming":
    case "input-available":
      return "...";
    default:
      return "?";
  }
}

function toolStateLabel(state: string): string {
  switch (state) {
    case "output-available":
      return "done";
    case "output-error":
      return "error";
    case "output-denied":
      return "denied";
    case "approval-requested":
      return "approval";
    case "approval-responded":
      return "approval";
    case "input-streaming":
    case "input-available":
      return "running";
    default:
      return state;
  }
}

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
          <div class="${messageClass}" data-message-id="${m.id}">
            <div class="whitespace-pre-wrap">${userText}</div>
          </div>
        `;
      }

      const partEls = m.renderParts.map((p) => {
        if (p.kind === "text") {
          return html`<div class="whitespace-pre-wrap break-words">${p.text}</div>`;
        }
        const badge = toolStateBadge(p.state);
        const label = toolStateLabel(p.state);
        const headerText =
          p.state === "output-available" ? "Used tool:" : "Using tool:";
        const borderTone =
          p.state === "output-error" || p.state === "output-denied"
            ? "border-red-500/40 bg-red-500/5"
            : "border-border-dark bg-surface-primary/35";

        return html`
          <details
            class="w-full rounded border shadow-[0_0_0_1px_rgba(255,255,255,0.02)_inset] ${borderTone}"
          >
            <summary class="flex items-center justify-between gap-3 px-3 py-2 cursor-pointer select-none list-none [&::-webkit-details-marker]:hidden">
              <div class="flex items-baseline gap-2 min-w-0">
                <span class="text-[10px] uppercase tracking-[0.16em] text-text-dim shrink-0">${badge}</span>
                <span class="text-xs text-text-muted shrink-0">${headerText}</span>
                <span class="text-xs font-semibold text-text-primary truncate">${p.toolName}</span>
                <span class="text-[10px] text-text-dim shrink-0">(${label})</span>
              </div>
              <span class="text-[10px] font-mono text-text-muted shrink-0">Show</span>
            </summary>
            <div class="border-t border-border-dark/70 px-3 py-2">
              ${p.usage || p.description
                ? html`
                    <div class="mb-3">
                      ${p.usage ? html`<div class="text-[10px] font-mono text-text-dim whitespace-pre-wrap break-words">${p.usage}</div>` : null}
                      ${p.description ? html`<div class="text-[11px] text-text-muted whitespace-pre-wrap break-words">${p.description}</div>` : null}
                    </div>
                  `
                : null}
              <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">Input</div>
              <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${p.inputJson}</pre>
              ${p.state === "output-error" && p.errorText
                ? html`
                    <div class="mt-3">
                      <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">Error</div>
                      <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-red-300">${p.errorText}</pre>
                    </div>
                  `
                : null}
              ${p.state === "output-available" && p.outputJson != null
                ? html`
                    <div class="mt-3 border-t border-border-dark/60 border-dashed pt-2">
                      <div class="text-[10px] text-text-dim uppercase tracking-wider mb-1">Output${p.preliminary ? " (preliminary)" : ""}</div>
                      <pre class="text-[11px] leading-4 whitespace-pre-wrap break-words text-text-primary">${p.outputJson}</pre>
                    </div>
                  `
                : null}
            </div>
          </details>
        `;
      });

      return html`
        <div class="${messageClass}" data-message-id="${m.id}">
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
