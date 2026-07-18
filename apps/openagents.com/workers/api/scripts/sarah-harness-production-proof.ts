#!/usr/bin/env node

import { Effect } from 'effect'

import { artanisMindComplete } from '../src/artanis-mind'
import { DEFAULT_HOSTED_RUNTIME_MODEL } from '../src/khala-hosted-runtime-dispatch'
import { defaultMakeKhalaSyncSqlClient } from '../src/khala-sync-push-routes'
import {
  SarahHarnessError,
  bindSarahHarnessForTurnPromise,
  reviewSarahHarnessHistory,
} from '../src/sarah-harness-service'
import { authorizeSarahOperation } from '../src/sarah-owner-routes'

const databaseUrl = process.env['KHALA_SYNC_DATABASE_URL']
const apiKey = process.env['GEMINI_API_KEY']
const ownerEmail = process.env['OPENAGENTS_OWNER_EMAIL']
if (databaseUrl === undefined || databaseUrl === '') {
  throw new SarahHarnessError({ reason: 'database_url_missing' })
}
if (apiKey === undefined || apiKey === '') {
  throw new SarahHarnessError({ reason: 'gemini_api_key_missing' })
}
if (ownerEmail === undefined || ownerEmail === '') {
  throw new SarahHarnessError({ reason: 'owner_email_missing' })
}

const client = await defaultMakeKhalaSyncSqlClient(databaseUrl)
try {
  const owners: Array<{
    owner_user_id: string
    thread_id: string
    terminal_count: number
  }> = await client.sql`
    SELECT turn.owner_user_id, turn.thread_id,
           count(*)::integer AS terminal_count
      FROM khala_sync_runtime_turns AS turn
     WHERE turn.thread_id LIKE 'thread.sarah.%'
       AND turn.lane = 'hosted_khala'
       AND turn.status IN ('completed', 'failed', 'interrupted', 'closed')
       AND (
         EXISTS (
           SELECT 1
             FROM users
            WHERE users.id = turn.owner_user_id
              AND lower(users.primary_email) = lower(${ownerEmail})
         )
         OR EXISTS (
           SELECT 1
             FROM auth_identities
            WHERE auth_identities.user_id = turn.owner_user_id
              AND lower(auth_identities.email) = lower(${ownerEmail})
         )
       )
     GROUP BY turn.owner_user_id, turn.thread_id
     ORDER BY max(turn.updated_at) DESC
  `
  if (owners.length !== 1 || (owners[0]?.terminal_count ?? 0) < 2) {
    throw new SarahHarnessError({
      reason: 'unique_sarah_owner_history_missing',
    })
  }
  const owner = owners[0]!
  const triggerRef = 'turn.proof.sarah-harness.owner-direction-20260718'
  const authority = await Effect.runPromise(
    authorizeSarahOperation(client.sql, {
      action: 'review_own_terminal_history_and_propose_harness',
      ownerUserId: owner.owner_user_id,
      resource: 'owner_private_sarah_harness',
      targetEvidenceRefs: ['proof:production_terminal_history_review'],
      threadRef: owner.thread_id,
      triggerRef,
    }),
  )
  if (!authority.allowed) {
    throw new SarahHarnessError({ reason: 'production_authority_refused' })
  }

  const outcome = await Effect.runPromise(
    reviewSarahHarnessHistory({
      complete: async review => {
        const result = await artanisMindComplete({
          apiKey,
          maxOutputTokens: 2_048,
          model: DEFAULT_HOSTED_RUNTIME_MODEL,
          prompt: review.prompt,
          system: review.system,
        })
        if ('error' in result) {
          throw new SarahHarnessError({
            reason: `production_${review.phase}_unavailable`,
          })
        }
        return result.text
      },
      ownerUserId: owner.owner_user_id,
      onProgress: stage => console.error(`sarah-harness-proof:${stage}`),
      sql: client.sql,
      threadId: owner.thread_id,
    }),
  )

  const proofTurnId = `turn.proof.sarah-harness.${outcome.reviewRef.slice(-24)}`
  const binding = await bindSarahHarnessForTurnPromise({
    ownerUserId: owner.owner_user_id,
    sql: client.sql,
    threadId: owner.thread_id,
    turnId: proofTurnId,
  })
  const checks: Array<{
    active_matches_candidate: boolean
    held_out_disjoint: boolean
    owner_private_experiences: boolean
  }> = await client.sql`
    SELECT
      EXISTS (
        SELECT 1
          FROM sarah_harness_active_bundles
         WHERE owner_user_id = ${owner.owner_user_id}
           AND bundle_ref = ${outcome.bundleRef}
      ) AS active_matches_candidate,
      NOT EXISTS (
        SELECT 1
          FROM sarah_harness_reviews AS review,
               jsonb_array_elements_text(review.training_experience_refs_json) AS training(ref),
               jsonb_array_elements_text(review.held_out_experience_refs_json) AS held_out(ref)
         WHERE review.owner_user_id = ${owner.owner_user_id}
           AND review.review_ref = ${outcome.reviewRef}
           AND training.ref = held_out.ref
      ) AS held_out_disjoint,
      NOT EXISTS (
        SELECT 1
          FROM sarah_harness_experiences
         WHERE owner_user_id = ${owner.owner_user_id}
           AND visibility <> 'owner_private'
      ) AS owner_private_experiences
  `
  const check = checks[0]
  console.log(
    JSON.stringify({
      activeMatchesCandidate: check?.active_matches_candidate === true,
      authorityAllowed: authority.allowed,
      authorityReceiptRef: authority.receiptRef,
      bindingDigest: binding.bundleDigest,
      bindingMatchesActive:
        binding.bundleRef === outcome.bundleRef &&
        binding.bundleDigest === outcome.bundleDigest,
      bindingRef: binding.bundleRef,
      bundleDigest: outcome.bundleDigest,
      bundleRef: outcome.bundleRef,
      evaluation: {
        approved: outcome.evaluation.approved,
        privacyScore: outcome.evaluation.privacyScore,
        qualityScore: outcome.evaluation.qualityScore,
        regressionScore: outcome.evaluation.regressionScore,
        safetyScore: outcome.evaluation.safetyScore,
      },
      experienceCount: outcome.experienceCount,
      heldOutDisjoint: check?.held_out_disjoint === true,
      heldOutExperienceCount: outcome.heldOutExperienceCount,
      model: DEFAULT_HOSTED_RUNTIME_MODEL,
      ownerPrivateExperiences: check?.owner_private_experiences === true,
      reviewRef: outcome.reviewRef,
      state: outcome.state,
      trainingExperienceCount: outcome.trainingExperienceCount,
    }),
  )
} finally {
  await client.end()
}
