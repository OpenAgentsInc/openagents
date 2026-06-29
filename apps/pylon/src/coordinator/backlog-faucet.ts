export type BacklogFaucetInput = {
  issueNumber: number
  title: string
  body: string
  budgetSats: number
  labels: string[]
}

export type BacklogFaucetWorkRequest = {
  kind: "nip-lbr.work_request"
  issueRef: string
  title: string
  summary: string
  budgetSats: number
  createdFromLabels: string[]
}

export type BacklogFaucetResult =
  | { ok: true; request: BacklogFaucetWorkRequest }
  | { ok: false; reason: string }

export function buildWorkRequestFromIssue(input: BacklogFaucetInput): BacklogFaucetResult {
  const title = input.title.trim()

  if (title.length === 0) {
    return { ok: false, reason: "title is required" }
  }

  if (input.budgetSats <= 0) {
    return { ok: false, reason: "budgetSats must be positive" }
  }

  if (!input.labels.includes("budgeted")) {
    return { ok: false, reason: "issue is not budgeted" }
  }

  return {
    ok: true,
    request: {
      kind: "nip-lbr.work_request",
      issueRef: `github.issue.${input.issueNumber}`,
      title,
      summary: input.body.trim() || title,
      budgetSats: input.budgetSats,
      createdFromLabels: [...input.labels],
    },
  }
}
