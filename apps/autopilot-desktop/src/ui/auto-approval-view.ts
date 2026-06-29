// #5468 (EPIC #5461): a PURE projection module that surfaces the BOUNDED
// auto-approve policy + its per-decision audit trail in the Supervise/Decisions
// roll-up.
//
// Honest scope (read this before extending):
// - The authoritative policy is the Pylon runtime: `apps/pylon/src/node/
//   auto-approval-policy.ts` (`--on-approval auto` on `pylon sessions exec`).
//   It is fail-closed: an approval is auto-APPROVED only when its `kind` is on
//   the allow-list AND nothing matches a hard danger signal AND the caps are not
//   exceeded; destructive / spend-or-secret / network-exfil ALWAYS deny;
//   out-of-scope / out-of-cap / over-window ESCALATE. This module never decides
//   anything — it only DISPLAYS what that policy grants and what it actually did.
// - The audit trail (`autoApprovals[]`) is produced by the headless `sessions
//   exec` driver and lives in the exec result + session record. It is refs +
//   stable reason enums only — NO raw command/path/prompt text. This module
//   keeps that projection-safe contract: it shows refs, kinds, categories, and
//   reason enums, never free text.
// - The bounds shown here (allow-list, hard-deny categories, caps, window) are
//   mirrored from the real policy module. The provenance pointer below is the
//   contract: when the runtime policy changes, update this display mirror in the
//   same change. Desktop must not import the node-side policy directly because
//   that pulls the Pylon runtime graph into the browser typecheck.
//
// Why a separate module: it is pure (no Foldkit Message, no RPC, no node deps),
// mirrors the existing `packages/autopilot-control-protocol/src/approvals-view.ts`
// pattern, and is independently unit-testable. view.ts renders it; model.ts and
// the control protocol carry the optional audit field through unchanged-by-default.

// Mirrored from apps/pylon/src/node/auto-approval-policy.ts.
export type AutoApprovalCategory = "allow" | "escalate" | "deny"

const DEFAULT_ALLOW_KINDS: ReadonlyArray<string> = [
  "read",
  "inspect",
  "file_read",
  "file_inspect",
  "edit",
  "file_edit",
  "patch",
  "apply_patch",
  "write_file",
  "verify",
  "verify_command",
  "test",
  "test_command",
  "dev_check",
  "git",
  "git_command",
  "worktree_edit",
  "bounded_safe",
]

// ── Audit trail rows (what the auto policy actually decided) ────────────────
// The wire/record shape, kept structurally identical to the Pylon exec result
// `autoApprovals[]` entries (refs + enums only). Re-declared here (not imported
// as a value type) so the desktop projection owns its own defensive parser.
export type AutoApprovalAuditEntry = Readonly<{
  approvalRef: string
  kind: string
  category: AutoApprovalCategory
  // The decision the policy returned ("approve" | "deny" | "pause"). Refs only.
  decision: string
  // Stable reason ref, e.g. "auto.allow.allow_listed_kind",
  // "auto.deny.destructive_command", "auto.escalate.out_of_scope_path".
  reason: string
}>

// A display-ready audit row: the raw refs plus a human label for the category
// and a plain-English reason gloss. Still refs-only — the gloss is derived from
// the stable reason ENUM, never from any command text.
export type AutoApprovalAuditRow = AutoApprovalAuditEntry &
  Readonly<{
    categoryLabel: string
    // True only for `allow`. The two non-allow categories are surfaced honestly
    // as "escalated to you" / "denied", never as silent approvals.
    autoApproved: boolean
    reasonGloss: string
  }>

// ── The static policy summary (what the bounded mode is allowed to do) ──────
export type AutoApprovalCategorySummary = Readonly<{
  id: AutoApprovalCategory
  label: string
  description: string
}>

export type AutoApprovalPolicySummary = Readonly<{
  // Refs to the authoritative runtime so the UI can be honest about provenance.
  policyRef: string
  cliFlag: string
  failClosed: true
  // The allow-listed approval kinds eligible for auto-approve (everything else
  // is out of bounds → escalate/deny).
  allowKinds: ReadonlyArray<string>
  // The default bounds (count cap + wall-clock window) the policy ships with.
  defaultMaxAutoApprovals: number
  defaultWindowMinutes: number
  // The categories that ALWAYS escalate or deny — they can never auto-approve.
  alwaysEscalates: ReadonlyArray<string>
  // The three bounded categories, for legend rendering.
  categories: ReadonlyArray<AutoApprovalCategorySummary>
}>

// Mirrored from `apps/pylon/src/node/auto-approval-policy.ts` (the private
// `DEFAULT_MAX_AUTO_APPROVALS` / `DEFAULT_WINDOW_MS`). Provenance: if the policy
// defaults change there, update these two constants in lock-step.
const POLICY_DEFAULT_MAX_AUTO_APPROVALS = 50
const POLICY_DEFAULT_WINDOW_MINUTES = 30

// The danger categories the policy hard-denies (it never auto-approves them and
// they are not even eligible for the softer escalate path). Honest copy: these
// always escalate to the operator / deny — never silently approved.
const ALWAYS_ESCALATES: ReadonlyArray<string> = [
  "Destructive commands (rm -rf, force-push, history rewrite, mkfs/dd)",
  "Spend, payments, wallets, or secrets (keys, tokens, .env, mnemonics)",
  "Network exfiltration (curl/wget/ssh/scp, publish, remote push)",
  "Anything out of the worktree scope or past the count/time caps",
]

const CATEGORY_SUMMARIES: ReadonlyArray<AutoApprovalCategorySummary> = [
  {
    id: "allow",
    label: "Auto-approved",
    description: "Allow-listed, in-scope, and within caps — approved without you.",
  },
  {
    id: "escalate",
    label: "Escalated to you",
    description: "Out of bounds but not dangerous — paused for your decision.",
  },
  {
    id: "deny",
    label: "Denied",
    description: "Hard danger signal — denied outright, never auto-approved.",
  },
]

// The single, honest policy summary the Supervise surface renders. It reflects
// the runtime's real bounds, not a mock.
export const boundedAutoApprovalPolicySummary: AutoApprovalPolicySummary = {
  policyRef: "apps/pylon/src/node/auto-approval-policy.ts",
  cliFlag: "pylon sessions exec --on-approval auto",
  failClosed: true,
  allowKinds: [...DEFAULT_ALLOW_KINDS],
  defaultMaxAutoApprovals: POLICY_DEFAULT_MAX_AUTO_APPROVALS,
  defaultWindowMinutes: POLICY_DEFAULT_WINDOW_MINUTES,
  alwaysEscalates: [...ALWAYS_ESCALATES],
  categories: CATEGORY_SUMMARIES,
}

// ── Defensive projection of the audit trail (refs-only, fail-soft) ──────────
type RawRecord = Record<string, unknown>

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function readString(value: unknown, fallback: string): string {
  return typeof value === "string" && value.length > 0 ? value : fallback
}

function parseCategory(value: unknown): AutoApprovalCategory {
  // Fail-closed display: an unknown/missing category is shown as "escalate"
  // (paused-to-you), never as an auto-approval.
  return value === "allow" || value === "deny" ? value : "escalate"
}

const CATEGORY_LABELS: Readonly<Record<AutoApprovalCategory, string>> = {
  allow: "Auto-approved",
  escalate: "Escalated to you",
  deny: "Denied",
}

// Plain-English gloss for the stable reason enum. New reasons fall back to the
// raw ref (still safe — reasons are enums, not free text).
const REASON_GLOSSES: Readonly<Record<string, string>> = {
  "auto.allow.allow_listed_kind": "kind is on the bounded allow-list",
  "auto.deny.destructive_command": "matched a destructive command pattern",
  "auto.deny.spend_or_secret": "touched spend, payment, wallet, or a secret",
  "auto.deny.network_exfil": "matched a network exfiltration pattern",
  "auto.escalate.out_of_scope_path": "path is outside the worktree scope",
  "auto.escalate.cap_max_auto_approvals": "hit the auto-approval count cap",
  "auto.escalate.cap_window_elapsed": "past the auto-approval time window",
  "auto.escalate.kind_not_allow_listed": "kind is not on the allow-list",
}

export function projectAutoApprovalAudit(records: unknown): ReadonlyArray<AutoApprovalAuditRow> {
  if (!Array.isArray(records)) return []
  const rows: AutoApprovalAuditRow[] = []
  for (const record of records) {
    if (!isRecord(record)) continue
    const category = parseCategory(record.category)
    const reason = readString(record.reason, "auto.unknown")
    rows.push({
      approvalRef: readString(record.approvalRef, "(unknown ref)"),
      kind: readString(record.kind, "unknown"),
      category,
      decision: readString(
        record.decision,
        category === "allow" ? "approve" : category === "deny" ? "deny" : "pause",
      ),
      reason,
      categoryLabel: CATEGORY_LABELS[category],
      autoApproved: category === "allow",
      reasonGloss: REASON_GLOSSES[reason] ?? reason,
    })
  }
  return rows
}

// A small honest roll-up of the audit trail for the card header.
export type AutoApprovalAuditSummary = Readonly<{
  total: number
  autoApproved: number
  escalated: number
  denied: number
  // True when the policy is observably active (any audited decision exists).
  active: boolean
}>

export function summarizeAutoApprovalAudit(
  rows: ReadonlyArray<AutoApprovalAuditRow>,
): AutoApprovalAuditSummary {
  let autoApproved = 0
  let escalated = 0
  let denied = 0
  for (const row of rows) {
    if (row.category === "allow") autoApproved += 1
    else if (row.category === "deny") denied += 1
    else escalated += 1
  }
  return {
    total: rows.length,
    autoApproved,
    escalated,
    denied,
    active: rows.length > 0,
  }
}
