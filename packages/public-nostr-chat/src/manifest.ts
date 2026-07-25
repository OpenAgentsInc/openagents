import { Schema as S } from "effect"

import {
  Hex64,
  PUBLIC_CHAT_ACCEPTED_KINDS,
  PUBLIC_CHAT_GROUP_ID,
  PUBLIC_CHAT_GROUP_STATE_KINDS,
  PUBLIC_CHAT_LIMITS,
  PUBLIC_CHAT_MANIFEST_SCHEMA,
  PUBLIC_CHAT_PROFILE,
  PUBLIC_CHAT_RELAY_INFO_URL,
  PUBLIC_CHAT_RELAY_URL,
  PUBLIC_CHAT_RICH_CONTENT_PROFILE,
  PUBLIC_CHAT_SIGNER_KINDS,
  PublicChatRejectionReason,
  groupNaddrFor,
} from "./profile.js"

export const PublicNostrChatManifest = S.Struct({
  acceptedKinds: S.Array(S.Int),
  agentPolicy: S.Struct({
    operatorLink: S.Literal("policy-dependent-separate-admission"),
    registrationRequired: S.Literal(false),
    sharedBotSecret: S.Literal(false),
  }),
  auth: S.Struct({
    applicationSession: S.Literal("none"),
    directRead: S.Literal("public"),
    directWrite: S.Literal("nip42-and-signed-event"),
    signerKinds: S.Array(S.Int),
    signers: S.Array(S.Literals(["nip07", "nip46", "nip55"])),
  }),
  examples: S.Struct({
    authenticate: S.Array(S.String),
    publish: S.Array(S.String),
    read: S.Array(S.String),
    reply: S.Array(S.String),
    retry: S.Array(S.String),
    richContent: S.Array(S.String),
  }),
  group: S.Struct({
    id: S.Literal(PUBLIC_CHAT_GROUP_ID),
    naddr: S.NullOr(S.String),
    nostrUri: S.NullOr(S.String),
    requiredTag: S.Tuple([S.Literal("h"), S.Literal(PUBLIC_CHAT_GROUP_ID)]),
    stateKinds: S.Array(S.Int),
  }),
  history: S.Struct({
    cursor: S.Literal("created_at-plus-all-event-ids-at-boundary"),
    deduplicateBy: S.Literal("event-id"),
    filter: S.Struct({
      "#h": S.Array(S.Literal(PUBLIC_CHAT_GROUP_ID)),
      kinds: S.Array(S.Int),
      limit: S.Int,
    }),
    gapRecovery: S.Literal(
      "replay-from-one-second-overlap-until-eose-then-deduplicate",
    ),
    previous: S.Literal(
      "up-to-three-from-last-fifty-seen-excluding-own-events",
    ),
  }),
  limits: S.Struct({
    attachmentBytes: S.Int,
    attachmentCount: S.Int,
    contentBytes: S.Int,
    eventBytes: S.Int,
    futureSkewSeconds: S.Int,
    historyPageSize: S.Int,
    maxAgeSeconds: S.Int,
    tags: S.Int,
  }),
  profileVersion: S.Literal(PUBLIC_CHAT_PROFILE),
  rateLimits: S.Struct({
    dimensions: S.Array(S.Literals(["ip", "pubkey", "operator"])),
    policySource: S.Literal("relay-nip11"),
    rotatingKeysBypassesOperatorLimit: S.Literal(false),
  }),
  readiness: S.Literals(["ready", "relay-self-required"]),
  rejectionReasons: S.Array(PublicChatRejectionReason),
  relay: S.Struct({
    informationUrl: S.Literal(PUBLIC_CHAT_RELAY_INFO_URL),
    selfPubkey: S.NullOr(Hex64),
    websocketUrl: S.Literal(PUBLIC_CHAT_RELAY_URL),
  }),
  richContentProfileVersion: S.Literal(PUBLIC_CHAT_RICH_CONTENT_PROFILE),
  schemaVersion: S.Literal(PUBLIC_CHAT_MANIFEST_SCHEMA),
})
export type PublicNostrChatManifest = typeof PublicNostrChatManifest.Type

export const publicNostrChatManifest = (
  relaySelfPubkey?: string,
): PublicNostrChatManifest => {
  const validRelaySelf =
    relaySelfPubkey !== undefined && S.is(Hex64)(relaySelfPubkey)
      ? relaySelfPubkey
      : null
  const naddr =
    validRelaySelf === null ? null : groupNaddrFor(validRelaySelf)
  return {
    acceptedKinds: [...PUBLIC_CHAT_ACCEPTED_KINDS],
    agentPolicy: {
      operatorLink: "policy-dependent-separate-admission",
      registrationRequired: false,
      sharedBotSecret: false,
    },
    auth: {
      applicationSession: "none",
      directRead: "public",
      directWrite: "nip42-and-signed-event",
      signerKinds: [...PUBLIC_CHAT_SIGNER_KINDS],
      signers: ["nip07", "nip46", "nip55"],
    },
    examples: {
      authenticate: [
        'wait for ["AUTH","<challenge>"]',
        'sign kind 22242 with relay and challenge tags, then send ["AUTH",event]',
      ],
      publish: [
        'sign kind 9 with ["h","openagents-public"] and up to three previous IDs',
        'send ["EVENT",event] and require ["OK",event.id,true,...]',
      ],
      read: [
        '["REQ","history",{"kinds":[9],"#h":["openagents-public"],"limit":50}]',
        'keep the subscription open after ["EOSE","history"] for live events',
      ],
      reply: [
        'add ["q","<parent-id>","wss://relay.openagents.com","<parent-pubkey>"]',
        "prefix content with the parent nostr:nevent reference",
      ],
      retry: [
        "reuse the same signed event after a lost receipt; deduplicate by event ID",
        "on reconnect, replay from one second before the latest cursor and deduplicate",
      ],
      richContent: [
        "put each attachment URL in content and add one matching imeta tag with m and x fields",
        'publish reactions as kind 7 and reports as kind 1984 with the same ["h","openagents-public"] tag',
      ],
    },
    group: {
      id: PUBLIC_CHAT_GROUP_ID,
      naddr,
      nostrUri: naddr === null ? null : `nostr:${naddr}`,
      requiredTag: ["h", PUBLIC_CHAT_GROUP_ID],
      stateKinds: [...PUBLIC_CHAT_GROUP_STATE_KINDS],
    },
    history: {
      cursor: "created_at-plus-all-event-ids-at-boundary",
      deduplicateBy: "event-id",
      filter: {
        "#h": [PUBLIC_CHAT_GROUP_ID],
        kinds: [9],
        limit: PUBLIC_CHAT_LIMITS.historyPageSize,
      },
      gapRecovery:
        "replay-from-one-second-overlap-until-eose-then-deduplicate",
      previous: "up-to-three-from-last-fifty-seen-excluding-own-events",
    },
    limits: { ...PUBLIC_CHAT_LIMITS },
    profileVersion: PUBLIC_CHAT_PROFILE,
    rateLimits: {
      dimensions: ["ip", "pubkey", "operator"],
      policySource: "relay-nip11",
      rotatingKeysBypassesOperatorLimit: false,
    },
    readiness: validRelaySelf === null ? "relay-self-required" : "ready",
    rejectionReasons: [
      "auth-required",
      "restricted",
      "duplicate",
      "invalid",
      "error",
      "blocked",
      "rate-limited",
      "pow",
      "wrong-kind",
      "wrong-group",
      "missing-previous",
      "unknown-previous",
      "content-too-large",
      "event-too-large",
      "too-many-tags",
      "stale",
      "future",
      "signature-invalid",
      "unsafe-media",
      "digest-mismatch",
      "relay-unavailable",
      "relay-self-unavailable",
      "fork-detected",
    ],
    relay: {
      informationUrl: PUBLIC_CHAT_RELAY_INFO_URL,
      selfPubkey: validRelaySelf,
      websocketUrl: PUBLIC_CHAT_RELAY_URL,
    },
    richContentProfileVersion: PUBLIC_CHAT_RICH_CONTENT_PROFILE,
    schemaVersion: PUBLIC_CHAT_MANIFEST_SCHEMA,
  }
}
