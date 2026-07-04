import { Schema as S } from 'effect'

// KS-8.11 (#8322): union type so the dual-write handle flows through.
import { type CrmEmailDatabase } from './crm-email-domain-store'
import {
  type EmailCampaignRuntime,
  systemEmailCampaignRuntime,
} from './email-campaigns'
import {
  type EnrollSubscriberResult,
  decodeEnrollSubscriberRequest,
  enrollSubscriberInSequence,
} from './email-sequence-authoring'
import {
  type ListSubscriberRecord,
  type NativeListsRuntime,
  type NativeListsServiceShape,
  makeNativeListsService,
  systemNativeListsRuntime,
} from './native-lists'

// Bridge layer that closes the site form -> native list -> email SEQUENCE loop
// (OpenAgents #4984 native lists + #4983 email sequences). This module is pure
// composition over the existing native-lists service (migration 0181) and the
// operator email-sequence authoring layer (migration 0063). It does NOT
// duplicate the multi-step dispatcher: scheduled sends produced through
// enrollSubscriberInSequence are still picked up by dispatchDueEmailCampaignSends
// exactly like the onboarding drip and operator-authored sequences. No new
// migration is introduced here.
//
// The bridge enforces one extra invariant on top of the sequence enroll path:
// the email must be an ACTIVE subscriber on the named list before we enroll it
// into a sequence. Suppression and per-email drip preference are still honored
// downstream by enrollSubscriberInSequence, so a list subscriber who has
// unsubscribed or opted out of drip mail is reported as skipped rather than
// enrolled.

export const LIST_SEQUENCE_SOURCE_AUTHORITY =
  'operator.list_sequence_enrollment.v1'

// Validation for the bridge request. `email` is required and must look like an
// email; `listId` and `sequenceSlug` are the existing identities from
// migrations 0181 and 0063 respectively.
export const EnrollListSubscriberRequest = S.Struct({
  displayName: S.optionalKey(S.NullishOr(S.String)),
  email: S.NonEmptyString.check(S.isIncludes('@')),
  listId: S.NonEmptyString,
  sequenceSlug: S.NonEmptyString,
  userId: S.optionalKey(S.NullishOr(S.String)),
})
export type EnrollListSubscriberRequest =
  typeof EnrollListSubscriberRequest.Type

export const decodeEnrollListSubscriberRequest = S.decodeUnknownSync(
  EnrollListSubscriberRequest,
)

// Request used by the form-capture path: capture a fresh subscriber onto a list
// and then bridge them into a sequence in one composed call.
export const EnrollNewSubscriberAndSequenceRequest = S.Struct({
  displayName: S.optionalKey(S.NullishOr(S.String)),
  email: S.NonEmptyString.check(S.isIncludes('@')),
  listId: S.NonEmptyString,
  metadata: S.optionalKey(S.NullishOr(S.Record(S.String, S.Unknown))),
  sequenceSlug: S.NonEmptyString,
  sourceRef: S.NonEmptyString,
  userId: S.optionalKey(S.NullishOr(S.String)),
})
export type EnrollNewSubscriberAndSequenceRequest =
  typeof EnrollNewSubscriberAndSequenceRequest.Type

export const decodeEnrollNewSubscriberAndSequenceRequest = S.decodeUnknownSync(
  EnrollNewSubscriberAndSequenceRequest,
)

// Result of the list -> sequence bridge. The 'enrolled' / 'skipped' variants
// are passed straight through from enrollSubscriberInSequence. The bridge adds
// guard variants for when the subscriber is not on the list, is not active on
// the list, or when the named sequence does not exist.
export type EnrollListSubscriberResult =
  | (EnrollSubscriberResult & { subscriber: ListSubscriberRecord })
  | Readonly<{
      reason:
        | 'subscriber_not_on_list'
        | 'subscriber_not_active'
        | 'sequence_not_found'
      status: 'skipped'
    }>

const findListSubscriber = async (
  service: NativeListsServiceShape,
  listId: string,
  email: string,
): Promise<ListSubscriberRecord | undefined> => {
  // Normalize the lookup email the same way the native-lists service does on
  // write so we compare apples to apples. listSubscribers returns the stored
  // (already-normalized) rows.
  const normalizedEmail = email.trim().toLowerCase()
  const subscribers = await service.listSubscribers({ listId })

  return subscribers.find(subscriber => subscriber.email === normalizedEmail)
}

// Bridge an existing native-list subscriber into an email nurture/sales
// sequence. The subscriber must already be present on the list (idempotent
// site-form capture should have written it). Resolution order:
//   1. subscriber must exist on the list           -> subscriber_not_on_list
//   2. subscriber must be active on the list        -> subscriber_not_active
//   3. sequence must exist (campaign slug resolves) -> sequence_not_found
//   4. delegate to enrollSubscriberInSequence, which honors suppression and
//      per-email drip preference and is idempotent on replay.
export const enrollListSubscriberInSequence = async (
  db: CrmEmailDatabase,
  request: EnrollListSubscriberRequest,
  operatorUserId: string,
  nativeListsRuntime: NativeListsRuntime = systemNativeListsRuntime,
  campaignRuntime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): Promise<EnrollListSubscriberResult> => {
  const service = makeNativeListsService(db, nativeListsRuntime)

  const subscriber = await findListSubscriber(
    service,
    request.listId,
    request.email,
  )

  if (subscriber === undefined) {
    return { reason: 'subscriber_not_on_list', status: 'skipped' }
  }

  if (subscriber.status !== 'active') {
    return { reason: 'subscriber_not_active', status: 'skipped' }
  }

  const enrollment = await enrollSubscriberInSequence(
    db,
    request.sequenceSlug,
    decodeEnrollSubscriberRequest({
      displayName: request.displayName ?? null,
      // Use the stored, normalized subscriber email as the enrollment identity
      // so the bridge and the list agree on a single canonical email.
      email: subscriber.email,
      userId: request.userId ?? null,
    }),
    operatorUserId,
    campaignRuntime,
  )

  if (enrollment === null) {
    return { reason: 'sequence_not_found', status: 'skipped' }
  }

  return { ...enrollment, subscriber }
}

// Form-capture path: add (or idempotently re-resolve) the subscriber on the
// list, then bridge them into the sequence. This is the single call a public
// site form handler can make to close site form -> list -> sequence. Add is
// idempotent (ON CONFLICT on the list_subscribers idempotency key), so a
// double form submit does not create duplicate subscribers or duplicate
// scheduled sends.
export const enrollNewSubscriberAndSequence = async (
  db: CrmEmailDatabase,
  request: EnrollNewSubscriberAndSequenceRequest,
  operatorUserId: string,
  nativeListsRuntime: NativeListsRuntime = systemNativeListsRuntime,
  campaignRuntime: EmailCampaignRuntime = systemEmailCampaignRuntime,
): Promise<
  EnrollListSubscriberResult & { listIdempotent: boolean }
> => {
  const service = makeNativeListsService(db, nativeListsRuntime)

  const { idempotent, subscriber } = await service.addSubscriber({
    email: request.email,
    listId: request.listId,
    metadata:
      (request.metadata as
        | Record<string, string | number | boolean | null>
        | null
        | undefined) ?? undefined,
    sourceRef: request.sourceRef,
  })

  // A subscriber who was previously unsubscribed/bounced and re-submits the
  // form is NOT silently reactivated by addSubscriber (the ON CONFLICT clause
  // does nothing). Surface that as a skip rather than enrolling a non-active
  // subscriber into the sequence.
  if (subscriber.status !== 'active') {
    return {
      listIdempotent: idempotent,
      reason: 'subscriber_not_active',
      status: 'skipped',
    }
  }

  const enrollment = await enrollSubscriberInSequence(
    db,
    request.sequenceSlug,
    decodeEnrollSubscriberRequest({
      displayName: request.displayName ?? null,
      email: subscriber.email,
      userId: request.userId ?? null,
    }),
    operatorUserId,
    campaignRuntime,
  )

  if (enrollment === null) {
    return {
      listIdempotent: idempotent,
      reason: 'sequence_not_found',
      status: 'skipped',
    }
  }

  return { ...enrollment, listIdempotent: idempotent, subscriber }
}

// COORDINATOR WIRING
// ------------------
// This module is the closure point for site form -> native list -> email
// sequence. It is pure composition; nothing here registers a route or runs a
// cron. Wire it from the layers that already own those boundaries:
//
// 1. Public site-form capture route (the form -> list -> sequence closure):
//    In the handler that today calls makeNativeListsService(...).addSubscriber
//    for a public capture form (native-lists-routes.ts, or a Sites form-capture
//    endpoint), replace the bare addSubscriber call with
//    enrollNewSubscriberAndSequence(db, { listId, email, sequenceSlug,
//    sourceRef, metadata, displayName, userId }, operatorUserId). The list's
//    metadata (or the form config) should carry which sequenceSlug the list
//    feeds; resolve that semantically/config-driven rather than string-matching
//    list names. Return 'enrolled' vs 'skipped' to the caller so the form can
//    show the right confirmation (subscribed but suppressed vs fully enrolled).
//
// 2. Operator list-management UI / API:
//    For "enroll this existing list into sequence X" bulk/single actions, call
//    enrollListSubscriberInSequence(db, { listId, email, sequenceSlug }, ...)
//    per active subscriber (e.g. iterate service.listSubscribers({ listId,
//    status: 'active' })). Each call is idempotent so re-running the action is
//    safe and will not double-schedule sends.
//
// 3. Dispatch:
//    Do NOT add a dispatcher here. The scheduled sends written by
//    enrollSubscriberInSequence are picked up by dispatchDueEmailCampaignSends
//    on the existing cron, identical to the onboarding drip and operator
//    sequences.
