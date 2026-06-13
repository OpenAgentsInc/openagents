export type AccountSummary = Readonly<{
  accountRefHash: string
  provider: string
  state: "ready" | "quota_blocked" | "unavailable"
  usage?: {
    used: number
    limit: number
  }
}>

export type AccountRowTone = "success" | "warning" | "danger" | "info" | "neutral"

export type AccountRowViewModel = {
  accountRefHash: string
  provider: string
  label: string
  statusLabel: AccountSummary["state"]
  tone: AccountRowTone
  usageText: string
  usageTone: AccountRowTone
}

const accountStateTone = (state: AccountSummary["state"]): AccountRowTone => {
  switch (state) {
    case "ready":
      return "success"
    case "quota_blocked":
      return "warning"
    case "unavailable":
      return "danger"
  }
}

const usageTone = (account: AccountSummary): AccountRowTone => {
  if (account.state === "quota_blocked") return "danger"
  if (account.usage === undefined) return "neutral"
  if (account.usage.limit <= 0) return "warning"

  return account.usage.used >= account.usage.limit ? "danger" : "info"
}

const usageText = (usage: AccountSummary["usage"]): string =>
  usage === undefined ? "quota: unknown" : `quota: ${usage.used}/${usage.limit}`

export function accountRowsViewModel(accounts: AccountSummary[]): AccountRowViewModel[] {
  return accounts.map((account) => ({
    accountRefHash: account.accountRefHash,
    provider: account.provider,
    label: account.accountRefHash,
    statusLabel: account.state,
    tone: accountStateTone(account.state),
    usageText: usageText(account.usage),
    usageTone: usageTone(account),
  }))
}
