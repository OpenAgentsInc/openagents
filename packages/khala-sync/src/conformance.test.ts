/**
 * Golden wire-fixture conformance tests.
 *
 * Every file in `fixtures/` is a checked-in wire-format example of one
 * protocol message type. These fixtures are the CROSS-IMPLEMENTATION
 * CONFORMANCE CONTRACT for Khala Sync: any other implementation (server,
 * SQLite client, mobile) must decode each fixture and re-encode it to a
 * deeply-equal JSON value. Do not edit a fixture to make a code change pass —
 * that is a wire-protocol change and needs a protocol-version review.
 *
 * File naming: `<MessageType>[.<variant>].json`; the segment before the
 * first `.` selects the codec.
 */
import { describe, expect, test } from "bun:test"
import { readdirSync, readFileSync } from "node:fs"
import { join } from "node:path"
import { Schema as S } from "effect"
import {
  BootstrapRequest,
  BootstrapResponse,
  ChangelogEntry,
  FleetAccountEntity,
  FleetAssignmentEntity,
  FleetRunEntity,
  FleetWorkerEntity,
  LiveFrame,
  LogPage,
  PushRequest,
  PushResponse,
  SyncError,
} from "./index.js"

const fixturesDir = join(import.meta.dir, "..", "fixtures")

type AnyCodec = S.Codec<any, any, never, never>

const codecs: Record<string, AnyCodec> = {
  BootstrapRequest,
  BootstrapResponse,
  ChangelogEntry,
  FleetAccountEntity,
  FleetAssignmentEntity,
  FleetRunEntity,
  FleetWorkerEntity,
  LiveFrame,
  LogPage,
  PushRequest,
  PushResponse,
  SyncError,
}

const fixtureFiles = readdirSync(fixturesDir)
  .filter((f) => f.endsWith(".json"))
  .sort()

describe("khala-sync golden fixture conformance", () => {
  test("fixture coverage: every codec has at least one fixture", () => {
    const covered = new Set(fixtureFiles.map((f) => f.split(".")[0]))
    for (const name of Object.keys(codecs)) {
      expect(covered.has(name)).toBe(true)
    }
  })

  test("fixture coverage: every LiveFrame variant has a fixture", () => {
    for (const tag of [
      "DeltaFrame",
      "MutationAckFrame",
      "MustRefetchFrame",
      "PingFrame",
    ]) {
      expect(fixtureFiles).toContain(`LiveFrame.${tag}.json`)
    }
  })

  test("fixture coverage: a ChangelogEntry tombstone fixture exists", () => {
    expect(fixtureFiles).toContain("ChangelogEntry.tombstone.json")
  })

  test("no orphan fixtures: every fixture maps to a known codec", () => {
    for (const file of fixtureFiles) {
      const typeName = file.split(".")[0] ?? ""
      expect(Object.keys(codecs)).toContain(typeName)
    }
  })

  for (const file of fixtureFiles) {
    const typeName = file.split(".")[0] ?? ""
    const codec = codecs[typeName]
    if (codec === undefined) continue

    test(`${file}: decode → re-encode is deeply equal (round-trip stable)`, () => {
      const raw = readFileSync(join(fixturesDir, file), "utf8")
      const wire = JSON.parse(raw) as unknown
      const decoded = S.decodeUnknownSync(codec)(wire)
      const reencoded = S.encodeSync(codec)(decoded)
      expect(reencoded).toEqual(wire)
      // Second pass: encoded form must decode/encode to itself (stability).
      const again = S.encodeSync(codec)(S.decodeUnknownSync(codec)(reencoded))
      expect(again).toEqual(reencoded)
    })
  }

  test("tombstone fixture carries no post-image", () => {
    const raw = readFileSync(
      join(fixturesDir, "ChangelogEntry.tombstone.json"),
      "utf8",
    )
    const entry = S.decodeUnknownSync(ChangelogEntry)(JSON.parse(raw))
    expect(entry.op).toBe("delete")
    expect(entry.postImageJson).toBeUndefined()
  })
})
