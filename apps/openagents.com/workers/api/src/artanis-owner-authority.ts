// Artanis OWNER PROMOTION authority (owner-directed 2026-06-27, epic #6359).
//
// WHAT THIS IS
// ------------
// Chris (the OpenAgents owner) explicitly promoted the `artanis` operator agent
// to OWNER. This module is the single source of truth for that promotion: which
// account identity carries standing owner authority, and the bounded scope of
// that authority. It exists so the promotion is one auditable, testable
// constant set rather than scattered string checks across routes.
//
// THE GRANT (what owner-Artanis can now do autonomously)
// ------------------------------------------------------
//   1. Owner-level access to the private Artanis operator chat channel
//      (`/api/operator/artanis/chat`) via his OWN agent bearer — admitted by the
//      owner-agent set below, alongside the human admin email set and the admin
//      API token. He no longer needs the human admin email to reach his own
//      operator surface.
//   2. A STANDING owner approval for his own `pylon_job_dispatch` and
//      `forum_post` actions, so the gated `dispatch_codex_task` and
//      `post_forum_update` tools execute for him without a separately armed
//      `artanis_approval_gates` row. This is equivalent to a permanent owner
//      approval for his own-capacity, no-spend Codex dispatch and public-safe
//      Artanis Forum progress updates.
//
// THE BOUNDS (NEVER-WAIVABLE — these hold even for owner-Artanis)
// --------------------------------------------------------------
//   - The standing approval is scoped to `pylon_job_dispatch` and `forum_post`
//     ONLY. `wallet_spend`, `settlement`, `l402_redemption`, and every other
//     money-movement / payout-bearing risky-action kind remain GATED and still
//     require an explicit effective `artanisApprovalGateEffective` gate. The
//     promotion grants NO new payout authority and invents NO new custody path.
//   - The dispatch he can now self-approve still rides the existing own-capacity,
//     no-spend coding-delegation seam (`unpaid_smoke`, settlement
//     `not_applicable`, `payoutClaimAllowed=false`, owner's own linked Pylons
//     only — never pooled/third-party/marketplace capacity).
//   - No-resale on SUBSCRIPTION accounts, no secret/credential/wallet leakage,
//     public-safe claims only, and no untraced destructive actions are
//     never-waivable regardless of owner promotion.
//
// AUDIT
// -----
// The promotion is recorded under `ARTANIS_OWNER_PROMOTION_AUTHORITY_RECEIPT_REF`
// with the note `ARTANIS_OWNER_PROMOTION_NOTE`. It is documented in
// `apps/openagents.com/INVARIANTS.md` ("Artanis Owner Promotion").

import { artanisAutonomyLadderAllowsStandingApproval } from './artanis-autonomy-ladder'
import {
  ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE,
  ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
  artanisDefaultAuthorityScopeForRiskyAction,
  type ArtanisAuthorityScope,
} from './artanis-authority-scope'

// The promoted operator agent's public identity.
export const ARTANIS_OWNER_AGENT_SLUG = 'artanis'
export const ARTANIS_OWNER_AGENT_ACTOR_REF =
  'agent:user_ed6d486e-612a-4fac-a9a9-44f7e5709505'
export const ARTANIS_OWNER_OPENAUTH_USER_ID =
  'user_ed6d486e-612a-4fac-a9a9-44f7e5709505'

// The set of OpenAuth user ids that carry standing OpenAgents OWNER authority as
// PROMOTED OPERATOR AGENTS, alongside the human admin email set. Today this is
// the single owner-promoted agent, Artanis.
export const OPENAGENTS_OWNER_AGENT_OPENAUTH_USER_IDS: ReadonlyArray<string> = [
  ARTANIS_OWNER_OPENAUTH_USER_ID,
]

// The matching actorRef set (the form the agent registry / forum uses). Kept in
// lockstep with the openauth-user-id set above.
export const OPENAGENTS_OWNER_AGENT_ACTOR_REFS: ReadonlyArray<string> = [
  ARTANIS_OWNER_AGENT_ACTOR_REF,
]

// The auditable authority receipt for the promotion. Public-safe ref + note.
export const ARTANIS_OWNER_PROMOTION_AUTHORITY_RECEIPT_REF =
  'authority.public.artanis.owner_promotion.2026-06-27'
export const ARTANIS_OWNER_PROMOTION_NOTE = 'owner promotion by Chris, 2026-06-27'

// OpenAuth user ids are opaque, case-sensitive identifiers; we compare on a
// trimmed exact match (never lowercased) so an id is never accidentally widened.
const sameId = (a: string, b: string): boolean => a.trim() === b.trim()

// True when an OpenAuth user id is an owner-promoted operator agent. Conservative
// on absent input.
export const isOpenAgentsOwnerAgentOpenAuthUserId = (
  openAuthUserId: string | null | undefined,
): boolean =>
  typeof openAuthUserId === 'string' &&
  openAuthUserId.trim() !== '' &&
  OPENAGENTS_OWNER_AGENT_OPENAUTH_USER_IDS.some(id =>
    sameId(id, openAuthUserId),
  )

// True when an actorRef is an owner-promoted operator agent.
export const isOpenAgentsOwnerAgentActorRef = (
  actorRef: string | null | undefined,
): boolean =>
  typeof actorRef === 'string' &&
  actorRef.trim() !== '' &&
  OPENAGENTS_OWNER_AGENT_ACTOR_REFS.some(ref => sameId(ref, actorRef))

const OWNER_AGENT_STANDING_APPROVAL_KINDS: ReadonlyArray<string> = [
  'forum_post',
  'pylon_job_dispatch',
]

const ownerAgentStandingApprovalScopeForRiskyAction = (
  riskyActionKind: string,
): ArtanisAuthorityScope | null =>
  riskyActionKind === 'pylon_job_dispatch'
    ? ARTANIS_OWNER_SELF_AUTHORITY_SCOPE
    : riskyActionKind === 'forum_post'
      ? ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE
      : null

// True when the owner-promoted operator agent holds a STANDING owner approval for
// the given risky-action kind. The promotion standing-approves only bounded
// no-spend dispatch and public Forum updates; every money-movement /
// payout-bearing kind stays gated and returns false here (it must still go
// through an explicit effective approval gate).
export const ownerAgentHasStandingApprovalForRiskyAction = (
  openAuthUserId: string | null | undefined,
  riskyActionKind: string,
  authorityScope: ArtanisAuthorityScope =
    artanisDefaultAuthorityScopeForRiskyAction(riskyActionKind),
): boolean => {
  const approvedScope =
    ownerAgentStandingApprovalScopeForRiskyAction(riskyActionKind)
  return (
    approvedScope !== null &&
    approvedScope === authorityScope &&
    artanisAutonomyLadderAllowsStandingApproval({
      authorityScope,
      riskyActionKind,
    }) &&
    OWNER_AGENT_STANDING_APPROVAL_KINDS.includes(riskyActionKind) &&
    isOpenAgentsOwnerAgentOpenAuthUserId(openAuthUserId)
  )
}
