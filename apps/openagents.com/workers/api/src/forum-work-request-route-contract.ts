import { Schema as S } from 'effect'

import {
  ForumWorkRequestLifecycleKind,
  type ForumWorkRequestRecord,
  type NormalizedForumWorkRequestInput,
} from './forum-work-requests'

const ForumWorkRequestRef = S.Trim.check(S.isNonEmpty(), S.isMaxLength(220))
const ForumWorkRequestRefs = S.Array(ForumWorkRequestRef)

const RequestedSlug = S.NullOr(
  S.Trim.check(
    S.isMinLength(3),
    S.isMaxLength(80),
    S.isPattern(/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/),
  ),
)

const CreateForumWorkRequestBody = S.Struct({
  budgetSats: S.Number,
  deadlineRef: ForumWorkRequestRef,
  objectiveRef: ForumWorkRequestRef,
  repositoryRefs: S.optionalKey(ForumWorkRequestRefs),
  requestedSlug: S.optionalKey(RequestedSlug),
  requiredCapabilityRefs: S.optionalKey(ForumWorkRequestRefs),
  title: S.Trim.check(S.isMinLength(3), S.isMaxLength(160)),
  verificationCommandRef: ForumWorkRequestRef,
})
export type CreateForumWorkRequestBody =
  typeof CreateForumWorkRequestBody.Type

const RelayNativeForumWorkRequestBody = S.Struct({
  event: S.Unknown,
  title: S.optionalKey(S.NullOr(S.Trim.check(S.isMaxLength(160)))),
})
export type RelayNativeForumWorkRequestBody =
  typeof RelayNativeForumWorkRequestBody.Type

const ForumWorkRequestLifecycleBody = S.Struct({
  lifecycleKind: ForumWorkRequestLifecycleKind,
  receiptRef: ForumWorkRequestRef,
})
export type ForumWorkRequestLifecycleBody =
  typeof ForumWorkRequestLifecycleBody.Type

const AcceptForumWorkRequestOfferBody = S.Struct({
  quoteRef: ForumWorkRequestRef,
})
export type AcceptForumWorkRequestOfferBody =
  typeof AcceptForumWorkRequestOfferBody.Type

const ForumWorkRequestNostrPubkey = S.Trim.check(
  S.isPattern(/^[0-9a-f]{64}$/i),
)

const SubmitForumWorkRequestOfferBody = S.Struct({
  amountSats: S.Number,
  capabilityRefs: S.optionalKey(ForumWorkRequestRefs),
  providerActorRef: ForumWorkRequestRef,
  providerPubkey: S.optionalKey(S.NullOr(ForumWorkRequestNostrPubkey)),
  quoteRef: ForumWorkRequestRef,
  relayEventRef: S.optionalKey(S.NullOr(ForumWorkRequestRef)),
})
export type SubmitForumWorkRequestOfferBody =
  typeof SubmitForumWorkRequestOfferBody.Type

const SubmitForumWorkRequestResultBody = S.Struct({
  artifactRefs: S.optionalKey(ForumWorkRequestRefs),
  closeoutRef: S.optionalKey(S.NullOr(ForumWorkRequestRef)),
  quoteRef: ForumWorkRequestRef,
  resultEventRef: ForumWorkRequestRef,
  verificationCommandRef: ForumWorkRequestRef,
})
export type SubmitForumWorkRequestResultBody =
  typeof SubmitForumWorkRequestResultBody.Type

const ReleaseForumWorkRequestBody = S.Struct({
  quoteRef: ForumWorkRequestRef,
  verificationVerdictRef: ForumWorkRequestRef,
})
export type ReleaseForumWorkRequestBody =
  typeof ReleaseForumWorkRequestBody.Type

const forbiddenWorkRequestBodyKeys = new Set([
  'bodyText',
  'credentials',
  'privateRepoContent',
  'prompt',
  'rawCommand',
  'rawContent',
  'rawPrompt',
  'secret',
])

export class ForumWorkRequestBodyValidationError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ForumWorkRequestBodyValidationError'
  }
}

const rejectForbiddenWorkRequestBodyKeys = (body: unknown): void => {
  if (body === null || typeof body !== 'object' || Array.isArray(body)) {
    return
  }

  const keys = Object.keys(body)
  const forbidden = keys.find(key => forbiddenWorkRequestBodyKeys.has(key))

  if (forbidden !== undefined) {
    throw new ForumWorkRequestBodyValidationError(
      `Forum work requests accept public refs only; ${forbidden} is not allowed.`,
    )
  }
}

export const decodeCreateForumWorkRequestBody = (
  body: unknown,
): CreateForumWorkRequestBody => {
  rejectForbiddenWorkRequestBodyKeys(body)

  return S.decodeUnknownSync(CreateForumWorkRequestBody)(body)
}

export const decodeRelayNativeForumWorkRequestBody = (
  body: unknown,
): RelayNativeForumWorkRequestBody => {
  rejectForbiddenWorkRequestBodyKeys(body)

  return S.decodeUnknownSync(RelayNativeForumWorkRequestBody)(body)
}

export const decodeForumWorkRequestLifecycleBody = (
  body: unknown,
): ForumWorkRequestLifecycleBody => {
  rejectForbiddenWorkRequestBodyKeys(body)

  return S.decodeUnknownSync(ForumWorkRequestLifecycleBody)(body)
}

export const decodeAcceptForumWorkRequestOfferBody = (
  body: unknown,
): AcceptForumWorkRequestOfferBody => {
  rejectForbiddenWorkRequestBodyKeys(body)

  return S.decodeUnknownSync(AcceptForumWorkRequestOfferBody)(body)
}

export const decodeSubmitForumWorkRequestOfferBody = (
  body: unknown,
): SubmitForumWorkRequestOfferBody => {
  rejectForbiddenWorkRequestBodyKeys(body)

  return S.decodeUnknownSync(SubmitForumWorkRequestOfferBody)(body)
}

export const decodeSubmitForumWorkRequestResultBody = (
  body: unknown,
): SubmitForumWorkRequestResultBody => {
  rejectForbiddenWorkRequestBodyKeys(body)

  return S.decodeUnknownSync(SubmitForumWorkRequestResultBody)(body)
}

export const decodeReleaseForumWorkRequestBody = (
  body: unknown,
): ReleaseForumWorkRequestBody => {
  rejectForbiddenWorkRequestBodyKeys(body)

  return S.decodeUnknownSync(ReleaseForumWorkRequestBody)(body)
}

const arraysEqual = (
  left: ReadonlyArray<string>,
  right: ReadonlyArray<string>,
): boolean =>
  left.length === right.length &&
  left.every((value, index) => value === right[index])

export const workRequestMatchesInput = (
  record: ForumWorkRequestRecord,
  input: NormalizedForumWorkRequestInput,
): boolean =>
  record.title === input.title &&
  record.objectiveRef === input.objectiveRef &&
  record.verificationCommandRef === input.verificationCommandRef &&
  record.deadlineRef === input.deadlineRef &&
  record.budgetSats === input.budgetSats &&
  arraysEqual(record.repositoryRefs, input.repositoryRefs) &&
  arraysEqual(record.requiredCapabilityRefs, input.requiredCapabilityRefs)
