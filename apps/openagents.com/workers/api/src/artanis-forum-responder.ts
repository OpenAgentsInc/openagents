import { Effect, Schema as S } from 'effect'

import { artanisMindComplete } from './artanis-mind'
import type {
  InferenceAdapterError,
  InferenceRequest,
  InferenceResult,
} from './inference/provider-adapter'
import {
  type ArtanisAskerProvenance,
  classifyAskerProvenance,
} from './artanis-responder-provenance'
import { recordArtanisResponderScanTick } from './artanis-responder-ticks'
import { parseJsonWithSchema } from './json-boundary'
import { randomUuid } from './runtime-primitives'

// The Artanis forum responder, scan stage (issue #4714; promise
// artanis.pylon_support_responder.v1). Each cron tick: read Forum
// topics created since the scan cursor, let the MIND classify which are
// Pylon device/training questions (typed semantic selection - never
// keyword matching, per the workspace semantic-routing rule), and
// record schema-validated respond_to_post proposals. Schema-invalid
// mind output records a blocked action with the raw proposal - never an
// action. Reply delivery and tipping are the composer stage (#4715).
//
// Identity decision (the two-Artanis question from the 2026-06-10
// full-status audit): conversational replies use the REGISTERED Artanis
// agent identity (the one with the post history, tips, and wallet);
// the seeded publication-queue identity remains for canonical status
// topics only. Recorded in docs/artanis/treasury-runbook.md's sibling
// docs and on issue #4714.

export const ARTANIS_RESPONDER_MAX_PER_TICK = 3
export const ARTANIS_RESPONDER_MAX_PER_DAY = 20
export const ARTANIS_RESPONDER_KHALA_MODEL = 'openagents/khala'
export const ARTANIS_RESPONDER_DEMAND_SOURCE = 'artanis'
export const ARTANIS_RESPONDER_DEMAND_CLIENT = 'artanis_forum_responder'

export type ArtanisResponderKhalaClient = (
  request: InferenceRequest,
) => Effect.Effect<InferenceResult, InferenceAdapterError>

const MindClassification = S.Struct({
  candidates: S.Array(
    S.Struct({
      questionClass: S.Literals([
        'device_capability',
        'training_run',
        'pylon_troubleshooting',
        'not_a_pylon_question',
      ]),
      respond: S.Boolean,
      topicId: S.String,
    }),
  ),
})

export type ResponderScanOutcome = Readonly<{
  scanned: number
  proposed: number
  blocked: number
  skipped: number
  dailyBudgetLeft: number
  skippedReason: string | null
}>

type CandidateTopic = Readonly<{
  topicId: string
  firstPostId: string
  title: string
  excerpt: string
  actorRef: string
  createdAt: string
}>

const buildClassificationPrompt = (
  candidates: ReadonlyArray<CandidateTopic>,
): string =>
  [
    'Classify each Forum topic below. respond=true ONLY for genuine questions from Pylon contributors about device capability, training-run participation, or Pylon troubleshooting that deserve an administrator reply. Output STRICT JSON only, shaped exactly as {"candidates":[{"topicId":"...","questionClass":"device_capability|training_run|pylon_troubleshooting|not_a_pylon_question","respond":true|false}]} with one entry per topic.',
    ...candidates.map(
      candidate =>
        `topicId ${candidate.topicId}\ntitle: ${candidate.title}\nexcerpt: ${candidate.excerpt.replace(/\n/g, ' ').slice(0, 400)}`,
    ),
  ].join('\n\n')

const ARTANIS_RESPONDER_CLASSIFICATION_SYSTEM =
  'You are Artanis, the Nexus administrator. You classify forum topics for response. You output strict JSON only - no prose, no markdown fences.'

export const buildArtanisResponderKhalaRequest = (
  input: Readonly<{ prompt: string; system: string }>,
): InferenceRequest => ({
  messages: [
    { content: input.system, role: 'system' },
    { content: input.prompt, role: 'user' },
  ],
  model: ARTANIS_RESPONDER_KHALA_MODEL,
  passthroughParams: {
    max_tokens: 4096,
    temperature: 0.2,
  },
  stream: false,
})

const completeResponderMind = async (
  deps: Readonly<{
    geminiApiKey: string | null
    gatewayToken?: string | undefined
    khalaClient?: ArtanisResponderKhalaClient | undefined
  }>,
  input: Readonly<{ prompt: string; system: string }>,
) => {
  if (deps.khalaClient !== undefined) {
    const khalaResult = await Effect.runPromiseExit(
      deps.khalaClient(buildArtanisResponderKhalaRequest(input)),
    )
    if (khalaResult._tag === 'Success') {
      const result = khalaResult.value
      return {
        gatewayId: null,
        model: result.servedModel,
        promptChars: input.prompt.length,
        responseChars: result.content.length,
        servedVia: 'openagents_khala' as const,
        text: result.content,
      }
    }
  }

  if (deps.geminiApiKey === null || deps.geminiApiKey === '') {
    return {
      attempts: [],
      error: 'artanis_mind_unavailable' as const,
    }
  }

  return artanisMindComplete({
    apiKey: deps.geminiApiKey,
    ...(deps.gatewayToken === undefined || deps.gatewayToken === ''
      ? {}
      : { gatewayToken: deps.gatewayToken }),
    prompt: input.prompt,
    system: input.system,
  })
}

const readScanCursor = async (
  db: D1Database,
  nowIso: string,
): Promise<{ cursorIso: string; responsesToday: number }> => {
  const row = (await db
    .prepare(
      'SELECT scan_cursor_iso, responses_today, responses_day FROM artanis_responder_state WHERE id = 1',
    )
    .first()) as
    | { scan_cursor_iso: string; responses_today: number; responses_day: string }
    | null

  if (row === null) {
    await db
      .prepare(
        `INSERT INTO artanis_responder_state (id, scan_cursor_iso, responses_today, responses_day, updated_at)
         VALUES (1, ?, 0, ?, ?)`,
      )
      .bind(nowIso, nowIso.slice(0, 10), nowIso)
      .run()
    return { cursorIso: nowIso, responsesToday: 0 }
  }

  const today = nowIso.slice(0, 10)
  const responsesToday = row.responses_day === today ? row.responses_today : 0
  return { cursorIso: row.scan_cursor_iso, responsesToday }
}

const readCandidates = async (
  db: D1Database,
  sinceIso: string,
  artanisActorRefs: ReadonlyArray<string>,
): Promise<ReadonlyArray<CandidateTopic>> => {
  const result = await db
    .prepare(
      `SELECT t.id AS topic_id, t.first_post_id, t.title, t.actor_ref,
              t.created_at, COALESCE(b.body_text, '') AS body_text
         FROM forum_topics t
         JOIN forum_forums f ON f.id = t.forum_id
    LEFT JOIN forum_post_bodies b ON b.post_id = t.first_post_id
        WHERE t.created_at > ?
          AND t.state = 'open'
          AND f.slug IN ('artanis', 'tassadar', 'agents', 'void')
        ORDER BY t.created_at ASC
        LIMIT 25`,
    )
    .bind(sinceIso)
    .all()

  return ((result.results ?? []) as Array<Record<string, unknown>>)
    .map(row => ({
      actorRef: String(row.actor_ref),
      createdAt: String(row.created_at),
      excerpt: String(row.body_text).slice(0, 600),
      firstPostId: String(row.first_post_id),
      title: String(row.title),
      topicId: String(row.topic_id),
    }))
    // Never propose responding to Artanis itself, including the legacy seeded
    // Forum actor that T12.4 keeps only for historical/idempotency reads.
    .filter(
      candidate =>
        !artanisActorRefs.includes(candidate.actorRef) &&
        classifyAskerProvenance(candidate.actorRef) !== 'artanis_self',
    )
}

export const runArtanisResponderScan = async (
  db: D1Database,
  deps: Readonly<{
    geminiApiKey: string | null
    gatewayToken?: string | undefined
    khalaClient?: ArtanisResponderKhalaClient | undefined
    artanisActorRefs: ReadonlyArray<string>
    // Owner/operator actor refs (e.g. the admin user posting test articles)
    // so they are classified owner_operator, never external_contributor.
    adminActorRefs?: ReadonlyArray<string>
    nowIso: string
  }>,
): Promise<ResponderScanOutcome> => {
  if (
    deps.khalaClient === undefined &&
    (deps.geminiApiKey === null || deps.geminiApiKey === '')
  ) {
    return {
      blocked: 0,
      dailyBudgetLeft: 0,
      proposed: 0,
      scanned: 0,
      skipped: 0,
      skippedReason: 'mind_unconfigured',
    }
  }

  const { cursorIso, responsesToday } = await readScanCursor(db, deps.nowIso)
  const dailyBudgetLeft = Math.max(
    0,
    ARTANIS_RESPONDER_MAX_PER_DAY - responsesToday,
  )

  const candidates = await readCandidates(db, cursorIso, deps.artanisActorRefs)

  // Bounded asker-provenance classification (blocker
  // external_contributor_flow_unproven). The mind still owns the
  // question-class decision; this only labels WHO asked so the public
  // external-contributor projection can be honest.
  const provenanceFor = (actorRef: string): ArtanisAskerProvenance =>
    classifyAskerProvenance(actorRef, {
      adminActorRefs: deps.adminActorRefs ?? [],
    })

  if (candidates.length === 0) {
    return {
      blocked: 0,
      dailyBudgetLeft,
      proposed: 0,
      scanned: 0,
      skipped: 0,
      skippedReason: null,
    }
  }

  // The mind classifies. Strict JSON contract; anything undecodable is a
  // blocked tick action, never a guess.
  const mindResult = await completeResponderMind(deps, {
    prompt: buildClassificationPrompt(candidates),
    system: ARTANIS_RESPONDER_CLASSIFICATION_SYSTEM,
  })

  let proposed = 0
  let blocked = 0
  let skipped = 0
  const maxNew = Math.min(ARTANIS_RESPONDER_MAX_PER_TICK, dailyBudgetLeft)

  if ('error' in mindResult) {
    blocked = candidates.length
    for (const candidate of candidates) {
      await db
        .prepare(
          `INSERT INTO artanis_responder_actions
           (id, topic_id, first_post_id, question_class, state, proposal_json, asker_actor_ref, asker_provenance, asked_at, created_at, updated_at)
           VALUES (?, ?, ?, NULL, 'blocked', ?, ?, ?, ?, ?, ?)
           ON CONFLICT (topic_id) DO NOTHING`,
        )
        .bind(
          randomUuid(),
          candidate.topicId,
          candidate.firstPostId,
          JSON.stringify({ reason: 'mind_unavailable' }),
          candidate.actorRef,
          provenanceFor(candidate.actorRef),
          candidate.createdAt,
          deps.nowIso,
          deps.nowIso,
        )
        .run()
    }
  } else {
    let classification: typeof MindClassification.Type | null = null
    try {
      const cleaned = mindResult.text
        .replace(/^```(?:json)?\s*/m, '')
        .replace(/```\s*$/m, '')
        .trim()
      classification = parseJsonWithSchema(MindClassification, cleaned)
    } catch {
      classification = null
    }

    if (classification === null) {
      // Schema-invalid mind output: record blocked with the raw text.
      blocked = candidates.length
      for (const candidate of candidates) {
        await db
          .prepare(
            `INSERT INTO artanis_responder_actions
             (id, topic_id, first_post_id, question_class, state, proposal_json, asker_actor_ref, asker_provenance, asked_at, created_at, updated_at)
             VALUES (?, ?, ?, NULL, 'blocked', ?, ?, ?, ?, ?, ?)
             ON CONFLICT (topic_id) DO NOTHING`,
          )
          .bind(
            randomUuid(),
            candidate.topicId,
            candidate.firstPostId,
            JSON.stringify({
              rawProposal: mindResult.text.slice(0, 1000),
              reason: 'schema_invalid_mind_output',
            }),
            candidate.actorRef,
            provenanceFor(candidate.actorRef),
            candidate.createdAt,
            deps.nowIso,
            deps.nowIso,
          )
          .run()
      }
    } else {
      const byTopic = new Map(
        classification.candidates.map(entry => [entry.topicId, entry]),
      )
      for (const candidate of candidates) {
        const verdict = byTopic.get(candidate.topicId)
        const shouldRespond =
          verdict !== undefined &&
          verdict.respond &&
          verdict.questionClass !== 'not_a_pylon_question' &&
          proposed < maxNew

        await db
          .prepare(
            `INSERT INTO artanis_responder_actions
             (id, topic_id, first_post_id, question_class, state, proposal_json, asker_actor_ref, asker_provenance, asked_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (topic_id) DO NOTHING`,
          )
          .bind(
            randomUuid(),
            candidate.topicId,
            candidate.firstPostId,
            verdict?.questionClass ?? null,
            shouldRespond ? 'proposed' : 'skipped',
            JSON.stringify({
              servedVia: mindResult.servedVia,
              title: candidate.title.slice(0, 120),
              verdict: verdict ?? null,
            }),
            candidate.actorRef,
            provenanceFor(candidate.actorRef),
            candidate.createdAt,
            deps.nowIso,
            deps.nowIso,
          )
          .run()

        if (shouldRespond) {
          proposed += 1
        } else {
          skipped += 1
        }
      }
    }
  }

  const latestCreatedAt = candidates[candidates.length - 1]!.createdAt
  await db
    .prepare(
      `UPDATE artanis_responder_state
       SET scan_cursor_iso = ?, updated_at = ?
       WHERE id = 1`,
    )
    .bind(latestCreatedAt, deps.nowIso)
    .run()

  return {
    blocked,
    dailyBudgetLeft: dailyBudgetLeft - proposed,
    proposed,
    scanned: candidates.length,
    skipped,
    skippedReason: null,
  }
}

export const runArtanisResponderScanScheduled = (
  db: D1Database,
  deps: Readonly<{
    enabled: boolean
    geminiApiKey: string | null
    gatewayToken?: string | undefined
    khalaClient?: ArtanisResponderKhalaClient | undefined
    artanisActorRefs: ReadonlyArray<string>
    adminActorRefs?: ReadonlyArray<string>
    nowIso: string
  }>,
): Effect.Effect<ResponderScanOutcome, never> =>
  deps.enabled
    ? Effect.tryPromise({
        catch: () => 'responder_scan_error' as const,
        try: () => runArtanisResponderScan(db, deps),
      }).pipe(
        Effect.catch(reason =>
          Effect.succeed({
            blocked: 0,
            dailyBudgetLeft: 0,
            proposed: 0,
            scanned: 0,
            skipped: 0,
            skippedReason: reason,
          } satisfies ResponderScanOutcome),
        ),
        Effect.flatMap(outcome =>
          Effect.promise(() =>
            recordArtanisResponderScanTick(db, {
              nowIso: deps.nowIso,
              outcome,
            }),
          ).pipe(Effect.as(outcome)),
        ),
      )
    : Effect.succeed({
        blocked: 0,
        dailyBudgetLeft: 0,
        proposed: 0,
        scanned: 0,
        skipped: 0,
        skippedReason: 'responder_disabled',
      })

export const ARTANIS_REGISTERED_ACTOR_REF =
  'agent:user_ed6d486e-612a-4fac-a9a9-44f7e5709505'
