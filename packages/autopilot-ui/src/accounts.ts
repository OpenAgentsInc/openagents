import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import type { AutopilotUiMessage, ChipTone } from "./view.js"
import { statusChip } from "./view.js"

export type AccountSummary = Readonly<{
  accountRefHash: string
  provider: "codex" | "claude" | string
  state: "ready" | "quota_blocked" | "unavailable"
  usage?: {
    used: number
    limit: number
  }
  rateLimitResetAt?: string | null
}>

const h = html<AutopilotUiMessage>()

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

const accountStateTone = (state: AccountSummary["state"]): ChipTone => {
  switch (state) {
    case "ready":
      return "success"
    case "quota_blocked":
      return "warning"
    case "unavailable":
      return "danger"
  }
}

const quotaTone = (account: AccountSummary): ChipTone => {
  if (account.state === "quota_blocked") return "danger"
  if (account.usage === undefined) return "neutral"
  if (account.usage.limit <= 0) return "warning"

  return account.usage.used >= account.usage.limit ? "danger" : "info"
}

const quotaLabel = (usage: AccountSummary["usage"]): string =>
  usage === undefined ? "quota: unknown" : `quota: ${usage.used}/${usage.limit}`

type CountdownProjection = Readonly<{
  label: string
  tone: ChipTone
  resetAt: string | null
  remainingMs: number | null
}>

const parseEpochMs = (value: string | number | Date | undefined): number => {
  if (value instanceof Date) return value.getTime()
  if (typeof value === "number") return value
  if (typeof value === "string") return Date.parse(value)
  return Date.now()
}

const countdownLabel = (remainingMs: number): string => {
  if (remainingMs <= 0) return "reset due"

  const totalMinutes = Math.max(1, Math.ceil(remainingMs / 60_000))
  const days = Math.floor(totalMinutes / 1_440)
  const hours = Math.floor((totalMinutes % 1_440) / 60)
  const minutes = totalMinutes % 60

  if (days > 0) {
    return hours > 0 ? `resets in ${days}d ${hours}h` : `resets in ${days}d`
  }
  if (hours > 0) {
    return minutes > 0 ? `resets in ${hours}h ${minutes}m` : `resets in ${hours}h`
  }
  return `resets in ${minutes}m`
}

export const rateLimitCountdownProjection = (input: {
  resetAt?: string | null
  now?: string | number | Date
}): CountdownProjection => {
  const resetAt = input.resetAt?.trim() ?? null
  if (resetAt === null || resetAt === "") {
    return { label: "reset: unknown", tone: "neutral", resetAt: null, remainingMs: null }
  }

  const resetEpochMs = Date.parse(resetAt)
  const nowEpochMs = parseEpochMs(input.now)
  if (!Number.isFinite(resetEpochMs) || !Number.isFinite(nowEpochMs)) {
    return { label: "reset: invalid", tone: "danger", resetAt, remainingMs: null }
  }

  const remainingMs = resetEpochMs - nowEpochMs
  return {
    label: countdownLabel(remainingMs),
    tone: remainingMs <= 0 ? "info" : "warning",
    resetAt,
    remainingMs,
  }
}

export const RateLimitCountdown = (input: {
  resetAt?: string | null
  now?: string | number | Date
  attrs?: ReadonlyArray<Attribute<AutopilotUiMessage>>
}): Html => {
  const countdown = rateLimitCountdownProjection(input)
  const attrs = [
    h.DataAttribute("autopilot-rate-limit-countdown", countdown.label),
    ...(countdown.resetAt === null
      ? []
      : [
          h.Attribute("datetime", countdown.resetAt),
          h.DataAttribute("autopilot-rate-limit-reset-at", countdown.resetAt),
        ]),
    ...(input.attrs ?? []),
  ]

  return h.time(attrs, [
    statusChip({
      label: countdown.label,
      tone: countdown.tone,
    }),
  ])
}

const accountCountdown = (
  account: AccountSummary,
  options?: { now?: string | number | Date },
): Html =>
  account.state === "quota_blocked" || account.rateLimitResetAt != null
    ? RateLimitCountdown({
        ...(account.rateLimitResetAt === undefined ? {} : { resetAt: account.rateLimitResetAt }),
        ...(options?.now === undefined ? {} : { now: options.now }),
      })
    : h.empty

export const AccountRow = (
  account: AccountSummary,
  options?: { now?: string | number | Date },
): Html =>
  h.article(
    [
      className(
        "account-row grid gap-2 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-3 text-[var(--text,#d7d8e5)] sm:grid-cols-[minmax(0,1.7fr)_7rem_8rem_8rem_minmax(8rem,10rem)] sm:items-center",
      ),
      h.DataAttribute("autopilot-account-ref", account.accountRefHash),
    ],
    [
      h.code([className("min-w-0 truncate font-mono text-sm text-[var(--primary,#fff)]")], [
        account.accountRefHash,
      ]),
      h.span([className("font-mono text-xs text-[var(--text-secondary,#8a8c93)]")], [
        account.provider,
      ]),
      statusChip({
        label: account.state,
        tone: accountStateTone(account.state),
        attrs: [h.DataAttribute("autopilot-account-state", account.state)],
      }),
      statusChip({
        label: quotaLabel(account.usage),
        tone: quotaTone(account),
        attrs: [h.DataAttribute("autopilot-account-quota", account.accountRefHash)],
      }),
      accountCountdown(account, options),
    ],
  )

export const AccountList = (input: {
  accounts: ReadonlyArray<AccountSummary>
  emptyLabel?: string
  now?: string | number | Date
}): Html =>
  h.section(
    [className("grid gap-2"), h.DataAttribute("autopilot-account-list", "")],
    input.accounts.length === 0
      ? [
          h.p([className("m-0 text-sm text-[var(--text-secondary,#8a8c93)]")], [
            input.emptyLabel ?? "No accounts",
          ]),
        ]
      : input.accounts.map((account) =>
          AccountRow(account, input.now === undefined ? undefined : { now: input.now }),
        ),
  )
