type HttpResponse = globalThis.Response

export const handlePublicProofReplayBundleRequest = async (
  ..._args: unknown[]
): Promise<HttpResponse> =>
  Response.json(
    {
      archived: true,
      backroomPath: 'openagents-prune-20260708-tassadar-psionic',
      blockerRefs: ['blocker.public_proof_replay.archived_to_backroom'],
      ok: false,
    },
    { status: 410 },
  )
