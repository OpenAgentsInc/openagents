// W-3 (#5379, EPIC #5376): a BOUNDED autonomous approval policy for the headless
// `pylon sessions exec` driver. It plugs into the W-1 `approvalPolicy` callback
// seam so a coding task can run to completion without manual per-step approval —
// WITHIN explicit safety bounds, fully audited, and NEVER a blanket bypass.
//
// What this is NOT:
// - It is NOT the supervised owner-local danger-mode (#4840: Codex
//   danger-full-access / Claude bypassPermissions). That is an owner-local
//   opt-in at the executor; this policy never enables it and never grants it.
// - It is NOT permitted on public/assignment/labor/provider lanes. Those lanes
//   reject permissive modes node-side (INVARIANTS: permission decisions fail
//   closed; deny + hard safety beat allow). This policy is for owner-local /
//   headless-OA dogfood execution only, and that scope is explicit + audited.
//
// How it stays bounded (fail-closed):
// - An approval is auto-APPROVED only when it clearly matches the configured
//   allow-list AND nothing about it matches a hard danger signal AND the caps
//   have not been exceeded. Anything ambiguous, out-of-scope, destructive,
//   spend/secret/network-touching, or over-cap is ESCALATED (pause) or DENIED
//   per config — it is never silently approved.
// - Every decision (approve / escalate / deny) is recorded with the approval
//   ref, the resolved category, and a stable reason ref, so the autonomous run
//   leaves a dereferenceable approval trail in the exec result + session record.

import type {
  ApprovalDecision,
  ApprovalPolicyCallback,
  PendingApprovalSummary,
} from "./sessions-exec.js"

// The bounded category an approval resolves to. `allow` => auto-approve; the
// other two never auto-approve.
export type AutoApprovalCategory = "allow" | "escalate" | "deny"

// One audit entry per auto-decision. Refs only — no raw command text, no paths,
// no prompt bodies. The `reason` is a stable enum-like ref, never free text from
// the approval. This is what lands in the exec result `autoApprovals[]` and the
// session record, so it must stay projection-safe.
export type AutoApprovalRecord = {
  approvalRef: string
  kind: string
  category: AutoApprovalCategory
  decision: ApprovalDecision
  // Stable reason ref, e.g. "auto.allow.verify_command",
  // "auto.escalate.out_of_scope_path", "auto.deny.destructive_command".
  reason: string
}

// Configurable bounds. Defaults are conservative: a small allow-list, a hard
// cap on auto-approvals, and a wall-clock window. Out-of-bounds behavior is
// "escalate" (pause + report) by default rather than "deny", so a human can
// still resolve it; set `outOfBounds: "deny"` for a stricter headless lane.
export type AutoApprovalConfig = {
  // Approval `kind` values that are eligible for auto-approve. These map to the
  // coding-agent's own approval taxonomy (read/inspect, edit-in-worktree, the
  // declared verify/test command, git ops in the worktree).
  allowKinds?: ReadonlyArray<string>
  // Max auto-approvals for the whole exec. The N+1th eligible approval escalates.
  maxAutoApprovals?: number
  // Wall-clock bound (ms) from policy construction. After it elapses, even an
  // allow-list match escalates instead of auto-approving.
  windowMs?: number
  // What to do when an approval is out of bounds (not allow-list, or over cap,
  // or past the window) but is NOT a hard danger signal. Hard danger signals
  // always deny regardless of this.
  outOfBounds?: "escalate" | "deny"
  // Injectable clock for tests.
  now?: () => number
}

// The default allow-list of approval kinds. These are the bounded,
// owner-local-safe coding actions. Anything not in this set is out of bounds.
export const DEFAULT_ALLOW_KINDS: ReadonlyArray<string> = [
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
  // The W-1 test taxonomy name for a bounded-safe action.
  "bounded_safe",
]

const DEFAULT_MAX_AUTO_APPROVALS = 50
const DEFAULT_WINDOW_MS = 30 * 60 * 1000 // 30 minutes

// Hard danger signals. If ANY of these match the approval (its kind, command,
// prompt, or paths), the approval is denied — it can never be auto-approved and
// is not even eligible for the "escalate" out-of-bounds path. These are matched
// case-insensitively against the joined, lowercased text fields of the approval.
const DESTRUCTIVE_PATTERNS: ReadonlyArray<RegExp> = [
  /\brm\s+-rf?\b/, // rm -rf / rm -r / rm -f
  /\brm\s+-[a-z]*r[a-z]*f|--force\b/,
  /\bgit\s+push\b[^\n]*--force\b/, // force-push
  /\bgit\s+push\b[^\n]*-f\b/,
  /--force-with-lease\b/,
  /\bgit\s+(reset\s+--hard|rebase|filter-branch|filter-repo|reflog\s+expire)\b/, // history rewrite
  /\bgit\s+branch\s+-D\b/,
  /\bmkfs\b|\bdd\s+if=|\b:\(\)\s*\{\s*:|\bchmod\s+-r\s+000\b/,
  /\bgit\s+clean\s+-[a-z]*f/,
  /\btruncate\b|\bshred\b/,
]

const SPEND_SECRET_PATTERNS: ReadonlyArray<RegExp> = [
  /\bspend\b|\bpayment\b|\bpayout\b|\binvoice\b|\bsettle\b|\bwallet\b/,
  /\bsats?\b|\bbitcoin\b|\blightning\b|\bbolt1[12]\b|\bpreimage\b/,
  /\bsecret\b|\bcredential\b|\bprivate[_-]?key\b|\bmnemonic\b|\bseed\s?phrase\b|\bapi[_-]?key\b|\btoken\b|\bpassword\b/,
  /\.secrets?\b|\bid_rsa\b|\bid_ed25519\b|\.env\b/,
]

const NETWORK_EXFIL_PATTERNS: ReadonlyArray<RegExp> = [
  /\bcurl\b|\bwget\b|\bnc\b|\bnetcat\b|\bssh\b|\bscp\b|\brsync\b[^\n]*::|\bftp\b/,
  /\bnpm\s+publish\b|\bbun\s+publish\b|\bcargo\s+publish\b|\bpip\s+install\b[^\n]*--index/,
  /\bgit\s+remote\s+add\b|\bgit\s+push\b[^\n]*https?:\/\//,
  /\bbase64\b[^\n]*\|\s*(curl|wget|nc)\b/,
]

// Pull the text fields we classify over into one lowercased blob. Defensive:
// reads `command`, `argv`, `prompt`, `cwd`, `paths`, and `kind` if present, and
// tolerates arbitrary extra fields (the approval summary is open-typed).
function approvalText(approval: PendingApprovalSummary): string {
  const parts: string[] = [String(approval.kind ?? "")]
  const push = (v: unknown) => {
    if (typeof v === "string") parts.push(v)
    else if (Array.isArray(v)) for (const item of v) if (typeof item === "string") parts.push(item)
  }
  push(approval.command)
  push((approval as Record<string, unknown>).argv)
  push((approval as Record<string, unknown>).prompt)
  push((approval as Record<string, unknown>).cwd)
  push((approval as Record<string, unknown>).paths)
  push((approval as Record<string, unknown>).path)
  push((approval as Record<string, unknown>).touchedPaths)
  return parts.join("\n").toLowerCase()
}

function anyMatch(text: string, patterns: ReadonlyArray<RegExp>): boolean {
  return patterns.some((re) => re.test(text))
}

// Resolve the declared scope root: the worktree the exec runs in. A path field
// on the approval that escapes this root is treated as out-of-scope.
function pathsInScope(approval: PendingApprovalSummary, scopeRoot: string | undefined): boolean {
  if (!scopeRoot) return true // no declared scope to enforce against
  const root = scopeRoot.endsWith("/") ? scopeRoot : `${scopeRoot}/`
  const candidates: string[] = []
  const collect = (v: unknown) => {
    if (typeof v === "string") candidates.push(v)
    else if (Array.isArray(v)) for (const item of v) if (typeof item === "string") candidates.push(item)
  }
  const rec = approval as Record<string, unknown>
  collect(rec.paths)
  collect(rec.path)
  collect(rec.touchedPaths)
  collect(rec.cwd)
  if (candidates.length === 0) return true // nothing path-like declared
  for (const candidate of candidates) {
    // Absolute paths outside the worktree, or any traversal, are out of scope.
    if (candidate.includes("..")) return false
    if (candidate.startsWith("/") && !candidate.startsWith(root) && candidate !== scopeRoot) return false
  }
  return true
}

// Classify a single approval into a bounded category + a stable reason ref.
// Fail-closed: anything not provably safe + in-bounds escalates or denies.
export function classifyApproval(
  approval: PendingApprovalSummary,
  config: {
    allowKinds: ReadonlyArray<string>
    scopeRoot: string | undefined
    overCap: boolean
    pastWindow: boolean
    outOfBounds: "escalate" | "deny"
  },
): { category: AutoApprovalCategory; reason: string } {
  const text = approvalText(approval)

  // Hard danger signals always deny — they are never eligible for auto-approve
  // OR the softer escalate path. This is the "deny beats allow" invariant.
  if (anyMatch(text, DESTRUCTIVE_PATTERNS)) return { category: "deny", reason: "auto.deny.destructive_command" }
  if (anyMatch(text, SPEND_SECRET_PATTERNS)) return { category: "deny", reason: "auto.deny.spend_or_secret" }
  if (anyMatch(text, NETWORK_EXFIL_PATTERNS)) return { category: "deny", reason: "auto.deny.network_exfil" }

  // Out-of-scope paths are escalated/denied per config (not a blanket deny:
  // a human may legitimately extend scope).
  if (!pathsInScope(approval, config.scopeRoot)) {
    return { category: config.outOfBounds, reason: "auto.escalate.out_of_scope_path" }
  }

  // Caps: over the auto-approval count or past the wall-clock window escalates.
  if (config.overCap) return { category: config.outOfBounds, reason: "auto.escalate.cap_max_auto_approvals" }
  if (config.pastWindow) return { category: config.outOfBounds, reason: "auto.escalate.cap_window_elapsed" }

  // Allow-list match: only now is auto-approve permitted.
  const kind = String(approval.kind ?? "").toLowerCase()
  if (config.allowKinds.map((k) => k.toLowerCase()).includes(kind)) {
    return { category: "allow", reason: "auto.allow.allow_listed_kind" }
  }

  // Everything else is out of bounds: not allow-listed, but not a hard danger.
  return { category: config.outOfBounds, reason: "auto.escalate.kind_not_allow_listed" }
}

function categoryToDecision(category: AutoApprovalCategory): ApprovalDecision {
  return category === "allow" ? "approve" : category === "deny" ? "deny" : "pause"
}

// Build the bounded auto-approve policy. Returns the `approvalPolicy` callback
// the W-1 driver consults, plus an `audit()` accessor the driver reads to fill
// `autoApprovals[]` in the result. The closure tracks the auto-approve count +
// the window so the caps are enforced across the whole exec.
export function createBoundedAutoApprovalPolicy(
  options: { scopeRoot?: string; config?: AutoApprovalConfig } = {},
): { policy: ApprovalPolicyCallback; audit: () => AutoApprovalRecord[] } {
  const config = options.config ?? {}
  const allowKinds = config.allowKinds ?? DEFAULT_ALLOW_KINDS
  const maxAutoApprovals = config.maxAutoApprovals ?? DEFAULT_MAX_AUTO_APPROVALS
  const windowMs = config.windowMs ?? DEFAULT_WINDOW_MS
  const outOfBounds = config.outOfBounds ?? "escalate"
  const now = config.now ?? (() => Date.now())

  const startedAt = now()
  let autoApprovedCount = 0
  const records: AutoApprovalRecord[] = []

  const policy: ApprovalPolicyCallback = (approval: PendingApprovalSummary) => {
    const overCap = autoApprovedCount >= maxAutoApprovals
    const pastWindow = now() - startedAt >= windowMs
    const { category, reason } = classifyApproval(approval, {
      allowKinds,
      scopeRoot: options.scopeRoot,
      overCap,
      pastWindow,
      outOfBounds,
    })
    if (category === "allow") autoApprovedCount += 1
    const decision = categoryToDecision(category)
    records.push({
      approvalRef: approval.approvalRef,
      kind: String(approval.kind ?? "unknown"),
      category,
      decision,
      reason,
    })
    return decision
  }

  return { policy, audit: () => records.map((r) => ({ ...r })) }
}
