import { Effect } from 'effect'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'
import { liveAtReadStaleness } from './public-projection-staleness'
import {
  buildCodingQuickWinPipelineReceipt,
  CodingQuickWinPipelineInvariantError,
  type CodingQuickWinPipelineInput,
} from './coding-quick-win-pipeline'

export const CodingQuickWinPipelineEndpoint = '/api/public/business/coding-quick-win-pipeline'

class InternalPipelineError {
  readonly _tag = 'InternalPipelineError'
}

const handlePostPipeline = (request: Request) =>
  Effect.gen(function* () {
    const body = yield* Effect.tryPromise({
      try: () => request.json() as Promise<unknown>,
      catch: () => 'invalid_json' as const,
    }).pipe(Effect.orElseSucceed(() => 'invalid_json' as const))

    if (body === 'invalid_json') {
      return noStoreJsonResponse(
        { error: 'invalid_request', reason: 'request body must be valid JSON' },
        { status: 400 },
      )
    }

    const input = body as CodingQuickWinPipelineInput
    const receipt = yield* Effect.try({
      try: () => buildCodingQuickWinPipelineReceipt(input),
      catch: (err) => {
        if (err instanceof CodingQuickWinPipelineInvariantError) {
          return err
        }
        return new InternalPipelineError()
      },
    })

    return noStoreJsonResponse({
      receipt,
      inert: true,
      promiseState: 'yellow',
      promiseIds: ['business.coding_quick_win.v1'],
      generatedAt: currentIsoTimestamp(),
      staleness: liveAtReadStaleness([]),
    })
  }).pipe(
    Effect.catchTag('CodingQuickWinPipelineInvariantError', (err) =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'pipeline_invariant_error', reason: err.reason },
          { status: 400 },
        )
      )
    ),
    Effect.catchTag('InternalPipelineError', () =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'internal_error', reason: 'Failed to build pipeline receipt' },
          { status: 500 },
        )
      )
    )
  )

export const handleCodingQuickWinPipelineApi = (request: Request): Effect.Effect<Response> => {
  if (request.method === 'POST') {
    return handlePostPipeline(request)
  }
  return Effect.succeed(methodNotAllowed(['POST']))
}



