import { strictDecode } from '@openagentsinc/forensic-contract'
import { Effect } from 'effect'

import type {
  ForensicPromptGovernanceError,
  ForensicPromptGovernanceStore,
} from './blueprint/repositories/forensic-prompt-governance'
import { BlueprintReleaseGate } from './blueprint/schemas/release-gate'
import {
  ForensicPromptCandidate,
  ForensicPromptEvaluation,
  type ForensicPromptActiveTransition as ForensicPromptActiveTransitionType,
  type ForensicPromptGovernanceState,
} from './blueprint/schemas/forensic-prompt-optimization'
import {
  promoteForensicPrompt,
  rollbackForensicPrompt,
} from './blueprint/services/forensic-prompt-compiler'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { isRecord, parseJsonUnknown } from './json-boundary'

export const BLUEPRINT_FORENSIC_PROMPT_GOVERNANCE_PATH =
  '/api/blueprint/forensic-prompt-governance'

type HttpResponse = globalThis.Response

/**
 * The production caller for the durable forensic prompt governance store.
 *
 * Two properties are deliberate. The decision is computed on this side of the
 * boundary: a caller submits the candidate, evaluation, and release gate and
 * the route runs `promoteForensicPrompt` / `rollbackForensicPrompt` against the
 * state it just read, so no caller can hand in a pre-sealed transition and skip
 * the gate. And `decidedAt` comes from the server clock, so a decision cannot
 * be backdated into the append-only history.
 */
export interface BlueprintForensicPromptGovernanceRoutesDependencies<Bindings> {
  readonly nowIso: () => string
  readonly requireAdminApiToken: (
    request: Request,
    env: Bindings,
  ) => Promise<boolean>
  readonly store: (env: Bindings) => ForensicPromptGovernanceStore
}

const invalidRequest = (message: string): HttpResponse =>
  noStoreJsonResponse({ error: 'invalid_request', message }, { status: 400 })

/**
 * A governance refusal is not a server fault and not a malformed request: the
 * gate looked at real inputs and declined. It is reported with its exact reason
 * so an operator can see which boundary refused.
 */
const refused = (message: string): HttpResponse =>
  noStoreJsonResponse({ error: 'refused', message }, { status: 422 })

const storeErrorResponse = (
  error: ForensicPromptGovernanceError,
  onInvalidTransition: number,
): HttpResponse => {
  if (error.code === 'storage_unavailable') {
    return noStoreJsonResponse(
      { error: 'storage_unavailable', message: error.message },
      { status: 503 },
    )
  }

  return noStoreJsonResponse(
    { error: error.code, message: error.message },
    { status: error.code === 'conflict' ? 409 : onInvalidTransition },
  )
}

const requiredRef = (value: unknown, label: string): string => {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new Error(`${label} is required`)
  }

  return value
}

const stateResponse = (
  state: ForensicPromptGovernanceState,
  transition?: ForensicPromptActiveTransitionType,
): HttpResponse =>
  noStoreJsonResponse({
    activePromptDigest: state.activePromptDigest,
    history: state.history,
    ownerRef: state.ownerRef,
    revision: state.revision,
    schema: state.schema,
    ...(transition === undefined ? {} : { transition }),
  })

const routeRead = <Bindings>(
  dependencies: BlueprintForensicPromptGovernanceRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const ownerRef = new URL(request.url).searchParams.get('ownerRef')?.trim()

    if (ownerRef === undefined || ownerRef === '') {
      return invalidRequest('ownerRef is required')
    }

    return yield* dependencies.store(env).read(ownerRef).pipe(
      Effect.map(state => stateResponse(state)),
      // A stored transition that fails validation on read is storage drift, not
      // a caller error, so it surfaces as a server fault rather than a 409.
      Effect.catch(error => Effect.succeed(storeErrorResponse(error, 500))),
    )
  })

const decideTransition = <Bindings>(
  dependencies: BlueprintForensicPromptGovernanceRoutesDependencies<Bindings>,
  body: Record<string, unknown>,
  state: ForensicPromptGovernanceState,
): ForensicPromptActiveTransitionType => {
  const decidedAt = dependencies.nowIso()
  const operatorDecisionRef = requiredRef(
    body['operatorDecisionRef'],
    'operatorDecisionRef',
  )
  const operatorIdentityRef = requiredRef(
    body['operatorIdentityRef'],
    'operatorIdentityRef',
  )
  const transitionRef = requiredRef(body['transitionRef'], 'transitionRef')

  if (body['decision'] === 'rollback') {
    return rollbackForensicPrompt({
      currentState: state,
      decidedAt,
      operatorDecisionRef,
      operatorIdentityRef,
      transitionRef,
    })
  }

  const evaluationRef = requiredRef(body['evaluationRef'], 'evaluationRef')

  return promoteForensicPrompt({
    candidate: strictDecode(ForensicPromptCandidate, body['candidate']),
    currentState: state,
    decidedAt,
    evaluation: strictDecode(ForensicPromptEvaluation, body['evaluation']),
    evaluationRef,
    operatorDecisionRef,
    operatorIdentityRef,
    releaseGate: strictDecode(BlueprintReleaseGate, body['releaseGate']),
    transitionRef,
  })
}

const readJsonObject = (
  request: Request,
): Effect.Effect<Record<string, unknown> | null> =>
  Effect.tryPromise({
    try: async () => {
      const value = parseJsonUnknown(await request.text())

      return isRecord(value) ? value : null
    },
    catch: () => null,
  }).pipe(Effect.catch(() => Effect.succeed(null)))

const routeDecide = <Bindings>(
  dependencies: BlueprintForensicPromptGovernanceRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const body = yield* readJsonObject(request)

    if (body === null) {
      return invalidRequest('request body must be a JSON object')
    }

    const ownerRef = body['ownerRef']

    if (typeof ownerRef !== 'string' || ownerRef.trim() === '') {
      return invalidRequest('ownerRef is required')
    }

    if (body['decision'] !== 'promote' && body['decision'] !== 'rollback') {
      return invalidRequest('decision must be promote or rollback')
    }

    const store = dependencies.store(env)

    return yield* store.read(ownerRef).pipe(
      Effect.flatMap(state =>
        Effect.try({
          try: () => decideTransition(dependencies, body, state),
          catch: error =>
            refused(
              error instanceof Error ? error.message : 'the decision was refused',
            ),
        }).pipe(
          Effect.flatMap(transition =>
            store
              .append(ownerRef, transition, state.revision)
              .pipe(
                Effect.map(appended => stateResponse(appended, transition)),
                Effect.catch(error =>
                  Effect.succeed(storeErrorResponse(error, 409)),
                ),
              ),
          ),
          Effect.catch(response => Effect.succeed(response)),
        ),
      ),
      Effect.catch(error => Effect.succeed(storeErrorResponse(error, 500))),
    )
  })

export const makeBlueprintForensicPromptGovernanceRoutes = <Bindings>(
  dependencies: BlueprintForensicPromptGovernanceRoutesDependencies<Bindings>,
) => ({
  handle: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> =>
    Effect.gen(function* () {
      if (request.method !== 'GET' && request.method !== 'POST') {
        return methodNotAllowed(['GET', 'POST'])
      }

      const authorized = yield* Effect.promise(() =>
        dependencies.requireAdminApiToken(request, env),
      )

      if (!authorized) {
        return unauthorized()
      }

      return yield* request.method === 'GET'
        ? routeRead(dependencies, request, env)
        : routeDecide(dependencies, request, env)
    }),
})
