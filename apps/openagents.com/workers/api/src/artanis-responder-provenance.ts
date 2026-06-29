// Artanis Pylon-support responder: asker-provenance classification and the
// public external-contributor-flow projection. This is the missing piece
// for the external_contributor_flow_unproven blocker on
// artanis.pylon_support_responder.v1.
//
// The responder loop (artanis-forum-responder.ts -> artanis-reply-composer.ts)
// already scans new Forum topics, classifies Pylon device/training
// questions, composes grounded replies under the registered Artanis
// identity, and tips good questions. What the loop never recorded or
// surfaced was the PROVENANCE of the asker: every demonstrated run so far
// used operator-authored test articles, so there was no machine-auditable
// way to tell whether the responder had ever answered a real EXTERNAL
// contributor (a non-owner, non-operator, non-Artanis identity) end to end.
//
// This module supplies (1) a typed classifier that maps an asking actor ref
// to a bounded provenance enum and (2) a read-only public projection that
// reports whether any external-contributor support interaction has been
// answered end to end, with the dereferenceable reply-post ref for each.
//
// Provenance classification is deterministic over a BOUNDED set of identity
// fields (the actor-ref prefix taxonomy operator:/owner:/agent:/user: plus
// the known internal Artanis and admin refs). This is identity-field
// classification, not user-facing intent routing, so the semantic-routing
// rule (no ad hoc string matching for intent) does not apply: the mind still
// owns the question-class decision; this only labels WHO asked.

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import {
  readArtanisResponderTickReadiness,
  type ArtanisResponderTickReadinessProjection,
} from './artanis-responder-ticks'

// Staleness contract (epic #4751): composed live from the responder-action
// ledger at read time, so it can never be older than the request.
export const ARTANIS_RESPONDER_SUPPORT_STALENESS: PublicProjectionStalenessContract =
  liveAtReadStaleness([
    'artanis_responder_actions.insert',
    'artanis_responder_actions.update',
  ])

// The bounded asker-provenance enum. external_contributor is the only class
// the green gate cares about; the others exist so the classifier is total
// and the projection can honestly say WHY a given interaction does not count.
export type ArtanisAskerProvenance =
  | 'external_contributor'
  | 'owner_operator'
  | 'artanis_self'
  | 'unknown'

// Known-internal exact actor refs. The Artanis registered identity and the
// seeded forum-delivery agent are never external; operator/owner refs and
// the admin user ref are the owner/operator side.
const ARTANIS_INTERNAL_ACTOR_REFS: ReadonlySet<string> = new Set([
  // Registered Artanis agent identity used for conversational replies.
  'agent:user_ed6d486e-612a-4fac-a9a9-44f7e5709505',
  // Seeded Artanis forum-delivery agent identity.
  'agent:agent_artanis',
])

// Classify an asking actor ref into the bounded provenance enum. The input
// is the Forum actor ref captured at scan time (operator:/owner:/agent:/user:
// per forum/actor-context.ts). adminActorRefs lets the caller pin owner/admin
// identities (e.g. the chris@openagents.com user) that post through the
// normal user path so operator-authored test articles never count as
// external.
export const classifyAskerProvenance = (
  actorRef: string | null | undefined,
  options: Readonly<{ adminActorRefs?: ReadonlyArray<string> }> = {},
): ArtanisAskerProvenance => {
  const ref = (actorRef ?? '').trim()
  if (ref === '') return 'unknown'

  if (ARTANIS_INTERNAL_ACTOR_REFS.has(ref)) return 'artanis_self'

  const adminRefs = new Set(options.adminActorRefs ?? [])
  if (adminRefs.has(ref)) return 'owner_operator'

  // The owner/operator side of the ref taxonomy.
  if (ref.startsWith('operator:') || ref.startsWith('owner:')) {
    return 'owner_operator'
  }

  // A registered external contributor: a normal user or a non-internal agent
  // identity. These are the only refs that can satisfy the external-contributor
  // gate.
  if (ref.startsWith('user:') || ref.startsWith('agent:')) {
    return 'external_contributor'
  }

  return 'unknown'
}

// A terminal responder state that means the asker actually received a reply.
const ANSWERED_STATES: ReadonlySet<string> = new Set(['responded', 'tipped'])

export type ArtanisResponderActionRow = Readonly<{
  id: unknown
  topic_id: unknown
  state: unknown
  question_class: unknown
  asker_actor_ref: unknown
  asker_provenance: unknown
  reply_post_id: unknown
  asked_at: unknown
  replied_at: unknown
  tip_receipt_ref: unknown
}>

export type ArtanisExternalSupportInteraction = Readonly<{
  topicId: string
  questionClass: string | null
  // The reply post a reader can dereference to inspect the answer.
  replyPostRef: string | null
  publicUrl: string | null
  tipReceiptRef: string | null
  askedAt: string | null
  repliedAt: string | null
  // Whether the asker was tipped (the full economic leg of the flow).
  tipped: boolean
}>

export type ArtanisResponderSupportProjection = Readonly<{
  kind: 'artanis_pylon_support_responder_external_flow'
  publicSafe: true
  authorityBoundary: string
  staleness: PublicProjectionStalenessContract
  // Counts over the projected window, split by provenance honesty.
  externalContributorAnsweredCount: number
  externalContributorTippedCount: number
  ownerOperatorAnsweredCount: number
  // True once at least one external contributor has been answered end to end
  // with a dereferenceable reply post. This is the gate the blocker tracks.
  externalContributorFlowProven: boolean
  // The external-contributor interactions, newest first, each with the
  // dereferenceable reply-post ref.
  externalInteractions: ReadonlyArray<ArtanisExternalSupportInteraction>
  tickReadiness?: ArtanisResponderTickReadinessProjection
  generatedAt: string
  notes: ReadonlyArray<string>
}>

const VALID_QUESTION_CLASSES: ReadonlySet<string> = new Set([
  'device_capability',
  'training_run',
  'pylon_troubleshooting',
])

// Only refs that themselves look like a public-safe ref token are projected,
// so nothing unexpected smuggled into a column can leak.
const safeRefPattern = /^[A-Za-z0-9._:-]{1,200}$/

const safeRef = (value: unknown): string | null => {
  const text = value === null || value === undefined ? '' : String(value)
  return text !== '' && safeRefPattern.test(text) ? text : null
}

export const projectArtanisResponderSupport = (
  rows: ReadonlyArray<ArtanisResponderActionRow>,
  nowIso: string,
  tickReadiness?: ArtanisResponderTickReadinessProjection,
): ArtanisResponderSupportProjection => {
  let externalContributorAnsweredCount = 0
  let externalContributorTippedCount = 0
  let ownerOperatorAnsweredCount = 0
  const externalInteractions: ArtanisExternalSupportInteraction[] = []

  for (const row of rows) {
    const state = String(row.state ?? '')
    if (!ANSWERED_STATES.has(state)) continue

    // Provenance is read from the recorded column; if a legacy row never
    // recorded it, re-derive from the recorded actor ref so the projection
    // is honest about older rows too.
    const recordedProvenance = String(row.asker_provenance ?? '')
    const provenance: ArtanisAskerProvenance = (
      [
        'external_contributor',
        'owner_operator',
        'artanis_self',
        'unknown',
      ] as const
    ).includes(recordedProvenance as ArtanisAskerProvenance)
      ? (recordedProvenance as ArtanisAskerProvenance)
      : classifyAskerProvenance(
          row.asker_actor_ref === null || row.asker_actor_ref === undefined
            ? null
            : String(row.asker_actor_ref),
        )

    if (provenance === 'owner_operator') {
      ownerOperatorAnsweredCount += 1
      continue
    }
    if (provenance !== 'external_contributor') continue

    externalContributorAnsweredCount += 1
    const tipped = state === 'tipped'
    if (tipped) externalContributorTippedCount += 1

    const replyPostRef = safeRef(row.reply_post_id)
    const questionClassRaw = String(row.question_class ?? '')
    const tipReceiptRef = safeRef(row.tip_receipt_ref)

    externalInteractions.push({
      askedAt: safeRef(row.asked_at),
      publicUrl:
        replyPostRef === null
          ? null
          : `/forum/t/${String(row.topic_id ?? '')}#post-${replyPostRef}`,
      questionClass: VALID_QUESTION_CLASSES.has(questionClassRaw)
        ? questionClassRaw
        : null,
      repliedAt: safeRef(row.replied_at),
      replyPostRef,
      tipped,
      tipReceiptRef,
      topicId: String(row.topic_id ?? ''),
    })
  }

  const externalContributorFlowProven = externalInteractions.some(
    interaction => interaction.replyPostRef !== null,
  )

  return {
    authorityBoundary:
      'Read-only projection over the Artanis responder-action ledger. Grants no dispatch, spend, assignment, settlement, moderation, or registry authority and cannot create an interaction, a reply, or a tip. Asker provenance is a bounded identity-field classification (operator/owner/agent/user prefix plus known internal Artanis/admin refs); the mind still owns the question-class decision.',
    externalContributorAnsweredCount,
    externalContributorFlowProven,
    externalContributorTippedCount,
    externalInteractions,
    generatedAt: nowIso,
    kind: 'artanis_pylon_support_responder_external_flow',
    notes: [
      'An external contributor is a registered non-owner, non-operator, non-Artanis identity (a user: actor or a non-internal agent: actor). Operator/owner test articles are classified owner_operator and never satisfy the external-contributor gate.',
      'externalContributorFlowProven becomes true only when at least one external contributor has been answered end to end with a dereferenceable reply post (state responded or tipped); the reply post is fetchable at the publicUrl.',
      'A tipped interaction additionally proves the budget-gated economic leg (reliable-tips ladder) on a real external post.',
    ],
    ownerOperatorAnsweredCount,
    publicSafe: true,
    staleness: ARTANIS_RESPONDER_SUPPORT_STALENESS,
    ...(tickReadiness === undefined ? {} : { tickReadiness }),
  }
}

export const ARTANIS_RESPONDER_SUPPORT_MAX_LIMIT = 200

export const boundedResponderSupportLimit = (raw: string | null): number => {
  const parsed = Number(raw ?? '100')
  if (!Number.isFinite(parsed)) return 100
  return Math.min(
    Math.max(1, Math.trunc(parsed)),
    ARTANIS_RESPONDER_SUPPORT_MAX_LIMIT,
  )
}

export const readArtanisResponderSupport = async (
  db: D1Database,
  input: Readonly<{ limit: number; nowIso: string }>,
): Promise<ArtanisResponderSupportProjection> => {
  const limit = boundedResponderSupportLimit(String(input.limit))
  const result = await db
    .prepare(
      `SELECT id,
              topic_id,
              state,
              question_class,
              asker_actor_ref,
              asker_provenance,
              reply_post_id,
              asked_at,
              replied_at,
              tip_receipt_ref
         FROM artanis_responder_actions
        WHERE state IN ('responded', 'tipped')
        ORDER BY created_at DESC
        LIMIT ?`,
    )
    .bind(limit)
    .all()

  const rows = (result.results ??
    []) as unknown as ReadonlyArray<ArtanisResponderActionRow>
  const tickReadiness = await readArtanisResponderTickReadiness(db, {
    limit: input.limit,
  })

  return projectArtanisResponderSupport(rows, input.nowIso, tickReadiness)
}
