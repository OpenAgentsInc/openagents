import {
  ISSUE31_COMMAND_SCHEMA,
  ISSUE31_PAIRING_SCHEMA,
  createIssue31PrivateEnvelope,
  decodeIssue31CommandRecord,
  decodeIssue31PairingRecord,
  foldIssue31Grant,
  issue31PrivateEnvelopeTimestamps,
  type Issue31CommandIntent,
  type Issue31CommandRecord,
  type Issue31GrantState,
  type Issue31PairingRecord,
  type Issue31PairingScope,
} from "@openagentsinc/sarah/issue31-nostr";

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
  openExpoIssue31LocalPairingRecordStore,
  openExpoIssue31OutboundEventStore,
} from "./issue31-outbound-event-store.ts";

export const OPENAGENTS_ISSUE31_RELAY_URLS = ["wss://nos.lol", "wss://relay.damus.io"] as const;

export const ISSUE31_MOBILE_REQUESTED_SCOPES = [
  "observe_issue31",
  "send_message",
  "interrupt_turn",
  "control_full_auto",
  "request_provider_handoff",
  "act_in_community",
] as const;

declare const require: (id: string) => unknown;

export interface Issue31DiscoveredHost {
  readonly hostRef: string;
  readonly hostPublicKeyHex: string;
  readonly sarahPublicKeyHex: string;
  readonly displayName: string;
  readonly hostFingerprint: string;
  readonly sarahFingerprint: string;
  readonly generation: number;
  readonly expiresAt: number;
}

export interface Issue31MobileNostrControlState {
  readonly phase: "discovering" | "ready" | "pairing" | "awaiting_grant" | "paired" | "failed";
  readonly deviceNpub: string | null;
  readonly hosts: ReadonlyArray<Issue31DiscoveredHost>;
  readonly selectedHostPublicKeyHex: string | null;
  readonly notice: string | null;
}

export const initialIssue31MobileNostrControlState = (): Issue31MobileNostrControlState => ({
  phase: "discovering",
  deviceNpub: null,
  hosts: [],
  selectedHostPublicKeyHex: null,
  notice: "Looking for signed Omega host announcements.",
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
  readonly close: () => void;
}

export interface Issue31MobileCommandRequest {
  readonly requiredScope: Issue31PairingScope;
  readonly actionRef: string;
  readonly idempotencyRef: string;
  readonly argumentsRef: string;
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
  try {
    identity = await openExpoIssue31DeviceIdentity();
    outboundStore = openExpoIssue31OutboundEventStore();
    pairingStore = openExpoIssue31LocalPairingRecordStore();
    const openedIdentity = identity;
    const openedOutboundStore = outboundStore;
    const openedPairingStore = pairingStore;
    const storedPairingRecords = openedPairingStore.load();
    const localEvents: Issue31ConfirmedEvent[] = storedPairingRecords.map((stored) => ({
      relayUrl: "local://issue31-device-outbox",
      room: "owner_private",
      event: stored.event,
      canonicalRecordId: stored.canonicalRecordId,
      privateRumorId: stored.canonicalRecordId,
      privateRecord: stored.record,
      hostAnnouncement: null,
    }));
    const localEventIds = new Set(localEvents.map((event) => event.canonicalRecordId));
    const rememberLocalEvent = (event: Issue31ConfirmedEvent): void => {
      if (localEventIds.has(event.canonicalRecordId)) return;
      localEventIds.add(event.canonicalRecordId);
      localEvents.push(event);
      while (localEvents.length > 32) {
        const removed = localEvents.shift();
        if (removed !== undefined) localEventIds.delete(removed.canonicalRecordId);
      }
    };
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
    });
    const emitControl = (): void => input.onControlState?.(controlState());
    const augmentedSnapshot = (
      snapshot: Issue31NostrClientSnapshot,
    ): Issue31NostrClientSnapshot => ({
      ...snapshot,
      confirmedEvents: [
        ...localEvents,
        ...snapshot.confirmedEvents.filter((event) => !localEventIds.has(event.canonicalRecordId)),
      ].filter((event) => {
        const record = event.privateRecord;
        return (
          record === null ||
          (selectedHostDiscoveryActive &&
            selectedHostPublicKeyHex !== null &&
            admittedHostPublicKeys.has(selectedHostPublicKeyHex) &&
            record.hostPublicKeyHex === selectedHostPublicKeyHex &&
            record.devicePublicKeyHex === openedIdentity.publicKeyHex)
        );
      }),
    });
    const emitSnapshot = (snapshot: Issue31NostrClientSnapshot): void => {
      latestSnapshot = augmentedSnapshot(snapshot);
      input.onSnapshot?.(latestSnapshot);
    };
    const createEnvelope = async (
      record: Issue31PairingRecord | Issue31CommandRecord,
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
      ...(input.communityAuthors === undefined ? {} : { communityAuthors: input.communityAuthors }),
      ...(input.communityGroupIds === undefined
        ? {}
        : { communityGroupIds: input.communityGroupIds }),
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
        if (
          grant === null ||
          grant.status !== "active" ||
          grant.expiresAt === null ||
          grant.expiresAt <= issuedAt ||
          !grant.scopes.includes(request.requiredScope)
        ) {
          throw new Error("No active Issue 31 grant permits this command.");
        }
        const intent = issue31MobileCommandIntentForGrant({
          grant,
          devicePublicKeyHex: openedIdentity.publicKeyHex,
          actionRef: request.actionRef,
          idempotencyRef: request.idempotencyRef,
          argumentsRef: request.argumentsRef,
          issuedAt,
          expiresAt: Math.min(grant.expiresAt, issuedAt + 300),
        });
        const secretKey = await crypto.getRandomBytesAsync(32);
        const envelope = await createEnvelope(intent, secretKey);
        if (activeGrant !== grant || grant.expiresAt <= Math.floor(Date.now() / 1_000)) {
          throw new Error("The Issue 31 grant changed before the command was queued.");
        }
        const publish = client.publish(envelope.giftWrap);
        return {
          intentEventId: envelope.rumor.id,
          giftWrapEventId: envelope.giftWrap.id,
          publish,
        };
      },
      close: () => {
        closed = true;
        if (discoveryExpiryTimer !== null) clearTimeout(discoveryExpiryTimer);
        discoveryExpiryTimer = null;
        client.close();
        openedOutboundStore.close();
        openedPairingStore.close();
        openedIdentity.close();
      },
    };
  } catch (error) {
    outboundStore?.close();
    pairingStore?.close();
    identity?.close();
    throw error;
  }
};
