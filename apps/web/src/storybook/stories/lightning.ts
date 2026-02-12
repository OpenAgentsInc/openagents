import { html, renderToolPart } from "@openagentsinc/effuse"

import { streamdown } from "../../lib/effuseStreamdown"
import { renderPaymentStateCard } from "../../effuse-pages/autopilot"

import type { TemplateResult, ToolPartModel } from "@openagentsinc/effuse"
import type { L402PaymentStateCardModel } from "../../effuse-pages/autopilot"
import type { Story } from "../types"

type L402PaneStatus = "completed" | "cached" | "blocked" | "failed"

type L402PanePayment = {
  readonly status: L402PaneStatus
  readonly toolCallId: string
  readonly messageId: string
  readonly runId: string | null
  readonly messageIndex: number
  readonly taskId?: string
  readonly host?: string
  readonly url?: string
  readonly method?: string
  readonly scope?: string
  readonly maxSpendMsats?: number
  readonly quotedAmountMsats?: number
  readonly cacheHit?: boolean
  readonly cacheStatus?: string
  readonly paid?: boolean
  readonly paymentBackend?: string
  readonly paymentId?: string
  readonly amountMsats?: number
  readonly proofReference?: string
  readonly denyReason?: string
  readonly denyReasonCode?: string
  readonly responseStatusCode?: number
  readonly responseContentType?: string
  readonly responseBytes?: number
  readonly responseBodySha256?: string
  readonly responseBodyTextPreview?: string
}

const formatSats = (value: number | undefined): string => {
  if (typeof value !== "number" || !Number.isFinite(value)) return "n/a"
  return `${value.toLocaleString()} sats`
}

const formatMsats = (value: number | undefined): string => {
  if (typeof value !== "number" || !Number.isFinite(value)) return "n/a"
  const sats = value / 1000
  return `${sats.toLocaleString(undefined, { maximumFractionDigits: 3 })} sats (${Math.round(value).toLocaleString()} msats)`
}

const statusBadgeClass = (status: L402PaneStatus): string => {
  if (status === "completed" || status === "cached") return "text-emerald-300 border-emerald-400/35 bg-emerald-500/10"
  if (status === "blocked") return "text-amber-300 border-amber-400/35 bg-amber-500/10"
  return "text-red-300 border-red-400/35 bg-red-500/10"
}

const l402WalletSummary = (
  payments: ReadonlyArray<L402PanePayment>,
): {
  readonly totalAttempts: number
  readonly statusCounts: Record<L402PaneStatus, number>
  readonly totalSpendMsats: number
  readonly maxSpendMsats: number
  readonly lastPaid: L402PanePayment | null
} => {
  const counts: Record<L402PaneStatus, number> = {
    completed: 0,
    cached: 0,
    blocked: 0,
    failed: 0,
  }
  let totalSpendMsats = 0
  let maxSpendMsats = 0
  for (const payment of payments) {
    counts[payment.status] += 1
    if ((payment.status === "completed" || payment.status === "cached") && typeof payment.amountMsats === "number") {
      totalSpendMsats += payment.amountMsats
    }
    if (typeof payment.maxSpendMsats === "number") {
      maxSpendMsats = Math.max(maxSpendMsats, payment.maxSpendMsats)
    }
  }
  const lastPaid =
    [...payments]
      .reverse()
      .find((payment) => payment.status === "completed" || payment.status === "cached") ?? null
  return {
    totalAttempts: payments.length,
    statusCounts: counts,
    totalSpendMsats,
    maxSpendMsats,
    lastPaid,
  }
}

const l402WalletPaneTemplate = (
  payments: ReadonlyArray<L402PanePayment>,
  opts?: {
    readonly walletOnline?: boolean
    readonly balanceSats?: number
    readonly statusNote?: string
  },
): TemplateResult => {
  const summary = l402WalletSummary(payments)
  const walletOnline = opts?.walletOnline ?? true
  return html`
    <div class="h-full overflow-auto bg-black p-4 text-sm font-mono text-white/85">
      <div class="text-[11px] uppercase tracking-wide text-white/55">L402 Wallet Summary</div>
      <div class="mt-3 grid grid-cols-[140px_1fr] gap-x-3 gap-y-2 text-xs">
        <div class="text-white/55">wallet status</div>
        <div>${walletOnline ? "online" : "offline"}${opts?.statusNote ? ` (${opts.statusNote})` : ""}</div>
        <div class="text-white/55">balance</div>
        <div>${formatSats(opts?.balanceSats)}</div>
        <div class="text-white/55">attempts</div>
        <div>${summary.totalAttempts}</div>
        <div class="text-white/55">completed</div>
        <div>${summary.statusCounts.completed}</div>
        <div class="text-white/55">cached</div>
        <div>${summary.statusCounts.cached}</div>
        <div class="text-white/55">blocked</div>
        <div>${summary.statusCounts.blocked}</div>
        <div class="text-white/55">failed</div>
        <div>${summary.statusCounts.failed}</div>
        <div class="text-white/55">spent</div>
        <div>${formatMsats(summary.totalSpendMsats)}</div>
        <div class="text-white/55">max request cap</div>
        <div>${summary.maxSpendMsats > 0 ? formatMsats(summary.maxSpendMsats) : "n/a"}</div>
        <div class="text-white/55">last result</div>
        <div>${summary.lastPaid?.status ?? "n/a"}</div>
        <div class="text-white/55">last amount</div>
        <div>${formatMsats(summary.lastPaid?.amountMsats)}</div>
        <div class="text-white/55">last cache</div>
        <div>
          ${summary.lastPaid
            ? summary.lastPaid.cacheHit === true
              ? "hit"
              : summary.lastPaid.cacheHit === false
                ? "miss"
                : "n/a"
            : "n/a"}${summary.lastPaid?.cacheStatus ? ` (${summary.lastPaid.cacheStatus})` : ""}
        </div>
        <div class="text-white/55">backend</div>
        <div>${summary.lastPaid?.paymentBackend ?? "n/a"}</div>
        <div class="text-white/55">allowlist/policy</div>
        <div>enforced via desktop executor + macaroon scope</div>
      </div>
      <div class="mt-4 rounded border border-white/15 bg-white/5 p-3 text-xs">
        <div class="text-white/60">last paid endpoint</div>
        <div class="mt-1 break-all text-white/90">${summary.lastPaid?.url ?? "none yet"}</div>
        ${summary.lastPaid
          ? html`<div class="mt-1 text-white/60">proof: ${summary.lastPaid.proofReference ?? "n/a"}</div>`
          : null}
      </div>
    </div>
  `
}

const l402TransactionsPaneTemplate = (payments: ReadonlyArray<L402PanePayment>): TemplateResult => {
  const rows = [...payments].reverse().slice(0, 10)

  return html`
    <div class="h-full overflow-auto bg-black p-3 text-xs font-mono text-white/85">
      <div class="mb-2 px-1 text-[11px] uppercase tracking-wide text-white/55">Recent L402 Attempts</div>
      ${rows.length === 0
        ? html`<div class="rounded border border-white/15 bg-white/5 p-3 text-white/60">No L402 payment attempts yet.</div>`
        : html`
            <div class="flex flex-col gap-2">
              ${rows.map((row) => html`
                  <div class="rounded border border-white/15 bg-white/5 p-2">
                    <div class="flex items-center justify-between gap-2">
                      <span
                        class="inline-flex items-center rounded border px-1.5 py-0.5 text-[10px] uppercase tracking-wide ${statusBadgeClass(row.status)}"
                        >${row.status}</span
                      >
                      <span class="text-[10px] text-white/55">${row.host ?? (() => {
                        try { return row.url ? new URL(row.url).host : "unknown-host" } catch { return "unknown-host" }
                      })()}</span>
                    </div>
                    <div class="mt-1 break-all text-white/90">${row.url ?? "unknown endpoint"}</div>
                    <div class="mt-1 text-[11px] text-white/65">
                      amount: ${formatMsats(row.amountMsats)} · cap: ${formatMsats(row.maxSpendMsats)} · task: ${row.taskId ?? "n/a"} · proof: ${row.proofReference ?? "n/a"}
                    </div>
                    <div class="mt-1 text-[11px] text-white/60">
                      toolCall: ${row.toolCallId} · cache: ${row.cacheHit === true ? "hit" : row.cacheHit === false ? "miss" : "n/a"}${row.cacheStatus ? ` (${row.cacheStatus})` : ""} · backend: ${row.paymentBackend ?? "n/a"}
                    </div>
                    <div class="mt-1 text-[11px] text-white/60">
                      resp: ${row.responseStatusCode ?? "n/a"} · bytes: ${typeof row.responseBytes === "number" ? row.responseBytes.toLocaleString() : "n/a"} · sha: ${row.responseBodySha256 ? row.responseBodySha256.slice(0, 16) : "n/a"}
                    </div>
                    ${row.denyReason ? html`<div class="mt-1 text-[11px] text-amber-200/90">deny: ${row.denyReason}</div>` : null}
                  </div>
                `)}
            </div>
          `}
    </div>
  `
}

const l402PaymentDetailPaneTemplate = (payment: L402PanePayment): TemplateResult => {
  return html`
    <div class="h-full overflow-auto bg-black p-4 text-sm font-mono text-white/85">
      <div class="grid grid-cols-[140px_1fr] gap-x-3 gap-y-2 text-xs">
        <div class="text-white/55">status</div>
        <div>${payment.status}</div>
        <div class="text-white/55">host</div>
        <div>${payment.host ?? (() => {
          try { return payment.url ? new URL(payment.url).host : "n/a" } catch { return "n/a" }
        })()}</div>
        <div class="text-white/55">url</div>
        <div class="break-all">${payment.url ?? "n/a"}</div>
        <div class="text-white/55">method</div>
        <div>${payment.method ?? "GET"}</div>
        <div class="text-white/55">scope</div>
        <div>${payment.scope ?? "default"}</div>
        <div class="text-white/55">cap</div>
        <div>${formatMsats(payment.maxSpendMsats)}</div>
        <div class="text-white/55">quoted</div>
        <div>${formatMsats(payment.quotedAmountMsats)}</div>
        <div class="text-white/55">cacheHit</div>
        <div>${payment.cacheHit === true ? "true" : payment.cacheHit === false ? "false" : "n/a"}${payment.cacheStatus ? ` (${payment.cacheStatus})` : ""}</div>
        <div class="text-white/55">paid</div>
        <div>${payment.paid === true ? "true" : payment.paid === false ? "false" : "n/a"}</div>
        <div class="text-white/55">backend</div>
        <div>${payment.paymentBackend ?? "n/a"}</div>
        <div class="text-white/55">taskId</div>
        <div>${payment.taskId ?? "n/a"}</div>
        <div class="text-white/55">paymentId</div>
        <div>${payment.paymentId ?? "n/a"}</div>
        <div class="text-white/55">amount</div>
        <div>${formatMsats(payment.amountMsats)}</div>
        <div class="text-white/55">proof</div>
        <div class="break-all">${payment.proofReference ?? "n/a"}</div>
        <div class="text-white/55">denyReason</div>
        <div>${payment.denyReason ?? "n/a"}</div>
        <div class="text-white/55">denyCode</div>
        <div>${payment.denyReasonCode ?? "n/a"}</div>
        <div class="text-white/55">responseStatus</div>
        <div>${payment.responseStatusCode ?? "n/a"}</div>
        <div class="text-white/55">contentType</div>
        <div>${payment.responseContentType ?? "n/a"}</div>
        <div class="text-white/55">responseBytes</div>
        <div>${typeof payment.responseBytes === "number" ? payment.responseBytes.toLocaleString() : "n/a"}</div>
        <div class="text-white/55">responseSha</div>
        <div class="break-all">${payment.responseBodySha256 ?? "n/a"}</div>
        <div class="text-white/55">messageId</div>
        <div>${payment.messageId}</div>
        <div class="text-white/55">runId</div>
        <div>${payment.runId ?? "n/a"}</div>
        <div class="text-white/55">toolCallId</div>
        <div class="break-all">${payment.toolCallId}</div>
      </div>

      <div class="mt-4 text-[11px] uppercase tracking-wide text-white/55">response preview</div>
      ${payment.responseBodyTextPreview
        ? html`
            <pre class="mt-2 whitespace-pre-wrap break-words rounded border border-white/15 bg-white/5 p-3 text-xs text-white/85">${payment.responseBodyTextPreview}</pre>
          `
        : html`<div class="mt-2 rounded border border-white/15 bg-white/5 p-3 text-xs text-white/60">No preview stored.</div>`}
    </div>
  `
}

const makeLightningToolPart = (opts: {
  readonly toolCallId: string
  readonly input: unknown
  readonly output: unknown
}): ToolPartModel => ({
  status: "tool-result",
  toolName: "lightning_l402_fetch",
  toolCallId: opts.toolCallId,
  summary: "payment.sent",
  details: {
    input: {
      preview: JSON.stringify(opts.input, null, 2),
      truncated: false,
    },
    output: {
      preview: JSON.stringify(opts.output, null, 2),
      truncated: false,
    },
  },
})

const paymentCardModelIntent: L402PaymentStateCardModel = {
  state: "payment.intent",
  toolCallId: "toolcall_l402_1",
  taskId: "task_l402_1",
  url: "https://api.lightninglabs.example/premium",
  method: "GET",
  maxSpendMsats: 100_000,
  host: "api.lightninglabs.example",
  statusLabel: "queued",
}

const paymentCardModelPaying: L402PaymentStateCardModel = {
  ...paymentCardModelIntent,
  taskId: undefined,
  statusLabel: "paying",
}

const paymentCardModelPaid: L402PaymentStateCardModel = {
  state: "payment.sent",
  toolCallId: "toolcall_l402_1",
  taskId: "task_l402_1",
  url: "https://api.lightninglabs.example/premium",
  method: "GET",
  maxSpendMsats: 100_000,
  quotedAmountMsats: 70_000,
  amountMsats: 70_000,
  responseStatusCode: 200,
  responseContentType: "application/json",
  responseBytes: 2480,
  responseBodySha256: "sha256:8d6c1f23e1b59c7b9b0c8a9e4d1a2f3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f",
  cacheHit: false,
  paid: true,
  cacheStatus: "miss",
  paymentBackend: "spark",
  proofReference: "preimage:aa11bb22cc33dd44",
  host: "api.lightninglabs.example",
  statusLabel: "completed",
}

const paymentCardModelCached: L402PaymentStateCardModel = {
  ...paymentCardModelPaid,
  state: "payment.cached",
  cacheHit: true,
  paid: false,
  cacheStatus: "hit",
  amountMsats: undefined,
  quotedAmountMsats: undefined,
  statusLabel: "cached",
}

const paymentCardModelBlocked: L402PaymentStateCardModel = {
  state: "payment.blocked",
  toolCallId: "toolcall_l402_2",
  taskId: "task_l402_2",
  url: "https://api.lightninglabs.example/expensive",
  method: "GET",
  maxSpendMsats: 100_000,
  quotedAmountMsats: 250_000,
  denyReason: "Blocked: quoted 250 sats > cap 100 sats",
  denyReasonCode: "amount_over_cap",
  host: "api.lightninglabs.example",
  statusLabel: "blocked",
}

const paymentCardModelFailed: L402PaymentStateCardModel = {
  state: "payment.failed",
  toolCallId: "toolcall_l402_3",
  taskId: "task_l402_3",
  url: "https://api.lightninglabs.example/premium",
  method: "GET",
  maxSpendMsats: 100_000,
  denyReason: "request_failed",
  host: "api.lightninglabs.example",
  statusLabel: "failed",
}

const samplePanePayments: ReadonlyArray<L402PanePayment> = [
  {
    status: "completed",
    toolCallId: "toolcall_l402_1",
    messageId: "m-assistant-1",
    runId: "run_1",
    messageIndex: 1,
    taskId: "task_l402_1",
    url: "https://api.lightninglabs.example/premium",
    method: "GET",
    scope: "default",
    host: "api.lightninglabs.example",
    maxSpendMsats: 100_000,
    quotedAmountMsats: 70_000,
    cacheHit: false,
    cacheStatus: "miss",
    paid: true,
    paymentBackend: "spark",
    paymentId: "spark_pay_1",
    amountMsats: 70_000,
    proofReference: "preimage:aa11bb22cc33dd44",
    responseStatusCode: 200,
    responseContentType: "application/json",
    responseBytes: 2480,
    responseBodySha256: "sha256:8d6c1f23e1b59c7b9b0c8a9e4d1a2f3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f",
    responseBodyTextPreview: "{\n  \"signal\": \"premium\",\n  \"value\": 0.8123\n}\n",
  },
  {
    status: "cached",
    toolCallId: "toolcall_l402_4",
    messageId: "m-assistant-2",
    runId: "run_2",
    messageIndex: 3,
    taskId: "task_l402_4",
    url: "https://api.lightninglabs.example/premium",
    method: "GET",
    scope: "default",
    host: "api.lightninglabs.example",
    maxSpendMsats: 100_000,
    cacheHit: true,
    cacheStatus: "hit",
    paid: false,
    paymentBackend: "spark",
    proofReference: "preimage:aa11bb22cc33dd44",
    responseStatusCode: 200,
    responseContentType: "application/json",
    responseBytes: 2480,
    responseBodySha256: "sha256:8d6c1f23e1b59c7b9b0c8a9e4d1a2f3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f",
  },
  {
    status: "blocked",
    toolCallId: "toolcall_l402_2",
    messageId: "m-assistant-3",
    runId: "run_3",
    messageIndex: 5,
    taskId: "task_l402_2",
    url: "https://api.lightninglabs.example/expensive",
    method: "GET",
    scope: "default",
    host: "api.lightninglabs.example",
    maxSpendMsats: 100_000,
    quotedAmountMsats: 250_000,
    denyReason: "Blocked: quoted 250 sats > cap 100 sats",
    denyReasonCode: "amount_over_cap",
  },
  {
    status: "failed",
    toolCallId: "toolcall_l402_3",
    messageId: "m-assistant-4",
    runId: null,
    messageIndex: 7,
    taskId: "task_l402_3",
    url: "https://api.lightninglabs.example/premium",
    method: "GET",
    scope: "default",
    host: "api.lightninglabs.example",
    maxSpendMsats: 100_000,
    denyReason: "request_failed",
  },
] as const

const storyShell = (content: TemplateResult) => html`
  <div class="flex h-full min-h-0 w-full min-w-0 flex-col gap-4">
    <div class="text-[11px] uppercase tracking-wide text-text-dim">Lightning UI</div>
    ${content}
  </div>
`

const chatMessageShell = (opts: {
  readonly role: "user" | "assistant"
  readonly content: TemplateResult
}): TemplateResult => {
  if (opts.role === "user") {
    return html`<div class="text-sm font-mono text-white/55 text-left max-w-[80%] self-end" data-chat-role="user">${opts.content}</div>`
  }

  return html`<div class="group text-sm font-mono text-white/90" data-chat-role="assistant">
    <div class="flex flex-col gap-2">
      ${opts.content}
    </div>
  </div>`
}

export const lightningStories: ReadonlyArray<Story> = [
  {
    id: "lightning-l402-payment-card-intent",
    title: "Lightning/L402 payment card (intent + approve)",
    kind: "molecule",
    render: () => storyShell(renderPaymentStateCard(paymentCardModelIntent)),
  },
  {
    id: "lightning-l402-payment-card-paying",
    title: "Lightning/L402 payment card (paying)",
    kind: "molecule",
    render: () => storyShell(renderPaymentStateCard(paymentCardModelPaying)),
  },
  {
    id: "lightning-l402-payment-card-paid",
    title: "Lightning/L402 payment card (paid)",
    kind: "molecule",
    render: () => storyShell(renderPaymentStateCard(paymentCardModelPaid)),
  },
  {
    id: "lightning-l402-payment-card-cached",
    title: "Lightning/L402 payment card (cached)",
    kind: "molecule",
    render: () => storyShell(renderPaymentStateCard(paymentCardModelCached)),
  },
  {
    id: "lightning-l402-payment-card-blocked",
    title: "Lightning/L402 payment card (blocked: over cap)",
    kind: "molecule",
    render: () => storyShell(renderPaymentStateCard(paymentCardModelBlocked)),
  },
  {
    id: "lightning-l402-payment-card-failed",
    title: "Lightning/L402 payment card (failed)",
    kind: "molecule",
    render: () => storyShell(renderPaymentStateCard(paymentCardModelFailed)),
  },
  {
    id: "lightning-chat-l402-approval",
    title: "Lightning/Home chat (L402 approval prompt)",
    kind: "organism",
    render: () => {
      const toolInput = {
        endpointPreset: "A",
        method: "GET",
        maxSpendMsats: 100_000,
      }

      const toolOutput = {
        status: "queued",
        approvalRequired: true,
        taskId: "task_l402_1",
        url: "https://api.lightninglabs.example/premium",
        maxSpendMsats: 100_000,
        host: "api.lightninglabs.example",
      }

      const toolPart = makeLightningToolPart({
        toolCallId: "toolcall_l402_1",
        input: toolInput,
        output: toolOutput,
      })

      return html`
        ${storyShell(html`
          <div class="rounded border border-white/15 bg-black/80 overflow-hidden">
            <div class="flex flex-col gap-2 overflow-y-auto p-4">
              ${chatMessageShell({
                role: "user",
                content: html`Fetch premium signal feed and summarize it. Max 100 sats.`,
              })}
              ${chatMessageShell({
                role: "assistant",
                content: html`
                  ${streamdown("I can fetch that via an L402-paid endpoint. Please approve the spend.", { mode: "static" })}
                  ${renderPaymentStateCard(paymentCardModelIntent)}
                  ${renderToolPart(toolPart)}
                `,
              })}
            </div>
          </div>
        `)}
      `
    },
  },
  {
    id: "lightning-chat-l402-paid-and-cached",
    title: "Lightning/Home chat (paid + cached follow-up)",
    kind: "organism",
    render: () => {
      const paidToolOutput = {
        status: "completed",
        taskId: "task_l402_1",
        url: "https://api.lightninglabs.example/premium",
        host: "api.lightninglabs.example",
        maxSpendMsats: 100_000,
        quotedAmountMsats: 70_000,
        amountMsats: 70_000,
        paid: true,
        cacheHit: false,
        cacheStatus: "miss",
        paymentBackend: "spark",
        proofReference: "preimage:aa11bb22cc33dd44",
        responseStatusCode: 200,
        responseContentType: "application/json",
        responseBytes: 2480,
        responseBodySha256: "sha256:8d6c1f23e1b59c7b9b0c8a9e4d1a2f3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f",
        responseBodyTextPreview: "{\\n  \\\"signal\\\": \\\"premium\\\",\\n  \\\"value\\\": 0.8123\\n}\\n",
      }

      const cachedToolOutput = {
        status: "cached",
        taskId: "task_l402_4",
        url: "https://api.lightninglabs.example/premium",
        host: "api.lightninglabs.example",
        maxSpendMsats: 100_000,
        paid: false,
        cacheHit: true,
        cacheStatus: "hit",
        paymentBackend: "spark",
        proofReference: "preimage:aa11bb22cc33dd44",
        responseStatusCode: 200,
        responseContentType: "application/json",
        responseBytes: 2480,
        responseBodySha256: "sha256:8d6c1f23e1b59c7b9b0c8a9e4d1a2f3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f",
      }

      const paidToolPart = makeLightningToolPart({
        toolCallId: "toolcall_l402_1",
        input: { endpointPreset: "A", method: "GET", maxSpendMsats: 100_000 },
        output: paidToolOutput,
      })

      const cachedToolPart = makeLightningToolPart({
        toolCallId: "toolcall_l402_4",
        input: { endpointPreset: "A", method: "GET", maxSpendMsats: 100_000 },
        output: cachedToolOutput,
      })

      return storyShell(html`
        <div class="rounded border border-white/15 bg-black/80 overflow-hidden">
          <div class="flex flex-col gap-2 overflow-y-auto p-4">
            ${chatMessageShell({
              role: "user",
              content: html`Fetch premium signal feed and summarize it. Max 100 sats.`,
            })}
            ${chatMessageShell({
              role: "assistant",
              content: html`
                ${streamdown("Paid once and fetched the premium payload.", { mode: "static" })}
                ${renderPaymentStateCard(paymentCardModelPaid)}
                ${renderToolPart(paidToolPart)}
              `,
            })}
            ${chatMessageShell({
              role: "user",
              content: html`Fetch it again (should hit cache).`,
            })}
            ${chatMessageShell({
              role: "assistant",
              content: html`
                ${streamdown("Cache hit: reused the credential, no additional payment.", { mode: "static" })}
                ${renderPaymentStateCard(paymentCardModelCached)}
                ${renderToolPart(cachedToolPart)}
              `,
            })}
          </div>
        </div>
      `)
    },
  },
  {
    id: "lightning-l402-wallet-pane-offline",
    title: "Lightning/L402 wallet pane (offline)",
    kind: "organism",
    render: () =>
      storyShell(
        html`<div class="h-[360px] rounded border border-white/15 overflow-hidden">${l402WalletPaneTemplate(samplePanePayments, {
          walletOnline: false,
          balanceSats: undefined,
          statusNote: "desktop executor heartbeat stale",
        })}</div>`,
      ),
  },
  {
    id: "lightning-l402-wallet-pane-empty",
    title: "Lightning/L402 wallet pane (no attempts)",
    kind: "organism",
    render: () =>
      storyShell(
        html`<div class="h-[360px] rounded border border-white/15 overflow-hidden">${l402WalletPaneTemplate([], {
          walletOnline: true,
          balanceSats: 1_250,
        })}</div>`,
      ),
  },
  {
    id: "lightning-l402-wallet-pane-mixed",
    title: "Lightning/L402 wallet pane (mixed states)",
    kind: "organism",
    render: () =>
      storyShell(
        html`<div class="h-[360px] rounded border border-white/15 overflow-hidden">${l402WalletPaneTemplate(samplePanePayments, {
          walletOnline: true,
          balanceSats: 9_420,
        })}</div>`,
      ),
  },
  {
    id: "lightning-l402-transactions-pane-empty",
    title: "Lightning/L402 transactions pane (empty)",
    kind: "organism",
    render: () =>
      storyShell(
        html`<div class="h-[360px] rounded border border-white/15 overflow-hidden">${l402TransactionsPaneTemplate([])}</div>`,
      ),
  },
  {
    id: "lightning-l402-transactions-pane-mixed",
    title: "Lightning/L402 transactions pane (mixed states)",
    kind: "organism",
    render: () =>
      storyShell(
        html`<div class="h-[420px] rounded border border-white/15 overflow-hidden">${l402TransactionsPaneTemplate(samplePanePayments)}</div>`,
      ),
  },
  {
    id: "lightning-l402-payment-detail-pane-paid",
    title: "Lightning/L402 payment detail pane (paid)",
    kind: "organism",
    render: () =>
      storyShell(
        html`<div class="h-[420px] rounded border border-white/15 overflow-hidden">${l402PaymentDetailPaneTemplate(samplePanePayments[0]!)}</div>`,
      ),
  },
] as const
