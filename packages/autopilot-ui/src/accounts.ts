import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
import type { AutopilotUiMessage, ChipTone } from "./view.js"
import { statusChip } from "./view.js"

export type AccountSummary = Readonly<{
  accountRefHash: string
  provider: "codex" | "claude" | string
  state:
    | "ready"
    | "credentials_missing"
    | "auth_refresh_failed"
    | "cooldown"
    | "weekly_exhausted"
    | "model_unavailable"
    | "execution_refused"
    | "quota_blocked"
    | "unavailable"
  usage?: {
    used: number
    limit: number
  }
  resetAt?: string | null
  cooldownSecondsRemaining?: number | null
  activeSlots?: number | null
  recentRefusalReason?: string | null
  lastSuccessfulTurn?: string | null
  manualReset?: {
    allowed: boolean
    remaining: number
  }
}>

const h = html<AutopilotUiMessage>()

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

const accountStateTone = (state: AccountSummary["state"]): ChipTone => {
  switch (state) {
    case "ready":
      return "success"
    case "cooldown":
    case "weekly_exhausted":
    case "quota_blocked":
      return "warning"
    case "credentials_missing":
    case "auth_refresh_failed":
    case "model_unavailable":
    case "execution_refused":
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

const accountDetails = (account: AccountSummary): string[] =>
  [
    account.resetAt === undefined || account.resetAt === null ? null : `reset_at: ${account.resetAt}`,
    account.cooldownSecondsRemaining === undefined || account.cooldownSecondsRemaining === null
      ? null
      : `cooldown: ${account.cooldownSecondsRemaining}s`,
    account.activeSlots === undefined || account.activeSlots === null
      ? null
      : `active slots: ${account.activeSlots}`,
    account.recentRefusalReason === undefined || account.recentRefusalReason === null
      ? null
      : `recent refusal: ${account.recentRefusalReason}`,
    account.lastSuccessfulTurn === undefined || account.lastSuccessfulTurn === null
      ? null
      : `last successful turn: ${account.lastSuccessfulTurn}`,
    account.manualReset === undefined
      ? null
      : account.manualReset.allowed
        ? `manual reset: available (${account.manualReset.remaining} left)`
        : `manual reset: not available (${account.manualReset.remaining} left)`,
  ].filter((value): value is string => value !== null)

export const AccountRow = (account: AccountSummary): Html =>
  h.article(
    [
      className(
        "account-row grid gap-2 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-3 text-[var(--text,#d7d8e5)] sm:grid-cols-[minmax(0,1.7fr)_7rem_8rem_8rem] sm:items-center",
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
      ...accountDetails(account).map((detail) =>
        h.span(
          [className("font-mono text-xs text-[var(--text-secondary,#8a8c93)] sm:col-span-4")],
          [detail],
        ),
      ),
    ],
  )

export const AccountList = (input: {
  accounts: ReadonlyArray<AccountSummary>
  emptyLabel?: string
}): Html =>
  h.section(
    [className("grid gap-2"), h.DataAttribute("autopilot-account-list", "")],
    input.accounts.length === 0
      ? [
          h.p([className("m-0 text-sm text-[var(--text-secondary,#8a8c93)]")], [
            input.emptyLabel ?? "No accounts",
          ]),
        ]
      : input.accounts.map(AccountRow),
  )
