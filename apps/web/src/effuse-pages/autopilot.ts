import { Effect } from "effect";
import { DomServiceTag, EffuseLive, html, renderToolPart } from "@openagentsinc/effuse";
import type { BoundedText, ToolPartModel } from "@openagentsinc/effuse";
import { streamdown } from "../lib/effuseStreamdown";
import type { TemplateResult } from "@openagentsinc/effuse";

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

export type DseBudgetModel = {
  readonly limits?: Record<string, number> | undefined;
  readonly usage?: Record<string, number> | undefined;
};

export type DseSignatureCardModel = {
  readonly id: string;
  readonly state: string;
  readonly signatureId: string;
  readonly compiled_id?: string | undefined;
  readonly receiptId?: string | undefined;
  readonly strategyId?: string | undefined;
  readonly strategyReason?: string | undefined;
  readonly durationMs?: number | undefined;
  readonly budget?: DseBudgetModel | undefined;
  readonly contextPressure?: BoundedText | undefined;
  readonly promptRenderStats?: BoundedText | undefined;
  readonly rlmTrace?: BoundedText | undefined;
  readonly rlmTraceBlobId?: string | undefined;
  readonly outputPreview?: BoundedText | undefined;
  readonly errorText?: BoundedText | undefined;
};

export type DseCompileCardModel = {
  readonly id: string;
  readonly state: string;
  readonly signatureId: string;
  readonly jobHash: string;
  readonly candidates?: number | undefined;
  readonly best?: { readonly compiled_id: string; readonly reward?: number | undefined } | undefined;
  readonly reportId?: string | undefined;
  readonly errorText?: BoundedText | undefined;
};

export type DsePromoteCardModel = {
  readonly id: string;
  readonly state: string;
  readonly signatureId: string;
  readonly from?: string | undefined;
  readonly to?: string | undefined;
  readonly reason?: string | undefined;
};

export type DseRollbackCardModel = {
  readonly id: string;
  readonly state: string;
  readonly signatureId: string;
  readonly from?: string | undefined;
  readonly to?: string | undefined;
  readonly reason?: string | undefined;
};

export type DseBudgetExceededCardModel = {
  readonly id: string;
  readonly state: string;
  readonly message?: string | undefined;
  readonly budget?: DseBudgetModel | undefined;
};

export type RenderDseSignaturePart = {
  readonly kind: "dse-signature";
  readonly model: DseSignatureCardModel;
};

export type RenderDseCompilePart = {
  readonly kind: "dse-compile";
  readonly model: DseCompileCardModel;
};

export type RenderDsePromotePart = {
  readonly kind: "dse-promote";
  readonly model: DsePromoteCardModel;
};

export type RenderDseRollbackPart = {
  readonly kind: "dse-rollback";
  readonly model: DseRollbackCardModel;
};

export type RenderDseBudgetExceededPart = {
  readonly kind: "dse-budget-exceeded";
  readonly model: DseBudgetExceededCardModel;
};

export type RenderPart =
  | RenderTextPart
  | RenderToolPart
  | RenderDseSignaturePart
  | RenderDseCompilePart
  | RenderDsePromotePart
  | RenderDseRollbackPart
  | RenderDseBudgetExceededPart;

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

const dseStateBadge = (state: string): TemplateResult => {
  const label = state === "ok" ? "ok" : state === "error" ? "error" : state === "start" ? "running" : state;
  const cls =
    state === "ok"
      ? "border-emerald-400/40 bg-emerald-500/10 text-emerald-300"
      : state === "error"
        ? "border-red-400/40 bg-red-500/10 text-red-300"
        : "border-white/30 bg-white/10 text-white/80";

  return html`<span class="inline-flex items-center rounded border px-2 py-0.5 text-[11px] uppercase tracking-wide ${cls}"
    >${label}</span
  >`;
};

const dseRow = (label: string, value: TemplateResult | string | number | null | undefined): TemplateResult => {
  if (value == null || value === "") return html``;
  return html`
    <div class="grid grid-cols-[108px_1fr] gap-2 text-xs leading-relaxed">
      <div class="text-white/60">${label}</div>
      <div class="text-white/90 font-mono break-words">${value}</div>
    </div>
  `;
};

const dseBoundedText = (value: BoundedText): TemplateResult => {
  const truncated = value.truncated ? " (truncated)" : "";
  const blob =
    value.truncated && value.blob
      ? html`<div class="mt-1 text-[11px] text-white/50 font-mono">blob: ${value.blob.id}${truncated}</div>`
      : value.truncated
        ? html`<div class="mt-1 text-[11px] text-white/50 font-mono">${truncated}</div>`
        : null;

  return html`
    <div class="rounded-lg border border-white/15 bg-white/5 px-2 py-2">
      <pre class="whitespace-pre-wrap break-words text-xs leading-relaxed font-mono text-white/90">${value.preview}</pre>
      ${blob}
    </div>
  `;
};

const dseCardShell = (opts: { readonly title: string; readonly state: string; readonly body: TemplateResult }): TemplateResult => {
  return html`
    <section
      data-dse-card="1"
      data-dse-card-title="${opts.title}"
      data-dse-card-state="${opts.state}"
      class="rounded-lg border border-white/15 bg-white/5 px-3 py-3"
    >
      <header class="flex items-center justify-between gap-3">
        <div class="text-xs text-white/60 uppercase tracking-wider">${opts.title}</div>
        ${dseStateBadge(opts.state)}
      </header>
      <div class="mt-2 flex flex-col gap-2">${opts.body}</div>
    </section>
  `;
};

const renderDseSignatureCard = (m: DseSignatureCardModel): TemplateResult => {
  const usage = m.budget?.usage;
  const budget = usage
    ? [
        `elapsedMs=${usage.elapsedMs ?? "?"}`,
        `lmCalls=${usage.lmCalls ?? "?"}`,
        ...(usage.toolCalls != null ? [`toolCalls=${usage.toolCalls}`] : []),
        ...(usage.rlmIterations != null ? [`rlmIterations=${usage.rlmIterations}`] : []),
        ...(usage.subLmCalls != null ? [`subLmCalls=${usage.subLmCalls}`] : []),
        `outputChars=${usage.outputChars ?? "?"}`,
      ].join(" ")
    : null;

  const body = html`
    ${dseRow("signatureId", m.signatureId)}
    ${dseRow("compiled_id", m.compiled_id)}
    ${dseRow("durationMs", m.durationMs)}
    ${dseRow("receiptId", m.receiptId)}
    ${m.receiptId
      ? dseRow(
          "receipt",
          html`<a
            href="/api/dse/receipt/${m.receiptId}"
            target="_blank"
            rel="noreferrer"
            data-dse-open-receipt="1"
            class="underline decoration-white/30 hover:decoration-white/70"
            >Open</a
          >`,
        )
      : html``}
    ${dseRow("strategyId", m.strategyId)}
    ${dseRow("strategyReason", m.strategyReason)}
    ${budget ? dseRow("budget", budget) : html``}
    ${m.receiptId && m.rlmTraceBlobId
      ? dseRow(
          "trace",
          html`<a
            href="/api/dse/blob/${m.receiptId}/${m.rlmTraceBlobId}"
            target="_blank"
            rel="noreferrer"
            data-dse-open-trace="1"
            class="underline decoration-white/30 hover:decoration-white/70"
            >Open</a
          >`,
        )
      : html``}
    ${m.contextPressure ? html`<div>${dseRow("contextPressure", "")}${dseBoundedText(m.contextPressure)}</div>` : html``}
    ${m.promptRenderStats ? html`<div>${dseRow("promptRenderStats", "")}${dseBoundedText(m.promptRenderStats)}</div>` : html``}
    ${m.rlmTrace ? html`<div>${dseRow("rlmTrace", "")}${dseBoundedText(m.rlmTrace)}</div>` : html``}
    ${m.outputPreview ? html`<div>${dseRow("outputPreview", "")}${dseBoundedText(m.outputPreview)}</div>` : html``}
    ${m.errorText ? html`<div>${dseRow("error", "")}${dseBoundedText(m.errorText)}</div>` : html``}
  `;

  return dseCardShell({ title: "DSE Signature", state: m.state, body });
};

const renderDseCompileCard = (m: DseCompileCardModel): TemplateResult => {
  const best = m.best ? `${m.best.compiled_id}${m.best.reward != null ? ` (reward=${m.best.reward})` : ""}` : null;

  const body = html`
    ${dseRow("signatureId", m.signatureId)}
    ${dseRow("jobHash", m.jobHash)}
    ${dseRow("candidates", m.candidates)}
    ${dseRow("best", best)}
    ${dseRow("reportId", m.reportId)}
    ${m.errorText ? html`<div>${dseRow("error", "")}${dseBoundedText(m.errorText)}</div>` : html``}
  `;

  return dseCardShell({ title: "DSE Compile", state: m.state, body });
};

const renderDsePromoteCard = (m: DsePromoteCardModel): TemplateResult => {
  const body = html`
    ${dseRow("signatureId", m.signatureId)}
    ${dseRow("from", m.from)}
    ${dseRow("to", m.to)}
    ${m.reason ? dseRow("reason", m.reason) : html``}
  `;

  return dseCardShell({ title: "DSE Promote", state: m.state, body });
};

const renderDseRollbackCard = (m: DseRollbackCardModel): TemplateResult => {
  const body = html`
    ${dseRow("signatureId", m.signatureId)}
    ${dseRow("from", m.from)}
    ${dseRow("to", m.to)}
    ${m.reason ? dseRow("reason", m.reason) : html``}
  `;

  return dseCardShell({ title: "DSE Rollback", state: m.state, body });
};

const renderDseBudgetExceededCard = (m: DseBudgetExceededCardModel): TemplateResult => {
  const budget = m.budget?.usage
    ? `elapsedMs=${m.budget?.usage?.elapsedMs ?? "?"} lmCalls=${m.budget?.usage?.lmCalls ?? "?"} outputChars=${m.budget?.usage?.outputChars ?? "?"}`
    : null;

  const body = html`
    ${m.message ? dseRow("message", m.message) : html``}
    ${budget ? dseRow("budget", budget) : html``}
  `;

  return dseCardShell({ title: "DSE Budget Stop", state: m.state, body });
};

export const autopilotChatTemplate = (data: AutopilotChatData): TemplateResult => {
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
        ? "max-w-[90%] px-3 py-2 text-sm leading-relaxed font-mono self-end rounded-lg border border-white/20 bg-white/10 text-white"
        : "max-w-[90%] px-3 py-2 text-sm leading-relaxed font-mono self-start text-white/90";

    if (m.role === "user") {
      return html`
        <div class="${messageClass}" data-message-id="${m.id}" data-chat-role="user">
          <div class="whitespace-pre-wrap">${userText}</div>
        </div>
      `;
    }

    const partEls = m.renderParts.map((p) => {
      if (p.kind === "text") {
        return streamdown(p.text, {
          mode: "streaming",
          isAnimating: p.state === "streaming",
          caret: "block",
        });
      }
      if (p.kind === "tool") {
        // Default tool card rendering: enforces toolCallId visibility + BlobRef view-full affordance.
        // Style is inherited from the surrounding typography.
        return renderToolPart(p.model);
      }
      if (p.kind === "dse-signature") return renderDseSignatureCard(p.model);
      if (p.kind === "dse-compile") return renderDseCompileCard(p.model);
      if (p.kind === "dse-promote") return renderDsePromoteCard(p.model);
      if (p.kind === "dse-rollback") return renderDseRollbackCard(p.model);
      if (p.kind === "dse-budget-exceeded") return renderDseBudgetExceededCard(p.model);

      return html``;
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
            class="absolute -top-12 left-1/2 -translate-x-1/2 inline-flex h-9 items-center justify-center rounded-lg px-3 text-xs font-medium bg-white/10 text-white border border-white/20 hover:bg-white/20 focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40 font-mono"
          >
            Scroll to bottom
          </button>
        `
      : null;

  const errorBanner = data.errorText
    ? html`
        <div
          data-autopilot-chat-error="1"
          class="mt-2 rounded-lg border border-red-400/40 bg-red-500/10 px-3 py-2 text-xs text-red-300"
        >
          <div class="whitespace-pre-wrap">${data.errorText}</div>
          ${
            data.auth.isAuthed
              ? html`
                  <div class="mt-2 flex items-center gap-2">
                    <button
                      type="button"
                      data-ez="autopilot.chat.retryInit"
                      class="inline-flex h-8 items-center justify-center rounded-md px-3 text-xs font-medium bg-white/10 text-white border border-white/20 hover:bg-white/20 focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40 font-mono"
                    >
                      Retry
                    </button>
                  </div>
                `
              : null
          }
        </div>
      `
    : null;

  const authPanel = (() => {
    if (data.auth.isAuthed || data.auth.step === "closed") return null;

    const errorBlock = data.auth.errorText
      ? html`<div class="mt-2 rounded-lg border border-red-400/40 bg-red-500/10 px-3 py-2 text-xs text-red-300">
          ${data.auth.errorText}
        </div>`
      : null;

    if (data.auth.step === "email") {
      return html`
        <form
          data-autopilot-auth="1"
          data-autopilot-auth-step="email"
          data-ez="autopilot.auth.email.submit"
          class="mt-2 rounded-lg border border-white/20 bg-white/5 px-3 py-3 backdrop-blur-sm"
        >
          <div class="flex items-center justify-between gap-3">
            <div class="text-xs text-white/70">Enter your email and we'll send a one-time code.</div>
            <button
              type="button"
              data-ez="autopilot.auth.close"
              class="text-xs text-white/60 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
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
              class="h-9 flex-1 rounded-md border border-white/20 bg-transparent px-3 text-sm text-white placeholder:text-white/50 outline-none focus:ring-1 focus:ring-white/30 font-mono"
            />
            <button
              type="submit"
              ${data.auth.isBusy ? "disabled" : ""}
              class="inline-flex h-9 items-center justify-center rounded-md px-3 text-sm font-medium bg-white text-black hover:bg-white/90 font-mono shrink-0 disabled:opacity-60"
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
        class="mt-2 rounded-lg border border-white/20 bg-white/5 px-3 py-3 backdrop-blur-sm"
      >
        <div class="text-xs text-white/70">
          Enter the code sent to <span class="text-white">${data.auth.email || "your email"}</span>.
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
            class="h-9 flex-1 rounded-md border border-white/20 bg-transparent px-3 text-sm text-white placeholder:text-white/50 outline-none focus:ring-1 focus:ring-white/30 font-mono"
          />
          <button
            type="submit"
            ${data.auth.isBusy ? "disabled" : ""}
            class="inline-flex h-9 items-center justify-center rounded-md px-3 text-sm font-medium bg-white text-black hover:bg-white/90 font-mono shrink-0 disabled:opacity-60"
          >
            ${data.auth.isBusy ? "Verifying..." : "Verify"}
          </button>
        </div>
        <div class="mt-2 flex items-center justify-between text-xs">
          <button
            type="button"
            data-ez="autopilot.auth.code.back"
            class="text-white/60 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
          >
            Back
          </button>
          <button
            type="button"
            data-ez="autopilot.auth.code.resend"
            class="text-white/60 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
          >
            Resend code
          </button>
        </div>
        ${errorBlock}
      </form>
    `;
  })();

  return html`
    <div class="flex-1 min-h-0 flex flex-col overflow-hidden" data-autopilot-chat>
      <section class="flex-1 min-h-0 flex flex-col px-4 py-4 sm:px-6 lg:px-8">
        <div class="flex-1 flex flex-col min-h-0 mx-auto w-full max-w-2xl">
          <div
            data-scroll-id="autopilot-chat-scroll"
            class="flex-1 overflow-y-auto overseer-scroll pr-1 scroll-smooth min-h-0"
          >
            <div class="flex flex-col gap-4 py-2">
              ${messageEls}
              <div data-autopilot-bottom></div>
            </div>
          </div>
          <div class="relative mt-4 shrink-0">
            ${scrollButton}
            ${errorBanner}
            ${authPanel}
            <form
              id="chat-form"
              data-ez="autopilot.chat.send"
              class="flex items-center gap-2 rounded-lg border border-white/20 bg-white/5 p-2 backdrop-blur-sm"
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
                class="h-10 flex-1 rounded-md border-0 bg-transparent px-3 text-sm text-white placeholder:text-white/50 outline-none focus:ring-1 focus:ring-white/30 font-mono"
              />
              ${data.isBusy
                ? html`
                    <button
                      type="button"
                      data-ez="autopilot.chat.stop"
                      class="inline-flex h-10 items-center justify-center rounded-md px-4 text-sm font-medium text-white/90 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/40 font-mono"
                    >
                      Stop
                    </button>
                  `
                : html`
                    <button
                      type="submit"
                      data-autopilot-chat-send="1"
                      class="inline-flex h-10 items-center justify-center rounded-md px-4 text-sm font-medium bg-white text-black hover:bg-white/90 focus:outline-none focus-visible:ring-2 focus-visible:ring-white font-mono"
                    >
                      Send
                    </button>
                  `}
            </form>
          </div>
        </div>
      </section>
    </div>
  `;
};

export function runAutopilotChat(
  container: Element,
  data: AutopilotChatData
): Effect.Effect<void> {
  return Effect.gen(function* () {
    const dom = yield* DomServiceTag;
    yield* dom.render(container, autopilotChatTemplate(data));
  }).pipe(
    Effect.provide(EffuseLive),
    Effect.catchAll((err) => {
      console.error("[Effuse autopilot chat]", err);
      return Effect.void;
    })
  );
}
