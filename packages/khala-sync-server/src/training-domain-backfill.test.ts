import { describe, expect, test } from "bun:test"

import {
  TRAINING_DOMAIN_TABLES,
  TRAINING_DOMAIN_TABLE_SPECS,
  compareTrainingChainFingerprints,
  compareTrainingNewestHashes,
  compareTrainingStateTallies,
  trainingChainFingerprintsFromRows,
  trainingDomainNewestHashesFromRows,
  trainingDomainRowHash,
  trainingLeaseSetFingerprintFromRows,
  trainingStateTallyFromRows,
} from "./training-domain-backfill.js"

const windowEvent = (
  id: string,
  windowRef: string,
  overrides: Partial<Record<string, unknown>> = {},
) => ({
  actor_ref: "operator.owner",
  archived_at: null,
  created_at: "2026-07-04T00:00:00.000Z",
  id,
  receipt_ref: `receipt.${id}`,
  state_from: "planned",
  state_to: "active",
  transition_kind: "activate",
  window_ref: windowRef,
  ...overrides,
})

describe("training domain backfill verifier", () => {
  test("registry covers the seven core tables with ref arbiters", () => {
    expect(TRAINING_DOMAIN_TABLES).toHaveLength(7)
    expect(TRAINING_DOMAIN_TABLE_SPECS.training_runs.keyColumns).toEqual([
      "training_run_ref",
    ])
    expect(TRAINING_DOMAIN_TABLE_SPECS.training_windows.keyColumns).toEqual([
      "window_ref",
    ])
    expect(
      TRAINING_DOMAIN_TABLE_SPECS.training_window_leases.keyColumns,
    ).toEqual(["lease_ref"])
    expect(
      TRAINING_DOMAIN_TABLE_SPECS.training_window_events.writeMode,
    ).toBe("insertIfAbsent")
    expect(
      TRAINING_DOMAIN_TABLE_SPECS.training_verification_events.writeMode,
    ).toBe("insertIfAbsent")
  })

  test("newest row hashes key by the live ref arbiter and detect drift", () => {
    const run = {
      archived_at: null,
      created_at: "2026-07-04T00:00:00.000Z",
      id: "run-id-1",
      manifest_json: null,
      max_allowed_stale: 5,
      promise_ref: "promise.decentralized-training",
      public_projection_json: '{"state":"active"}',
      receipt_refs_json: '["receipt.plan.1"]',
      seal_in_flight_at: null,
      seal_publication_cadence_windows: 1,
      source_refs_json: '["issue.4851"]',
      state: "active",
      training_run_ref: "run.tassadar.executor.20260615",
      updated_at: "2026-07-04T00:00:00.000Z",
    }
    const newest = trainingDomainNewestHashesFromRows("training_runs", [run])
    expect(newest[0]?.key).toBe("run.tassadar.executor.20260615")
    expect(compareTrainingNewestHashes(newest, newest)).toEqual([])

    // A single-byte receipt change (public-claim payload) flips the hash.
    const drifted = trainingDomainNewestHashesFromRows("training_runs", [
      { ...run, receipt_refs_json: '["receipt.plan.2"]' },
    ])
    expect(compareTrainingNewestHashes(newest, drifted)).toHaveLength(1)
  })

  test("row hash treats missing column and NULL identically (D1 SELECT * parity)", () => {
    const base = windowEvent("evt-1", "window.w1")
    const { state_from: _omitted, ...withoutStateFrom } = base
    expect(
      trainingDomainRowHash("training_window_events", {
        ...base,
        state_from: null,
      }),
    ).toBe(trainingDomainRowHash("training_window_events", withoutStateFrom))
  })

  test("window-event chain fingerprints catch missing and reordered transitions", () => {
    const chain = [
      windowEvent("evt-1", "window.w1", {
        state_from: null,
        state_to: "planned",
        created_at: "2026-07-04T00:00:00.000Z",
      }),
      windowEvent("evt-2", "window.w1", {
        state_from: "planned",
        state_to: "active",
        created_at: "2026-07-04T00:01:00.000Z",
      }),
      windowEvent("evt-3", "window.w1", {
        state_from: "active",
        state_to: "sealed",
        created_at: "2026-07-04T00:02:00.000Z",
      }),
    ]
    const d1 = trainingChainFingerprintsFromRows(
      "training_window_events",
      chain,
    )
    // Input order does not matter — the fingerprint orders (created_at, id).
    const shuffled = trainingChainFingerprintsFromRows(
      "training_window_events",
      [chain[2]!, chain[0]!, chain[1]!],
    )
    expect(compareTrainingChainFingerprints(d1, shuffled)).toEqual([])

    // A dropped middle link is a mismatch (contiguity acceptance).
    const missing = trainingChainFingerprintsFromRows(
      "training_window_events",
      [chain[0]!, chain[2]!],
    )
    const mismatches = compareTrainingChainFingerprints(d1, missing)
    expect(mismatches).toHaveLength(1)
    expect(mismatches[0]?.groupKey).toBe("window.w1")
  })

  test("verification-event chains group by challenge_ref", () => {
    const events = [
      {
        archived_at: null,
        challenge_ref: "challenge.c1",
        created_at: "2026-07-04T00:00:00.000Z",
        failure_codes_json: "[]",
        id: "vevt-1",
        receipt_refs_json: "[]",
        state_from: null,
        state_to: "Queued",
        transition_kind: "create",
        validator_ref: null,
      },
      {
        archived_at: null,
        challenge_ref: "challenge.c1",
        created_at: "2026-07-04T00:01:00.000Z",
        failure_codes_json: "[]",
        id: "vevt-2",
        receipt_refs_json: "[]",
        state_from: "Queued",
        state_to: "Verified",
        transition_kind: "finalize",
        validator_ref: "validator.device.1",
      },
    ]
    const chains = trainingChainFingerprintsFromRows(
      "training_verification_events",
      events,
    )
    expect(chains).toHaveLength(1)
    expect(chains[0]?.groupKey).toBe("challenge.c1")
    expect(chains[0]?.count).toBe(2)
  })

  test("lease-set fingerprint is order-insensitive and catches a double lease", () => {
    const lease = (leaseRef: string, windowRef: string) => ({
      archived_at: null,
      claimed_at: "2026-07-04T00:00:00.000Z",
      lease_expires_at: "2026-07-04T01:00:00.000Z",
      lease_ref: leaseRef,
      pylon_ref: "pylon.orrery",
      state: "active",
      window_ref: windowRef,
    })
    const a = trainingLeaseSetFingerprintFromRows([
      lease("lease.1", "window.w1"),
      lease("lease.2", "window.w2"),
    ])
    const b = trainingLeaseSetFingerprintFromRows([
      lease("lease.2", "window.w2"),
      lease("lease.1", "window.w1"),
    ])
    expect(a).toEqual(b)

    // An extra active lease on the same window (double-lease risk) drifts.
    const doubled = trainingLeaseSetFingerprintFromRows([
      lease("lease.1", "window.w1"),
      lease("lease.2", "window.w2"),
      lease("lease.3", "window.w1"),
    ])
    expect(doubled.count).toBe(3)
    expect(doubled.digest).not.toBe(a.digest)
  })

  test("state tallies compare exactly", () => {
    const d1 = trainingStateTallyFromRows([
      { state: "pending" },
      { state: "paired" },
      { state: "pending" },
    ])
    expect(d1).toEqual([
      { state: "paired", total: 1 },
      { state: "pending", total: 2 },
    ])
    expect(compareTrainingStateTallies(d1, d1)).toBe(true)
    expect(
      compareTrainingStateTallies(d1, [
        { state: "paired", total: 1 },
        { state: "pending", total: 1 },
      ]),
    ).toBe(false)
  })
})
