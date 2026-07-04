// KS-8.8 (#8319): Treasury backfill core — idempotency + money-exact verify.
//
// The load-bearing properties for the HIGHEST-STAKES domain:
//   1. Running the same backfill page twice yields an IDENTICAL Postgres
//      state (second run inserts zero rows; tallies, money SUMs, and
//      newest-N hashes unchanged).
//   2. `ON CONFLICT DO NOTHING` never regresses a settlement state or an
//      amount the dual-write mirror already advanced (a stale snapshot
//      page can never un-settle a payment).
//   3. Verify compares exact counts AND exact money-column SUMs per
//      (state, rail) group — a single millisat of drift trips the report.
//   4. Replay-guard tables (mpp_*_replay) port key-exactly.
// Every one of the 27 domain tables takes at least one row here, so the
// registry's column lists are proven against the 0012 DDL — not just
// eyeballed.

import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import {
  TREASURY_BACKFILL_TABLES,
  compareTreasuryTallies,
  d1TreasuryNewestRowHashes,
  d1TreasuryTallyFromGroups,
  d1TreasuryTallySql,
  postgresTreasuryNewestRowHashes,
  postgresTreasuryTally,
  treasuryRowHash,
  upsertTreasuryRows,
  type D1SourceRow,
  type TreasuryBackfillTable,
} from "./treasury-backfill.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const ISO = "2026-07-04T00:00:00.000Z"

/** One representative D1 row per table (registry-order columns must all
 * exist in the 0012 DDL or the INSERT fails loudly; CHECK constraints must
 * all pass or the INSERT fails loudly). */
const sampleRow = (table: TreasuryBackfillTable, n: number): D1SourceRow => {
  switch (table) {
    case "treasury_transactions":
      return {
        amount_sat: 1000 * n,
        bolt11: null,
        created_at: ISO,
        direction: n === 1 ? "out" : "in",
        expires_at: null,
        failure_reason_ref: null,
        id: `treasury-tx-${n}`,
        owed_ref: null,
        owed_sat: null,
        payment_ref: `payment-${n}`,
        recipient_confirmation_ref: null,
        recipient_confirmation_state: "unconfirmed",
        recipient_confirmed_at: null,
        recipient_ref: n === 1 ? "actor.forum.recipient" : null,
        redacted_destination_ref: null,
        settled_at: n === 1 ? ISO : null,
        state: n === 1 ? "settled" : "pending",
      }
    case "nexus_payout_target_approvals":
      return {
        agent_ref: null,
        approval_policy_ref: "policy.payout_target.v1",
        approval_ref: `approval-${n}`,
        approved_by_ref: "operator.owner",
        archived_at: null,
        created_at: ISO,
        expires_at: null,
        id: `approval-id-${n}`,
        idempotency_key_hash: `approval-idem-${n}`,
        owner_user_id: null,
        payout_target_ref: `target-${n}`,
        public_projection_json: "{}",
        pylon_ref: null,
        redacted_destination_ref: `redacted:target-${n}`,
        scope_refs_json: "[]",
        status: "active",
        updated_at: ISO,
      }
    case "nexus_treasury_payout_intents":
      return {
        accepted_work_refs_json: "[]",
        actor_ref: "actor.pylon.worker",
        adapter_kind: "spark_treasury",
        amount_asset: "bitcoin",
        amount_denomination: "bitcoin_millisatoshi",
        amount_minor_units: 21_000 * n,
        archived_at: null,
        artanis_dispatch_ref: null,
        assignment_ref: `assignment-${n}`,
        buyer_payment_ref: null,
        created_at: ISO,
        id: `intent-id-${n}`,
        idempotency_key_hash: `intent-idem-${n}`,
        metadata_refs_json: "[]",
        owner_user_id: null,
        payout_intent_ref: `intent-${n}`,
        payout_target_approval_ref: "approval-1",
        payout_target_ref: "target-1",
        policy_snapshot_ref: "policy.snapshot.v1",
        public_projection_json: "{}",
        pylon_job_ref: null,
        source_kind: "accepted_work",
        spend_cap_amount_minor_units: 42_000,
        spend_cap_asset: "bitcoin",
        spend_cap_denomination: "bitcoin_millisatoshi",
        status: n === 1 ? "settled" : "approved",
        updated_at: ISO,
      }
    case "nexus_treasury_payout_attempts":
      return {
        adapter_attempt_ref: `adapter-attempt-${n}`,
        adapter_kind: "spark_treasury",
        amount_asset: "bitcoin",
        amount_denomination: "bitcoin_millisatoshi",
        amount_minor_units: 21_000 * n,
        archived_at: null,
        created_at: ISO,
        id: `attempt-id-${n}`,
        idempotency_key_hash: `attempt-idem-${n}`,
        metadata_refs_json: "[]",
        payout_attempt_ref: `attempt-${n}`,
        payout_intent_ref: "intent-1",
        public_projection_json: "{}",
        redacted_destination_ref: "redacted:target-1",
        redacted_payment_ref: n === 1 ? "redacted:payment-1" : null,
        status: n === 1 ? "confirmed" : "pending",
        updated_at: ISO,
      }
    case "nexus_treasury_payout_reconciliation_events":
      return {
        adapter_kind: "spark_treasury",
        archived_at: null,
        created_at: ISO,
        event_ref: `recon-${n}`,
        external_event_ref: `external-${n}`,
        id: `recon-id-${n}`,
        idempotency_key_hash: `recon-idem-${n}`,
        metadata_refs_json: "[]",
        payout_attempt_ref: "attempt-1",
        payout_intent_ref: "intent-1",
        provider_ref: "provider.spark",
        public_projection_json: "{}",
        result_ref: `result-${n}`,
        status: n === 1 ? "matched" : "observed",
      }
    case "nexus_payment_authority_receipts":
      return {
        archived_at: null,
        audience: "public",
        created_at: ISO,
        event_ref: null,
        id: `receipt-id-${n}`,
        metadata_refs_json: "[]",
        payout_attempt_ref: null,
        payout_intent_ref: "intent-1",
        public_projection_json: "{}",
        receipt_kind: n === 1 ? "intent_created" : "settlement_recorded",
        receipt_ref: `nexus-receipt-${n}`,
      }
    case "nexus_release_gates":
      return {
        archived_at: null,
        blocker_refs_json: "[]",
        created_at: ISO,
        evidence_refs_json: "[]",
        gate_kind: "public_receipt",
        gate_ref: `gate-${n}`,
        id: `gate-id-${n}`,
        idempotency_key_hash: `gate-idem-${n}`,
        public_projection_json: "{}",
        status: n === 1 ? "passed" : "pending",
        updated_at: ISO,
      }
    case "forum_money_actions":
      return {
        action_kind: "post_reward",
        actor_ref: "actor.forum.payer",
        amount_asset: "sats",
        amount_value: 210 * n,
        archived_at: null,
        created_at: ISO,
        earning_actor_ref: "actor.forum.recipient",
        id: `money-action-${n}`,
        idempotency_key: `money-action-idem-${n}`,
        payment_event_id: `payment-event-${n}`,
        public_projection_json: "{}",
        receipt_id: `forum-receipt-${n}`,
        target_forum_id: null,
        target_post_id: `post-${n}`,
        target_topic_id: `topic-${n}`,
      }
    case "forum_payment_events":
      return {
        amount_asset: "sats",
        amount_value: 210 * n,
        archived_at: null,
        created_at: ISO,
        external_ref: `external-${n}`,
        id: `payment-event-${n}`,
        money_action_id: `money-action-${n}`,
        provider_ref: "provider.spark",
        public_projection_json: "{}",
        redacted_evidence_ref: `redacted:evidence-${n}`,
      }
    case "forum_receipts":
      return {
        action_kind: "post_reward",
        amount_asset: "sats",
        amount_value: 210 * n,
        archived_at: null,
        created_at: ISO,
        id: `forum-receipt-${n}`,
        public_projection_json: "{}",
        receipt_ref: `receipt.forum.${n}`,
        recipient_actor_ref: "actor.forum.recipient",
        redacted_payment_ref: `redacted:payment-${n}`,
        target_forum_id: null,
        target_post_id: `post-${n}`,
        target_topic_id: `topic-${n}`,
      }
    case "forum_l402_challenges":
      return {
        action_kind: "post_reward",
        actor_ref: "actor.forum.payer",
        archived_at: null,
        created_at: ISO,
        expires_at: "2026-07-04T01:00:00.000Z",
        id: `challenge-${n}`,
        idempotency_key: `challenge-idem-${n}`,
        l402_credential_ref: null,
        l402_endpoint_ref: null,
        l402_entitlement_scope_refs_json: null,
        l402_replay_nonce_ref: null,
        l402_www_authenticate: null,
        mdk_checkout_launch_path: null,
        mdk_checkout_ref: null,
        mdk_checkout_url_ref: null,
        mdk_environment: null,
        mdk_implementation_state: null,
        mdk_invoice_ref: null,
        mdk_payment_hash_ref: null,
        mdk_provider_ref: null,
        mdk_sandbox: null,
        method: "POST",
        path: `/api/forum/posts/post-${n}/reward`,
        price_asset: "sats",
        price_value: 210 * n,
        public_projection_json: "{}",
        recipient_actor_ref: null,
        recipient_readiness_ref: null,
        request_body_digest: `digest-${n}`,
        route_params_json: "{}",
        spend_cap_asset: "sats",
        spend_cap_value: 1000,
        target_forum_id: null,
        target_post_id: `post-${n}`,
        target_topic_id: `topic-${n}`,
      }
    case "forum_l402_redemptions":
      return {
        actor_ref: "actor.forum.payer",
        archived_at: null,
        challenge_id: `challenge-${n}`,
        created_at: ISO,
        entitlement_ref: `entitlement-${n}`,
        id: `redemption-${n}`,
        idempotency_key: `redemption-idem-${n}`,
        proof_ref: `proof-${n}`,
        public_projection_json: "{}",
        receipt_id: `forum-receipt-${n}`,
        replayed: 0,
      }
    case "forum_direct_tip_attempts":
      return {
        amount_sats: 21 * n,
        archived_at: null,
        created_at: ISO,
        external_ref: `tip-external-${n}`,
        id: `tip-attempt-${n}`,
        idempotency_key: `tip-attempt-idem-${n}`,
        payer_actor_ref: "actor.forum.payer",
        payment_event_id: null,
        payment_event_status: n === 1 ? "confirmed" : "observed",
        payment_mode: "live",
        provider_ref: "provider.spark",
        receipt_ref: n === 1 ? "receipt.forum.1" : null,
        recipient_actor_ref: "actor.forum.recipient",
        redacted_evidence_ref: `redacted:tip-${n}`,
        status: n === 1 ? "settled" : "recovery_pending",
        target_post_id: `post-${n}`,
        target_post_permalink: null,
        target_topic_id: `topic-${n}`,
        updated_at: ISO,
      }
    case "forum_direct_tip_webhook_events":
      return {
        amount_sats: 21 * n,
        archived_at: null,
        delivery_count: 1,
        direct_tip_attempt_id: "tip-attempt-1",
        event_body_digest_ref: `digest-${n}`,
        external_ref: `tip-external-${n}`,
        first_seen_at: ISO,
        id: `webhook-event-${n}`,
        last_seen_at: ISO,
        payment_event_status: "confirmed",
        provider_event_ref: `provider-event-${n}`,
        provider_ref: "provider.spark",
        reconciliation_result: "settled_existing_attempt",
        reconciliation_status: "settled",
        redacted_evidence_ref: `redacted:webhook-${n}`,
        signature_binding_ref: `signature-${n}`,
      }
    case "forum_tip_recipient_wallets":
      return {
        actor_ref: `actor.forum.wallet-owner-${n}`,
        archived_at: null,
        bolt12_offer: n === 1 ? "lno1backfillsample" : null,
        caveat_refs_json: "[]",
        claim_policy_refs_json: "[]",
        created_at: ISO,
        custody_policy_refs_json: "[]",
        disabled_at: null,
        id: `wallet-${n}`,
        lightning_address: null,
        payout_target_approval_ref: null,
        provider_class: "hosted_mdk",
        public_projection_json: "{}",
        readiness_refs_json: "[]",
        receive_capability_ref: `capability-${n}`,
        source_ref: "source.wallet_registration",
        spark_address: null,
        state: "ready",
        updated_at: ISO,
        wallet_ref: `wallet-ref-${n}`,
      }
    case "forum_tip_settlement_claims":
      return {
        archived_at: null,
        created_at: ISO,
        id: `claim-${n}`,
        idempotency_key: `claim-idem-${n}`,
        public_projection_json: "{}",
        receipt_id: `forum-receipt-${n}`,
        receipt_ref: `receipt.forum.${n}`,
        recipient_actor_ref: "actor.forum.recipient",
        settlement_evidence_refs_json: "[]",
        settlement_ref: `settlement-${n}`,
        source_ref: "source.tip_claim",
      }
    case "x_claim_reward_ledger":
      return {
        agent_user_id: null,
        amount_sats: 1000,
        challenge_id: `x-challenge-${n}`,
        claim_id: `x-claim-${n}`,
        created_at: ISO,
        evidence_refs_json: "[]",
        id: `x-reward-${n}`,
        owner_user_id: `owner-${n}`,
        receipt_ref: `receipt.x_claim.${n}`,
        state: n === 1 ? "settled" : "dispatch_requested",
        state_reason_ref: null,
        treasury_payment_id: n === 1 ? "treasury-tx-1" : null,
        updated_at: ISO,
        x_account_ref: `x.account.${n}`,
      }
    case "agent_claim_reward_ledger":
      return {
        agent_claim_ref: `agent-claim-${n}`,
        amount_sats: 1000,
        campaign_ref: "campaign.agent_claim.v1",
        caveat_refs_json: "[]",
        created_at: ISO,
        destination_kind: "lightning_address",
        dispatch_attempt_ref: null,
        id: `agent-reward-${n}`,
        idempotency_key: `agent-reward-idem-${n}`,
        owner_ref: `owner-${n}`,
        payout_intent_ref: null,
        policy_refs_json: "[]",
        redacted_destination_ref: null,
        rejection_reason: null,
        settlement_ref: null,
        state: n === 1 ? "settled" : "pending",
        tweet_ref: `tweet-${n}`,
        updated_at: ISO,
        x_account_ref: `x.account.${n}`,
      }
    case "agent_balances":
      return {
        actor_ref: `actor.balance-${n}`,
        balance_msat: 21_000_000 * n,
        created_at: ISO,
        held_msat: n === 1 ? 1_000_000 : 0,
        receive_credits_below_sat: 10,
        send_credits_below_sat: 10,
        sweep_enabled: 1,
        sweep_threshold_sat: 210,
        updated_at: ISO,
        usd_credit_msat: 0,
      }
    case "labor_escrows":
      return {
        acceptance_event_ref: null,
        amount_msat: 5_000_000 * n,
        archived_at: null,
        created_at: ISO,
        forfeit_condition_ref: null,
        forfeit_destination: null,
        forfeit_destination_actor_ref: null,
        forfeit_receipt_ref: null,
        forfeited_at: null,
        funding_source: "ledger_balance",
        id: `escrow-${n}`,
        idempotency_key: `escrow-idem-${n}`,
        job_event_id: `job-event-${n}`,
        provider_actor_ref: n === 1 ? "actor.forum.provider" : null,
        public_projection_json: "{}",
        refund_receipt_ref: null,
        refunded_at: null,
        release_receipt_ref: n === 1 ? "receipt.escrow.release.1" : null,
        released_at: n === 1 ? ISO : null,
        requester_actor_ref: "actor.forum.requester",
        reserve_receipt_ref: `receipt.escrow.reserve.${n}`,
        state: n === 1 ? "released_to_provider" : "reserved",
        updated_at: ISO,
        work_request_id: `work-request-${n}`,
      }
    case "labor_escrow_receipts":
      return {
        amount_msat: 5_000_000 * n,
        created_at: ISO,
        escrow_id: `escrow-${n}`,
        evidence_ref: null,
        forfeit_destination: null,
        forfeit_destination_actor_ref: null,
        id: `escrow-receipt-${n}`,
        idempotency_key: `escrow-receipt-idem-${n}`,
        provider_actor_ref: null,
        public_projection_json: "{}",
        receipt_ref: `receipt.escrow.reserve.${n}`,
        requester_actor_ref: "actor.forum.requester",
        state_after: "reserved",
        transition_kind: "reserve",
        work_request_id: `work-request-${n}`,
      }
    case "partner_payout_ledger_entries":
      return {
        amount: 2500 * n,
        archived_at: null,
        asset: "usd",
        beneficiary_user_id: null,
        caveat_refs_json: "[]",
        created_at: ISO,
        evidence_refs_json: "[]",
        id: `partner-entry-${n}`,
        idempotency_key: `partner-entry-idem-${n}`,
        partner_ref: "partner.design.1",
        partner_role: "design_partner",
        partner_user_id: "partner-user-1",
        payout_ref: `partner-payout-${n}`,
        period_key: "2026-07",
        policy_refs_json: "[]",
        previous_entry_id: null,
        qualifying_amount: 10_000 * n,
        qualifying_event_kind: "subscription_payment",
        qualifying_event_ref: `qualifying-${n}`,
        reversal_of_entry_id: null,
        state: n === 1 ? "settled" : "eligible",
        state_reason_ref: null,
      }
    case "partner_agreements":
      return {
        agreement_ref: `agreement-${n}`,
        archived_at: null,
        created_at: ISO,
        customer_user_id: `customer-${n}`,
        effective_from: ISO,
        effective_until: null,
        id: `agreement-id-${n}`,
        partner_ref: "partner.design.1",
        partner_user_id: "partner-user-1",
        policy_state: "active",
        role: "design_partner",
      }
    case "site_referral_payout_ledger_entries":
      return {
        amount_sats: 210 * n,
        archived_at: null,
        caveat_refs_json: "[]",
        created_at: ISO,
        evidence_refs_json: "[]",
        id: `referral-entry-${n}`,
        idempotency_key: `referral-entry-idem-${n}`,
        payout_ref: `referral-payout-${n}`,
        period_key: "2026-07",
        policy_refs_json: "[]",
        previous_entry_id: null,
        qualifying_amount_sats: 1000 * n,
        qualifying_event_kind: "site_payment",
        qualifying_event_ref: `qualifying-${n}`,
        referral_attribution_id: `attribution-${n}`,
        referral_invite_id: null,
        referral_source_id: `source-${n}`,
        referred_user_id: null,
        referrer_user_id: `referrer-${n}`,
        reversal_of_entry_id: null,
        state: n === 1 ? "settled" : "eligible",
        state_reason_ref: null,
      }
    case "revenue_event_provenance":
      return {
        amount_cents: 2000 * n,
        amount_sats: null,
        caveat_refs_json: "[]",
        created_at: ISO,
        demand_provenance: "external",
        event_ref: `revenue-event-${n}`,
        evidence_bundle_ref: `evidence-bundle-${n}`,
        idempotency_key: `revenue-idem-${n}`,
        ledger_row_ref: `ledger-row-${n}`,
        ledger_table: "khala_code_paid_plan_payment_intents",
        payment_state: n === 1 ? "settled" : "requires_payment",
        product_ref: "khala_code",
        public_evidence_refs_json: "[]",
        receipt_ref: `revenue-receipt-${n}`,
        recorded_at: ISO,
        revenue_surface_ref: "khala_code.paid_plan",
        source_refs_json: "[]",
        updated_at: ISO,
      }
    case "mpp_lightning_replay":
      return {
        challenge_id: `mpp-challenge-${n}`,
        consumed_at: ISO,
        payment_hash: `${"ab".repeat(31)}${n}${n}`,
      }
    case "mpp_spt_replay":
      return {
        challenge_id: `mpp-challenge-${n}`,
        consumed_at: ISO,
        payment_intent_id: n === 1 ? "pi_backfill_sample" : null,
        spt: `spt_backfill_${n}`,
      }
  }
}

describe("treasuryRowHash (pure)", () => {
  test("identical rows hash identically; a single sat of drift diverges", () => {
    const a = sampleRow("treasury_transactions", 1)
    const b = sampleRow("treasury_transactions", 1)
    expect(treasuryRowHash("treasury_transactions", a)).toBe(
      treasuryRowHash("treasury_transactions", b),
    )
    expect(
      treasuryRowHash("treasury_transactions", { ...a, amount_sat: 1001 }),
    ).not.toBe(treasuryRowHash("treasury_transactions", a))
  })

  test("extra columns in the D1 export (e.g. d1_rowid) do not affect the hash", () => {
    const row = sampleRow("mpp_spt_replay", 2)
    expect(treasuryRowHash("mpp_spt_replay", { ...row, d1_rowid: 42 })).toBe(
      treasuryRowHash("mpp_spt_replay", row),
    )
  })

  test("registry covers all 27 domain tables", () => {
    expect(TREASURY_BACKFILL_TABLES.length).toBe(27)
  })

  test("d1 tally SQL casts money SUMs to text (bigint-safe)", () => {
    const sql = d1TreasuryTallySql("nexus_treasury_payout_intents")
    expect(sql).toContain("CAST(COALESCE(SUM(amount_minor_units), 0) AS TEXT)")
    expect(sql).toContain("status AS status_value")
    expect(sql).toContain("adapter_kind AS rail_value")
  })
})

describe.skipIf(!hasLocalPostgres())(
  "treasury backfill against local Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_treasury_backfill")
      await admin.end()
      const url = pg.urlFor("khala_treasury_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0016_treasury_domain.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("all 27 tables accept registry-shaped rows; run twice → identical state", async () => {
      const pageFor = (table: TreasuryBackfillTable): Array<D1SourceRow> => [
        sampleRow(table, 1),
        sampleRow(table, 2),
      ]

      let firstInserted = 0
      for (const table of TREASURY_BACKFILL_TABLES) {
        firstInserted += await upsertTreasuryRows(sql, table, pageFor(table))
      }
      expect(firstInserted).toBe(27 * 2)

      const talliesAfterFirst = await Promise.all(
        TREASURY_BACKFILL_TABLES.map((table) =>
          postgresTreasuryTally(sql, table),
        ),
      )
      const hashesAfterFirst = await Promise.all(
        TREASURY_BACKFILL_TABLES.map((table) =>
          postgresTreasuryNewestRowHashes(sql, table, 10),
        ),
      )

      // Second sweep: same pages again — nothing inserted, nothing changed
      // (counts, money sums, hashes all identical).
      let secondInserted = 0
      for (const table of TREASURY_BACKFILL_TABLES) {
        secondInserted += await upsertTreasuryRows(sql, table, pageFor(table))
      }
      expect(secondInserted).toBe(0)

      expect(
        await Promise.all(
          TREASURY_BACKFILL_TABLES.map((table) =>
            postgresTreasuryTally(sql, table),
          ),
        ),
      ).toEqual(talliesAfterFirst)
      expect(
        await Promise.all(
          TREASURY_BACKFILL_TABLES.map((table) =>
            postgresTreasuryNewestRowHashes(sql, table, 10),
          ),
        ),
      ).toEqual(hashesAfterFirst)
    })

    test("DO NOTHING never regresses a settlement the mirror advanced", async () => {
      // The dual-write mirror advanced the transaction (pending → settled).
      await rawSql`
        UPDATE treasury_transactions
           SET state = 'settled', settled_at = '2026-07-04T01:00:00.000Z'
         WHERE id = 'treasury-tx-2'`

      // A stale backfill page (snapshot taken before settlement) re-runs.
      await upsertTreasuryRows(sql, "treasury_transactions", [
        sampleRow("treasury_transactions", 2),
      ])

      const rows = await rawSql`
        SELECT state FROM treasury_transactions WHERE id = 'treasury-tx-2'`
      expect(rows[0]?.state).toBe("settled")
    })

    test("replay-guard key set ports exactly (second consume collides)", async () => {
      const guard = sampleRow("mpp_lightning_replay", 1)
      const inserted = await upsertTreasuryRows(sql, "mpp_lightning_replay", [
        { ...guard, challenge_id: "a-DIFFERENT-challenge" },
      ])
      // Same payment_hash → DO NOTHING: the original consumption wins.
      expect(inserted).toBe(0)
      const rows = await rawSql`
        SELECT challenge_id FROM mpp_lightning_replay
         WHERE payment_hash = ${String(guard["payment_hash"])}`
      expect(rows[0]?.challenge_id).toBe("mpp-challenge-1")
    })

    test("verify catches a single-unit money drift per (state, rail) group", async () => {
      const pgTally = await postgresTreasuryTally(
        sql,
        "nexus_treasury_payout_intents",
      )
      const pgNewest = await postgresTreasuryNewestRowHashes(
        sql,
        "nexus_treasury_payout_intents",
        10,
      )

      // Matching D1 side → green report.
      const matchingGroups = [
        {
          rail_value: "spark_treasury",
          row_count: 1,
          status_value: "approved",
          sum_amount_minor_units: "42000",
          sum_spend_cap_amount_minor_units: "42000",
        },
        {
          rail_value: "spark_treasury",
          row_count: 1,
          status_value: "settled",
          sum_amount_minor_units: "21000",
          sum_spend_cap_amount_minor_units: "42000",
        },
      ]
      const d1Rows = [
        sampleRow("nexus_treasury_payout_intents", 2),
        sampleRow("nexus_treasury_payout_intents", 1),
      ]
      const green = compareTreasuryTallies(
        "nexus_treasury_payout_intents",
        d1TreasuryTallyFromGroups("nexus_treasury_payout_intents", matchingGroups),
        pgTally,
        d1TreasuryNewestRowHashes("nexus_treasury_payout_intents", d1Rows),
        pgNewest,
      )
      expect(green.countsMatch).toBe(true)
      expect(green.groupMismatches).toEqual([])
      expect(green.newestHashMismatches).toEqual([])

      // ONE millisat of drift in one group → the group trips even though
      // counts still match.
      const drifted = compareTreasuryTallies(
        "nexus_treasury_payout_intents",
        d1TreasuryTallyFromGroups("nexus_treasury_payout_intents", [
          matchingGroups[0]!,
          { ...matchingGroups[1]!, sum_amount_minor_units: "21001" },
        ]),
        pgTally,
        d1TreasuryNewestRowHashes("nexus_treasury_payout_intents", d1Rows),
        pgNewest,
      )
      expect(drifted.countsMatch).toBe(true)
      expect(drifted.groupMismatches.length).toBe(1)
      expect(drifted.groupMismatches[0]?.group).toBe("settled|spark_treasury")
    })

    test("agent_balances tally reconciles balance/held/usd msat sums exactly", async () => {
      const tally = await postgresTreasuryTally(sql, "agent_balances")
      expect(tally.total).toBe(2)
      const group = tally.byGroup["1|<all>"]
      expect(group?.count).toBe(2)
      expect(group?.sums["balance_msat"]).toBe("63000000")
      expect(group?.sums["held_msat"]).toBe("1000000")
      expect(group?.sums["usd_credit_msat"]).toBe("0")
    })
  },
)
