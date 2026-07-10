import { describe, expect, test } from 'vitest'

import { materializeHttpResult } from './http/responses'
import { handlePublicProofReplayBundleRequest } from './public-proof-replay-routes'

const archivedReplayPaths = [
  '/api/public/proof-replays',
  '/api/public/tassadar-replays/first-real-settlement',
]

describe('archived public proof replay routes', () => {
  test.each(archivedReplayPaths)(
    '%s remains a stable 410 evidence path',
    async path => {
      const response = materializeHttpResult(
        await handlePublicProofReplayBundleRequest(
          new Request(`https://openagents.com${path}`),
        ),
      )

      expect(response.status).toBe(410)
      expect(response.headers.get('content-type')).toBe('application/json')
      expect(await response.json()).toEqual({
        archived: true,
        backroomPath: 'openagents-prune-20260708-tassadar-psionic',
        blockerRefs: ['blocker.public_proof_replay.archived_to_backroom'],
        ok: false,
      })
    },
  )
})
