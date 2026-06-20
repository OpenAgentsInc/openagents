import type {
  MultiSessionAccountSelector,
  MultiSessionPlanEntry,
} from "../../scripts/multi-session-run.js"

export type CoordinatorIntent = {
  intentId: string
  title: string
  body: string
  scopeHint?: string | readonly string[]
}

export type CoordinatorPlanOptions = {
  adapter?: MultiSessionPlanEntry["adapter"]
  availableAccounts: readonly MultiSessionAccountSelector[]
  repoRef?: MultiSessionPlanEntry["repoRef"]
  worktreePath?: string
  verify?: readonly string[]
  timeoutSeconds?: number
  noNetwork?: boolean
}

function slug(value: string, fallback: string): string {
  const normalized = value.toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "")
  return normalized || fallback
}

function normalizePart(value: string): string {
  return value.replace(/^\s*(?:[-*]|\d+[.)]|\[[ xX]\])\s+/, "").trim()
}

function partsFromIntent(intent: CoordinatorIntent): string[] {
  if (Array.isArray(intent.scopeHint)) {
    const parts = intent.scopeHint.map(normalizePart).filter(part => part.length > 0)
    if (parts.length > 0) return parts
  }

  if (typeof intent.scopeHint === "string" && intent.scopeHint.trim().length > 0) {
    const hinted = intent.scopeHint.split(/\r?\n|,/).map(normalizePart).filter(part => part.length > 0)
    if (hinted.length > 1) return hinted
  }

  const scopedChecklist = intent.body
    .split(/\r?\n/)
    .filter(line => /^\s*(?:[-*]|\d+[.)]|\[[ xX]\])\s+/.test(line))
    .map(normalizePart)
    .filter(part => part.length > 0)
  return scopedChecklist.length > 1 ? scopedChecklist : [intent.title.trim() || intent.intentId]
}

function workspaceSelector(options: CoordinatorPlanOptions): Pick<MultiSessionPlanEntry, "repoRef" | "worktreePath"> {
  if (options.repoRef !== undefined && options.worktreePath !== undefined) {
    throw new Error("coordinator planner options must use only one workspace selector")
  }
  if (options.repoRef !== undefined) return { repoRef: options.repoRef }
  if (options.worktreePath !== undefined) return { worktreePath: options.worktreePath }
  throw new Error("coordinator planner options need repoRef or worktreePath")
}

function objectiveFor(intent: CoordinatorIntent, part: string, total: number): string {
  const title = intent.title.trim() || intent.intentId
  if (total === 1) {
    return `Implement intent ${intent.intentId}: ${title}\n\n${intent.body.trim()}`.trim()
  }
  return `Implement intent ${intent.intentId} part: ${part}\n\nParent intent: ${title}\n\n${intent.body.trim()}`.trim()
}

export function planIntent(
  intent: CoordinatorIntent,
  options: CoordinatorPlanOptions,
): MultiSessionPlanEntry[] {
  const parts = partsFromIntent(intent)
  const workspace = workspaceSelector(options)
  const adapter = options.adapter ?? "codex"
  const verify = options.verify === undefined ? ["bun", "--version"] : [...options.verify]
  const accountPool = options.availableAccounts.map(account => ({ ...account }))
  const baseId = slug(intent.intentId, "intent")

  return parts.map((part, index) => ({
    id: parts.length === 1 ? baseId : `${baseId}-${String(index + 1).padStart(2, "0")}-${slug(part, "part")}`,
    adapter,
    ...workspace,
    objective: objectiveFor(intent, part, parts.length),
    verify: [...verify],
    ...(accountPool.length === 0 ? {} : { accountPool: accountPool.map(account => ({ ...account })) }),
    ...(options.timeoutSeconds === undefined ? {} : { timeoutSeconds: options.timeoutSeconds }),
    ...(options.noNetwork === undefined ? {} : { noNetwork: options.noNetwork }),
  }))
}
