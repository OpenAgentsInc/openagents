// CL-16 approvals. A read-first queue of pending operator approvals (e.g. a
// labor job's first-run gate). The list is read-only; the only mutation is
// resolve(approve/deny/answer), which is EXACTLY-ONCE per approval — duplicate
// resolutions return the original decision and never re-apply. Backed by the
// shared protocol ledger so the exactly-once semantics match the clients.

import { createApprovalLedger, type ApprovalDecision } from "@openagentsinc/autopilot-control-protocol"

export type PendingApproval = {
  approvalRef: string
  kind: string
  prompt: string
  createdAt: string
  // Optional labor binding so resolving "approve" can grant the real first-run
  // approval for that job type / policy.
  jobType?: string
  policyRef?: string
}

export type ApprovalResolution = {
  approvalRef: string
  decision: ApprovalDecision
  answer?: string
  resolvedAt: string
}

export type ResolveResult = {
  applied: boolean
  duplicate: boolean
  decision: ApprovalDecision
  // The pending entry that was resolved (present on the first, applied resolve).
  resolved?: PendingApproval
  error?: string
}

export type ApprovalQueue = {
  enqueue: (input: Omit<PendingApproval, "createdAt"> & { createdAt?: string }) => PendingApproval
  list: () => PendingApproval[]
  history: () => ApprovalResolution[]
  resolve: (approvalRef: string, decision: ApprovalDecision, opts?: { answer?: string; now?: string }) => ResolveResult
}

export function createApprovalQueue(): ApprovalQueue {
  const pending = new Map<string, PendingApproval>()
  const resolutions: ApprovalResolution[] = []
  const ledger = createApprovalLedger()

  return {
    enqueue(input) {
      const ref = String(input.approvalRef ?? "").trim()
      if (ref.length === 0) throw new Error("approvalRef is required")
      const existing = pending.get(ref)
      if (existing !== undefined) return existing // idempotent enqueue
      const entry: PendingApproval = {
        approvalRef: ref,
        kind: input.kind,
        prompt: input.prompt,
        createdAt: input.createdAt ?? new Date().toISOString(),
        ...(input.jobType ? { jobType: input.jobType } : {}),
        ...(input.policyRef ? { policyRef: input.policyRef } : {}),
      }
      pending.set(ref, entry)
      return entry
    },

    list() {
      return [...pending.values()]
    },

    history() {
      return resolutions.map((r) => ({ ...r }))
    },

    resolve(approvalRef, decision, opts) {
      if (decision === "answer" && (opts?.answer ?? "").trim().length === 0) {
        return { applied: false, duplicate: false, decision, error: "answer_required" }
      }
      // Exactly-once: the ledger keeps the first decision for a key forever.
      const record = ledger.record(approvalRef, decision)
      if (record.duplicate) {
        return { applied: false, duplicate: true, decision: record.decision }
      }
      const resolved = pending.get(approvalRef)
      pending.delete(approvalRef)
      resolutions.push({
        approvalRef,
        decision,
        ...(opts?.answer ? { answer: opts.answer } : {}),
        resolvedAt: opts?.now ?? new Date().toISOString(),
      })
      return { applied: true, duplicate: false, decision, ...(resolved ? { resolved } : {}) }
    },
  }
}
