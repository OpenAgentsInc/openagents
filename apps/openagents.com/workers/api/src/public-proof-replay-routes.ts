import { type JsonHttpResult, jsonHttpResult } from './http/responses'

export const handlePublicProofReplayBundleRequest = async (
  ..._args: unknown[]
): Promise<JsonHttpResult> =>
  jsonHttpResult(
    {
      archived: true,
      backroomPath: 'openagents-prune-20260708-tassadar-psionic',
      blockerRefs: ['blocker.public_proof_replay.archived_to_backroom'],
      ok: false,
    },
    { status: 410 },
  )
