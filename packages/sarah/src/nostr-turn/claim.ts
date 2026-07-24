import type { SarahTurnClaim } from "./types.ts";

/**
 * Exactly-one claim discipline for Sarah turns on Nostr.
 *
 * The production path will make the claim *be* the durable `turn.started`
 * write (SARAH-NR-00), serialized by storage uniqueness on
 * `(turn_id, sequence)`. This in-memory table models that rule for the
 * turn-service unit surface and local dogfood.
 */
export class SarahTurnClaimStore {
  private readonly claims = new Map<string, SarahTurnClaim>();

  tryClaim(input: {
    readonly turnRef: string;
    readonly conversation: string;
    readonly claimEventId?: string;
    readonly nowMs?: number;
  }): SarahTurnClaim | null {
    if (this.claims.has(input.turnRef)) {
      return null;
    }
    const claim: SarahTurnClaim = {
      turnRef: input.turnRef,
      conversation: input.conversation,
      claimedAtMs: input.nowMs ?? Date.now(),
      ...(input.claimEventId !== undefined
        ? { claimEventId: input.claimEventId }
        : {}),
    };
    this.claims.set(input.turnRef, claim);
    return claim;
  }

  get(turnRef: string): SarahTurnClaim | undefined {
    return this.claims.get(turnRef);
  }

  release(turnRef: string): void {
    this.claims.delete(turnRef);
  }

  size(): number {
    return this.claims.size;
  }
}
