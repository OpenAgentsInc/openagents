import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vite-plus/test";
import { Effect, Schema } from "effect";
import { EventKind, UnixTimestamp } from "nostr-effect/core";
import { createRumor, createSeal, createWrap } from "nostr-effect/nip59";
import { finalizeEvent, getPublicKey } from "nostr-effect/pure";
import {
  activeSupersedingQuote,
  analyzeStatusSequence,
  authorizationDecision,
  deliveryDeduplicationDecision,
  evidenceDecision,
  expiryDecision,
  recoveryDecision,
  reserveCapacity,
  settlementDecision,
} from "./state.js";
import {
  MktTransportError,
  serializeSignedEvent,
  unwrapPrivateRecord,
  type MktTransportCode,
} from "./transport.js";

interface ClientCase {
  readonly id: string;
  readonly inputs: unknown;
  readonly expected: unknown;
}

const fixture = JSON.parse(
  readFileSync(
    fileURLToPath(new URL("../contract/fixtures/nipmkt/client-only-cases.json", import.meta.url)),
    "utf8",
  ),
) as {
  readonly actors: Readonly<Record<string, string>>;
  readonly session_id: string;
  readonly cases: readonly ClientCase[];
};
const decodeEventKind = Schema.decodeUnknownSync(EventKind);
const decodeUnixTimestamp = Schema.decodeUnknownSync(UnixTimestamp);

function replayWrapperBinding(caseId: string) {
  const providerPrivateKey = new Uint8Array(32).fill(2);
  const requesterPrivateKey = new Uint8Array(32).fill(3);
  const attackerPrivateKey = new Uint8Array(32).fill(4);
  const providerPublicKey = getPublicKey(providerPrivateKey);
  const requesterPublicKey = getPublicKey(requesterPrivateKey);
  const attackerPublicKey = getPublicKey(attackerPrivateKey);
  const session = "31".repeat(32);
  const innerKind = caseId === "wrapper-inner-kind-mismatch" ? 39606 : 39605;
  const innerPrivateKey =
    caseId === "wrapper-inner-signer-mismatch" ? attackerPrivateKey : providerPrivateKey;
  const counterpartyPublicKey =
    caseId === "wrapper-inner-recipient-mismatch" ? attackerPublicKey : requesterPublicKey;
  const kindSpecificTags =
    innerKind === 39606
      ? [
          ["p", providerPublicKey, "", "provider"],
          ["e", "42".repeat(32), "", "quote"],
        ]
      : [
          ["p", counterpartyPublicKey, "", "requester"],
          ["e", "42".repeat(32), "", "rfq"],
          ["expiration", "20"],
          ["quote", "firm"],
          ["reservation", "hard"],
        ];
  const inner = finalizeEvent(
    {
      created_at: 10,
      kind: innerKind,
      tags: [
        ["d", "32".repeat(32)],
        ["session", session],
        ["profile", "conformance", "1"],
        ["alt", "Wrapper binding fixture"],
        ...kindSpecificTags,
      ],
      content: JSON.stringify({
        schema: "openagents.mkt.v1",
        profile: "conformance",
        profile_version: 1,
        session_id: session,
      }),
    },
    innerPrivateKey,
    new Uint8Array(32),
  );
  const rumor = createRumor(
    {
      created_at: decodeUnixTimestamp(inner.created_at),
      kind: decodeEventKind(caseId === "wrapper-inner-kind-mismatch" ? 39605 : inner.kind),
      tags: [
        ["p", requesterPublicKey],
        ["d", "33".repeat(32)],
      ],
      content: serializeSignedEvent(inner),
    },
    providerPrivateKey,
  );
  const wrap = createWrap(
    createSeal(rumor, providerPrivateKey, requesterPublicKey),
    requesterPublicKey,
  );
  return Effect.flip(
    unwrapPrivateRecord(wrap, requesterPrivateKey, [{ id: "conformance", version: 1 }]),
  ).pipe(
    Effect.map((error): MktTransportCode => {
      if (!(error instanceof MktTransportError)) throw error;
      return error.code;
    }),
  );
}

describe("Immortal client-only corpus", () => {
  test("replays every client projection case", () =>
    // Vite Plus does not expose the Effect test extension in this workspace.
    // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests
    Effect.runPromise(
      Effect.gen(function* () {
        const replayed = new Set<string>();
        for (const clientCase of fixture.cases) {
          replayed.add(clientCase.id);
          switch (clientCase.id) {
            case "quote-supersession": {
              const inputs = clientCase.inputs as {
                records: readonly { id: string; previous?: string }[];
              };
              const expected = clientCase.expected as {
                active_quote_id: string;
                retain_ids: readonly string[];
              };
              const result = activeSupersedingQuote(inputs.records);
              expect(result).toEqual({
                decision: "supersede",
                activeQuoteId: expected.active_quote_id,
                retainIds: expected.retain_ids,
              });
              break;
            }
            case "double-reservation": {
              const inputs = clientCase.inputs as {
                capacity: number;
                quotes: readonly { id: string; reservation: string; units: number }[];
              };
              const expected = clientCase.expected as {
                decision: string;
                code: string;
                retain_ids: readonly string[];
                effective_reserved_units: number;
              };
              const result = reserveCapacity(inputs.capacity, inputs.quotes);
              expect(result).toEqual({
                decision: expected.decision,
                code: expected.code,
                retainIds: expected.retain_ids,
                effectiveReservedUnits: expected.effective_reserved_units,
              });
              break;
            }
            case "status-sequence-gap": {
              const inputs = clientCase.inputs as {
                order_id: string;
                author: string;
                statuses: readonly { id: string; seq: number; previous?: string }[];
              };
              const expected = clientCase.expected as {
                missing_sequences: readonly number[];
                last_contiguous_seq: number;
                retain_ids: readonly string[];
              };
              const statuses = inputs.statuses.map((status) =>
                status.previous === undefined
                  ? {
                      id: status.id,
                      seq: status.seq,
                      sessionId: fixture.session_id,
                      orderId: inputs.order_id,
                      author: inputs.author,
                    }
                  : {
                      id: status.id,
                      seq: status.seq,
                      previous: status.previous,
                      sessionId: fixture.session_id,
                      orderId: inputs.order_id,
                      author: inputs.author,
                    },
              );
              const result = analyzeStatusSequence(statuses);
              expect(result.decision).toBe("gap");
              if (result.decision === "gap") {
                expect(result.missingSequences).toEqual(expected.missing_sequences);
                expect(result.lastContiguousSeq).toBe(expected.last_contiguous_seq);
                expect(result.retainIds).toEqual(expected.retain_ids);
              }
              break;
            }
            case "status-sequence-fork": {
              const inputs = clientCase.inputs as {
                order_id: string;
                author: string;
                statuses: readonly { id: string; seq: number; state: string }[];
              };
              const expected = clientCase.expected as {
                fork_key: { order_id: string; author: string; seq: number };
                retain_ids: readonly string[];
                advance_state: false;
              };
              const statuses = inputs.statuses.map((status) => ({
                id: status.id,
                seq: status.seq,
                state: status.state,
                sessionId: fixture.session_id,
                orderId: inputs.order_id,
                author: inputs.author,
              }));
              const result = analyzeStatusSequence(statuses);
              expect(result.decision).toBe("fork");
              if (result.decision === "fork") {
                expect(result.retainIds).toEqual(expected.retain_ids);
                expect(result.advanceState).toBe(expected.advance_state);
                expect(result.forkKey).toEqual({
                  sessionId: fixture.session_id,
                  orderId: expected.fork_key.order_id,
                  author: expected.fork_key.author,
                  seq: expected.fork_key.seq,
                });
              }
              break;
            }
            case "wrapper-inner-signer-mismatch": {
              expect(yield* replayWrapperBinding(clientCase.id)).toBe(
                "wrapper_inner_signer_mismatch",
              );
              break;
            }
            case "wrapper-inner-kind-mismatch": {
              expect(yield* replayWrapperBinding(clientCase.id)).toBe(
                "wrapper_inner_kind_mismatch",
              );
              break;
            }
            case "wrapper-inner-recipient-mismatch": {
              expect(yield* replayWrapperBinding(clientCase.id)).toBe(
                "wrapper_inner_recipient_mismatch",
              );
              break;
            }
            case "evidence-mismatch": {
              const inputs = clientCase.inputs as {
                status: { evidence_id: string; claimed_rung: string; id: string };
                evidence: { id: string; subject_id: string; rung: string };
              };
              const expected = clientCase.expected as {
                decision: "mismatch";
                code: "evidence_subject_or_rung_mismatch";
                display_rung: string;
                advance_verified_state: false;
              };
              expect(
                evidenceDecision(
                  {
                    evidenceId: inputs.status.evidence_id,
                    subjectId: inputs.status.id,
                    claimedRung: inputs.status.claimed_rung,
                  },
                  {
                    id: inputs.evidence.id,
                    subjectId: inputs.evidence.subject_id,
                    rung: inputs.evidence.rung,
                  },
                ),
              ).toEqual({
                decision: expected.decision,
                code: expected.code,
                displayRung: expected.display_rung,
                advanceVerifiedState: expected.advance_verified_state,
              });
              break;
            }
            case "recovery-loss": {
              const inputs = clientCase.inputs as {
                local_ids: readonly string[];
                causal_ids: readonly string[];
              };
              const expected = clientCase.expected as {
                decision: "loss";
                code: "missing_causal_record";
                missing_ids: readonly string[];
                synthesize_history: false;
              };
              expect(recoveryDecision(inputs.local_ids, inputs.causal_ids)).toEqual({
                decision: expected.decision,
                code: expected.code,
                missingIds: expected.missing_ids,
                synthesizeHistory: expected.synthesize_history,
              });
              break;
            }
            case "settlement-overclaim": {
              const inputs = clientCase.inputs as {
                settlement_evidence: { rung: string; final: boolean };
              };
              const expected = clientCase.expected as {
                decision: "overclaim";
                code: "settlement_overclaim";
                display_rung: string;
                settled: false;
              };
              expect(settlementDecision(inputs.settlement_evidence)).toEqual({
                decision: expected.decision,
                code: expected.code,
                displayRung: expected.display_rung,
                settled: expected.settled,
              });
              break;
            }
            case "expired-order": {
              const inputs = clientCase.inputs as {
                observed_at: number;
                order: { expiration: number };
              };
              const expected = clientCase.expected as {
                decision: "reject";
                code: "expired";
                inclusive: true;
                perform_external_effect: false;
              };
              expect(expiryDecision(inputs.order.expiration, inputs.observed_at)).toEqual({
                decision: expected.decision,
                code: expected.code,
                inclusive: expected.inclusive,
                performExternalEffect: expected.perform_external_effect,
              });
              break;
            }
            case "unauthorized-status":
            case "unauthorized-cancel":
            case "unauthorized-close": {
              const inputs = clientCase.inputs as {
                record: { author: string; kind: number };
                allowed_authors: readonly string[];
              };
              const expected = clientCase.expected as Record<string, unknown>;
              const result = authorizationDecision(
                inputs.record.kind as 39607 | 39608 | 39609,
                inputs.record.author,
                inputs.allowed_authors,
              );
              expect(result.decision).toBe(expected.decision);
              expect(result.code).toBe(expected.code);
              if ("advance_state" in expected)
                expect(result.advanceState).toBe(expected.advance_state);
              if ("cancelled" in expected) expect(result.cancelled).toBe(expected.cancelled);
              if ("terminal" in expected) expect(result.terminal).toBe(expected.terminal);
              break;
            }
            case "rewrapped-inner-deduplication": {
              const inputs = clientCase.inputs as {
                inner: { id: string };
                deliveries: readonly { wrap_id: string }[];
              };
              const result = deliveryDeduplicationDecision(
                inputs.deliveries.map((delivery) => ({
                  event: inputs.inner,
                  wrapId: delivery.wrap_id,
                })),
              );
              const expected = clientCase.expected as {
                decision: "deduplicate";
                dedup_key: string;
                logical_records: number;
                retain_delivery_provenance: readonly string[];
                repeat_external_effect: false;
              };
              expect(result).toEqual({
                decision: expected.decision,
                dedupKey: expected.dedup_key,
                logicalRecords: expected.logical_records,
                retainDeliveryProvenance: expected.retain_delivery_provenance,
                repeatExternalEffect: expected.repeat_external_effect,
              });
              break;
            }
            default:
              throw new Error(`unhandled client fixture: ${clientCase.id}`);
          }
        }
        expect(replayed.size).toBe(15);
      }),
    ));
});
