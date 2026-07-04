import { Effect } from 'effect'

import {
  ArtanisMindEscalatedMaxOutputTokens,
  artanisMindComplete,
} from './artanis-mind'
import {
  artanisAuthorityDb,
  mirrorArtanisRows,
  type ArtanisDatabase,
} from './artanis-domain-store'
import { artanisDiagnosisGroundingPolicy } from './artanis-diagnosis-grounding-gate'
import { artanisOperationalGrounding } from './artanis-operational-grounding'
import { recordArtanisResponderComposeTick } from './artanis-responder-ticks'
import { publicProductPromisesDocument } from './product-promises'
import {
  TIP_LADDER_RECEIPT_REF_PREFIX,
  artanisResponderTipReceiptRef,
  isTipLadderReceiptRef,
} from './tip-ladder'

// The Artanis grounded reply composer + tip budget (issue #4715;
// promise artanis.pylon_support_responder.v1). For each proposed
// responder action: assemble live platform context (the asker's own
// post - which carries the device inventory the Pylon embedded - plus
// the current promise registry states for the Pylon/Tassadar lanes),
// have the mind compose a reply whose every platform claim comes from
// that context, deliver it as the REGISTERED Artanis identity through
// the real forum route (in-process, full policy), and tip the question
// under the per-tick/per-day budget through the reliable-tips ladder.
// Response windows (asked_at -> replied_at) are recorded per action.

export const ARTANIS_COMPOSER_MAX_PER_TICK = 2
export const ARTANIS_TIP_AMOUNT_SAT = 50
export const ARTANIS_TIP_BUDGET_PER_DAY_SAT = 210

export type ComposerForumPost = (input: {
  topicId: string
  bodyText: string
  idempotencyKey: string
}) => Promise<{ postId: string } | { error: string }>

export type ComposerTip = (input: {
  postId: string
  amountSat: number
  idempotencyKey: string
  publicReceiptRef: string
}) => Promise<
  | {
      ladderReason: string
      payInId: string
      receiptRef: string
      rung: string
    }
  | { error: string }
>

export type ComposerTickOutcome = Readonly<{
  considered: number
  responded: number
  tipped: number
  blocked: number
  skippedReason: string | null
}>

const tipReceiptRefIsDereferenceable = async (
  db: D1Database,
  receiptRef: string,
): Promise<boolean> => {
  if (!isTipLadderReceiptRef(receiptRef)) {
    return false
  }

  const row = await db
    .prepare(
      `SELECT COALESCE(
                p.public_receipt_ref,
                ? || 'payin.' || p.id
              ) AS receipt_ref
         FROM pay_ins p
         JOIN pay_in_legs payout
           ON payout.pay_in_id = p.id
          AND payout.direction = 'out'
        WHERE p.pay_in_type = 'tip'
          AND (p.public_receipt_ref = ?
               OR (p.public_receipt_ref IS NULL
                   AND ? || 'payin.' || p.id = ?))
          AND p.state IN ('paid', 'forwarding')
          AND p.context_ref LIKE 'forum.post.%'
        ORDER BY CASE WHEN p.state = 'paid' THEN 0 ELSE 1 END,
                 p.created_at DESC,
                 p.id DESC
        LIMIT 1`,
    )
    .bind(
      TIP_LADDER_RECEIPT_REF_PREFIX,
      receiptRef,
      TIP_LADDER_RECEIPT_REF_PREFIX,
      receiptRef,
    )
    .first<{ receipt_ref: string | null }>()
    .catch(() => null)

  return row?.receipt_ref === receiptRef
}

const groundingPromises = () => {
  const document = publicProductPromisesDocument()
  return document.promises
    .filter(
      promise =>
        promise.promiseId.startsWith('pylon.') ||
        promise.promiseId.startsWith('compute.') ||
        promise.promiseId.startsWith('artanis.') ||
        promise.promiseId.startsWith('payments.'),
    )
    .map(promise => ({
      claim: promise.safeCopy.slice(0, 280),
      promiseId: promise.promiseId,
      state: promise.state,
    }))
}

export const runArtanisComposerTick = async (
  database: ArtanisDatabase,
  deps: Readonly<{
    geminiApiKey: string | null
    gatewayToken?: string | undefined
    forumPost: ComposerForumPost
    tip: ComposerTip
    artanisActorRef: string
    nowIso: string
  }>,
): Promise<ComposerTickOutcome> => {
  // The authoritative D1 handle; every write below mirrors to Postgres
  // through the KS-8.6 seam before the tick returns (fail-soft).
  const db = artanisAuthorityDb(database)
  if (deps.geminiApiKey === null || deps.geminiApiKey === '') {
    return {
      blocked: 0,
      considered: 0,
      responded: 0,
      skippedReason: 'mind_unconfigured',
      tipped: 0,
    }
  }

  const proposals = ((
    await db
      .prepare(
        `SELECT a.id, a.topic_id, a.first_post_id, a.question_class, a.asked_at,
                  t.title, COALESCE(b.body_text, '') AS body_text
             FROM artanis_responder_actions a
             JOIN forum_topics t ON t.id = a.topic_id
        LEFT JOIN forum_post_bodies b ON b.post_id = a.first_post_id
            WHERE a.state = 'proposed'
            ORDER BY a.created_at ASC
            LIMIT ?`,
      )
      .bind(ARTANIS_COMPOSER_MAX_PER_TICK)
      .all()
  ).results ?? []) as Array<Record<string, unknown>>

  if (proposals.length === 0) {
    return {
      blocked: 0,
      considered: 0,
      responded: 0,
      skippedReason: null,
      tipped: 0,
    }
  }

  // Daily tip budget from the ledger itself - no separate counter to
  // drift: sum of Artanis's paid tip pay-ins created today.
  // The budget gates the RESPONDER's spend specifically (its own
  // idempotency namespace), not every tip the Artanis identity has ever
  // sent - operator-driven smoke tips must not starve the responder.
  const tipBudgetRow = (await db
    .prepare(
      `SELECT COALESCE(SUM(cost_msat), 0) AS spent
         FROM pay_ins
        WHERE payer_ref = ? AND pay_in_type = 'tip' AND state = 'paid'
          AND idempotency_key LIKE 'artanis-responder-tip:%'
          AND created_at >= ?`,
    )
    .bind(deps.artanisActorRef, `${deps.nowIso.slice(0, 10)}T00:00:00.000Z`)
    .first()) as { spent: number } | null
  let tipBudgetLeftSat =
    ARTANIS_TIP_BUDGET_PER_DAY_SAT -
    Math.floor(Number(tipBudgetRow?.spent ?? 0) / 1000)

  let responded = 0
  let tipped = 0
  let blocked = 0

  for (const proposal of proposals) {
    const topicId = String(proposal.topic_id)
    const actionId = String(proposal.id)
    const grounding = {
      // Operational runbook facts come FIRST so the mind answers the
      // concrete how-to question (e.g. making payout-target ready, keeping
      // executor-trace capability refs live through heartbeat) instead of
      // restating promise-registry copy (#5540 defect 2).
      diagnosisGrounding: artanisDiagnosisGroundingPolicy(),
      operationalDocs: artanisOperationalGrounding(),
      promiseRegistry: groundingPromises(),
      question: {
        bodyText: String(proposal.body_text).slice(0, 3000),
        questionClass: proposal.question_class,
        title: String(proposal.title),
      },
    }

    const mindResult = await artanisMindComplete({
      apiKey: deps.geminiApiKey,
      ...(deps.gatewayToken === undefined || deps.gatewayToken === ''
        ? {}
        : { gatewayToken: deps.gatewayToken }),
      // Grounded replies are long (150-350 words over a rich grounding
      // payload); give them headroom up front and let the mind escalate
      // once more on truncation rather than posting a cut-off answer (#5540
      // defect 3).
      maxOutputTokens: ArtanisMindEscalatedMaxOutputTokens,
      prompt: [
        'Compose a reply to this Pylon contributor question. GROUNDING RULES (absolute): every claim about the platform, promises, capabilities, dispatch, or payments must come from the grounding JSON below - if the grounding does not answer part of the question, say so plainly rather than inventing. When the question is a concrete operational how-to (e.g. making payout-target/send readiness true, keeping capability refs live through heartbeat, running a no-spend lane), answer it directly from operationalDocs with the specific commands, blocker names, and readiness states - do NOT just restate promiseRegistry status copy. Device facts come only from the question body (the Pylon embedded its own inventory there). Do not assert a root cause or propose a remediation for autonomous ops failures unless diagnosisGrounding is satisfied at GROUNDED with all required refs present and matching the claim. Be specific, useful, and honest about what is yellow vs green. 150-350 words, plain text. End with: - Artanis (automated responder; the mind proposes, schemas validate, gates hold)',
        `GROUNDING: ${JSON.stringify(grounding)}`,
      ].join('\n\n'),
      system:
        'You are Artanis, the Nexus administrator of OpenAgents - the AI agent that distributes work to Pylons and keeps devices utilized. You answer contributor questions with grounded platform facts only, preferring the concrete operational runbook facts when the question is operational.',
    })

    if ('error' in mindResult) {
      blocked += 1
      await db
        .prepare(
          `UPDATE artanis_responder_actions
           SET state = 'blocked', proposal_json = ?, updated_at = ?
           WHERE id = ? AND state = 'proposed'`,
        )
        .bind(
          JSON.stringify({ reason: 'mind_unavailable_at_compose' }),
          deps.nowIso,
          actionId,
        )
        .run()
      continue
    }

    // Receipt honesty (#5540 defect 1): a reply may carry a tip-receipt ref
    // ONLY if the tip actually settled and the ref the ladder returned is a
    // dereferenceable public receipt ref. So we attempt the tip FIRST, then
    // decide whether to embed a ref. The previous code embedded a synthetic
    // ref keyed on topicId before the tip ran, leaving a non-dereferenceable
    // ref in the public reply whenever the tip failed (the cinder-atlas 404
    // with tipStats stuck at 0).
    const shouldTip = tipBudgetLeftSat >= ARTANIS_TIP_AMOUNT_SAT

    type SettledTip = {
      ladderReason: string
      payInId: string
      receiptRef: string
      rung: string
    }
    let settledTip: SettledTip | null = null
    if (shouldTip) {
      const tipResult = await deps.tip({
        amountSat: ARTANIS_TIP_AMOUNT_SAT,
        idempotencyKey: `artanis-responder-tip:${topicId}`,
        postId: String(proposal.first_post_id),
        publicReceiptRef: artanisResponderTipReceiptRef(topicId),
      })
      // Trust only a settled tip whose returned ref resolves through the
      // public Forum receipt API; a syntactically valid 404 still gets no
      // public ref.
      if (
        !('error' in tipResult) &&
        (await tipReceiptRefIsDereferenceable(db, tipResult.receiptRef))
      ) {
        settledTip = tipResult
      }
    }

    const replyBodyText =
      settledTip === null
        ? mindResult.text
        : `${mindResult.text}\n\nResponder tip receipt: ${settledTip.receiptRef}`

    const posted = await deps.forumPost({
      bodyText: replyBodyText,
      idempotencyKey: `artanis-responder:${topicId}`,
      topicId,
    })

    if ('error' in posted) {
      blocked += 1
      await db
        .prepare(
          `UPDATE artanis_responder_actions
           SET state = 'blocked', proposal_json = ?, updated_at = ?
           WHERE id = ? AND state = 'proposed'`,
        )
        .bind(
          JSON.stringify({
            reason: `forum_post_failed:${posted.error}`.slice(0, 200),
            // The tip (if any) already settled on-ledger and remains
            // dereferenceable; we just could not deliver the reply this tick.
            ...(settledTip === null
              ? {}
              : { tipReceiptRef: settledTip.receiptRef }),
          }),
          deps.nowIso,
          actionId,
        )
        .run()
      continue
    }

    responded += 1
    await db
      .prepare(
        `UPDATE artanis_responder_actions
         SET state = 'responded', reply_post_id = ?, replied_at = ?, updated_at = ?
         WHERE id = ?`,
      )
      .bind(posted.postId, deps.nowIso, deps.nowIso, actionId)
      .run()
    await db
      .prepare(
        `UPDATE artanis_responder_state
         SET responses_today = CASE WHEN responses_day = ? THEN responses_today + 1 ELSE 1 END,
             responses_day = ?, updated_at = ?
         WHERE id = 1`,
      )
      .bind(deps.nowIso.slice(0, 10), deps.nowIso.slice(0, 10), deps.nowIso)
      .run()

    if (settledTip !== null) {
      tipped += 1
      tipBudgetLeftSat -= ARTANIS_TIP_AMOUNT_SAT
      await db
        .prepare(
          `UPDATE artanis_responder_actions
           SET state = 'tipped',
               tip_receipt_ref = ?,
               tip_pay_in_id = ?,
               tip_ladder_rung = ?,
               tip_ladder_reason = ?,
               updated_at = ?
           WHERE id = ?`,
        )
        .bind(
          settledTip.receiptRef,
          settledTip.payInId,
          settledTip.rung,
          settledTip.ladderReason,
          deps.nowIso,
          actionId,
        )
        .run()
    }
  }

  // KS-8.6 dual-write: converge every action row this tick touched (and
  // the response counter) into Postgres. Never throws — a mirror failure
  // never fails the compose tick.
  await mirrorArtanisRows(
    database,
    'artanis_responder_actions',
    'topic_id',
    proposals.map(proposal => String(proposal.topic_id)),
  )
  await mirrorArtanisRows(database, 'artanis_responder_state', 'id', [1])

  return {
    blocked,
    considered: proposals.length,
    responded,
    skippedReason: null,
    tipped,
  }
}

export const runArtanisComposerScheduled = (
  db: ArtanisDatabase,
  deps: Readonly<{
    enabled: boolean
    geminiApiKey: string | null
    gatewayToken?: string | undefined
    forumPost: ComposerForumPost
    tip: ComposerTip
    artanisActorRef: string
    nowIso: string
  }>,
): Effect.Effect<ComposerTickOutcome, never> =>
  deps.enabled
    ? Effect.tryPromise({
        catch: () => 'composer_tick_error' as const,
        try: () => runArtanisComposerTick(db, deps),
      }).pipe(
        Effect.catch(reason =>
          Effect.succeed({
            blocked: 0,
            considered: 0,
            responded: 0,
            skippedReason: reason,
            tipped: 0,
          } satisfies ComposerTickOutcome),
        ),
        Effect.flatMap(outcome =>
          Effect.promise(() =>
            recordArtanisResponderComposeTick(db, {
              nowIso: deps.nowIso,
              outcome,
            }),
          ).pipe(Effect.as(outcome)),
        ),
      )
    : Effect.succeed({
        blocked: 0,
        considered: 0,
        responded: 0,
        skippedReason: 'responder_disabled',
        tipped: 0,
      })
