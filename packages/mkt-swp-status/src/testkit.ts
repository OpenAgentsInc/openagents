/**
 * Fixture builders for the status projection, including the deliberate gap
 * and fork corpora issue #9321 depends on until immortal#14 provides a live
 * counterparty stream.
 */
import type {
  CloseRecord,
  LossAccounting,
  ParticipantRole,
  StatusClaim,
  SwapEvidence,
} from "./model.js";
import { admittedSignerFor, HAPPY_PATH } from "./states.js";
import type { SwapFlow } from "./model.js";

export const TEST_SESSION_ID = "a".repeat(64);
export const TEST_ORDER_ID = "b".repeat(64);
export const REQUESTER_PUBKEY = "c".repeat(64);
export const PROVIDER_PUBKEY = "d".repeat(64);

export interface ClaimOverrides {
  readonly id?: string;
  readonly seq?: number;
  readonly previous?: string;
  readonly baseState?: string;
  readonly createdAt?: number;
  readonly sessionId?: string;
  readonly orderId?: string;
}

export function statusId(role: ParticipantRole, seq: number, variant = 0): string {
  const prefix = role === "requester" ? "1" : "2";
  return (prefix + String(seq).padStart(3, "0") + String(variant)).padEnd(64, "e");
}

export function claim(
  role: ParticipantRole,
  seq: number,
  swpState: string,
  overrides: ClaimOverrides = {},
): StatusClaim {
  const previous = overrides.previous ?? (seq === 0 ? undefined : statusId(role, seq - 1));
  return {
    id: overrides.id ?? statusId(role, seq),
    sessionId: overrides.sessionId ?? TEST_SESSION_ID,
    orderId: overrides.orderId ?? TEST_ORDER_ID,
    author: role === "requester" ? REQUESTER_PUBKEY : PROVIDER_PUBKEY,
    role,
    seq: overrides.seq ?? seq,
    ...(previous === undefined ? {} : { previous }),
    ...(overrides.baseState === undefined ? {} : { baseState: overrides.baseState }),
    swpState,
    createdAt: overrides.createdAt ?? 1_785_859_200 + seq,
  };
}

/**
 * A well-formed dual-lane happy-path status set for a flow: each state is
 * claimed by its admitted signer (observation states by the requester),
 * with per-lane sequences and previous references correctly chained.
 */
export function happyPathClaims(flow: SwapFlow, upToState?: string): StatusClaim[] {
  const claims: StatusClaim[] = [];
  const laneSeq: Record<ParticipantRole, number> = { requester: 0, provider: 0 };
  for (const state of HAPPY_PATH[flow]) {
    const admitted = admittedSignerFor(flow, state);
    const role: ParticipantRole = admitted === "either_observation" ? "requester" : admitted;
    claims.push(claim(role, laneSeq[role], state));
    laneSeq[role] += 1;
    if (state === upToState) break;
  }
  return claims;
}

export function evidence(overrides: Partial<SwapEvidence> = {}): SwapEvidence {
  return {
    class: "bitcoin_output",
    rung: "measured",
    authority: "bitcoin_adapter",
    reference: `${"f".repeat(64)}:0`,
    ...overrides,
  };
}

export const COMPLETE_LOSS_ACCOUNTING: LossAccounting = {
  input_asset_id: "bitcoin.mainnet.btc",
  output_asset_id: "lightning.mainnet.btc",
  input_committed: "100000",
  input_recovered: "0",
  output_received: "99000",
  provider_fee_paid: "600",
  miner_fee_paid: "300",
  lightning_routing_fee_paid: "100",
  guarantee_recovery_received: "0",
  principal_unresolved: "0",
  reservation_released: "100000",
};

export function close(
  role: ParticipantRole,
  outcome: CloseRecord["outcome"],
  overrides: { readonly [Key in keyof CloseRecord]?: CloseRecord[Key] | undefined } = {},
): CloseRecord {
  const merged = {
    id: `close-${role}-${outcome}`,
    author: role === "requester" ? REQUESTER_PUBKEY : PROVIDER_PUBKEY,
    role,
    outcome,
    lossAccounting: COMPLETE_LOSS_ACCOUNTING,
    ...overrides,
  };
  return Object.fromEntries(
    Object.entries(merged).filter(([, value]) => value !== undefined),
  ) as unknown as CloseRecord;
}

/** Deterministic shuffle for out-of-order arrival tests. */
export function shuffled<T>(items: readonly T[], seed: number): T[] {
  const result = items.slice();
  let state = seed >>> 0;
  for (let index = result.length - 1; index > 0; index -= 1) {
    state = (state * 1_664_525 + 1_013_904_223) >>> 0;
    const swap = state % (index + 1);
    const held = result[index]!;
    result[index] = result[swap]!;
    result[swap] = held;
  }
  return result;
}
