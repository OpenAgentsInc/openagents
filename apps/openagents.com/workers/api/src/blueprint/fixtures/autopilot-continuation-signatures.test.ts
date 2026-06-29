import { describe, expect, test } from 'vitest'

import {
  blueprintModuleVersionIsProduction,
  blueprintModuleVersionRequiresOperatorPromotion,
} from '../schemas/module'
import { blueprintReleaseGateCanPromote } from '../schemas/release-gate'
import {
  AUTOPILOT_CONTINUATION_ACTIONS,
  AUTOPILOT_CONTINUATION_MODULE_VERSIONS,
  AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES,
  AUTOPILOT_CONTINUATION_RELEASE_GATES,
} from './autopilot-continuation-signatures'

describe('Autopilot continuation Program Signature catalog', () => {
  test('covers every required continuation action with a signature, module, and release gate', () => {
    expect(AUTOPILOT_CONTINUATION_ACTIONS).toEqual([
      'continue',
      'email_decisioning',
      'escalate',
      'fix',
      'prepare_review',
      'proof_projection',
      'request_context',
      'research_policy',
      'retry_account',
      'route_selection',
      'stop',
      'summarize',
      'test',
    ])
    expect(AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES).toHaveLength(
      AUTOPILOT_CONTINUATION_ACTIONS.length,
    )
    expect(AUTOPILOT_CONTINUATION_MODULE_VERSIONS).toHaveLength(
      AUTOPILOT_CONTINUATION_ACTIONS.length,
    )
    expect(AUTOPILOT_CONTINUATION_RELEASE_GATES).toHaveLength(
      AUTOPILOT_CONTINUATION_ACTIONS.length,
    )
  })

  test('keeps seeded signatures draft and module versions unpromoted', () => {
    expect(
      AUTOPILOT_CONTINUATION_PROGRAM_SIGNATURES.every(
        signature => signature.status === 'draft',
      ),
    ).toBe(true)
    expect(
      AUTOPILOT_CONTINUATION_MODULE_VERSIONS.every(
        moduleVersion =>
          !blueprintModuleVersionIsProduction(moduleVersion) &&
          blueprintModuleVersionRequiresOperatorPromotion(moduleVersion),
      ),
    ).toBe(true)
  })

  test('requires release gates before continuation catalog promotion', () => {
    expect(
      AUTOPILOT_CONTINUATION_RELEASE_GATES.every(
        gate => !blueprintReleaseGateCanPromote(gate),
      ),
    ).toBe(true)
  })
})
