// SITE PAGE FORM-CAPTURE WIRING (#5523 / DE-9 #5532; promise
// autopilot_sites.native_email_sequences.v1, yellow).
//
// `site-page-form-routes.ts` builds the transport-only public capture route
// (POST /api/sites/forms/:formId/submit) and deliberately injects both its
// lead-persistence sink and its `lookupFormSpec(env, formId)` resolver.
// `site-form-spec-registry.ts` decodes a published site/version `metadata_json`
// into typed `FormCaptureSpec`s. Until now NOTHING joined those two to a real
// request environment, so the capture route was unmounted — the open blocker
// `blocker.product_promises.site_form_capture_route_unmounted`.
//
// This module is that last-mile glue, gated by an additive, default-OFF feature
// flag (`SITE_FORM_CAPTURE_ENABLED`) so it changes nothing on the live Worker
// until armed:
//   - it resolves a page's `FormCaptureSpec` from the active site version's
//     `metadata_json` via the registry (`resolveSiteFormSpec`), through an
//     injected `readSiteFormMetadata` reader so the resolver stays testable
//     without D1, and
//   - it persists captured leads through the already-landed native-lists
//     `addSubscriber` sink (migration 0181, table `list_subscribers`).
//
// When the flag is OFF, `routeSitePageFormCaptureRequest` returns `undefined`
// for every request, so the omni dispatch chain falls through exactly as it
// does today (no new live surface, no anonymous write path). Arming the flag
// clears ONLY the route-unmounted blocker; the customer authoring UI, the
// wired send service, and deliverability/bounce handling stay owner/product-
// gated and the promise stays yellow (no green flip — that is owner-signed per
// proof.claim_upgrade_receipts.v1).

import type { Effect } from 'effect'

import { resolveSiteFormSpec } from './site-form-spec-registry'
import {
  type SitePageFormRoutesDependencies,
  makeSitePageFormRoutes,
} from './site-page-form-routes'
import type { FormCaptureSink, FormCaptureSpec } from './site-page-kinds'

type HttpResponse = globalThis.Response

export const SITE_FORM_CAPTURE_PROMISE_ID =
  'autopilot_sites.native_email_sequences.v1'

// The single blocker this wiring clears once armed. The other blockers on the
// promise (customer UI, send-service integration, deliverability) are NOT
// touched here and stay owner/product-gated.
export const SITE_FORM_CAPTURE_BLOCKER_CLEARED =
  'blocker.product_promises.site_form_capture_route_unmounted'

// Additive, default-OFF feature flag. Mirrors the established truthy flag parser
// convention: absent or any non-truthy value => disabled.
export const isSiteFormCaptureEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

// Resolve the published `FormCaptureSpec` that owns `formId` from the active
// site version's metadata. `readSiteFormMetadata` returns the raw
// `metadata_json` (string, parsed object, or undefined when no published site
// owns the form). The registry turns any malformed/absent metadata into
// `undefined`, which the capture route renders as a 404 — anonymous capture can
// never crash on a broken metadata blob.
export const resolveSiteFormSpecFromMetadata = async <Bindings>(
  readSiteFormMetadata: (
    env: Bindings,
    formId: string,
  ) => Promise<unknown>,
  env: Bindings,
  formId: string,
): Promise<FormCaptureSpec | undefined> => {
  const metadataJson = await readSiteFormMetadata(env, formId)

  return resolveSiteFormSpec(metadataJson, formId)
}

export type SitePageFormCaptureDependencies<Bindings> = Readonly<{
  // Per-request gate. When it returns false the route is fully inert and
  // returns undefined (omni-chain fallthrough). Evaluated per request so the
  // flag is read from the live request environment, not at module init.
  isEnabled: (env: Bindings) => boolean
  // Build the lead-persistence sink for the request environment. Wired to
  // makeNativeListsService(openAgentsDatabase(env)).addSubscriber in index.ts.
  makeSink: (env: Bindings) => FormCaptureSink
  // Read the raw active-site-version metadata_json that owns `formId`.
  readSiteFormMetadata: (env: Bindings, formId: string) => Promise<unknown>
  nowIso?: () => string
}>

// Build the flag-gated, fully-wired site page form-capture routes. When the
// per-request gate returns false the route returns undefined (no behavior
// change). When true, requests to the capture route are served by the
// underlying transport route with a registry-backed spec resolver and the
// native-lists sink.
export const makeSitePageFormCaptureRoutes = <
  Bindings extends Readonly<Record<string, unknown>>,
>(
  dependencies: SitePageFormCaptureDependencies<Bindings>,
) => {
  const routeDependencies: SitePageFormRoutesDependencies<Bindings> = {
    makeSink: dependencies.makeSink,
    lookupFormSpec: (env, formId) =>
      resolveSiteFormSpecFromMetadata(
        dependencies.readSiteFormMetadata,
        env,
        formId,
      ),
    // Only forward nowIso when provided — exactOptionalPropertyTypes forbids
    // assigning `undefined` to an optional property.
    ...(dependencies.nowIso === undefined
      ? {}
      : { nowIso: dependencies.nowIso }),
  }

  const routes = makeSitePageFormRoutes<Bindings>(routeDependencies)

  return {
    routeSitePageFormCaptureRequest: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      if (!dependencies.isEnabled(env)) {
        return undefined
      }

      return routes.routeSitePageFormRequest(request, env, ctx)
    },
  }
}
