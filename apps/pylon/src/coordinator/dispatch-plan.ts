import { join } from "node:path"

import type {
  MultiSessionAccountSelector,
  MultiSessionPlanEntry,
} from "../../scripts/multi-session-run.js"

export type CoordinatorDispatchPlan = {
  sessions: MultiSessionPlanEntry[]
  accountPool: MultiSessionAccountSelector[]
}

export type CoordinatorDispatchPlanOptions = {
  accountPool: Array<{ codexHome: string }>
  worktreeBase: string
}

function safePathPart(value: string | undefined, fallback: string): string {
  if (value === undefined) return fallback
  const normalized = value.toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "")
  return normalized || fallback
}

export function toMultiSessionPlan(
  intentPlan: readonly MultiSessionPlanEntry[],
  opts: CoordinatorDispatchPlanOptions,
): CoordinatorDispatchPlan {
  return {
    sessions: intentPlan.map((entry, index) => {
      const worktreePath = join(
        opts.worktreeBase,
        `${String(index + 1).padStart(2, "0")}-${safePathPart(entry.id, "session")}`,
      )
      const { repoRef: _repoRef, worktreePath: _worktreePath, accountPool: _accountPool, ...runnable } = entry

      return {
        ...runnable,
        verify: [...entry.verify],
        worktreePath,
      }
    }),
    accountPool: opts.accountPool.map(account => ({ ...account })),
  }
}
