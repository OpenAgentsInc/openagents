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
