// COORDINATOR WIRING (#4982 — native page kinds + form-capture):
//
// This module is pure typed metadata plus a thin server handler. It needs NO
// migration: page kinds are not persisted as their own table. A site version's
// page kind (and any attached form spec) lives inside the existing
// `metadata_json` carried by the site/version record, or is supplied by the
// builder/generator at request time. The form-capture handler delegates lead
// persistence to the already-landed native-lists `addSubscriber` path
// (migration 0181, table `list_subscribers`).
//
// To wire the form-capture endpoint into the Worker, build a native lists
// service and hand its `addSubscriber` to `captureFormSubmission`. In
// workers/api/src/index.ts, near the other route factories:
//
//   import {
//     SITE_PAGE_KIND_DEFINITIONS,
//     captureFormSubmission,
//     describeSitePageKind,
//   } from './site-page-kinds'
//   import { makeNativeListsService } from './native-lists'
//
//   // Inside a route handler that owns the page's form spec:
//   const lists = makeNativeListsService(openAgentsDatabase(env))
//   const result = await captureFormSubmission(
//     {
//       formSpec,            // FormCaptureSpec resolved from the page's metadata
//       submission,          // decoded request body (unknown JSON object)
//       sourceRef: `site_form.${formSpec.id}`,
//     },
//     { addSubscriber: lists.addSubscriber },
//   )
//   // result is a tagged FormCaptureOutcome: 'captured' | 'idempotent' |
//   // 'validation_error'. Map it to a 201/200/400 JSON response.
//
// `describeSitePageKind(kind)` and `SITE_PAGE_KIND_DEFINITIONS` give the
// generator/builder the expected sections for landing / sales / opt-in /
// thank-you pages so it can scaffold and validate page structure natively.

import { Effect, Schema as S } from 'effect'

// ---------------------------------------------------------------------------
// Page kinds
// ---------------------------------------------------------------------------

export const SitePageKind = S.Literals([
  'landing',
  'sales',
  'opt_in',
  'thank_you',
])
export type SitePageKind = typeof SitePageKind.Type

// The native section vocabulary a page kind can be composed from. Kept small
// and typed so the builder/generator validates against a closed set rather
// than free-form string matching.
export const SitePageSection = S.Literals([
  'hero',
  'value_props',
  'features',
  'social_proof',
  'testimonials',
  'pricing',
  'faq',
  'lead_form',
  'cta',
  'confirmation',
  'next_steps',
  'footer',
])
export type SitePageSection = typeof SitePageSection.Type

export type SitePageKindDefinition = Readonly<{
  kind: SitePageKind
  title: string
  summary: string
  // Sections that must be present for the page to satisfy this kind.
  requiredSections: ReadonlyArray<SitePageSection>
  // Sections that are conventional but optional.
  optionalSections: ReadonlyArray<SitePageSection>
  // Whether this kind captures leads (drives whether a FormCaptureSpec is
  // expected on the page).
  capturesLeads: boolean
}>

const definition = (input: SitePageKindDefinition): SitePageKindDefinition =>
  input

export const SITE_PAGE_KIND_DEFINITIONS: Readonly<
  Record<SitePageKind, SitePageKindDefinition>
> = {
  landing: definition({
    kind: 'landing',
    title: 'Landing page',
    summary:
      'Top-of-funnel page that introduces the offer and drives a single primary action.',
    requiredSections: ['hero', 'cta'],
    optionalSections: ['value_props', 'features', 'social_proof', 'footer'],
    capturesLeads: false,
  }),
  sales: definition({
    kind: 'sales',
    title: 'Sales page',
    summary:
      'Long-form persuasion page that builds the case for a paid offer and converts to purchase.',
    requiredSections: ['hero', 'value_props', 'pricing', 'cta'],
    optionalSections: ['features', 'social_proof', 'testimonials', 'faq', 'footer'],
    capturesLeads: false,
  }),
  opt_in: definition({
    kind: 'opt_in',
    title: 'Opt-in page',
    summary:
      'Lead-capture page that trades an incentive for an email address into a native list.',
    requiredSections: ['hero', 'lead_form'],
    optionalSections: ['value_props', 'social_proof', 'footer'],
    capturesLeads: true,
  }),
  thank_you: definition({
    kind: 'thank_you',
    title: 'Thank-you page',
    summary:
      'Post-conversion confirmation page that acknowledges the action and points to next steps.',
    requiredSections: ['confirmation', 'next_steps'],
    optionalSections: ['cta', 'social_proof', 'footer'],
    capturesLeads: false,
  }),
}

export const describeSitePageKind = (
  kind: SitePageKind,
): SitePageKindDefinition => SITE_PAGE_KIND_DEFINITIONS[kind]

export const ALL_SITE_PAGE_KINDS: ReadonlyArray<SitePageKind> = [
  'landing',
  'sales',
  'opt_in',
  'thank_you',
]

// Validate a proposed set of sections against a page kind. Returns the list of
// required sections that are missing (empty array == satisfied). This gives the
// builder a deterministic, closed-set structural check.
export const missingRequiredSections = (
  kind: SitePageKind,
  sections: ReadonlyArray<SitePageSection>,
): ReadonlyArray<SitePageSection> => {
  const present = new Set(sections)
  return describeSitePageKind(kind).requiredSections.filter(
    section => !present.has(section),
  )
}

// ---------------------------------------------------------------------------
// Form-capture primitive
// ---------------------------------------------------------------------------

export const FormFieldKind = S.Literals(['email', 'text', 'phone', 'consent'])
export type FormFieldKind = typeof FormFieldKind.Type

export const FormCaptureField = S.Struct({
  name: S.String,
  kind: FormFieldKind,
  label: S.optionalKey(S.String),
  required: S.optionalKey(S.Boolean),
})
export type FormCaptureField = typeof FormCaptureField.Type

// A typed descriptor for a page's lead-capture form. `listId` binds the form to
// a native subscriber list (#4984); `fields` declares the captured shape. The
// `email` field is implicit and always required, but may be named explicitly in
// `fields` for labelling.
export const FormCaptureSpec = S.Struct({
  id: S.String,
  listId: S.String,
  sequenceSlug: S.optionalKey(S.String),
  fields: S.Array(FormCaptureField),
})
export type FormCaptureSpec = typeof FormCaptureSpec.Type

const emailPattern = /^[^\s@]+@[^\s@]+\.[^\s@]+$/

const clampText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

// The decoded lead persistence dependency. Matches the shape of
// NativeListsServiceShape['addSubscriber'] so callers can pass it directly.
export type FormCaptureSink = Readonly<{
  addSubscriber: (
    input: Readonly<{
      email: string
      listId: string
      metadata?: Record<string, string | number | boolean | null> | undefined
      sequenceSlug?: string | undefined
      sourceRef: string
    }>,
  ) => Promise<
    Readonly<{
      idempotent: boolean
      sequenceEnrollment?: FormCaptureSequenceEnrollment | undefined
      subscriber: { email: string }
    }>
  >
}>

export type FormCaptureSequenceEnrollment =
  | Readonly<{
      scheduledSendCount: number
      status: 'enrolled'
    }>
  | Readonly<{
      reason:
        | 'drip_preference_disabled'
        | 'drip_suppressed'
        | 'sequence_not_found'
        | 'subscriber_not_active'
        | 'subscriber_not_on_list'
      status: 'skipped'
    }>

export type FormCaptureOutcome =
  | Readonly<{
      _tag: 'captured'
      email: string
      listId: string
      sequenceEnrollment?: FormCaptureSequenceEnrollment | undefined
    }>
  | Readonly<{
      _tag: 'idempotent'
      email: string
      listId: string
      sequenceEnrollment?: FormCaptureSequenceEnrollment | undefined
    }>
  | Readonly<{ _tag: 'validation_error'; reason: string }>

// Extract metadata from a submission constrained to the spec's declared
// non-email fields, with values clamped to safe scalars. Unknown keys are
// dropped — the form spec is the closed schema, not the raw submission.
const collectMetadata = (
  spec: FormCaptureSpec,
  submission: Record<string, unknown>,
): Record<string, string | number | boolean | null> => {
  const metadata: Record<string, string | number | boolean | null> = {}
  for (const field of spec.fields) {
    if (field.kind === 'email') {
      continue
    }
    const raw = submission[field.name]
    if (raw === undefined) {
      continue
    }
    if (typeof raw === 'string') {
      metadata[field.name] = clampText(raw, 240)
    } else if (typeof raw === 'number' || typeof raw === 'boolean') {
      metadata[field.name] = raw
    } else if (raw === null) {
      metadata[field.name] = null
    }
  }
  return metadata
}

const emailFieldName = (spec: FormCaptureSpec): string => {
  const explicit = spec.fields.find(field => field.kind === 'email')
  return explicit?.name ?? 'email'
}

export type CaptureFormSubmissionInput = Readonly<{
  formSpec: FormCaptureSpec
  submission: Record<string, unknown>
  sourceRef?: string | undefined
}>

// Validate a submission against the form spec and, if valid, persist the lead
// into its native list via the sink (addSubscriber idempotency path).
export const captureFormSubmission = async (
  input: CaptureFormSubmissionInput,
  sink: FormCaptureSink,
): Promise<FormCaptureOutcome> => {
  const { formSpec, submission } = input

  const rawEmail = submission[emailFieldName(formSpec)]
  if (typeof rawEmail !== 'string') {
    return { _tag: 'validation_error', reason: 'An email address is required.' }
  }
  const email = rawEmail.trim().toLowerCase()
  if (!emailPattern.test(email)) {
    return {
      _tag: 'validation_error',
      reason: 'A valid email address is required.',
    }
  }

  // Enforce required non-email fields declared by the spec.
  for (const field of formSpec.fields) {
    if (field.kind === 'email' || field.required !== true) {
      continue
    }
    const value = submission[field.name]
    if (
      value === undefined ||
      value === null ||
      (typeof value === 'string' && value.trim() === '')
    ) {
      return {
        _tag: 'validation_error',
        reason: `Field "${field.name}" is required.`,
      }
    }
  }

  const metadata = collectMetadata(formSpec, submission)

  const result = await sink.addSubscriber({
    email,
    listId: formSpec.listId,
    metadata: Object.keys(metadata).length === 0 ? undefined : metadata,
    sequenceSlug: formSpec.sequenceSlug,
    sourceRef:
      input.sourceRef === undefined || input.sourceRef.trim() === ''
        ? `site_form.${formSpec.id}`
        : input.sourceRef,
  })

  return result.idempotent
    ? {
        _tag: 'idempotent',
        email: result.subscriber.email,
        listId: formSpec.listId,
        ...(result.sequenceEnrollment === undefined
          ? {}
          : { sequenceEnrollment: result.sequenceEnrollment }),
      }
    : {
        _tag: 'captured',
        email: result.subscriber.email,
        listId: formSpec.listId,
        ...(result.sequenceEnrollment === undefined
          ? {}
          : { sequenceEnrollment: result.sequenceEnrollment }),
      }
}

// Effect-wrapped variant for callers already composing in Effect.
export const captureFormSubmissionEffect = (
  input: CaptureFormSubmissionInput,
  sink: FormCaptureSink,
): Effect.Effect<FormCaptureOutcome> =>
  Effect.promise(() => captureFormSubmission(input, sink))
