import { Effect, Stream } from "@effect-native/core/effect";
import { readFileSync } from "node:fs";
import { decodeIssue31HostAdjunct } from "@openagentsinc/sarah/issue31-workroom";
import {
  ISSUE31_COMMAND_SCHEMA,
  ISSUE31_HOST_ANNOUNCEMENT_SCHEMA,
  ISSUE31_PAIRING_SCHEMA,
  decodeIssue31HostAnnouncement,
  decodeIssue31PairingRecord,
  type Issue31CommandArguments,
} from "@openagentsinc/sarah/issue31-nostr";
import { describe, expect, test } from "vite-plus/test";

import {
  ISSUE31_CAPABILITY_DESCRIPTORS,
  decodeIssue31SourceSnapshot,
  issue31RowsForRoom,
  projectIssue31WorkroomReadModel,
  type Issue31SourceSnapshot,
} from "../src/workroom/issue31-workroom-read-model";
import { issue31SourceSnapshotsFromNostr } from "../src/workroom/issue31-nostr-read-model";
import { emptyIssue31OwnerPrivateReadModel } from "../src/workroom/issue31-owner-private-read-model";
import {
  buildHomeProgram,
  chromeProps,
  renderContentView,
  renderDrawerView,
} from "../src/screens/home-core";

const observedAt = "2026-07-24T16:00:00.000Z";
const eventRef = "a".repeat(64);

const readySources = (): ReadonlyArray<Issue31SourceSnapshot> =>
  ISSUE31_CAPABILITY_DESCRIPTORS.map((descriptor) =>
    decodeIssue31SourceSnapshot({
      capabilityId: descriptor.id,
      authority: descriptor.expectedAuthority,
      sourceRef: descriptor.sourceRef,
      status: "ready",
      freshness: "live",
      observedAt,
      recordRefs:
        descriptor.expectedAuthority === "signed_nostr_record"
          ? [eventRef]
          : [`projection.issue31.${descriptor.id}`],
      reasonRef: null,
      role: descriptor.room === "community" ? "member" : "owner",
      roleStatus: "active",
      actionState:
        descriptor.id === "full_auto"
          ? {
              kind: "terminal",
              intentRef: "intent.full-auto.stop.test",
              actionRef: "action.full-auto.stop",
              state: "stopped",
              outcomeRef: "outcome.full-auto.stop.test",
            }
          : { kind: "idle" },
    }),
  );

const settle = Effect.gen(function* () {
  yield* Effect.yieldNow;
  yield* Effect.yieldNow;
});

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), (option) => {
    if (option._tag !== "Some") throw new Error("expected state");
    return option.value;
  });

describe("Issue31WorkroomReadModel", () => {
  test("freezes one honest row for every issue 31 capability", () => {
    const model = projectIssue31WorkroomReadModel({ projectedAt: observedAt });
    expect(model.rows.map((row) => row.id)).toEqual(
      ISSUE31_CAPABILITY_DESCRIPTORS.map((row) => row.id),
    );
    expect(model.coverage).toEqual({
      total: 11,
      ready: 0,
      unavailable: 11,
      gaps: 0,
      pending: 0,
      refused: 0,
      terminal: 0,
    });
    expect(
      model.rows.every(
        (row) =>
          row.source.status === "unavailable" &&
          row.source.reasonRef === `reason.issue31.source_not_connected:${row.id}`,
      ),
    ).toBe(true);
  });

  test("accepts only the frozen authority and source identity for each row", () => {
    const model = projectIssue31WorkroomReadModel({
      projectedAt: observedAt,
      sources: readySources(),
    });
    expect(model.coverage.ready).toBe(11);
    expect(model.coverage.terminal).toBe(1);
    expect(model.rows.filter((row) => row.source.authority === "signed_nostr_record")).toHaveLength(
      8,
    );
    expect(model.rows.filter((row) => row.source.authority === "omega_host_adjunct")).toHaveLength(
      3,
    );

    expect(() =>
      decodeIssue31SourceSnapshot({
        ...readySources()[0],
        authority: "omega_host_adjunct",
      }),
    ).toThrow(/source identity/);
    expect(() =>
      decodeIssue31SourceSnapshot({
        ...readySources()[0],
        recordRefs: [],
      }),
    ).toThrow(/authority record/);
    expect(() =>
      decodeIssue31SourceSnapshot({
        ...readySources()[0],
        recordRefs: ["file:///Users/owner/private"],
      }),
    ).toThrow();
    expect(() =>
      projectIssue31WorkroomReadModel({
        projectedAt: observedAt,
        sources: [readySources()[0], readySources()[0]],
      }),
    ).toThrow(/more than once/);
  });

  test("keeps owner-private and community records in distinct room projections", () => {
    const model = projectIssue31WorkroomReadModel({
      projectedAt: observedAt,
      sources: readySources(),
    });
    const owner = issue31RowsForRoom(model, "owner_private");
    const community = issue31RowsForRoom(model, "community");
    expect(owner.map((row) => row.id)).not.toContain("community_work");
    expect(community.map((row) => row.id)).not.toContain("owner_private_sarah");
    expect(owner.filter((row) => row.room === "shared").map((row) => row.id)).toEqual(
      community.filter((row) => row.room === "shared").map((row) => row.id),
    );
  });

  test("keeps owner-key ciphertext as an exact device projection gap", () => {
    const sourceEventId = "c".repeat(64);
    const sources = issue31SourceSnapshotsFromNostr(
      {
        devicePublicKeyHex: "b".repeat(64),
        admittedHostPublicKeys: ["d".repeat(64)],
        selectedHostPublicKeys: ["d".repeat(64)],
        ownerPrivateAuthors: ["3".repeat(64)],
        ownerRecipientPublicKeys: ["d".repeat(64)],
        relays: [],
        storedEventIds: {},
        publishRefusals: {},
        confirmedEvents: [
          {
            relayUrl: "wss://relay.example.com",
            room: "owner_private",
            canonicalRecordId: sourceEventId,
            privateRumorId: null,
            privateRecord: null,
            hostAnnouncement: null,
            event: {
              id: sourceEventId,
              pubkey: "3".repeat(64),
              created_at: 1_700_000_000,
              kind: 30_174,
              tags: [
                ["d", "memory-slot"],
                ["p", "d".repeat(64)],
              ],
              content: "nip44-ciphertext",
              sig: "e".repeat(128),
            },
          },
        ],
      },
      1_700_000_100,
    );
    const memory = sources.find((source) => source.capabilityId === "memory");
    expect(memory).toMatchObject({
      status: "gap",
      recordRefs: [sourceEventId],
      reasonRef: "reason.issue31.device_projection_missing:memory",
    });
    expect(sources.some((source) => source.capabilityId === "owner_private_sarah")).toBe(false);
  });

  /**
   * omega#49. The memory row was a fixed `gap` emitted on the mere presence of a
   * raw engram event, so a device whose projection had arrived, decrypted, and
   * was rendering the engram in the room below still read
   * `device_projection_missing:memory`. The row is about projection, so it is
   * computed from projections.
   */
  test("reads memory as ready once the host projection for every visible engram has been read", () => {
    const projectedId = "c".repeat(64);
    const unprojectedId = "f".repeat(64);
    const engramEvent = (id: string) => ({
      relayUrl: "wss://relay.example.com",
      room: "owner_private" as const,
      canonicalRecordId: id,
      privateRumorId: null,
      privateRecord: null,
      hostAnnouncement: null,
      event: {
        id,
        pubkey: "3".repeat(64),
        created_at: 1_700_000_000,
        kind: 30_174,
        tags: [["d", "memory-slot"], ["p", "d".repeat(64)]],
        content: "nip44-ciphertext",
        sig: "e".repeat(128),
      },
    });
    const snapshot = (ids: ReadonlyArray<string>) => ({
      devicePublicKeyHex: "b".repeat(64),
      admittedHostPublicKeys: ["d".repeat(64)],
      selectedHostPublicKeys: ["d".repeat(64)],
      ownerPrivateAuthors: ["3".repeat(64)],
      ownerRecipientPublicKeys: ["d".repeat(64)],
      relays: [],
      storedEventIds: {},
      publishRefusals: {},
      confirmedEvents: ids.map(engramEvent),
    });
    const readFor = (
      ids: ReadonlyArray<string>,
      projected: ReadonlyArray<string>,
    ) =>
      issue31SourceSnapshotsFromNostr(snapshot(ids), 1_700_000_100, {
        memory: projected.map((sourceEventId) => ({
          sourceEventId,
          sourceCreatedAt: 1_700_000_000,
        })),
        readStateAndReminders: [],
      }).find((source) => source.capabilityId === "memory");

    expect(readFor([projectedId], [projectedId])).toMatchObject({
      status: "ready",
      reasonRef: null,
      recordRefs: [projectedId],
    });
    // One engram on the wire with no projection keeps the whole row a gap: a
    // partially projected set must not read as a complete one.
    expect(readFor([projectedId, unprojectedId], [projectedId])).toMatchObject({
      status: "gap",
      reasonRef: "reason.issue31.device_projection_missing:memory",
    });
    // A projection with no raw source event is still evidence. The relay serves
    // the owner-addressed wrap; the raw record need never reach this device.
    expect(readFor([], [projectedId])).toMatchObject({
      status: "ready",
      recordRefs: [projectedId],
    });
    // And a device that has read nothing states no memory row at all, rather
    // than a ready one over an empty set.
    expect(readFor([], [])).toBeUndefined();
  });

  /**
   * A revoked device must never claim a ready source. The owner-private
   * projection returns the empty model for a revoked grant, whose `projected`
   * set is empty, so the row falls back to the gap it can prove.
   */
  test("stops reading memory as ready once the grant is gone", () => {
    const projectedId = "c".repeat(64);
    const sources = issue31SourceSnapshotsFromNostr(
      {
        devicePublicKeyHex: "b".repeat(64),
        admittedHostPublicKeys: ["d".repeat(64)],
        selectedHostPublicKeys: ["d".repeat(64)],
        ownerPrivateAuthors: ["3".repeat(64)],
        ownerRecipientPublicKeys: ["d".repeat(64)],
        relays: [],
        storedEventIds: {},
        publishRefusals: {},
        confirmedEvents: [
          {
            relayUrl: "wss://relay.example.com",
            room: "owner_private",
            canonicalRecordId: projectedId,
            privateRumorId: null,
            privateRecord: null,
            hostAnnouncement: null,
            event: {
              id: projectedId,
              pubkey: "3".repeat(64),
              created_at: 1_700_000_000,
              kind: 30_174,
              tags: [["d", "memory-slot"], ["p", "d".repeat(64)]],
              content: "nip44-ciphertext",
              sig: "e".repeat(128),
            },
          },
        ],
      },
      1_700_000_100,
      emptyIssue31OwnerPrivateReadModel().projected,
    );
    expect(sources.find((source) => source.capabilityId === "memory")).toMatchObject({
      status: "gap",
      reasonRef: "reason.issue31.device_projection_missing:memory",
    });
  });

  test("requires one nonexpired discovery identity matching the active device grant", () => {
    const devicePublicKeyHex = "b".repeat(64);
    const hostPublicKeyHex = "d".repeat(64);
    const grantEventId = "1".repeat(64);
    const discoveryEventId = "2".repeat(64);
    const pairingResponseEventId = "3".repeat(64);
    const pairingChallengeEventId = "5".repeat(64);
    const pairingRequestEventId = "6".repeat(64);
    const grant = decodeIssue31PairingRecord({
      schema: ISSUE31_PAIRING_SCHEMA,
      recordType: "scoped_grant",
      hostRef: "omega.host.local",
      hostPublicKeyHex,
      sarahPublicKeyHex: "3".repeat(64),
      devicePublicKeyHex,
      issuedAt: 1_700_000_000,
      pairingResponseEventId,
      grantRef: "grant.omega.device_1",
      generation: 1,
      scopes: ["observe_issue31"],
      expiresAt: 1_700_003_000,
    });
    const announcement = decodeIssue31HostAnnouncement({
      schema: ISSUE31_HOST_ANNOUNCEMENT_SCHEMA,
      hostRef: "omega.host.local",
      hostPublicKeyHex,
      sarahPublicKeyHex: "3".repeat(64),
      displayName: "Local Omega",
      protocols: [ISSUE31_PAIRING_SCHEMA, ISSUE31_COMMAND_SCHEMA],
      relayUrls: ["wss://relay.example.com"],
      generation: 1,
      issuedAt: 1_700_000_000,
      expiresAt: 1_700_001_000,
    });
    const baseEvent = {
      created_at: 1_700_000_000,
      kind: 1_059,
      tags: [],
      content: "ciphertext",
      sig: "e".repeat(128),
    } as const;
    const privatePairingEvent = (
      canonicalRecordId: string,
      privateRecord: ReturnType<typeof decodeIssue31PairingRecord>,
      pubkey: string,
    ) => ({
      relayUrl: "wss://relay.example.com",
      room: "owner_private" as const,
      canonicalRecordId,
      privateRumorId: canonicalRecordId,
      privateRecord,
      hostAnnouncement: null,
      event: { ...baseEvent, id: "7".repeat(64), pubkey },
    });
    const snapshot = {
      devicePublicKeyHex,
      admittedHostPublicKeys: [hostPublicKeyHex],
      selectedHostPublicKeys: [hostPublicKeyHex],
      ownerPrivateAuthors: ["3".repeat(64)],
      ownerRecipientPublicKeys: [hostPublicKeyHex],
      relays: [],
      storedEventIds: {},
      publishRefusals: {},
      confirmedEvents: [
        privatePairingEvent(
          pairingRequestEventId,
          decodeIssue31PairingRecord({
            schema: ISSUE31_PAIRING_SCHEMA,
            recordType: "pairing_request",
            hostRef: "omega.host.local",
            hostPublicKeyHex,
            devicePublicKeyHex,
            issuedAt: 1_699_999_700,
            pairingRequestRef: "pairing.request.device_1",
            requestedScopes: ["observe_issue31"],
            expiresAt: 1_700_000_500,
          }),
          devicePublicKeyHex,
        ),
        privatePairingEvent(
          pairingChallengeEventId,
          decodeIssue31PairingRecord({
            schema: ISSUE31_PAIRING_SCHEMA,
            recordType: "pairing_challenge",
            hostRef: "omega.host.local",
            hostPublicKeyHex,
            devicePublicKeyHex,
            issuedAt: 1_699_999_800,
            pairingChallengeRef: "pairing.challenge.device_1",
            pairingRequestEventId,
            challenge: "8".repeat(64),
            expiresAt: 1_700_000_500,
          }),
          hostPublicKeyHex,
        ),
        privatePairingEvent(
          pairingResponseEventId,
          decodeIssue31PairingRecord({
            schema: ISSUE31_PAIRING_SCHEMA,
            recordType: "pairing_response",
            hostRef: "omega.host.local",
            hostPublicKeyHex,
            devicePublicKeyHex,
            issuedAt: 1_699_999_900,
            pairingResponseRef: "pairing.response.device_1",
            pairingChallengeEventId,
            challenge: "8".repeat(64),
            expiresAt: 1_700_000_500,
          }),
          devicePublicKeyHex,
        ),
        {
          relayUrl: "wss://relay.example.com",
          room: "owner_private" as const,
          canonicalRecordId: grantEventId,
          privateRumorId: grantEventId,
          privateRecord: grant,
          hostAnnouncement: null,
          event: { ...baseEvent, id: "4".repeat(64), pubkey: hostPublicKeyHex },
        },
        {
          relayUrl: "wss://relay.example.com",
          room: "discovery" as const,
          canonicalRecordId: discoveryEventId,
          privateRumorId: null,
          privateRecord: null,
          hostAnnouncement: announcement,
          event: {
            ...baseEvent,
            id: discoveryEventId,
            pubkey: hostPublicKeyHex,
            kind: 31_990,
          },
        },
      ],
    };
    expect(
      issue31SourceSnapshotsFromNostr(snapshot, 1_700_000_100).find(
        (source) => source.capabilityId === "connection_and_identity",
      ),
    ).toMatchObject({ status: "ready", recordRefs: [grantEventId, discoveryEventId] });
    expect(
      issue31SourceSnapshotsFromNostr(
        { ...snapshot, devicePublicKeyHex: "c".repeat(64) },
        1_700_000_100,
      ).some((source) => source.capabilityId === "connection_and_identity"),
    ).toBe(false);
    expect(
      issue31SourceSnapshotsFromNostr(snapshot, 1_700_002_000).find(
        (source) => source.capabilityId === "connection_and_identity",
      ),
    ).toMatchObject({
      status: "gap",
      reasonRef: "reason.issue31.host_discovery_missing_or_mismatched",
    });
    const mismatchedSarahSnapshot = {
      ...snapshot,
      confirmedEvents: snapshot.confirmedEvents.map((event) =>
        event.hostAnnouncement === null
          ? event
          : {
              ...event,
              hostAnnouncement: decodeIssue31HostAnnouncement({
                ...event.hostAnnouncement,
                sarahPublicKeyHex: "4".repeat(64),
              }),
            },
      ),
    };
    expect(
      issue31SourceSnapshotsFromNostr(mismatchedSarahSnapshot, 1_700_000_100).find(
        (source) => source.capabilityId === "connection_and_identity",
      ),
    ).toMatchObject({
      status: "gap",
      reasonRef: "reason.issue31.host_discovery_missing_or_mismatched",
    });
  });

  test("joins host-only state without replacing signed Nostr connection authority", () => {
    const hostAdjunct = decodeIssue31HostAdjunct(
      JSON.parse(
        readFileSync(
          new URL(
            "../../../packages/sarah/fixtures/issue31-workroom/openagents.omega.issue31.host.v1.canonical.json",
            import.meta.url,
          ),
          "utf8",
        ),
      ),
    );
    const model = projectIssue31WorkroomReadModel({
      projectedAt: observedAt,
      hostAdjunct,
    });
    const connection = model.rows.find((row) => row.id === "connection_and_identity");
    expect(connection?.source.authority).toBe("signed_nostr_record");
    expect(connection?.source.status).toBe("unavailable");
    expect(connection?.hostObservation?.projection.capability).toBe("connection_identity");
    expect(model.rows.find((row) => row.id === "full_auto")?.source.status).toBe("gap");
    expect(model.rows.find((row) => row.id === "full_auto")?.source.actionState.kind).toBe(
      "pending",
    );
    expect(model.rows.find((row) => row.id === "provider_accounts")?.source.status).toBe("ready");
    expect(model.rows.find((row) => row.id === "provider_accounts")?.source.actionState.kind).toBe(
      "refused",
    );
    expect(model.rows.find((row) => row.id === "evidence_chain")?.source.status).toBe("ready");
    expect(model.rows.find((row) => row.id === "evidence_chain")?.source.actionState.kind).toBe(
      "terminal",
    );
    expect(model.coverage).toMatchObject({ pending: 1, refused: 1, terminal: 1 });
  });

  test("reads a host that looked and found nothing as idle, not as broken", () => {
    // omega#97, found by running the packaged omega-effectd against this
    // device: a real host with an empty run registry emits `gap: complete` with
    // no record refs, because there is no run to cite. `decodeIssue31SourceSnapshot`
    // refused that with "needs an authority record" — so EVERY idle host was
    // unreadable, and the phone told its owner the host was broken when the
    // host was fine and merely idle.
    //
    // The row's authority is the `host.v1` snapshot itself, which is signed,
    // delivery-bound, and carried on the row. A host that could not look
    // publishes silence instead, and silence never reaches here.
    const canonical = JSON.parse(
      readFileSync(
        new URL(
          "../../../packages/sarah/fixtures/issue31-workroom/openagents.omega.issue31.host.v1.canonical.json",
          import.meta.url,
        ),
        "utf8",
      ),
    ) as Readonly<{ projections: ReadonlyArray<Record<string, unknown>> }>;
    const idle = decodeIssue31HostAdjunct({
      ...canonical,
      projections: canonical.projections.map((projection) => ({
        ...projection,
        freshness: "current",
        gap: "complete",
        recordRefs: projection["capability"] === "connection_identity" ? projection["recordRefs"] : [],
        commandState: { kind: "idle" },
      })),
    });
    const model = projectIssue31WorkroomReadModel({
      projectedAt: observedAt,
      hostAdjunct: idle,
    });
    for (const row of model.rows.filter((entry) => entry.expectedAuthority === "omega_host_adjunct")) {
      expect(row.source.status).toBe("ready");
      expect(row.source.recordRefs).toEqual([]);
      expect(row.source.reasonRef).toBeNull();
      // Sourced, not unsourced: the snapshot that said "this is all of them".
      expect(row.hostObservation?.snapshotRef).toBe(idle.snapshotRef);
    }
    // And a signed-Nostr row still may not claim `ready` with nothing to cite.
    expect(() =>
      decodeIssue31SourceSnapshot({
        capabilityId: "memory",
        authority: "signed_nostr_record",
        sourceRef: "source.issue31.sarah.nostr_memory",
        status: "ready",
        freshness: "live",
        observedAt,
        recordRefs: [],
        reasonRef: null,
        role: "owner",
        roleStatus: "active",
        actionState: { kind: "idle" },
      }),
    ).toThrow("needs an authority record");
  });

  test("refuses a host reading it can only partly build, rather than shortening it", () => {
    // A host snapshot the contract admits and this read model cannot build a
    // row from. The two are separate validators with separate reference
    // grammars — `@openagentsinc/sarah` admits `^[A-Za-z0-9._:-]+$`, and the
    // row schema here demands a lowercase dotted ref — so `RECORD-1` is a
    // record reference a conforming host may publish and this device may not
    // render. That divergence is a live seam between two codebases, not a
    // hypothetical.
    //
    // Half a host reading rendered beside a reason is a reading the owner could
    // mistake for the whole one, so none of it is kept — including
    // `hostObservation`, which would otherwise leave the row quoting a snapshot
    // whose contents were refused.
    //
    // And it may not throw: a throw here takes the owner-private transcript,
    // memory, read state and both community rooms down with it, because one
    // machine sent one bad record.
    const canonical = JSON.parse(
      readFileSync(
        new URL(
          "../../../packages/sarah/fixtures/issue31-workroom/openagents.omega.issue31.host.v1.canonical.json",
          import.meta.url,
        ),
        "utf8",
      ),
    ) as Readonly<{ projections: ReadonlyArray<Record<string, unknown>> }>;
    const contradictory = decodeIssue31HostAdjunct({
      ...canonical,
      projections: canonical.projections.map((projection) =>
        projection["capability"] === "evidence_chain"
          ? { ...projection, recordRefs: ["RECORD-1"] }
          : projection,
      ),
    });
    const model = projectIssue31WorkroomReadModel({
      projectedAt: observedAt,
      sources: readySources().filter(
        (source) => source.authority === "signed_nostr_record",
      ),
      hostAdjunct: contradictory,
    });
    expect(model.rows).toHaveLength(ISSUE31_CAPABILITY_DESCRIPTORS.length);
    for (const row of model.rows) {
      if (row.expectedAuthority === "omega_host_adjunct") {
        expect(row.source.status).toBe("unavailable");
        expect(row.source.reasonRef).toBe(
          `reason.issue31.host_projection_unreadable:${row.id}`,
        );
        expect(row.hostObservation).toBeNull();
        continue;
      }
      // Bounded: every signed-Nostr row is still exactly what it was.
      expect(row.source.status).toBe("ready");
    }
  });

  test("opens the Workroom route, exposes all missing sources, and switches isolated rooms", async () => {
    const program = buildHomeProgram();
    expect(program.initialState.syncPhase).toBe("unconfigured");
    expect(JSON.stringify(renderDrawerView(program.initialState))).toContain("Workroom");
    program.workroom.open();
    await Effect.runPromise(settle);
    let state = await Effect.runPromise(lastState(program));
    expect(state.workbenchRoute).toBe("workroom");
    expect(chromeProps(state).glassComposerVisible).toBe(false);
    let view = JSON.stringify(renderContentView(state));
    expect(view).toContain("Connection and identity");
    expect(view).toContain("Owner-private Sarah");
    expect(view).not.toContain("Community work");
    expect(view).toContain("reason.issue31.source_not_connected:full_auto");

    program.workroom.selectRoom("community");
    await Effect.runPromise(settle);
    state = await Effect.runPromise(lastState(program));
    view = JSON.stringify(renderContentView(state));
    expect(view).toContain("Community membership");
    expect(view).toContain("Community work");
    expect(view).toContain("v1 awards experience and pays no money");
    expect(view).not.toContain("issue31-capability-owner_private_sarah");
  });

  test("selects a signed discovered host and invokes the production pairing capability", async () => {
    const hostPublicKeyHex = "d".repeat(64);
    const selections: string[] = [];
    let pairingRequests = 0;
    const program = buildHomeProgram({
      issue31Nostr: {
        selectHost: async (selected) => {
          selections.push(selected);
        },
        requestPairing: async () => {
          pairingRequests += 1;
        },
      },
    });
    program.workroom.open();
    program.workroom.setNostrControl({
      phase: "ready",
      deviceNpub: `npub1${"q".repeat(58)}`,
      hosts: [
        {
          hostRef: "omega.host.local",
          hostPublicKeyHex,
          sarahPublicKeyHex: "3".repeat(64),
          displayName: "Local Omega",
          hostFingerprint: "dddddddddddd…dddddddd",
          sarahFingerprint: "333333333333…33333333",
          generation: 3,
          expiresAt: 1_800_000_000,
        },
      ],
      selectedHostPublicKeyHex: null,
      notice: null,
      communityHistoryNotice: null,
    });
    await Effect.runPromise(settle);
    let state = await Effect.runPromise(lastState(program));
    expect(JSON.stringify(renderContentView(state))).toContain(
      "Local Omega · dddddddddddd…dddddddd · generation 3",
    );
    expect(JSON.stringify(renderContentView(state))).toContain("Sarah 333333333333…33333333");
    program.workroom.selectHost(hostPublicKeyHex);
    await Effect.runPromise(settle);
    expect(selections).toEqual([hostPublicKeyHex]);
    program.workroom.requestPairing();
    await Effect.runPromise(settle);
    state = await Effect.runPromise(lastState(program));
    expect(pairingRequests).toBe(1);
    expect(state.issue31NostrControl.phase).toBe("pairing");
  });

  test("publishes owner-private v2 actions without treating host handling as completion", async () => {
    const hostPublicKeyHex = "d".repeat(64);
    const commands: Issue31CommandArguments[] = [];
    let clears = 0;
    const program = buildHomeProgram({
      issue31Nostr: {
        selectHost: async () => undefined,
        requestPairing: async () => undefined,
        publishCommandIntent: async (request) => {
          commands.push(request.arguments);
        },
        clearOwnerPrivateLocalData: () => {
          clears += 1;
        },
      },
    });
    program.workroom.setNostrControl({
      phase: "paired",
      deviceNpub: `npub1${"q".repeat(58)}`,
      hosts: [
        {
          hostRef: "omega.host.local",
          hostPublicKeyHex,
          sarahPublicKeyHex: "3".repeat(64),
          displayName: "Local Omega",
          hostFingerprint: "dddddddddddd…dddddddd",
          sarahFingerprint: "333333333333…33333333",
          generation: 3,
          expiresAt: 1_800_000_000,
          supportsCommandV2: true,
          conversation: "sarah.0123456789abcdef01234567",
        },
      ],
      selectedHostPublicKeyHex: hostPublicKeyHex,
      notice: null,
      communityHistoryNotice: null,
    });
    program.workroom.changeOwnerDraft("hello from mobile");
    program.workroom.sendOwnerMessage();
    await Effect.runPromise(settle);
    let state = await Effect.runPromise(lastState(program));
    expect(commands[0]).toMatchObject({
      kind: "send_message",
      text: "hello from mobile",
      conversation: "sarah.0123456789abcdef01234567",
    });
    expect(state.issue31OwnerDraft).toBe("");
    expect(state.issue31CommandNotice).toContain("handling is pending");
    expect(state.issue31CommandNotice).not.toContain("completed");

    program.workroom.changeOwnerReminderDraft("check the release");
    program.workroom.createOwnerReminder();
    await Effect.runPromise(settle);
    expect(commands[1]).toMatchObject({
      kind: "reminder_create",
      note: "check the release",
    });
    program.workroom.clearOwnerLocalData();
    await Effect.runPromise(settle);
    state = await Effect.runPromise(lastState(program));
    expect(clears).toBe(1);
    expect(state.issue31CommandNotice).toContain("cleared");

    program.workroom.open();
    await Effect.runPromise(settle);
    const view = JSON.stringify(renderContentView(await Effect.runPromise(lastState(program))));
    expect(view).toContain("Message Sarah through your Omega host");
    expect(view).toContain("Authority receipts");
    expect(view).toContain("Reminders");
  });
});
