import { describe, expect, test } from "bun:test"
import { Schema as S } from "effect"
import {
  ChangelogEntry,
  ClientGroupId,
  ClientId,
  decodeChangelogEntry,
  decodeLiveFrame,
  DeltaFrame,
  EntityId,
  EntityType,
  encodeChangelogEntry,
  encodeLiveFrame,
  fleetRunScope,
  KHALA_SYNC_PROTOCOL_VERSION,
  MustRefetchFrame,
  MutationEnvelope,
  MutationId,
  MutatorName,
  personalScope,
  publicScope,
  PushRequest,
  SyncCursor,
  SyncSchemaVersion,
  SyncScope,
  SyncVersion,
} from "./index.js"

const decodeScope = S.decodeUnknownSync(SyncScope)
const decodeVersion = S.decodeUnknownSync(SyncVersion)
const decodeEnvelope = S.decodeUnknownSync(MutationEnvelope)
const decodePush = S.decodeUnknownSync(PushRequest)

describe("khala-sync scopes", () => {
  test("scope constructors produce valid structured scopes", () => {
    expect(String(personalScope("user_123"))).toBe("scope.user.user_123")
    expect(String(fleetRunScope("fr_9"))).toBe("scope.fleet_run.fr_9")
    expect(String(publicScope("tokens-served"))).toBe("scope.public.tokens-served")
  })

  test("rejects unstructured scope strings", () => {
    expect(() => decodeScope("not-a-scope")).toThrow()
    expect(() => decodeScope("scope..x")).toThrow()
  })
})

describe("khala-sync versions", () => {
  test("versions are positive integers", () => {
    expect(Number(decodeVersion(1))).toBe(1)
    expect(() => decodeVersion(0)).toThrow()
    expect(() => decodeVersion(1.5)).toThrow()
    expect(() => decodeVersion(-3)).toThrow()
  })
})

describe("khala-sync wire round-trips", () => {
  const scope = fleetRunScope("fr_1")

  const entry = new ChangelogEntry({
    scope,
    version: SyncVersion.make(7),
    entityType: EntityType.make("fleet_worker"),
    entityId: EntityId.make("w1"),
    op: "upsert",
    postImageJson: JSON.stringify({ id: "w1", status: "running" }),
    mutationRef: "mut.cg1.c1.4",
    committedAt: "2026-07-04T00:00:00.000Z",
  })

  test("ChangelogEntry encodes/decodes losslessly", () => {
    const decoded = decodeChangelogEntry(encodeChangelogEntry(entry))
    expect(decoded.version).toBe(SyncVersion.make(7))
    expect(decoded.op).toBe("upsert")
    expect(decoded.postImageJson).toBe(entry.postImageJson)
  })

  test("delete entries carry no post-image", () => {
    const tombstone = new ChangelogEntry({
      scope,
      version: SyncVersion.make(8),
      entityType: EntityType.make("fleet_worker"),
      entityId: EntityId.make("w1"),
      op: "delete",
      committedAt: "2026-07-04T00:00:01.000Z",
    })
    expect(tombstone.postImageJson).toBeUndefined()
  })

  test("PushRequest round-trips with mutations", () => {
    const req = new PushRequest({
      protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
      schemaVersion: SyncSchemaVersion.make(1),
      clientGroupId: ClientGroupId.make("cg1"),
      clientId: ClientId.make("c1"),
      mutations: [
        new MutationEnvelope({
          mutationId: MutationId.make(4),
          name: MutatorName.make("fleet.pauseWorker"),
          argsJson: JSON.stringify({ workerId: "w1" }),
        }),
      ],
    })
    const decoded = decodePush(S.encodeSync(PushRequest)(req))
    expect(decoded.mutations).toHaveLength(1)
    expect(String(decoded.mutations[0]?.name)).toBe("fleet.pauseWorker")
  })

  test("mutator names must be dotted lower-camel identifiers", () => {
    expect(() =>
      decodeEnvelope({
        mutationId: 1,
        name: "DROP TABLE",
        argsJson: "{}",
      }),
    ).toThrow()
  })

  test("PushRequest requires protocol version match", () => {
    expect(() =>
      decodePush({
        protocolVersion: 999,
        schemaVersion: 1,
        clientGroupId: "cg1",
        clientId: "c1",
        mutations: [],
      }),
    ).toThrow()
  })
})

describe("khala-sync live frames", () => {
  test("LiveFrame union discriminates by tag", () => {
    const delta = new DeltaFrame({
      scope: publicScope("tokens-served"),
      entries: [],
      cursor: SyncVersion.make(42),
    })
    const back = decodeLiveFrame(encodeLiveFrame(delta))
    expect(back._tag).toBe("DeltaFrame")
    if (back._tag === "DeltaFrame") {
      expect(back.cursor).toBe(SyncVersion.make(42))
    }
  })

  test("MustRefetch reasons are the closed spec set", () => {
    const frame = new MustRefetchFrame({
      scope: personalScope("u1"),
      reason: "cursor_behind_retained_window",
    })
    expect(frame.reason).toBe("cursor_behind_retained_window")
    expect(() =>
      decodeLiveFrame({
        _tag: "MustRefetchFrame",
        scope: "scope.user.u1",
        reason: "because",
      }),
    ).toThrow()
  })

  test("SyncCursor pairs scope and version", () => {
    const c = new SyncCursor({
      scope: personalScope("u1"),
      version: SyncVersion.make(3),
    })
    expect(String(c.scope)).toBe("scope.user.u1")
  })
})
