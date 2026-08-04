import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vite-plus/test";
import { Effect, Schema } from "effect";
import { EventKind, UnixTimestamp } from "nostr-effect/core";
import { finalizeEvent, getPublicKey } from "nostr-effect/pure";
import { decrypt, encrypt, getConversationKey } from "nostr-effect/nip44";
import { createRumor, createSeal, createWrap, unwrapEvent } from "nostr-effect/nip59";
import {
  MktTransportError,
  serializeSignedEvent,
  unwrapPrivateRecord,
  wrapPrivateRecord,
  wrapPrivateRecordCopies,
} from "./transport.js";

const fixture = JSON.parse(
  readFileSync(
    fileURLToPath(new URL("../contract/fixtures/nipmkt/client-transport.json", import.meta.url)),
    "utf8",
  ),
) as {
  deterministic_round_trip: { inner_event_id: string; outer_event_id: string };
};
const nip44Fixture = JSON.parse(
  readFileSync(
    fileURLToPath(new URL("../contract/fixtures/nip44/market-client.json", import.meta.url)),
    "utf8",
  ),
) as {
  secret_one: string;
  secret_two: string;
  conversation_key: string;
  nonce: string;
  plaintext: string;
  payload: string;
};

function hexBytes(value: string): Uint8Array {
  return Uint8Array.from(value.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
}

function bytesHex(value: Uint8Array): string {
  return Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("");
}
const decodeEventKind = Schema.decodeUnknownSync(EventKind);
const decodeUnixTimestamp = Schema.decodeUnknownSync(UnixTimestamp);

describe("NIP-MKT transport", () => {
  test("rejects unknown and wrong-kind wrap values through the Effect error channel", () =>
    // Vite Plus does not expose the Effect test extension in this workspace.
    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests
    Effect.runPromise(
      Effect.gen(function* () {
        const recipientPrivateKey = new Uint8Array(32).fill(2);
        for (const value of [
          { kind: 1059 },
          {
            ...finalizeEvent(
              { created_at: 1, kind: 1, tags: [], content: "" },
              recipientPrivateKey,
            ),
          },
        ]) {
          const error = yield* Effect.flip(unwrapPrivateRecord(value, recipientPrivateKey, []));
          expect(error).toBeInstanceOf(MktTransportError);
          if (error instanceof MktTransportError) expect(error.code).toBe("invalid_gift_wrap");
        }
      }),
    ));

  test("replays the pinned NIP-44 vector", () => {
    const secretOne = hexBytes(nip44Fixture.secret_one);
    const secretTwo = hexBytes(nip44Fixture.secret_two);
    const conversationKey = getConversationKey(secretOne, getPublicKey(secretTwo));
    expect(bytesHex(conversationKey)).toBe(nip44Fixture.conversation_key);
    expect(encrypt(nip44Fixture.plaintext, conversationKey, hexBytes(nip44Fixture.nonce))).toBe(
      nip44Fixture.payload,
    );
    expect(decrypt(nip44Fixture.payload, conversationKey)).toBe(nip44Fixture.plaintext);
  });

  test("replays the deterministic Immortal round trip", () =>
    // Vite Plus does not expose the Effect test extension in this workspace.
    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests
    Effect.runPromise(
      Effect.gen(function* () {
        const senderPrivateKey = new Uint8Array(32).fill(1);
        const recipientPrivateKey = new Uint8Array(32).fill(2);
        const recipientPublicKey = getPublicKey(recipientPrivateKey);
        const session = "11".repeat(32);
        const signed = finalizeEvent(
          {
            created_at: 10,
            kind: 39604,
            tags: [
              ["d", "22".repeat(32)],
              ["session", session],
              ["profile", "local-dev", "1"],
              ["p", recipientPublicKey, "", "provider"],
              ["alt", "Local development RFQ"],
            ],
            content: JSON.stringify({
              profile: "local-dev",
              profile_version: 1,
              schema: "openagents.mkt.v1",
              session_id: session,
            }),
          },
          senderPrivateKey,
          new Uint8Array(32),
        );
        const raw = serializeSignedEvent(signed);
        expect(signed.id).toBe(fixture.deterministic_round_trip.inner_event_id);

        const profiles = [{ id: "local-dev", version: 1 }];
        const createdRumor = createRumor(
          {
            created_at: decodeUnixTimestamp(signed.created_at),
            kind: decodeEventKind(signed.kind),
            tags: [["p", recipientPublicKey]],
            content: raw,
          },
          senderPrivateKey,
        );
        const rumor = {
          id: createdRumor.id,
          pubkey: createdRumor.pubkey,
          created_at: createdRumor.created_at,
          kind: createdRumor.kind,
          tags: createdRumor.tags,
          content: createdRumor.content,
        };
        const seal = createSeal(rumor, senderPrivateKey, recipientPublicKey, {
          sealCreatedAt: 8,
          sealNonce: new Uint8Array(32).fill(3),
          sealAuxiliaryRandomData: new Uint8Array(32),
        });
        const canonicalSeal = {
          id: seal.id,
          pubkey: seal.pubkey,
          created_at: seal.created_at,
          kind: seal.kind,
          tags: seal.tags,
          content: seal.content,
          sig: seal.sig,
        };
        const wrapped = createWrap(canonicalSeal, recipientPublicKey, {
          wrapCreatedAt: 9,
          wrapNonce: new Uint8Array(32).fill(4),
          wrapPrivateKey: new Uint8Array(32).fill(5),
          wrapAuxiliaryRandomData: new Uint8Array(32),
        });

        expect(wrapped.id).toBe(fixture.deterministic_round_trip.outer_event_id);
        expect(unwrapEvent(wrapped, recipientPrivateKey).content).toBe(raw);
        const legacyError = yield* Effect.flip(
          unwrapPrivateRecord(wrapped, recipientPrivateKey, profiles, {
            validateKindTags: false,
          }),
        );
        expect(legacyError).toBeInstanceOf(MktTransportError);
      }),
    ));

  test("adds the required unique rumor identifier for production wraps", () =>
    // Vite Plus does not expose the Effect test extension in this workspace.
    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests
    Effect.runPromise(
      Effect.gen(function* () {
        const senderPrivateKey = new Uint8Array(32).fill(1);
        const recipientPrivateKey = new Uint8Array(32).fill(2);
        const recipientPublicKey = getPublicKey(recipientPrivateKey);
        const session = "31".repeat(32);
        const signed = finalizeEvent(
          {
            created_at: 20,
            kind: 39604,
            tags: [
              ["d", "32".repeat(32)],
              ["session", session],
              ["profile", "local-dev", "1"],
              ["p", recipientPublicKey, "", "provider"],
              ["alt", "Production RFQ"],
              ["a", `39601:${recipientPublicKey}:offering`, "", "offering"],
              ["expiration", "30"],
            ],
            content: JSON.stringify({
              schema: "openagents.mkt.v1",
              profile: "local-dev",
              profile_version: 1,
              session_id: session,
            }),
          },
          senderPrivateKey,
          new Uint8Array(32),
        );
        const wrapped = yield* wrapPrivateRecord(
          serializeSignedEvent(signed),
          senderPrivateKey,
          recipientPublicKey,
          [{ id: "local-dev", version: 1 }],
          {
            rumorIdentifier: "33".repeat(32),
            sealCreatedAt: 18,
            wrapCreatedAt: 19,
            sealNonce: new Uint8Array(32).fill(6),
            wrapNonce: new Uint8Array(32).fill(7),
            wrapPrivateKey: new Uint8Array(32).fill(8),
          },
        );
        expect(unwrapEvent(wrapped, recipientPrivateKey).tags).toEqual([
          ["p", recipientPublicKey],
          ["d", "33".repeat(32)],
        ]);
        const delivered = yield* unwrapPrivateRecord(
          wrapped,
          recipientPrivateKey,
          [{ id: "local-dev", version: 1 }],
          { receivedAt: 21 },
        );
        expect(delivered.sealId).toMatch(/^[0-9a-f]{64}$/);
        expect(delivered.rumorId).toMatch(/^[0-9a-f]{64}$/);
        expect(delivered.wrapId).toBe(wrapped.id);
        expect(delivered.receivedAt).toBe(21);
        expect(delivered.verifiedProvenance).toEqual({
          wrapId: delivered.wrapId,
          sealId: delivered.sealId,
          rumorId: delivered.rumorId,
        });
        expect(delivered.sourceProvenance).toEqual([]);

        const copies = yield* wrapPrivateRecordCopies(
          serializeSignedEvent(signed),
          senderPrivateKey,
          recipientPublicKey,
          [{ id: "local-dev", version: 1 }],
          {
            rumorIdentifier: "34".repeat(32),
            sealNonce: new Uint8Array(32).fill(9),
            wrapNonce: new Uint8Array(32).fill(10),
            wrapPrivateKey: new Uint8Array(32).fill(11),
          },
          {
            rumorIdentifier: "35".repeat(32),
            sealNonce: new Uint8Array(32).fill(12),
            wrapNonce: new Uint8Array(32).fill(13),
            wrapPrivateKey: new Uint8Array(32).fill(14),
          },
        );
        expect(copies.counterparty.id).not.toBe(copies.senderRecovery.id);
        const recovered = yield* unwrapPrivateRecord(copies.senderRecovery, senderPrivateKey, [
          { id: "local-dev", version: 1 },
        ]);
        expect(recovered.event.id).toBe(signed.id);
      }),
    ));
});
