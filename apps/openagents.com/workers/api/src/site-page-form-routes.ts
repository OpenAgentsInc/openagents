// COORDINATOR WIRING (#4982 — site page form-capture HTTP route):
//
// This module mounts the form-capture primitive from `site-page-kinds.ts` on a
// public HTTP route so a site page's lead form can POST submissions straight
// into a native subscriber list (migration 0181, table `list_subscribers`).
// It needs NO new migration.
//
// The route is intentionally PUBLIC (no operator gate): lead capture happens
// from anonymous visitors on a published site page. It validates strictly via
// `captureFormSubmission`, which enforces a valid email and the spec's declared
// required fields, and only persists scalars declared by the resolved
// `FormCaptureSpec`.
//
// The route does not own where a page's `FormCaptureSpec` lives. The caller
// injects a `lookupFormSpec(env, formId)` so the spec can be resolved from the
// site/version `metadata_json`, the builder/generator, or a future spec store
// without this route making that decision.
//
// In workers/api/src/index.ts, construct the routes alongside the other route
// factories (near makeNativeListsRoutes / makeAutopilotDecisionRoutes):
//
//   import { makeSitePageFormRoutes } from './site-page-form-routes'
//   import { makeNativeListsService } from './native-lists'
//
//   const sitePageFormRoutes = makeSitePageFormRoutes<WorkerBindings>({
//     makeSink: env => {
//       const lists = makeNativeListsService(openAgentsDatabase(env))
//       return { addSubscriber: lists.addSubscriber }
//     },
//     // Resolve the page's typed form spec however the page metadata is held.
//     lookupFormSpec: async (env, formId) => resolveFormSpecFor(env, formId),
//   })
//
// Then chain it into the omni dispatch chain (routeOmniRequest), e.g.:
//
//   routeOmniRequest: (request, env, ctx) =>
//     omniRoutes.routeOmniRequest(request, env, ctx) ??
//     nativeListsRoutes.routeNativeListsRequest(request, env, ctx) ??
//     sitePageFormRoutes.routeSitePageFormRequest(request, env, ctx),
//
// Outcome → status mapping:
//   captured          -> 201
//   idempotent        -> 200
//   validation_error  -> 400
//   unknown formId    -> 404

import { Effect, Match as M } from 'effect'

import {
  type FormCaptureSink,
  type FormCaptureSpec,
  captureFormSubmission,
} from './site-page-kinds'
import {
  methodNotAllowed,
  noStoreJsonResponse,
} from './http/responses'
import { readJsonObject } from './json-boundary'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

type SitePageFormRouteEnv = Readonly<Record<string, unknown>>

export type SitePageFormRoutesDependencies<Bindings> = Readonly<{
  // Build the lead-persistence sink for the request's environment. Wired to
  // makeNativeListsService(openAgentsDatabase(env)).addSubscriber in index.ts.
  makeSink: (env: Bindings) => FormCaptureSink
  // Resolve the typed FormCaptureSpec that owns this formId. Returning
  // undefined means "no such form" and produces a 404. The route never invents
  // a spec from the raw request body.
  lookupFormSpec: (
    env: Bindings,
    formId: string,
  ) => Promise<FormCaptureSpec | undefined>
  nowIso?: () => string
}>

const FORM_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,239}$/

const routeNowIso = <Bindings>(
  dependencies: SitePageFormRoutesDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const formIdFromPath = (pathname: string): string | undefined => {
  const match = /^\/api\/sites\/forms\/([^/]+)\/submit$/.exec(pathname)

  return match?.[1] === undefined ? undefined : decodeURIComponent(match[1])
}

const badRequest = (reason: string): HttpResponse =>
  noStoreJsonResponse(
    { error: 'site_page_form_validation_error', reason },
    { status: 400 },
  )

const notFound = (): HttpResponse =>
  noStoreJsonResponse(
    {
      error: 'site_page_form_not_found',
      reason: 'No form-capture spec is published for this form.',
    },
    { status: 404 },
  )

// Read the request body as a plain JSON object. A malformed/non-object body
// degrades to an empty object so the spec-driven validator (not a parser
// exception) produces the 400.
const readSubmission = (
  request: Request,
): Effect.Effect<Record<string, unknown>> =>
  Effect.promise(() =>
    readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    ),
  )

// Public form-capture endpoint: POST /api/sites/forms/:formId/submit
const submitForm = <Bindings extends SitePageFormRouteEnv>(
  dependencies: SitePageFormRoutesDependencies<Bindings>,
  request: Request,
  env: Bindings,
  formId: string,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)

    if (!FORM_ID_PATTERN.test(formId)) {
      return badRequest('A valid form id is required.')
    }

    const formSpec = yield* Effect.promise(() =>
      dependencies.lookupFormSpec(env, formId),
    )

    if (formSpec === undefined) {
      return notFound()
    }

    const submission = yield* readSubmission(request)
    const sink = dependencies.makeSink(env)
    const outcome = yield* Effect.promise(() =>
      captureFormSubmission(
        {
          formSpec,
          submission,
          sourceRef: `site_form.${formSpec.id}`,
        },
        sink,
      ),
    )

    return M.value(outcome).pipe(
      M.tag('captured', captured =>
        noStoreJsonResponse(
          {
            email: captured.email,
            generatedAt: nowIso,
            idempotent: false,
            listId: captured.listId,
          },
          { status: 201 },
        ),
      ),
      M.tag('idempotent', idempotent =>
        noStoreJsonResponse(
          {
            email: idempotent.email,
            generatedAt: nowIso,
            idempotent: true,
            listId: idempotent.listId,
          },
          { status: 200 },
        ),
      ),
      M.tag('validation_error', error => badRequest(error.reason)),
      M.exhaustive,
    )
  })

export const makeSitePageFormRoutes = <Bindings extends SitePageFormRouteEnv>(
  dependencies: SitePageFormRoutesDependencies<Bindings>,
) => ({
  routeSitePageFormRequest: (
    request: Request,
    env: Bindings,
    _ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    const formId = formIdFromPath(url.pathname)

    if (formId === undefined) {
      return undefined
    }

    return M.value(request.method).pipe(
      M.when('POST', () => submitForm(dependencies, request, env, formId)),
      M.orElse(() => Effect.succeed(methodNotAllowed(['POST']))),
    )
  },
})
