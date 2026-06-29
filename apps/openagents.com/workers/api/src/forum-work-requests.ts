import {
  LBR_AGENTIC_CODING_REQUEST_KIND,
  LBR_AGENTIC_CODING_RESULT_KIND,
  LbrProtocolError,
  decodeLbrAgenticCodingRequestEvent,
  lbrAgenticCodingRequestToDraft,
  makeLbrAgenticCodingRequest,
  type LbrAgenticCodingRequest,
  type LbrUnsignedEventDraft,
} from '@openagentsinc/nip90'
import { Effect, Schema as S } from 'effect'

import { sha256Hex } from './agent-registration'
import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import {
  ForumStorageError,
  ForumValidationError,
  type ForumRepositoryRuntime,
  systemForumRepositoryRuntime,
} from './forum/repository'
import {
  decodeForumPublicProjection,
  type ForumPublicProjection,
} from './forum/schemas'

export const ForumWorkRequestsForumSlug = 'work-requests'
export const DefaultForumWorkRequestRelayUrl =
  'wss://relay.openagents.com'
export const DefaultForumWorkRequestBridgeActorRef =
  'agent:openagents_market_bridge'
export const DefaultForumWorkRequestRepositoryRef = 'repo.public.openagents'
export const DefaultForumWorkRequestCapabilityRef =
  'capability.pylon.local_claude_agent'

export const ForumWorkRequestState = S.Literals([
  'open',
  'quote_received',
  'quote_accepted',
  'running',
  'delivered',
  'accepted',
  'settled',
  'cancelled',
  'expired',
])
export type ForumWorkRequestState = typeof ForumWorkRequestState.Type

export const ForumWorkRequestLifecycleKind = S.Literals([
  'quote_received',
  'quote_accepted',
  'running',
  'delivered',
  'accepted',
  'settled',
  'cancelled',
  'expired',
])
export type ForumWorkRequestLifecycleKind =
  typeof ForumWorkRequestLifecycleKind.Type

export type ForumWorkRequestInput = Readonly<{
  budgetSats: number
  deadlineRef: string
  objectiveRef: string
  repositoryRefs?: ReadonlyArray<string> | undefined
  requiredCapabilityRefs?: ReadonlyArray<string> | undefined
  title: string
  verificationCommandRef: string
}>

export type NormalizedForumWorkRequestInput = Readonly<{
  budgetMsats: number
  budgetSats: number
  deadlineRef: string
  objectiveRef: string
  repositoryRefs: ReadonlyArray<string>
  requiredCapabilityRefs: ReadonlyArray<string>
  title: string
  verificationCommandRef: string
}>

export type ForumWorkRequestRelayPublishInput = Readonly<{
  bridgeActorRef: string
  draft: LbrUnsignedEventDraft
  idempotencyKey: string
  lbrRequest: LbrAgenticCodingRequest
  relayUrl: string
  topicId: string
  workRequestId: string
}>

export type ForumWorkRequestRelayPublishReceipt = Readonly<{
  accepted: boolean
  event: unknown
  jobEventId: string
  relayRef: string
  relayUrl: string
}>

// Requester-side quote acceptance published to the scoped market relay as a
// ref-only NIP-LBR kind-7000 acceptance feedback event so the watching
// provider executes. Only public-safe refs cross this boundary: the job event
// id (64-hex), the public quote ref, the public escrow reserve receipt ref,
// the public acceptance ref, and the provider's nostr pubkey. No wallet,
// invoice, preimage, payment-hash, or credential material is ever included.
export type ForumWorkRequestAcceptanceRelayPublishInput = Readonly<{
  acceptanceRef: string
  escrowReceiptRef: string
  jobEventId: string
  providerPubkey: string
  quoteRef: string
  relayUrl: string
  workRequestId: string
}>

export type ForumWorkRequestAcceptanceRelayPublishReceipt = Readonly<{
  accepted: boolean
  acceptanceEventId: string | null
  event: unknown
  relayRef: string
  relayUrl: string
}>

export type ForumWorkRequestRelayPublisher = Readonly<{
  publishWorkRequest: (
    input: ForumWorkRequestRelayPublishInput,
  ) => Promise<ForumWorkRequestRelayPublishReceipt>
  publishAcceptance?: (
    input: ForumWorkRequestAcceptanceRelayPublishInput,
  ) => Promise<ForumWorkRequestAcceptanceRelayPublishReceipt>
}>

export type ForumWorkRequestRecord = Readonly<{
  budgetMsats: number
  budgetSats: number
  createdAt: string
  deadlineRef: string
  firstPostId: string
  idempotencyKey: string
  jobEventId: string
  jobEventKind: number
  jobResultKind: number
  objectiveRef: string
  publicProjection: ForumPublicProjection
  quoteCount: number
  relayUrl: string
  repositoryRefs: ReadonlyArray<string>
  requesterActorRef: string
  requiredCapabilityRefs: ReadonlyArray<string>
  state: ForumWorkRequestState
  title: string
  topicId: string
  updatedAt: string
  verificationCommandRef: string
  workRequestId: string
}>

export type ForumWorkRequestRelayLink = Readonly<{
  bridgeActorRef: string
  createdAt: string
  eventJson: string
  jobEventId: string
  jobEventKind: number
  linkId: string
  relayRef: string
  relayUrl: string
  topicId: string
  workRequestId: string
}>

export type ForumWorkRequestLifecyclePost = Readonly<{
  createdAt: string
  idempotencyKey: string
  lifecycleKind: ForumWorkRequestLifecycleKind
  lifecyclePostId: string
  postId: string
  receiptRef: string
  stateAfter: ForumWorkRequestState
  topicId: string
  workRequestId: string
}>

type ForumWorkRequestRow = Readonly<{
  budget_msats: number
  budget_sats: number
  created_at: string
  deadline_ref: string
  first_post_id: string
  id: string
  idempotency_key: string
  job_event_id: string
  job_event_kind: number
  job_result_kind: number
  objective_ref: string
  public_projection_json: string
  quote_count: number
  relay_url: string
  repository_refs_json: string
  requester_actor_ref: string
  required_capability_refs_json: string
  state: ForumWorkRequestState
  title: string
  topic_id: string
  updated_at: string
  verification_command_ref: string
}>

type ForumWorkRequestRelayLinkRow = Readonly<{
  bridge_actor_ref: string
  created_at: string
  event_json: string
  id: string
  job_event_id: string
  job_event_kind: number
  relay_ref: string
  relay_url: string
  topic_id: string
  work_request_id: string
}>

type ForumWorkRequestLifecyclePostRow = Readonly<{
  created_at: string
  id: string
  idempotency_key: string
  lifecycle_kind: ForumWorkRequestLifecycleKind
  post_id: string
  receipt_ref: string
  state_after: ForumWorkRequestState
  topic_id: string
  work_request_id: string
}>

export class ForumWorkRequestUnsafe extends Error {
  readonly reason: string

  constructor(reason: string) {
    super(reason)
    this.name = 'ForumWorkRequestUnsafe'
    this.reason = reason
  }
}

const unsafePublicWorkRequestPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|bearer\s+|cookie|file:\/\/|github\.com\/[^:/\s]+\/private|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?(key|repo)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(command|content|invoice|payment|payload|prompt|repo|runner|state)|secret|seed[_-]?phrase|sk-[a-z0-9]|ssh:\/\/|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

const storageError = (operation: string, error: unknown): ForumStorageError =>
  new ForumStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, ForumStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const publicProjectionFromJson = (value: string): ForumPublicProjection =>
  decodeForumPublicProjection(parseJsonRecord(value) ?? {})

export const assertPublicSafeWorkRequestMaterial = (
  value: unknown,
  field: string,
): void => {
  if (unsafePublicWorkRequestPattern.test(JSON.stringify(value))) {
    throw new ForumWorkRequestUnsafe(
      `${field} contains private, payment, credential, or raw prompt material.`,
    )
  }
}

const assertNoUnsafePublicMaterial = assertPublicSafeWorkRequestMaterial

const uniquePublicRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  Array.from(
    new Set(
      [...(refs ?? [])]
        .map(ref => ref.trim())
        .filter(ref => ref.length > 0),
    ),
  )

const validateArrayBound = (
  refs: ReadonlyArray<string>,
  field: string,
): void => {
  if (refs.length > 12) {
    throw new ForumWorkRequestUnsafe(`${field} accepts at most 12 refs.`)
  }
}

export const normalizeForumWorkRequestInput = (
  input: ForumWorkRequestInput,
): NormalizedForumWorkRequestInput => {
  const repositoryRefs = uniquePublicRefs(input.repositoryRefs)
  const requiredCapabilityRefs = uniquePublicRefs(input.requiredCapabilityRefs)
  const normalized: NormalizedForumWorkRequestInput = {
    budgetMsats: input.budgetSats * 1000,
    budgetSats: input.budgetSats,
    deadlineRef: input.deadlineRef.trim(),
    objectiveRef: input.objectiveRef.trim(),
    repositoryRefs:
      repositoryRefs.length === 0
        ? [DefaultForumWorkRequestRepositoryRef]
        : repositoryRefs,
    requiredCapabilityRefs:
      requiredCapabilityRefs.length === 0
        ? [DefaultForumWorkRequestCapabilityRef]
        : requiredCapabilityRefs,
    title: input.title.trim(),
    verificationCommandRef: input.verificationCommandRef.trim(),
  }

  if (
    !Number.isInteger(normalized.budgetSats) ||
    normalized.budgetSats <= 0 ||
    normalized.budgetSats > 21_000_000_000_000
  ) {
    throw new ForumWorkRequestUnsafe(
      'budgetSats must be a positive integer sat amount.',
    )
  }

  if (normalized.title.length < 3 || normalized.title.length > 160) {
    throw new ForumWorkRequestUnsafe('title must be 3-160 characters.')
  }

  validateArrayBound(normalized.repositoryRefs, 'repositoryRefs')
  validateArrayBound(normalized.requiredCapabilityRefs, 'requiredCapabilityRefs')
  assertNoUnsafePublicMaterial(normalized, 'work request')

  return normalized
}

export const buildForumWorkRequestLbrDraft = (
  input: NormalizedForumWorkRequestInput,
  options: Readonly<{
    relayUrl: string
    topicId: string
  }>,
): Readonly<{
  draft: LbrUnsignedEventDraft
  request: LbrAgenticCodingRequest
}> => {
  const request = makeLbrAgenticCodingRequest({
    bidMsats: input.budgetMsats,
    deadline: input.deadlineRef,
    forumTopicRef: `topic.public.forum.${options.topicId}`,
    objectiveRef: input.objectiveRef,
    relays: [options.relayUrl],
    repositoryRefs: input.repositoryRefs,
    requiredCapabilityRefs: input.requiredCapabilityRefs,
    verificationCommandRef: input.verificationCommandRef,
  })

  return {
    draft: lbrAgenticCodingRequestToDraft(request),
    request,
  }
}

export const forumWorkRequestEventRef = (jobEventId: string): string =>
  `nostr.event.${jobEventId}`

export const defaultForumWorkRequestRelayPublisher =
  (): ForumWorkRequestRelayPublisher => ({
    publishWorkRequest: async input => {
      const jobEventId = await sha256Hex(JSON.stringify(input.draft))

      return {
        accepted: false,
        event: {
          content: input.draft.content,
          id: jobEventId,
          kind: input.draft.kind,
          pubkey: input.bridgeActorRef,
          tags: input.draft.tags,
        },
        jobEventId,
        relayRef: `relay.public.unconfigured.${(
          await sha256Hex(input.relayUrl)
        ).slice(0, 32)}`,
        relayUrl: input.relayUrl,
      }
    },
  })

export const forumWorkRequestBodyText = (
  input: NormalizedForumWorkRequestInput,
  options: Readonly<{
    jobEventId: string
    relayUrl: string
    workRequestId: string
  }>,
): string => {
  const lines = [
    `Work request: ${input.title}`,
    `Request ref: work_request.public.${options.workRequestId}`,
    `Objective ref: ${input.objectiveRef}`,
    `Verification command ref: ${input.verificationCommandRef}`,
    `Budget: ${input.budgetSats} sats`,
    `Deadline ref: ${input.deadlineRef}`,
    `Repository refs: ${input.repositoryRefs.join(', ')}`,
    `Required capability refs: ${input.requiredCapabilityRefs.join(', ')}`,
    `NIP-LBR request kind: ${LBR_AGENTIC_CODING_REQUEST_KIND}`,
    `NIP-LBR result kind: ${LBR_AGENTIC_CODING_RESULT_KIND}`,
    `Relay ref: relay.public.openagents_market`,
    `Job event ref: ${forumWorkRequestEventRef(options.jobEventId)}`,
    'Escrow: pending; this Forum route does not move funds.',
  ]
  const bodyText = lines.join('\n')
  assertNoUnsafePublicMaterial(bodyText, 'work request body')

  return bodyText
}

export const forumWorkRequestLifecycleBodyText = (
  lifecycleKind: ForumWorkRequestLifecycleKind,
  receiptRef: string,
  workRequestId: string,
): string => {
  const bodyText = [
    `Work request lifecycle update: ${lifecycleKind}`,
    `Request ref: work_request.public.${workRequestId}`,
    `Receipt ref: ${receiptRef}`,
  ].join('\n')
  assertNoUnsafePublicMaterial(bodyText, 'work request lifecycle body')

  return bodyText
}

const workRequestFromRow = (row: ForumWorkRequestRow): ForumWorkRequestRecord => ({
  budgetMsats: row.budget_msats,
  budgetSats: row.budget_sats,
  createdAt: row.created_at,
  deadlineRef: row.deadline_ref,
  firstPostId: row.first_post_id,
  idempotencyKey: row.idempotency_key,
  jobEventId: row.job_event_id,
  jobEventKind: row.job_event_kind,
  jobResultKind: row.job_result_kind,
  objectiveRef: row.objective_ref,
  publicProjection: publicProjectionFromJson(row.public_projection_json),
  quoteCount: row.quote_count,
  relayUrl: row.relay_url,
  repositoryRefs: parseJsonStringArray(row.repository_refs_json),
  requesterActorRef: row.requester_actor_ref,
  requiredCapabilityRefs: parseJsonStringArray(
    row.required_capability_refs_json,
  ),
  state: row.state,
  title: row.title,
  topicId: row.topic_id,
  updatedAt: row.updated_at,
  verificationCommandRef: row.verification_command_ref,
  workRequestId: row.id,
})

const relayLinkFromRow = (
  row: ForumWorkRequestRelayLinkRow,
): ForumWorkRequestRelayLink => ({
  bridgeActorRef: row.bridge_actor_ref,
  createdAt: row.created_at,
  eventJson: row.event_json,
  jobEventId: row.job_event_id,
  jobEventKind: row.job_event_kind,
  linkId: row.id,
  relayRef: row.relay_ref,
  relayUrl: row.relay_url,
  topicId: row.topic_id,
  workRequestId: row.work_request_id,
})

const lifecyclePostFromRow = (
  row: ForumWorkRequestLifecyclePostRow,
): ForumWorkRequestLifecyclePost => ({
  createdAt: row.created_at,
  idempotencyKey: row.idempotency_key,
  lifecycleKind: row.lifecycle_kind,
  lifecyclePostId: row.id,
  postId: row.post_id,
  receiptRef: row.receipt_ref,
  stateAfter: row.state_after,
  topicId: row.topic_id,
  workRequestId: row.work_request_id,
})

export const readForumWorkRequestById = (
  db: D1Database,
  workRequestId: string,
): Effect.Effect<ForumWorkRequestRecord | null, ForumStorageError> =>
  d1Effect('forumWorkRequests.readById', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_requests
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(workRequestId)
      .first<ForumWorkRequestRow>(),
  ).pipe(Effect.map(row => (row === null ? null : workRequestFromRow(row))))

export const readForumWorkRequestByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<ForumWorkRequestRecord | null, ForumStorageError> =>
  d1Effect('forumWorkRequests.readByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_requests
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ForumWorkRequestRow>(),
  ).pipe(Effect.map(row => (row === null ? null : workRequestFromRow(row))))

export const readForumWorkRequestByJobEventId = (
  db: D1Database,
  jobEventId: string,
): Effect.Effect<ForumWorkRequestRecord | null, ForumStorageError> =>
  d1Effect('forumWorkRequests.readByJobEventId', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_requests
          WHERE job_event_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(jobEventId)
      .first<ForumWorkRequestRow>(),
  ).pipe(Effect.map(row => (row === null ? null : workRequestFromRow(row))))

export const readForumWorkRequestRelayLinkByWorkRequestId = (
  db: D1Database,
  workRequestId: string,
): Effect.Effect<ForumWorkRequestRelayLink | null, ForumStorageError> =>
  d1Effect('forumWorkRequests.readRelayLinkByWorkRequestId', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_request_relay_links
          WHERE work_request_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(workRequestId)
      .first<ForumWorkRequestRelayLinkRow>(),
  ).pipe(Effect.map(row => (row === null ? null : relayLinkFromRow(row))))

export const readForumWorkRequestRelayLinkByJobEventId = (
  db: D1Database,
  jobEventId: string,
): Effect.Effect<ForumWorkRequestRelayLink | null, ForumStorageError> =>
  d1Effect('forumWorkRequests.readRelayLinkByJobEventId', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_request_relay_links
          WHERE job_event_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(jobEventId)
      .first<ForumWorkRequestRelayLinkRow>(),
  ).pipe(Effect.map(row => (row === null ? null : relayLinkFromRow(row))))

export const listOpenForumWorkRequests = (
  db: D1Database,
  limit = 50,
): Effect.Effect<ReadonlyArray<ForumWorkRequestRecord>, ForumStorageError> =>
  d1Effect('forumWorkRequests.listOpen', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_requests
          WHERE archived_at IS NULL
            AND state IN ('open', 'quote_received', 'quote_accepted', 'running')
          ORDER BY created_at DESC, id DESC
          LIMIT ?`,
      )
      .bind(Math.max(1, Math.min(100, Math.trunc(limit))))
      .all<ForumWorkRequestRow>(),
  ).pipe(
    Effect.map(result => (result.results ?? []).map(workRequestFromRow)),
  )

export const recordForumWorkRequest = (
  db: D1Database,
  input: Readonly<{
    bridgeActorRef: string
    firstPostId: string
    idempotencyKey: string
    jobEventId: string
    publicProjection: ForumPublicProjection
    relayEvent: unknown
    relayRef: string
    relayUrl: string
    request: NormalizedForumWorkRequestInput
    requesterActorRef: string
    topicId: string
    workRequestId: string
  }>,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<ForumWorkRequestRecord, ForumStorageError | ForumValidationError> =>
  Effect.gen(function* () {
    const now = runtime.nowIso()

    yield* d1Effect('forumWorkRequests.insertWorkRequest', () =>
      db
        .prepare(
          `INSERT INTO forum_work_requests (
             id,
             idempotency_key,
             topic_id,
             first_post_id,
             requester_actor_ref,
             title,
             objective_ref,
             verification_command_ref,
             repository_refs_json,
             required_capability_refs_json,
             budget_sats,
             budget_msats,
             deadline_ref,
             relay_url,
             job_event_id,
             job_event_kind,
             job_result_kind,
             state,
             quote_count,
             public_projection_json,
             created_at,
             updated_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'open', 0, ?, ?, ?)`,
        )
        .bind(
          input.workRequestId,
          input.idempotencyKey,
          input.topicId,
          input.firstPostId,
          input.requesterActorRef,
          input.request.title,
          input.request.objectiveRef,
          input.request.verificationCommandRef,
          JSON.stringify(input.request.repositoryRefs),
          JSON.stringify(input.request.requiredCapabilityRefs),
          input.request.budgetSats,
          input.request.budgetMsats,
          input.request.deadlineRef,
          input.relayUrl,
          input.jobEventId,
          LBR_AGENTIC_CODING_REQUEST_KIND,
          LBR_AGENTIC_CODING_RESULT_KIND,
          JSON.stringify(input.publicProjection),
          now,
          now,
        )
        .run(),
    )

    yield* d1Effect('forumWorkRequests.insertRelayLink', () =>
      db
        .prepare(
          `INSERT INTO forum_work_request_relay_links (
             id,
             work_request_id,
             topic_id,
             job_event_id,
             job_event_kind,
             relay_url,
             relay_ref,
             bridge_actor_ref,
             event_json,
             created_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          `${input.workRequestId}:relay-link`,
          input.workRequestId,
          input.topicId,
          input.jobEventId,
          LBR_AGENTIC_CODING_REQUEST_KIND,
          input.relayUrl,
          input.relayRef,
          input.bridgeActorRef,
          JSON.stringify(input.relayEvent),
          now,
        )
        .run(),
    )

    const record = yield* readForumWorkRequestById(db, input.workRequestId)

    if (record === null) {
      return yield* new ForumValidationError({
        reason: 'Forum work request was not persisted.',
      })
    }

    return record
  })

export const readForumWorkRequestLifecycleByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<ForumWorkRequestLifecyclePost | null, ForumStorageError> =>
  d1Effect('forumWorkRequests.readLifecycleByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_work_request_lifecycle_posts
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ForumWorkRequestLifecyclePostRow>(),
  ).pipe(Effect.map(row => (row === null ? null : lifecyclePostFromRow(row))))

const stateAfterLifecycle = (
  lifecycleKind: ForumWorkRequestLifecycleKind,
): ForumWorkRequestState => lifecycleKind

export const recordForumWorkRequestLifecyclePost = (
  db: D1Database,
  input: Readonly<{
    idempotencyKey: string
    lifecycleKind: ForumWorkRequestLifecycleKind
    lifecyclePostId: string
    postId: string
    receiptRef: string
    topicId: string
    workRequestId: string
  }>,
  runtime: ForumRepositoryRuntime = systemForumRepositoryRuntime,
): Effect.Effect<
  ForumWorkRequestLifecyclePost,
  ForumStorageError | ForumValidationError
> =>
  Effect.gen(function* () {
    const now = runtime.nowIso()
    const stateAfter = stateAfterLifecycle(input.lifecycleKind)

    yield* d1Effect('forumWorkRequests.insertLifecyclePost', () =>
      db
        .prepare(
          `INSERT INTO forum_work_request_lifecycle_posts (
             id,
             work_request_id,
             topic_id,
             post_id,
             idempotency_key,
             lifecycle_kind,
             receipt_ref,
             state_after,
             created_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          input.lifecyclePostId,
          input.workRequestId,
          input.topicId,
          input.postId,
          input.idempotencyKey,
          input.lifecycleKind,
          input.receiptRef,
          stateAfter,
          now,
        )
        .run(),
    )

    yield* d1Effect('forumWorkRequests.updateLifecycleState', () =>
      db
        .prepare(
          `UPDATE forum_work_requests
              SET state = ?,
                  quote_count = quote_count + CASE WHEN ? = 'quote_received' THEN 1 ELSE 0 END,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(stateAfter, input.lifecycleKind, now, input.workRequestId)
        .run(),
    )

    const recorded = yield* readForumWorkRequestLifecycleByIdempotencyKey(
      db,
      input.idempotencyKey,
    )

    if (recorded === null) {
      return yield* new ForumValidationError({
        reason: 'Forum work request lifecycle post was not persisted.',
      })
    }

    return recorded
  })

export const decodeRelayNativeLbrWorkRequest = (
  event: unknown,
): Readonly<{
  eventId: string
  request: NormalizedForumWorkRequestInput
}> => {
  const parsedEvent = parseJsonRecord(JSON.stringify(event)) ?? {}
  const eventId = String(parsedEvent.id ?? '').toLowerCase()

  if (!/^[a-f0-9]{64}$/.test(eventId)) {
    throw new ForumWorkRequestUnsafe('relay event id must be a 64-byte hex ref.')
  }

  const request = decodeLbrAgenticCodingRequestEvent(event)

  return {
    eventId,
    request: normalizeForumWorkRequestInput({
      budgetSats: Math.ceil(request.bidMsats / 1000),
      deadlineRef: request.deadline ?? 'deadline.public.unspecified',
      objectiveRef: request.objectiveRef,
      repositoryRefs: request.repositoryRefs,
      requiredCapabilityRefs: request.requiredCapabilityRefs,
      title: `Relay work request ${eventId.slice(0, 12)}`,
      verificationCommandRef: request.verificationCommandRef,
    }),
  }
}

export const forumWorkRequestErrorToValidationError = (
  error: unknown,
): ForumValidationError =>
  error instanceof ForumWorkRequestUnsafe || error instanceof LbrProtocolError
    ? new ForumValidationError({ reason: error.message })
    : new ForumValidationError({
        reason:
          error instanceof Error
            ? error.message
            : 'Forum work request is invalid.',
      })
