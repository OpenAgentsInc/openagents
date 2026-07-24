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
  /** Terminal turnRefs must never be re-claimed (exactly-once answers). */
  private readonly terminal = new Set<string>();

  tryClaim(input: {
    readonly turnRef: string;
    readonly conversation: string;
    readonly claimEventId?: string;
    readonly nowMs?: number;
  }): SarahTurnClaim | null {
    if (this.claims.has(input.turnRef) || this.terminal.has(input.turnRef)) {
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

  /** Mark turn terminal and drop the active claim. Ref cannot be claimed again. */
  complete(turnRef: string): void {
    this.claims.delete(turnRef);
    this.terminal.add(turnRef);
  }

  /** @deprecated Prefer complete() so terminal turns stay unreclaimable. */
  release(turnRef: string): void {
    this.complete(turnRef);
  }

  size(): number {
    return this.claims.size;
  }
}
