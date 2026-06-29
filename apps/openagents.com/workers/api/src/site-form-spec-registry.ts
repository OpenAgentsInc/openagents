// SITE FORM-SPEC REGISTRY (#4983/#4984 last-mile — the "home for form-specs"):
//
// `site-page-form-routes.ts` mounts the public form-capture endpoint
// (POST /api/sites/forms/:formId/submit) but deliberately does not own where a
// page's `FormCaptureSpec` lives — it injects a `lookupFormSpec(env, formId)`
// resolver. This module is that resolver's data home: it defines the canonical
// shape that a published site/version carries in its `metadata_json` for lead
// forms, decodes it defensively, and resolves a typed `FormCaptureSpec` by
// `formId`.
//
// Keeping this separate from the HTTP route means:
//   - the route stays transport-only and spec-source-agnostic, and
//   - the spec source (site metadata today, a dedicated spec store later) can
//     evolve without touching the public capture endpoint.
//
// The decoded shape lives under `metadata_json.formSpecs` as a map of
// formId -> FormCaptureSpec. A malformed or absent entry resolves to
// `undefined` (the route turns that into a 404) rather than throwing, so a
// broken metadata blob can never crash anonymous lead capture.

import { Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import { FormCaptureSpec } from './site-page-kinds'

// The canonical metadata envelope a published site/version carries for its
// lead forms. Only `formSpecs` is consumed here; the surrounding metadata may
// carry unrelated keys, so the decoder is intentionally tolerant of extra
// fields and only validates the `formSpecs` map it owns.
export const SiteFormSpecMetadata = S.Struct({
  formSpecs: S.optionalKey(S.Record(S.String, FormCaptureSpec)),
})
export type SiteFormSpecMetadata = typeof SiteFormSpecMetadata.Type

const decodeMetadata = S.decodeUnknownSync(SiteFormSpecMetadata)

// Resolve every published form spec from a site/version `metadata_json` value.
// Accepts the raw parsed JSON (object) or a JSON string; anything that does not
// decode to the canonical envelope yields an empty registry rather than an
// error, so anonymous capture degrades to 404 instead of 500.
export const resolveSiteFormSpecs = (
  metadataJson: unknown,
): ReadonlyMap<string, FormCaptureSpec> => {
  const parsed =
    typeof metadataJson === 'string'
      ? parseJsonRecord(metadataJson)
      : metadataJson

  if (parsed === undefined) {
    return new Map()
  }

  let formSpecs: Record<string, FormCaptureSpec> = {}
  try {
    formSpecs = decodeMetadata(parsed).formSpecs ?? {}
  } catch {
    return new Map()
  }

  // Only surface specs whose own `id` agrees with their registry key, so a
  // mismatched/misfiled entry cannot be captured under the wrong formId.
  return new Map(
    Object.entries(formSpecs).filter(([formId, spec]) => spec.id === formId),
  )
}

// Resolve a single published form spec by id. Returns undefined when the site
// has no such form, which the capture route renders as a 404.
export const resolveSiteFormSpec = (
  metadataJson: unknown,
  formId: string,
): FormCaptureSpec | undefined => resolveSiteFormSpecs(metadataJson).get(formId)
