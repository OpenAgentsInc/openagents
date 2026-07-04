import { describe, expect, test } from "bun:test"

import {
  compareMessageChainFingerprints,
  compareNewestHashes,
  khalaCodeProductStateNewestHashesFromRows,
  membershipSetFingerprintFromRows,
  messageChainFingerprintsFromRows,
} from "./khala-code-product-state-backfill.js"

describe("Khala Code product-state backfill verifier", () => {
  test("newest row hashes compare by product-state natural keys", () => {
    const newest = khalaCodeProductStateNewestHashesFromRows("team_memberships", [
      {
        id: "membership_1",
        team_id: "team_1",
        user_id: "user_1",
        role: "member",
        status: "active",
        created_at: "2026-07-04T00:00:00.000Z",
        updated_at: "2026-07-04T00:00:00.000Z",
      },
    ])

    expect(newest[0]?.key).toBe("team_1:user_1")
    expect(compareNewestHashes(newest, newest)).toEqual([])
    expect(
      compareNewestHashes(newest, [{ key: "team_1:user_1", hash: "bad" }]),
    ).toEqual([
      {
        d1Hash: newest[0]?.hash,
        key: "team_1:user_1",
        postgresHash: "bad",
      },
    ])
  })

  test("membership set fingerprint is order-insensitive", () => {
    const a = membershipSetFingerprintFromRows([
      {
        team_id: "team_1",
        user_id: "user_1",
        role: "owner",
        status: "active",
        removed_at: null,
      },
      {
        team_id: "team_1",
        user_id: "user_2",
        role: "member",
        status: "active",
        removed_at: null,
      },
    ])
    const b = membershipSetFingerprintFromRows([
      {
        team_id: "team_1",
        user_id: "user_2",
        role: "member",
        status: "active",
        removed_at: null,
      },
      {
        team_id: "team_1",
        user_id: "user_1",
        role: "owner",
        status: "active",
        removed_at: null,
      },
    ])

    expect(a).toEqual(b)
  })

  test("message-chain fingerprints catch missing or reordered messages", () => {
    const d1 = messageChainFingerprintsFromRows("team_chat_messages", [
      {
        id: "msg_1",
        team_id: "team_1",
        project_id: null,
        autopilot_thread_id: "thread_1",
        created_at: "2026-07-04T00:00:00.000Z",
      },
      {
        id: "msg_2",
        team_id: "team_1",
        project_id: null,
        autopilot_thread_id: "thread_1",
        created_at: "2026-07-04T00:00:01.000Z",
      },
    ])
    const postgres = messageChainFingerprintsFromRows("team_chat_messages", [
      {
        id: "msg_1",
        team_id: "team_1",
        project_id: null,
        autopilot_thread_id: "thread_1",
        created_at: "2026-07-04T00:00:00.000Z",
      },
    ])

    expect(compareMessageChainFingerprints(d1, d1)).toEqual([])
    expect(compareMessageChainFingerprints(d1, postgres)).toEqual([
      {
        d1: d1[0],
        groupKey: "team_1\x1f\x1fthread_1",
        postgres: postgres[0],
      },
    ])
  })
})
