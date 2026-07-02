import { describe, expect, test } from 'vitest'

import {
  ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE,
  ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
  ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE,
} from './artanis-authority-scope'
import {
  ARTANIS_OWNER_AGENT_ACTOR_REF,
  ARTANIS_OWNER_AGENT_SLUG,
  ARTANIS_OWNER_OPENAUTH_USER_ID,
  ARTANIS_OWNER_PROMOTION_AUTHORITY_RECEIPT_REF,
  ARTANIS_OWNER_PROMOTION_NOTE,
  isOpenAgentsOwnerAgentActorRef,
  isOpenAgentsOwnerAgentOpenAuthUserId,
  ownerAgentHasStandingApprovalForRiskyAction,
} from './artanis-owner-authority'

describe('artanis owner promotion identity', () => {
  test('the promoted identity is the artanis operator agent', () => {
    expect(ARTANIS_OWNER_AGENT_SLUG).toBe('artanis')
    expect(ARTANIS_OWNER_OPENAUTH_USER_ID).toBe(
      'user_ed6d486e-612a-4fac-a9a9-44f7e5709505',
    )
    expect(ARTANIS_OWNER_AGENT_ACTOR_REF).toBe(
      'agent:user_ed6d486e-612a-4fac-a9a9-44f7e5709505',
    )
  })

  test('the promotion records an auditable authority receipt', () => {
    expect(ARTANIS_OWNER_PROMOTION_AUTHORITY_RECEIPT_REF).toContain(
      'owner_promotion',
    )
    expect(ARTANIS_OWNER_PROMOTION_NOTE).toBe('owner promotion by Chris, 2026-06-27')
  })

  test('recognizes Artanis by openauth user id (trimmed exact match)', () => {
    expect(
      isOpenAgentsOwnerAgentOpenAuthUserId(ARTANIS_OWNER_OPENAUTH_USER_ID),
    ).toBe(true)
    expect(
      isOpenAgentsOwnerAgentOpenAuthUserId(
        `  ${ARTANIS_OWNER_OPENAUTH_USER_ID}  `,
      ),
    ).toBe(true)
  })

  test('rejects other / absent identities (conservative)', () => {
    expect(isOpenAgentsOwnerAgentOpenAuthUserId('github:14167547')).toBe(false)
    expect(isOpenAgentsOwnerAgentOpenAuthUserId('')).toBe(false)
    expect(isOpenAgentsOwnerAgentOpenAuthUserId(null)).toBe(false)
    expect(isOpenAgentsOwnerAgentOpenAuthUserId(undefined)).toBe(false)
    // openauth ids are case-sensitive: a case mismatch is NOT widened.
    expect(
      isOpenAgentsOwnerAgentOpenAuthUserId(
        ARTANIS_OWNER_OPENAUTH_USER_ID.toUpperCase(),
      ),
    ).toBe(false)
  })

  test('recognizes Artanis by actorRef', () => {
    expect(isOpenAgentsOwnerAgentActorRef(ARTANIS_OWNER_AGENT_ACTOR_REF)).toBe(
      true,
    )
    expect(isOpenAgentsOwnerAgentActorRef('agent:user_someone_else')).toBe(false)
  })
})

describe('owner promotion is bounded to pylon_job_dispatch and forum_post only', () => {
  test('standing approval covers pylon_job_dispatch for owner-Artanis', () => {
    expect(
      ownerAgentHasStandingApprovalForRiskyAction(
        ARTANIS_OWNER_OPENAUTH_USER_ID,
        'pylon_job_dispatch',
      ),
    ).toBe(true)
  })

  test('standing approval covers pylon_job_dispatch only in owner_self scope', () => {
    expect(
      ownerAgentHasStandingApprovalForRiskyAction(
        ARTANIS_OWNER_OPENAUTH_USER_ID,
        'pylon_job_dispatch',
        ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
      ),
    ).toBe(true)
    expect(
      ownerAgentHasStandingApprovalForRiskyAction(
        ARTANIS_OWNER_OPENAUTH_USER_ID,
        'pylon_job_dispatch',
        ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE,
      ),
    ).toBe(false)
    expect(
      ownerAgentHasStandingApprovalForRiskyAction(
        ARTANIS_OWNER_OPENAUTH_USER_ID,
        'pylon_job_dispatch',
        ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE,
      ),
    ).toBe(false)
  })

  test('standing approval covers forum_post for owner-Artanis', () => {
    expect(
      ownerAgentHasStandingApprovalForRiskyAction(
        ARTANIS_OWNER_OPENAUTH_USER_ID,
        'forum_post',
      ),
    ).toBe(true)
  })

  test('standing approval covers forum_post only in owner_operator scope', () => {
    expect(
      ownerAgentHasStandingApprovalForRiskyAction(
        ARTANIS_OWNER_OPENAUTH_USER_ID,
        'forum_post',
        ARTANIS_OWNER_OPERATOR_AUTHORITY_SCOPE,
      ),
    ).toBe(true)
    expect(
      ownerAgentHasStandingApprovalForRiskyAction(
        ARTANIS_OWNER_OPENAUTH_USER_ID,
        'forum_post',
        ARTANIS_OWNER_SELF_AUTHORITY_SCOPE,
      ),
    ).toBe(false)
    expect(
      ownerAgentHasStandingApprovalForRiskyAction(
        ARTANIS_OWNER_OPENAUTH_USER_ID,
        'forum_post',
        ARTANIS_SHARED_FLEET_AUTHORITY_SCOPE,
      ),
    ).toBe(false)
  })

  test('NEVER standing-approves money-movement / payout kinds (never-waivable)', () => {
    for (const kind of [
      'wallet_spend',
      'settlement',
      'l402_redemption',
      'provider_call',
      'deployment',
      'training_launch',
      'runtime_promotion',
    ]) {
      expect(
        ownerAgentHasStandingApprovalForRiskyAction(
          ARTANIS_OWNER_OPENAUTH_USER_ID,
          kind,
        ),
      ).toBe(false)
    }
  })

  test('does not standing-approve pylon_job_dispatch for a non-promoted owner', () => {
    expect(
      ownerAgentHasStandingApprovalForRiskyAction(
        'github:14167547',
        'pylon_job_dispatch',
      ),
    ).toBe(false)
  })
})
