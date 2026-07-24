import { Schema as S } from "effect";

import type { SarahNostrEventTemplate } from "../nostr-identity/types.ts";
import {
  SARAH_TURN_RECORD_KIND,
  TURN_RECORD_ALT,
  SarahTurnRecordPayload,
  type SarahNostrCipher,
  type SarahTurnConversation,
  type SarahTurnEntry,
  type SarahTurnParent,
} from "./types.ts";

const decodePayload = S.decodeUnknownSync(SarahTurnRecordPayload);

export const buildTurnRecordPayload = (input: {
  readonly entry: SarahTurnEntry;
  readonly conversation: string;
  readonly turnRef: string;
  readonly seq: number;
  readonly timestamp?: string;
  readonly parents?: ReadonlyArray<SarahTurnParent>;
  readonly payload?: Record<string, unknown>;
}): SarahTurnRecordPayload =>
  decodePayload({
    schema: "openagents.sarah.turn_record.v1",
    entry: input.entry,
    conversation: input.conversation,
    turnRef: input.turnRef,
    seq: input.seq,
    timestamp: input.timestamp ?? new Date().toISOString(),
    parents: input.parents ?? [],
    payload: input.payload ?? {},
  });

/**
 * Build an unsigned durable turn-record event (kind 44300).
 * Content is ciphertext from the injected cipher (NIP-44 in production).
 */
export const buildDurableTurnRecordTemplate = (input: {
  readonly conversation: SarahTurnConversation;
  readonly entry: SarahTurnEntry;
  readonly turnRef: string;
  readonly seq: number;
  readonly cipher: SarahNostrCipher;
  readonly parents?: ReadonlyArray<SarahTurnParent>;
  readonly payload?: Record<string, unknown>;
  readonly createdAt?: number;
}): {
  readonly template: SarahNostrEventTemplate;
  readonly decrypted: SarahTurnRecordPayload;
} => {
  const decrypted = buildTurnRecordPayload({
    entry: input.entry,
    conversation: input.conversation.conversation,
    turnRef: input.turnRef,
    seq: input.seq,
    ...(input.parents !== undefined ? { parents: input.parents } : {}),
    ...(input.payload !== undefined ? { payload: input.payload } : {}),
  });

  const ciphertext = input.cipher.encryptToOwner(JSON.stringify(decrypted));
  if (!ciphertext || ciphertext.trim() === "") {
    throw new Error("sarah_nostr_turn: cipher returned empty content");
  }
  // Fail closed if a broken cipher echoed plaintext JSON.
  if (ciphertext.includes('"schema":"openagents.sarah.turn_record.v1"')) {
    throw new Error("sarah_nostr_turn: cipher must not return plaintext payload");
  }

  const tags: string[][] = [
    ["p", input.conversation.ownerPubkey],
    ["agent", input.conversation.sarahPubkey],
    ["conversation", input.conversation.conversation],
    ["entry", input.entry],
    ["turn", input.turnRef],
    ["alt", TURN_RECORD_ALT],
  ];
  for (const parent of decrypted.parents) {
    tags.push(["e", parent.eventId, "", parent.marker]);
  }

  return {
    decrypted,
    template: {
      kind: SARAH_TURN_RECORD_KIND,
      created_at: input.createdAt ?? Math.floor(Date.now() / 1000),
      tags,
      content: ciphertext,
    },
  };
};

/** Live NIP-AO telemetry frame (ephemeral kind 24200). Not durable. */
export const buildLiveAoFrameTemplate = (input: {
  readonly conversation: SarahTurnConversation;
  readonly turnRef: string;
  readonly seq: number;
  readonly frameType: "tool.call" | "tool.result" | "tool.error" | "cancel_turn";
  readonly body: Record<string, unknown>;
  readonly createdAt?: number;
}): SarahNostrEventTemplate => ({
  kind: 24200,
  created_at: input.createdAt ?? Math.floor(Date.now() / 1000),
  tags: [
    ["p", input.conversation.ownerPubkey],
    ["conversation", input.conversation.conversation],
    ["turn", input.turnRef],
    ["seq", String(input.seq)],
    ["ao", input.frameType],
    ["alt", "OpenAgents Sarah live activity (ephemeral)"],
  ],
  content: JSON.stringify(input.body),
});
