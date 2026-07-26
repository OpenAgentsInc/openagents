import {
  ISSUE31_COMMAND_SCHEMA,
  ISSUE31_COMMAND_SCHEMA_V2,
  ISSUE31_OWNER_PROJECTION_SCHEMA,
  ISSUE31_WITHHELD_SOURCES_SCHEMA,
  ISSUE31_PAIRING_SCHEMA,
  createIssue31PrivateEnvelope,
  decodeIssue31CommandRecord,
  decodeIssue31CommandRecordV2,
  decodeIssue31PairingRecord,
  foldIssue31Grant,
  issue31PrivateEnvelopeTimestamps,
  type Issue31CommandIntent,
  type Issue31CommandArguments,
  type Issue31CommandIntentV2,
  type Issue31CommandRecord,
  type Issue31CommandRecordV2,
  type Issue31GrantState,
  type Issue31PairingRecord,
  type Issue31PairingScope,
} from "@openagentsinc/sarah/issue31-nostr";

import {
  COMMUNITY_ARBITRATION_FEEDBACK_KIND,
} from "@openagentsinc/sarah/community-arbitration";
import {
  NIP_29_GROUP_CHAT_KIND,
  NIP_29_PUT_USER_KIND,
  NIP_29_REMOVE_USER_KIND,
  assertCommunityGroupIdIsNotPrivateConversation,
  communityRoleFor,
  foldCommunityLedgerFromEvents,
} from "@openagentsinc/sarah/community";

import {
  openExpoIssue31DeviceIdentity,
  type Issue31DeviceIdentity,
  type Issue31SecureStore,
} from "./issue31-device-key-vault.ts";
import {
  createIssue31NostrClient,
  type Issue31ConfirmedEvent,
  type Issue31NostrClient,
  type Issue31NostrClientSnapshot,
  type Issue31PublishReceipt,
  type Issue31WebSocketLike,
} from "./issue31-nostr-client.ts";
import { createIssue31SecureRelayCursorStore } from "./issue31-relay-cursor-store.ts";
import {
  issue31PersistedPairingEventsForRequeue,
  openExpoIssue31LocalConfirmedRecordStore,
  openExpoIssue31LocalPairingRecordStore,
  openExpoIssue31OutboundEventStore,
} from "./issue31-outbound-event-store.ts";
import {
  issue31CommunityConfirmedEventsFrom,
  issue31MergeCommunityHistory,
  openExpoIssue31CommunityRecordStore,
  type Issue31CommunityRecordStore,
} from "./issue31-community-record-store.ts";

export const OPENAGENTS_ISSUE31_RELAY_URLS = ["wss://nos.lol", "wss://relay.damus.io"] as const;

/**
 * The relays this build reads issue #31 records from.
 *
 * The default is the pair above. It is overridable at build time because a
 * device proof has to put the phone and the Omega host on the same relay, and
 * with a compile-time constant the only way to do that was to edit this file —
 * which means the binary that was proven is not the binary that ships. An
 * override is refused wholesale rather than partially: a malformed entry falls
 * back to the default set, because a client silently reading half the relays it
 * was told to read is worse than one reading the ones it shipped with.
 *
 * This is deliberately *not* a runtime setting. A relay the user can retarget
 * from inside the app is a relay an attacker can retarget, and the client's
 * whole admission model assumes its relay list came from out of band.
 */
export const issue31RelayUrlsFromEnvironment = (
  value: string | undefined = process.env.EXPO_PUBLIC_OMEGA_RELAY_URLS,
): ReadonlyArray<string> => {
  if (value === undefined || value.trim() === "") return OPENAGENTS_ISSUE31_RELAY_URLS;
  const urls = [...new Set(value.split(",").map((entry) => entry.trim()))];
  if (urls.length === 0 || urls.length > 8) return OPENAGENTS_ISSUE31_RELAY_URLS;
  if (urls.some((url) => !/^wss?:\/\/[^\s@?#]+$/.test(url))) return OPENAGENTS_ISSUE31_RELAY_URLS;
  return urls;
};

export const ISSUE31_MOBILE_REQUESTED_SCOPES = [
  "observe_issue31",
  "send_message",
  "interrupt_turn",
  "control_full_auto",
  "request_provider_handoff",
  "act_in_community",
] as const;

declare const require: (id: string) => unknown;
const ISSUE31_MAX_LOCAL_PRIVATE_EVENTS = 2_080;

/**
 * The owner-private records a local wipe removes.
 *
 * The host's coverage statement goes with the projections it describes. Leaving
 * it behind would leave the device asserting "this is everything" over a list
 * the owner had just emptied — the exact confident-but-short rendering that
 * signal exists to prevent. After a wipe the room reads "unknown" until the
 * host restates coverage.
 */
const isClearableOwnerPrivateSchema = (schema: string | undefined): boolean =>
  schema === ISSUE31_OWNER_PROJECTION_SCHEMA || schema === ISSUE31_WITHHELD_SOURCES_SCHEMA;

export interface Issue31DiscoveredHost {
  readonly hostRef: string;
  readonly hostPublicKeyHex: string;
  readonly sarahPublicKeyHex: string;
  readonly displayName: string;
  readonly hostFingerprint: string;
  readonly sarahFingerprint: string;
  readonly generation: number;
  readonly expiresAt: number;
  readonly supportsCommandV2?: boolean;
  readonly conversation?: string | null;
}

export interface Issue31MobileNostrControlState {
  readonly phase: "discovering" | "ready" | "pairing" | "awaiting_grant" | "paired" | "failed";
  readonly deviceNpub: string | null;
  readonly hosts: ReadonlyArray<Issue31DiscoveredHost>;
  readonly selectedHostPublicKeyHex: string | null;
  readonly notice: string | null;
  /**
   * Why a community record was not written to durable history, if one was not.
   *
   * A record the room shows but cannot persist is a record that disappears at
   * the next launch, and the burn set is carried in exactly those records. So
   * the failure is surfaced rather than swallowed.
   */
  readonly communityHistoryNotice: string | null;
}

export const initialIssue31MobileNostrControlState = (): Issue31MobileNostrControlState => ({
  phase: "discovering",
  deviceNpub: null,
  hosts: [],
  selectedHostPublicKeyHex: null,
  notice: "Looking for signed Omega host announcements.",
  communityHistoryNotice: null,
});

export interface Issue31MobileNostrRuntime {
  readonly publicKeyHex: string;
  readonly npub: string;
  readonly client: Issue31NostrClient;
  readonly selectHost: (hostPublicKeyHex: string) => Promise<void>;
  readonly requestPairing: () => Promise<void>;
  readonly publishCommandIntent: (
    request: Issue31MobileCommandRequest,
  ) => Promise<Issue31MobileCommandPublishReceipt>;
  /**
   * Publish one operator-signed community action.
   *
   * Refuses before signing when the folded role does not permit the action.
   * The read model already withholds the control, so this is the second gate:
   * a control that never renders and an action that would still publish if it
   * were called anyway are not the same guarantee.
   */
  readonly publishCommunityAction: (
    request: Issue31CommunityActionRequest,
  ) => Promise<Issue31PublishReceipt>;
  readonly clearOwnerPrivateLocalData: () => void;
  readonly close: () => void;
}

export interface Issue31MobileCommandRequest {
  readonly idempotencyRef: string;
  readonly arguments: Issue31CommandArguments;
}

export interface Issue31MobileCommandPublishReceipt {
  readonly intentEventId: string;
  readonly giftWrapEventId: string;
  readonly publish: Issue31PublishReceipt;
}

export interface Issue31MobileOwnerPrivateScope {
  readonly selectedHostPublicKeys: ReadonlyArray<string>;
  readonly ownerAuthors: ReadonlyArray<string>;
  readonly ownerRecipientPublicKeys: ReadonlyArray<string>;
}

export const issue31MobileOwnerPrivateScope = (
  host: Issue31DiscoveredHost | undefined,
  grant: Issue31GrantState | null,
  nowUnixSeconds: number,
): Issue31MobileOwnerPrivateScope =>
  host === undefined || host.expiresAt <= nowUnixSeconds
    ? {
        selectedHostPublicKeys: [],
        ownerAuthors: [],
        ownerRecipientPublicKeys: [],
      }
    : {
        selectedHostPublicKeys: [host.hostPublicKeyHex],
        ownerAuthors:
          grant?.status === "active" &&
          grant.expiresAt !== null &&
          grant.expiresAt > nowUnixSeconds &&
          grant.hostRef === host.hostRef &&
          grant.hostPublicKeyHex === host.hostPublicKeyHex &&
          grant.sarahPublicKeyHex === host.sarahPublicKeyHex
            ? [host.sarahPublicKeyHex]
            : [],
        ownerRecipientPublicKeys:
          grant?.status === "active" &&
          grant.expiresAt !== null &&
          grant.expiresAt > nowUnixSeconds &&
          grant.hostRef === host.hostRef &&
          grant.hostPublicKeyHex === host.hostPublicKeyHex &&
          grant.sarahPublicKeyHex === host.sarahPublicKeyHex
            ? [host.hostPublicKeyHex]
            : [],
      };

export const issue31MobileCommandIntentForGrant = (
  input: Readonly<{
    grant: Issue31GrantState;
    devicePublicKeyHex: string;
    actionRef: string;
    idempotencyRef: string;
    argumentsRef: string;
    issuedAt: number;
    expiresAt: number;
  }>,
): Issue31CommandIntent =>
  decodeIssue31CommandRecord({
    schema: ISSUE31_COMMAND_SCHEMA,
    recordType: "command_intent",
    hostRef: input.grant.hostRef,
    hostPublicKeyHex: input.grant.hostPublicKeyHex,
    devicePublicKeyHex: input.devicePublicKeyHex,
    grantRef: input.grant.grantRef,
    actionRef: input.actionRef,
    idempotencyRef: input.idempotencyRef,
    expectedGeneration: input.grant.generation,
    argumentsRef: input.argumentsRef,
    issuedAt: input.issuedAt,
    expiresAt: input.expiresAt,
  }) as Issue31CommandIntent;

export const issue31MobileCommandIntentV2ForGrant = (
  input: Readonly<{
    grant: Issue31GrantState;
    devicePublicKeyHex: string;
    idempotencyRef: string;
    arguments: Issue31CommandArguments;
    issuedAt: number;
    expiresAt: number;
  }>,
): Issue31CommandIntentV2 =>
  decodeIssue31CommandRecordV2({
    schema: ISSUE31_COMMAND_SCHEMA_V2,
    recordType: "command_intent",
    hostRef: input.grant.hostRef,
    hostPublicKeyHex: input.grant.hostPublicKeyHex,
    devicePublicKeyHex: input.devicePublicKeyHex,
    grantRef: input.grant.grantRef,
    idempotencyRef: input.idempotencyRef,
    expectedGeneration: input.grant.generation,
    arguments: input.arguments,
    issuedAt: input.issuedAt,
    expiresAt: input.expiresAt,
  }) as Issue31CommandIntentV2;

const requiredScopeForCommand = (argumentsValue: Issue31CommandArguments): Issue31PairingScope => {
  if (argumentsValue.kind === "interrupt_turn") return "interrupt_turn";
  if (argumentsValue.kind === "read_state_patch") return "observe_issue31";
  return "send_message";
};

const bytesToHex = (bytes: Uint8Array): string =>
  [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");

const HEX_64 = /^[0-9a-f]{64}$/;

export const issue31AdmittedHostPublicKeysFromEnvironment = (
  value: string | undefined = process.env.EXPO_PUBLIC_OMEGA_HOST_PUBLIC_KEYS,
): ReadonlyArray<string> => {
  if (value === undefined || value.trim() === "") return [];
  const publicKeys = [...new Set(value.split(",").map((entry) => entry.trim()))];
  if (publicKeys.length > 16 || publicKeys.some((publicKey) => !HEX_64.test(publicKey))) return [];
  return publicKeys;
};

const publicKeyList = (value: string | undefined): ReadonlyArray<string> => {
  if (value === undefined || value.trim() === "") return [];
  const keys = [...new Set(value.split(",").map((entry) => entry.trim().toLowerCase()))];
  if (keys.length > 16 || keys.some((key) => !HEX_64.test(key))) return [];
  return keys;
};

export interface Issue31CommunityRoomConfig {
  readonly groupId: string | null;
  /** Group admin keys. Out of band — never learned from the relay. */
  readonly adminPubkeys: ReadonlyArray<string>;
  /** Keys admitted to publish XP awards, rank, and badges. */
  readonly scorerPubkeys: ReadonlyArray<string>;
  /** The registered owner appeal identity, when one exists. */
  readonly ownerAppealPubkey: string | null;
}

/**
 * The community room's out-of-band configuration.
 *
 * Every one of these is an authority the relay must not be able to assert for
 * itself: who may admit a member, who may publish a score, and who may rule on
 * an appeal. They are read from the app's own build configuration for the same
 * reason the admitted Omega host keys are — a client that learns its
 * authorities from the stream it is verifying has no authority at all.
 *
 * An unconfigured room projects as unavailable with a named reason rather than
 * falling back to something permissive.
 */
export const issue31CommunityConfigFromEnvironment = (
  env: Readonly<Record<string, string | undefined>> = process.env as unknown as Readonly<
    Record<string, string | undefined>
  >,
): Issue31CommunityRoomConfig => {
  const rawGroupId = env["EXPO_PUBLIC_OMEGA_COMMUNITY_GROUP_ID"]?.trim() ?? "";
  const groupId =
    rawGroupId === "" || rawGroupId.length > 128 || /\s/.test(rawGroupId) ? null : rawGroupId;
  const ownerAppeal = env["EXPO_PUBLIC_OMEGA_COMMUNITY_OWNER_APPEAL_PUBKEY"]?.trim().toLowerCase();
  return {
    groupId,
    adminPubkeys: publicKeyList(env["EXPO_PUBLIC_OMEGA_COMMUNITY_ADMIN_PUBKEYS"]),
    scorerPubkeys: publicKeyList(env["EXPO_PUBLIC_OMEGA_COMMUNITY_SCORER_PUBKEYS"]),
    ownerAppealPubkey:
      ownerAppeal !== undefined && HEX_64.test(ownerAppeal) ? ownerAppeal : null,
  };
};

/**
 * A community action this device can actually sign.
 *
 * The phone holds the human operator's key and no agent key, so the closed set
 * here is exactly the operator-signed half of the lifecycle. Quoting,
 * executing, returning a result, verifying a peer, and publishing a persona
 * attestation are signed by an agent key on the operator's own compute and are
 * never minted here.
 */
export type Issue31CommunityActionRequest =
  | Readonly<{ kind: "post_message"; text: string }>
  | Readonly<{ kind: "invite_member"; subjectPubkey: string }>
  | Readonly<{ kind: "revoke_member"; subjectPubkey: string }>
  | Readonly<{ kind: "revoke_agent"; subjectPubkey: string }>
  | Readonly<{
      kind: "file_appeal";
      decisionEventId: string;
      requestEventId: string;
      resultEventId: string;
      appealRef: string;
      grounds: string;
      groundsSummary: string;
    }>;

const uint32 = (bytes: Uint8Array): number => {
  if (bytes.length !== 4) throw new Error("Issue 31 random timestamp bytes are invalid.");
  return (
    ((bytes[0] ?? 0) * 0x1_00_00_00 +
      (bytes[1] ?? 0) * 0x1_00_00 +
      (bytes[2] ?? 0) * 0x1_00 +
      (bytes[3] ?? 0)) >>>
    0
  );
};

export const openIssue31MobileNostrRuntime = async (
  input: Readonly<{
    relayUrls: ReadonlyArray<string>;
    admittedHostPublicKeys: ReadonlyArray<string>;
    communityAuthors?: ReadonlyArray<string>;
    communityGroupIds?: ReadonlyArray<string>;
    /** Out-of-band community authorities. Absent means the room stays closed. */
    community?: Issue31CommunityRoomConfig;
    /** Owner-private conversation refs, checked against the group id. */
    ownerPrivateConversationRefs?: ReadonlyArray<string>;
    onSnapshot?: (snapshot: Issue31NostrClientSnapshot) => void;
    onControlState?: (state: Issue31MobileNostrControlState) => void;
  }>,
): Promise<Issue31MobileNostrRuntime> => {
  const store = require("expo-secure-store") as Issue31SecureStore;
  const crypto = require("expo-crypto") as Readonly<{
    getRandomBytesAsync: (length: number) => Promise<Uint8Array>;
  }>;
  const WebSocketImpl = globalThis.WebSocket as unknown as new (
    url: string,
  ) => Issue31WebSocketLike;
  if (typeof WebSocketImpl !== "function") {
    throw new Error("The Omega Nostr WebSocket client is unavailable.");
  }
  // The community room's group id must never equal an owner-private
  // conversation ref. Both rooms are event-sourced over the same relay by the
  // same device, so an equal identifier is not cosmetic: each room's
  // subscription would match the other's records.
  if (input.community?.groupId != null) {
    assertCommunityGroupIdIsNotPrivateConversation({
      groupId: input.community.groupId,
      privateConversationRefs: input.ownerPrivateConversationRefs ?? [],
    });
  }
  const admittedHostPublicKeys = new Set(input.admittedHostPublicKeys);
  if (
    admittedHostPublicKeys.size !== input.admittedHostPublicKeys.length ||
    admittedHostPublicKeys.size > 16 ||
    [...admittedHostPublicKeys].some((publicKey) => !HEX_64.test(publicKey))
  ) {
    throw new Error("The admitted Omega host key list is invalid.");
  }
  let identity: Issue31DeviceIdentity | null = null;
  let outboundStore: ReturnType<typeof openExpoIssue31OutboundEventStore> | null = null;
  let pairingStore: ReturnType<typeof openExpoIssue31LocalPairingRecordStore> | null = null;
  let confirmedStore: ReturnType<typeof openExpoIssue31LocalConfirmedRecordStore> | null = null;
  let communityStore: Issue31CommunityRecordStore | null = null;
  try {
    identity = await openExpoIssue31DeviceIdentity();
    outboundStore = openExpoIssue31OutboundEventStore();
    pairingStore = openExpoIssue31LocalPairingRecordStore();
    confirmedStore = openExpoIssue31LocalConfirmedRecordStore();
    // A separate database file, opened only when a group is configured. The
    // owner-private stores above never learn that this one exists.
    communityStore =
      input.community?.groupId == null
        ? null
        : openExpoIssue31CommunityRecordStore(input.community.groupId);
    const openedCommunityStore = communityStore;
    const openedIdentity = identity;
    const openedOutboundStore = outboundStore;
    const openedPairingStore = pairingStore;
    const openedConfirmedStore = confirmedStore;
    const storedPairingRecords = openedPairingStore.load();
    const storedConfirmedRecords = openedConfirmedStore.load();
    const localEvents: Issue31ConfirmedEvent[] = [
      ...storedPairingRecords,
      ...storedConfirmedRecords.filter(
        (stored) =>
          !storedPairingRecords.some(
            (pairing) => pairing.canonicalRecordId === stored.canonicalRecordId,
          ),
      ),
    ].map((stored) => ({
      relayUrl: "local://issue31-device-outbox",
      room: "owner_private",
      event: stored.event,
      canonicalRecordId: stored.canonicalRecordId,
      privateRumorId: stored.canonicalRecordId,
      privateRecord: stored.record,
      hostAnnouncement: null,
    }));
    const localEventIds = new Set(localEvents.map((event) => event.canonicalRecordId));
    const clearedOwnerProjectionIds = new Set<string>();
    const rememberLocalEvent = (event: Issue31ConfirmedEvent): void => {
      if (localEventIds.has(event.canonicalRecordId)) return;
      localEventIds.add(event.canonicalRecordId);
      localEvents.push(event);
      while (localEvents.length > ISSUE31_MAX_LOCAL_PRIVATE_EVENTS) {
        const removed = localEvents.shift();
        if (removed !== undefined) localEventIds.delete(removed.canonicalRecordId);
      }
    };
    // Community history from earlier launches. The per-room replay cursor is
    // already past these records, so the relay will not serve them again: if
    // they are not restored from disk the ledger starts empty every launch, and
    // a re-invitation after a restart re-admits a key an earlier revocation
    // burned. The fold is order-independent, so restoring them anywhere in the
    // merged sequence is enough.
    let communityHistory: ReadonlyArray<Issue31ConfirmedEvent> = [];
    let communityHistoryNotice: string | null = null;
    // Unreadable history closes the room rather than opening it without its
    // revocations. A room folded from an admission stream with the burn set
    // missing is worse than a room that shows nothing and says why.
    let communityHistoryReadable = true;
    if (openedCommunityStore !== null) {
      try {
        communityHistory = issue31CommunityConfirmedEventsFrom(openedCommunityStore.load());
      } catch (error) {
        communityHistoryReadable = false;
        communityHistoryNotice = `Stored community history could not be read, so this room stays closed until it can be: ${
          error instanceof Error ? error.message : "unknown reason"
        }`;
      }
    }
    const persistedCommunityIds = new Set(communityHistory.map((row) => row.event.id));
    const persistCommunityEvents = (
      snapshot: Issue31NostrClientSnapshot,
    ): void => {
      if (openedCommunityStore === null) return;
      for (const row of snapshot.confirmedEvents) {
        if (row.room !== "community" || persistedCommunityIds.has(row.event.id)) continue;
        try {
          openedCommunityStore.put(row.event);
          persistedCommunityIds.add(row.event.id);
        } catch (error) {
          // Never silence. A revocation-preserving bound refuses rather than
          // discarding a `9001`, and that refusal has to be visible.
          communityHistoryNotice =
            error instanceof Error
              ? `A community record was not persisted: ${error.message}`
              : "A community record was not persisted.";
        }
      }
    };
    /** Live community records plus everything restored from durable history. */
    const communityRecordsForFold = (): ReadonlyArray<Issue31ConfirmedEvent> =>
      issue31MergeCommunityHistory(client.snapshot(), communityHistory).confirmedEvents.filter(
        (event) => event.room === "community",
      );
    const answeredChallenges = new Set(
      localEvents.flatMap((event) =>
        event.privateRecord?.recordType === "pairing_response"
          ? [event.privateRecord.pairingChallengeEventId]
          : [],
      ),
    );
    let latestSnapshot: Issue31NostrClientSnapshot | null = null;
    let discoveredHosts: Issue31DiscoveredHost[] = [];
    let selectedHostPublicKeyHex: string | null = null;
    let pendingRequestEventId: string | null = null;
    let pendingResponseEventId: string | null = null;
    let activeGrant: Issue31GrantState | null = null;
    let selectedHostDiscoveryActive = false;
    let discoveryExpiryTimer: ReturnType<typeof setTimeout> | null = null;
    let phase: Issue31MobileNostrControlState["phase"] = "discovering";
    let notice: string | null =
      admittedHostPublicKeys.size === 0
        ? "Configure an Omega host public key out of band before discovery can admit a host."
        : null;
    let closed = false;
    let client: Issue31NostrClient;
    const communityGroupIds = !communityHistoryReadable
      ? []
      : (input.communityGroupIds ??
        (input.community?.groupId == null ? [] : [input.community.groupId]));
    const communityAuthors =
      input.communityAuthors ?? input.community?.scorerPubkeys ?? [];
    const persistedRequest = localEvents.findLast(
      (event) => event.privateRecord?.recordType === "pairing_request",
    );
    const persistedResponse = localEvents.findLast((event) => {
      if (
        event.privateRecord?.recordType !== "pairing_response" ||
        persistedRequest?.privateRecord?.recordType !== "pairing_request"
      ) {
        return false;
      }
      const response = event.privateRecord;
      const challenge = localEvents.find(
        (candidate) =>
          candidate.canonicalRecordId === response.pairingChallengeEventId &&
          candidate.privateRecord?.recordType === "pairing_challenge",
      );
      return (
        challenge?.privateRecord?.recordType === "pairing_challenge" &&
        challenge.privateRecord.pairingRequestEventId === persistedRequest.canonicalRecordId &&
        response.hostPublicKeyHex === persistedRequest.privateRecord.hostPublicKeyHex &&
        admittedHostPublicKeys.has(response.hostPublicKeyHex) &&
        response.devicePublicKeyHex === openedIdentity.publicKeyHex
      );
    });
    if (
      persistedRequest?.privateRecord?.recordType === "pairing_request" &&
      admittedHostPublicKeys.has(persistedRequest.privateRecord.hostPublicKeyHex)
    ) {
      selectedHostPublicKeyHex = persistedRequest.privateRecord.hostPublicKeyHex;
      pendingRequestEventId = persistedRequest.canonicalRecordId;
      phase = "pairing";
      notice = "Restored and re-queued the exact signed pairing request.";
    }
    if (
      persistedResponse?.privateRecord?.recordType === "pairing_response" &&
      admittedHostPublicKeys.has(persistedResponse.privateRecord.hostPublicKeyHex)
    ) {
      selectedHostPublicKeyHex = persistedResponse.privateRecord.hostPublicKeyHex;
      pendingResponseEventId = persistedResponse.canonicalRecordId;
      phase = "awaiting_grant";
      notice = "Restored and re-queued the exact signed pairing response.";
    }
    let ownerAuthorScopeRef =
      selectedHostPublicKeyHex === null ? "" : `${selectedHostPublicKeyHex}:host-only`;

    const controlState = (): Issue31MobileNostrControlState => ({
      phase,
      deviceNpub: openedIdentity.npub,
      hosts: discoveredHosts,
      selectedHostPublicKeyHex,
      notice,
      communityHistoryNotice,
    });
    const emitControl = (): void => input.onControlState?.(controlState());
    const augmentedSnapshot = (
      rawSnapshot: Issue31NostrClientSnapshot,
    ): Issue31NostrClientSnapshot => {
      // Persist first, then merge, so a record that arrived this launch is in
      // durable history before it is ever rendered.
      persistCommunityEvents(rawSnapshot);
      const snapshot = issue31MergeCommunityHistory(rawSnapshot, communityHistory);
      return {
        ...snapshot,
        confirmedEvents: [
          ...localEvents,
          ...snapshot.confirmedEvents.filter(
            (event) => !localEventIds.has(event.canonicalRecordId),
          ),
        ].filter((event) => {
          const record = event.privateRecord;
          if (
            isClearableOwnerPrivateSchema(record?.schema) &&
            clearedOwnerProjectionIds.has(event.canonicalRecordId)
          ) {
            return false;
          }
          return (
            record === null ||
            (selectedHostDiscoveryActive &&
              selectedHostPublicKeyHex !== null &&
              admittedHostPublicKeys.has(selectedHostPublicKeyHex) &&
              record.hostPublicKeyHex === selectedHostPublicKeyHex &&
              record.devicePublicKeyHex === openedIdentity.publicKeyHex)
          );
        }),
      };
    };
    const emitSnapshot = (snapshot: Issue31NostrClientSnapshot): void => {
      const priorCommunityNotice = communityHistoryNotice;
      latestSnapshot = augmentedSnapshot(snapshot);
      input.onSnapshot?.(latestSnapshot);
      if (communityHistoryNotice !== priorCommunityNotice) emitControl();
    };
    const createEnvelope = async (
      record: Issue31PairingRecord | Issue31CommandRecord | Issue31CommandRecordV2,
      secretKey: Uint8Array,
    ) => {
      const createdAt = Math.floor(Date.now() / 1_000);
      const randomTimestampBytes = await crypto.getRandomBytesAsync(8);
      const timestampValues = issue31PrivateEnvelopeTimestamps(
        createdAt,
        (() => {
          let offset = 0;
          return () => {
            const value = uint32(randomTimestampBytes.slice(offset, offset + 4));
            offset += 4;
            return value;
          };
        })(),
      );
      return createIssue31PrivateEnvelope({
        signer: openedIdentity.signer,
        recipientPublicKeyHex: record.hostPublicKeyHex,
        record,
        randomSecretKey: () => secretKey,
        createdAt,
        ...timestampValues,
      });
    };
    const publishPairingRecord = async (record: Issue31PairingRecord): Promise<string> => {
      const secretKey = await crypto.getRandomBytesAsync(32);
      const envelope = await createEnvelope(record, secretKey);
      openedPairingStore.put({
        canonicalRecordId: envelope.rumor.id,
        event: envelope.giftWrap,
        record,
      });
      rememberLocalEvent({
        relayUrl: "local://issue31-device-outbox",
        room: "owner_private",
        event: envelope.giftWrap,
        canonicalRecordId: envelope.rumor.id,
        privateRumorId: envelope.rumor.id,
        privateRecord: record,
        hostAnnouncement: null,
      });
      client.publish(envelope.giftWrap);
      if (latestSnapshot !== null) emitSnapshot(client.snapshot());
      return envelope.rumor.id;
    };
    const reconcileControl = (snapshot: Issue31NostrClientSnapshot): void => {
      const now = Math.floor(Date.now() / 1_000);
      if (discoveryExpiryTimer !== null) clearTimeout(discoveryExpiryTimer);
      discoveryExpiryTimer = null;
      const nextDiscoveryExpiry = snapshot.confirmedEvents
        .flatMap((event) => {
          const host = event.hostAnnouncement;
          return host !== null &&
            host.expiresAt > now &&
            admittedHostPublicKeys.has(host.hostPublicKeyHex)
            ? [host.expiresAt]
            : [];
        })
        .sort((left, right) => left - right)[0];
      if (nextDiscoveryExpiry !== undefined) {
        const delayMs = Math.min(2_147_483_647, Math.max(1, (nextDiscoveryExpiry - now) * 1_000));
        discoveryExpiryTimer = setTimeout(() => {
          discoveryExpiryTimer = null;
          if (!closed && latestSnapshot !== null) reconcileControl(latestSnapshot);
        }, delayMs);
      }
      const hosts = snapshot.confirmedEvents.flatMap((event) => {
        const host = event.hostAnnouncement;
        return host === null ||
          host.expiresAt <= now ||
          !admittedHostPublicKeys.has(host.hostPublicKeyHex)
          ? []
          : [
              {
                hostRef: host.hostRef,
                hostPublicKeyHex: host.hostPublicKeyHex,
                sarahPublicKeyHex: host.sarahPublicKeyHex,
                displayName: host.displayName,
                hostFingerprint: `${host.hostPublicKeyHex.slice(0, 12)}…${host.hostPublicKeyHex.slice(-8)}`,
                sarahFingerprint: `${host.sarahPublicKeyHex.slice(0, 12)}…${host.sarahPublicKeyHex.slice(-8)}`,
                generation: host.generation,
                expiresAt: host.expiresAt,
                supportsCommandV2: host.protocols.some(
                  (protocol: string) => protocol === ISSUE31_COMMAND_SCHEMA_V2,
                ),
                conversation:
                  host.schema === "openagents.omega.issue31.host_discovery.v2"
                    ? host.conversation
                    : null,
              },
            ];
      });
      discoveredHosts = [
        ...new Map(hosts.map((host) => [host.hostPublicKeyHex, host])).values(),
      ].sort((left, right) => left.displayName.localeCompare(right.displayName));
      const selectedHost = discoveredHosts.find(
        (host) => host.hostPublicKeyHex === selectedHostPublicKeyHex,
      );
      selectedHostDiscoveryActive =
        issue31MobileOwnerPrivateScope(selectedHost, null, now).selectedHostPublicKeys.length === 1;
      if (!selectedHostDiscoveryActive) {
        activeGrant = null;
        if (selectedHostPublicKeyHex !== null) {
          phase = discoveredHosts.length === 0 ? "discovering" : "ready";
          notice = "The selected Omega host announcement is absent or expired.";
        }
      }
      const pairingEvents = snapshot.confirmedEvents.flatMap((event) =>
        event.privateRecord?.schema === ISSUE31_PAIRING_SCHEMA &&
        selectedHostDiscoveryActive &&
        selectedHostPublicKeyHex !== null &&
        admittedHostPublicKeys.has(selectedHostPublicKeyHex) &&
        event.privateRecord.hostPublicKeyHex === selectedHostPublicKeyHex &&
        event.privateRecord.devicePublicKeyHex === openedIdentity.publicKeyHex
          ? [{ eventId: event.canonicalRecordId, record: event.privateRecord }]
          : [],
      );
      const matchingGrantRefs = new Set(
        pairingEvents.flatMap(({ record }) =>
          "grantRef" in record &&
          record.hostPublicKeyHex === selectedHostPublicKeyHex &&
          record.devicePublicKeyHex === openedIdentity.publicKeyHex &&
          (record.recordType !== "scoped_grant" ||
            pendingResponseEventId === null ||
            record.pairingResponseEventId === pendingResponseEventId)
            ? [record.grantRef]
            : [],
        ),
      );
      activeGrant =
        [...matchingGrantRefs]
          .flatMap((grantRef) => {
            try {
              const grant = foldIssue31Grant(pairingEvents, grantRef);
              return grant?.status === "active" &&
                grant.expiresAt !== null &&
                grant.expiresAt > now &&
                selectedHost !== undefined &&
                grant.hostRef === selectedHost.hostRef &&
                grant.sarahPublicKeyHex === selectedHost.sarahPublicKeyHex
                ? [grant]
                : [];
            } catch {
              return [];
            }
          })
          .sort(
            (left, right) =>
              right.generation - left.generation ||
              right.issuedAt - left.issuedAt ||
              right.sourceEventId.localeCompare(left.sourceEventId),
          )[0] ?? null;
      const ownerPrivateScope = issue31MobileOwnerPrivateScope(selectedHost, activeGrant, now);
      const nextOwnerAuthorScopeRef =
        selectedHost === undefined
          ? ""
          : `${selectedHost.hostPublicKeyHex}:${activeGrant?.sourceEventId ?? "pairing-only"}`;
      if (ownerAuthorScopeRef !== nextOwnerAuthorScopeRef) {
        ownerAuthorScopeRef = nextOwnerAuthorScopeRef;
        void client.updateSubscriptionScope(ownerPrivateScope).catch(() => {
          phase = "failed";
          notice = "The signed host and Sarah author binding could not be subscribed.";
          emitControl();
        });
      }
      if (activeGrant !== null) {
        phase = "paired";
        notice = "Device grant confirmed by signed Nostr records.";
      } else if (phase === "discovering" && discoveredHosts.length > 0) {
        phase = "ready";
        notice = "Confirm the admitted host fingerprint before pairing.";
      }
      emitControl();
    };
    const answerChallenge = async (
      eventId: string,
      challenge: Extract<Issue31PairingRecord, Readonly<{ recordType: "pairing_challenge" }>>,
    ): Promise<void> => {
      if (
        closed ||
        !selectedHostDiscoveryActive ||
        pendingRequestEventId === null ||
        challenge.pairingRequestEventId !== pendingRequestEventId ||
        challenge.hostPublicKeyHex !== selectedHostPublicKeyHex ||
        challenge.devicePublicKeyHex !== openedIdentity.publicKeyHex ||
        challenge.expiresAt <= Math.floor(Date.now() / 1_000) ||
        answeredChallenges.has(eventId)
      ) {
        return;
      }
      answeredChallenges.add(eventId);
      const issuedAt = Math.floor(Date.now() / 1_000);
      try {
        const response = decodeIssue31PairingRecord({
          schema: ISSUE31_PAIRING_SCHEMA,
          recordType: "pairing_response",
          hostRef: challenge.hostRef,
          hostPublicKeyHex: challenge.hostPublicKeyHex,
          devicePublicKeyHex: openedIdentity.publicKeyHex,
          issuedAt,
          pairingResponseRef: `pairing.response.${bytesToHex(await crypto.getRandomBytesAsync(12))}`,
          pairingChallengeEventId: eventId,
          challenge: challenge.challenge,
          expiresAt: Math.min(challenge.expiresAt, issuedAt + 300),
        });
        pendingResponseEventId = await publishPairingRecord(response);
        phase = "awaiting_grant";
        notice = "Challenge answered; waiting for the signed scoped grant.";
      } catch {
        phase = "failed";
        notice = "The signed pairing challenge could not be answered.";
      }
      emitControl();
    };
    const handleSnapshot = (snapshot: Issue31NostrClientSnapshot): void => {
      for (const event of snapshot.confirmedEvents) {
        const record = event.privateRecord;
        if (
          isClearableOwnerPrivateSchema(record?.schema) &&
          clearedOwnerProjectionIds.has(event.canonicalRecordId)
        ) {
          continue;
        }
        if (
          record !== null &&
          record.schema !== ISSUE31_PAIRING_SCHEMA &&
          selectedHostDiscoveryActive &&
          selectedHostPublicKeyHex !== null &&
          record.hostPublicKeyHex === selectedHostPublicKeyHex &&
          record.devicePublicKeyHex === openedIdentity.publicKeyHex &&
          !localEventIds.has(event.canonicalRecordId)
        ) {
          try {
            openedConfirmedStore.put({
              canonicalRecordId: event.canonicalRecordId,
              event: event.event,
              record,
            });
            rememberLocalEvent({ ...event, relayUrl: "local://issue31-confirmed-history" });
          } catch {
            phase = "failed";
            notice = "Confirmed owner-private history could not be stored without truncation.";
          }
        }
        if (
          record?.schema !== ISSUE31_PAIRING_SCHEMA ||
          !selectedHostDiscoveryActive ||
          selectedHostPublicKeyHex === null ||
          !admittedHostPublicKeys.has(selectedHostPublicKeyHex) ||
          record.hostPublicKeyHex !== selectedHostPublicKeyHex ||
          record.devicePublicKeyHex !== openedIdentity.publicKeyHex ||
          localEventIds.has(event.canonicalRecordId)
        ) {
          continue;
        }
        try {
          openedPairingStore.put({
            canonicalRecordId: event.canonicalRecordId,
            event: event.event,
            record,
          });
          rememberLocalEvent({
            ...event,
            relayUrl: "local://issue31-device-pairing",
          });
        } catch {
          phase = "failed";
          notice = "The confirmed pairing chain could not be stored on this device.";
        }
      }
      const augmented = augmentedSnapshot(snapshot);
      emitSnapshot(snapshot);
      reconcileControl(augmented);
      for (const event of augmented.confirmedEvents) {
        if (event.privateRecord?.recordType === "pairing_challenge") {
          void answerChallenge(event.canonicalRecordId, event.privateRecord);
        }
      }
    };

    client = createIssue31NostrClient({
      relayUrls: input.relayUrls,
      signer: openedIdentity.signer,
      webSocket: WebSocketImpl,
      cursorStore: createIssue31SecureRelayCursorStore(store),
      outboundStore: openedOutboundStore,
      admittedHostPublicKeys: [...admittedHostPublicKeys],
      // Scorer keys are the only author-scoped community subscription; the rest
      // of the room is addressed by its group id.
      ...(communityAuthors.length === 0 ? {} : { communityAuthors }),
      ...(communityGroupIds.length === 0 ? {} : { communityGroupIds }),
      onSnapshot: handleSnapshot,
    });
    for (const event of issue31PersistedPairingEventsForRequeue(storedPairingRecords, {
      selectedHostPublicKeyHex,
      devicePublicKeyHex: openedIdentity.publicKeyHex,
      admittedHostPublicKeys,
    })) {
      client.publish(event);
    }
    await client.start();
    emitControl();
    return {
      publicKeyHex: openedIdentity.publicKeyHex,
      npub: openedIdentity.npub,
      client,
      selectHost: async (hostPublicKeyHex) => {
        const host = discoveredHosts.find(
          (candidate) => candidate.hostPublicKeyHex === hostPublicKeyHex,
        );
        if (host === undefined) {
          throw new Error("The selected Omega host is not in signed discovery.");
        }
        const ownerPrivateScope = issue31MobileOwnerPrivateScope(
          host,
          null,
          Math.floor(Date.now() / 1_000),
        );
        if (ownerPrivateScope.selectedHostPublicKeys.length === 0) {
          throw new Error("The selected Omega host announcement has expired.");
        }
        selectedHostPublicKeyHex = hostPublicKeyHex;
        activeGrant = null;
        phase = "ready";
        notice = null;
        ownerAuthorScopeRef = `${hostPublicKeyHex}:pairing-only`;
        await client.updateSubscriptionScope(ownerPrivateScope);
        emitControl();
      },
      requestPairing: async () => {
        const host = discoveredHosts.find(
          (candidate) => candidate.hostPublicKeyHex === selectedHostPublicKeyHex,
        );
        if (host === undefined) throw new Error("Select a discovered Omega host first.");
        const issuedAt = Math.floor(Date.now() / 1_000);
        const request = decodeIssue31PairingRecord({
          schema: ISSUE31_PAIRING_SCHEMA,
          recordType: "pairing_request",
          hostRef: host.hostRef,
          hostPublicKeyHex: host.hostPublicKeyHex,
          devicePublicKeyHex: openedIdentity.publicKeyHex,
          issuedAt,
          pairingRequestRef: `pairing.request.${bytesToHex(await crypto.getRandomBytesAsync(12))}`,
          requestedScopes: ISSUE31_MOBILE_REQUESTED_SCOPES,
          expiresAt: issuedAt + 300,
        });
        pendingRequestEventId = await publishPairingRecord(request);
        pendingResponseEventId = null;
        phase = "pairing";
        notice = "Pairing request signed and queued; waiting for the host challenge.";
        emitControl();
      },
      publishCommandIntent: async (request) => {
        const grant = activeGrant;
        const issuedAt = Math.floor(Date.now() / 1_000);
        const host = discoveredHosts.find(
          (candidate) => candidate.hostPublicKeyHex === grant?.hostPublicKeyHex,
        );
        if (
          grant === null ||
          grant.status !== "active" ||
          grant.expiresAt === null ||
          grant.expiresAt <= issuedAt ||
          !grant.scopes.includes(requiredScopeForCommand(request.arguments))
        ) {
          throw new Error("No active Issue 31 grant permits this command.");
        }
        if (host?.supportsCommandV2 !== true) {
          throw new Error("The selected Omega host does not advertise Issue 31 command v2.");
        }
        const intent = issue31MobileCommandIntentV2ForGrant({
          grant,
          devicePublicKeyHex: openedIdentity.publicKeyHex,
          idempotencyRef: request.idempotencyRef,
          arguments: request.arguments,
          issuedAt,
          expiresAt: Math.min(grant.expiresAt, issuedAt + 300),
        });
        const secretKey = await crypto.getRandomBytesAsync(32);
        const envelope = await createEnvelope(intent, secretKey);
        if (activeGrant !== grant || grant.expiresAt <= Math.floor(Date.now() / 1_000)) {
          throw new Error("The Issue 31 grant changed before the command was queued.");
        }
        openedConfirmedStore.put({
          canonicalRecordId: envelope.rumor.id,
          event: envelope.giftWrap,
          record: intent,
        });
        rememberLocalEvent({
          relayUrl: "local://issue31-command-outbox",
          room: "owner_private",
          event: envelope.giftWrap,
          canonicalRecordId: envelope.rumor.id,
          privateRumorId: envelope.rumor.id,
          privateRecord: intent,
          hostAnnouncement: null,
        });
        const publish = client.publish(envelope.giftWrap);
        if (latestSnapshot !== null) emitSnapshot(client.snapshot());
        return {
          intentEventId: envelope.rumor.id,
          giftWrapEventId: envelope.giftWrap.id,
          publish,
        };
      },
      publishCommunityAction: async (request) => {
        const community = input.community;
        if (community === undefined || community.groupId === null) {
          throw new Error("The community room is not configured on this build.");
        }
        if (community.adminPubkeys.length === 0) {
          throw new Error("The community room has no admitted admin keys.");
        }
        const groupId = community.groupId;

        // Fold the current record set and check this device's own role before
        // signing anything. Roles come from records; nothing here assumes one.
        const fold = foldCommunityLedgerFromEvents({
          groupId,
          adminPubkeys: community.adminPubkeys,
          // Durable history included. Folding only this launch's records would
          // let a restart hand a revoked member their controls back, and would
          // refuse a genuine member their own room until the relay happened to
          // re-serve their admission.
          events: communityRecordsForFold()
            .map((event) => ({
              id: event.event.id,
              pubkey: event.event.pubkey,
              created_at: event.event.created_at,
              kind: event.event.kind,
              tags: event.event.tags,
              content: event.event.content,
            })),
        });
        const role = communityRoleFor(fold, openedIdentity.publicKeyHex);
        const isActiveMember =
          role.status === "active" &&
          (role.role === "member" || role.role === "agent_operator" || role.role === "owner");

        const createdAt = Math.floor(Date.now() / 1_000);
        let template: {
          kind: number;
          created_at: number;
          tags: string[][];
          content: string;
        };

        if (request.kind === "post_message") {
          if (!isActiveMember) {
            throw new Error("Only an active community member can post in this room.");
          }
          const text = request.text.trim();
          if (text === "" || text.length > 4_096) {
            throw new Error("A community message must be between 1 and 4096 characters.");
          }
          template = {
            kind: NIP_29_GROUP_CHAT_KIND,
            created_at: createdAt,
            tags: [["h", groupId]],
            content: text,
          };
        } else if (request.kind === "invite_member" || request.kind === "revoke_member" || request.kind === "revoke_agent") {
          // Admitting and removing are group-admin acts. A member cannot invite
          // themselves or revoke somebody else, and the refusal happens before
          // a signature exists rather than after the relay stores one.
          if (!role.isGroupAdmin) {
            throw new Error("Only a group admin can admit or remove a community key.");
          }
          const subject = request.subjectPubkey.trim().toLowerCase();
          if (!HEX_64.test(subject)) {
            throw new Error("A community admin action needs a 64 character hex pubkey.");
          }
          template = {
            kind:
              request.kind === "invite_member"
                ? NIP_29_PUT_USER_KIND
                : NIP_29_REMOVE_USER_KIND,
            created_at: createdAt,
            tags: [
              ["h", groupId],
              ["p", subject],
            ],
            content: "",
          };
        } else {
          if (!isActiveMember) {
            throw new Error("Only an active community member can file an appeal.");
          }
          for (const [label, value] of [
            ["decisionEventId", request.decisionEventId],
            ["requestEventId", request.requestEventId],
            ["resultEventId", request.resultEventId],
          ] as const) {
            if (!HEX_64.test(value)) {
              throw new Error(`A community appeal needs a 32-byte hex ${label}.`);
            }
          }
          const summary = request.groundsSummary.trim().slice(0, 500);
          if (summary === "") {
            throw new Error("A community appeal needs a stated reason.");
          }
          template = {
            kind: COMMUNITY_ARBITRATION_FEEDBACK_KIND,
            created_at: createdAt,
            tags: [
              ["h", groupId],
              ["e", request.decisionEventId, "", "decision"],
              ["e", request.requestEventId, "", "request"],
              ["e", request.resultEventId, "", "result"],
              ["p", openedIdentity.publicKeyHex],
              ["status", "appeal_open"],
              ["cw_feedback_type", "dispute_appeal"],
              ["cw_appeal_ref", request.appealRef],
              ["cw_grounds", request.grounds],
              ["cw_grounds_summary", summary],
              ["cw_arbiter", "owner"],
              ["cw_filed_at", new Date(createdAt * 1_000).toISOString()],
            ],
            content: "",
          };
        }

        const signed = await openedIdentity.signer.signEvent(template);
        // Published into the community room's own outbound budget. A room that
        // fills its queue never refuses the other room's next publish.
        return client.publish(signed, "community");
      },
      clearOwnerPrivateLocalData: () => {
        for (const event of client.snapshot().confirmedEvents) {
          if (isClearableOwnerPrivateSchema(event.privateRecord?.schema)) {
            clearedOwnerProjectionIds.add(event.canonicalRecordId);
          }
        }
        openedConfirmedStore.clearOwnerProjections();
        for (let index = localEvents.length - 1; index >= 0; index -= 1) {
          if (isClearableOwnerPrivateSchema(localEvents[index]?.privateRecord?.schema)) {
            const removed = localEvents.splice(index, 1)[0];
            if (removed !== undefined) {
              clearedOwnerProjectionIds.add(removed.canonicalRecordId);
              localEventIds.delete(removed.canonicalRecordId);
            }
          }
        }
        if (latestSnapshot !== null) emitSnapshot(client.snapshot());
      },
      close: () => {
        closed = true;
        if (discoveryExpiryTimer !== null) clearTimeout(discoveryExpiryTimer);
        discoveryExpiryTimer = null;
        client.close();
        openedOutboundStore.close();
        openedPairingStore.close();
        openedConfirmedStore.close();
        openedCommunityStore?.close();
        openedIdentity.close();
      },
    };
  } catch (error) {
    outboundStore?.close();
    pairingStore?.close();
    confirmedStore?.close();
    communityStore?.close();
    identity?.close();
    throw error;
  }
};
