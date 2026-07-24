import type {
  SarahNostrSignedEvent,
  SarahNostrSigner,
} from "../nostr-identity/types.ts";
import { assertSarahNostrPublicSafe } from "../nostr-identity/redaction.ts";
import { SarahTurnClaimStore } from "./claim.ts";
import {
  buildDurableTurnRecordTemplate,
  buildLiveAoFrameTemplate,
} from "./ladder.ts";
import type {
  SarahNostrCipher,
  SarahTurnConversation,
  SarahTurnEntry,
  SarahTurnParent,
} from "./types.ts";

export interface SarahNostrTurnPublishResult {
  readonly claimHeld: boolean;
  readonly durable?: SarahNostrSignedEvent;
  readonly live?: SarahNostrSignedEvent;
  readonly entry: SarahTurnEntry | "cancel_turn";
  readonly seq: number;
}

/**
 * Sarah turn service publish path (SARAH-NR-05 core).
 *
 * Reuses the sealed signer from SARAH-NR-04. Does not run the agent loop —
 * callers feed entry/payload after `runSarahAgentTurn` (or a test double).
 * Exact Cloud SQL metering stays outside this module (publish NIP-AM later,
 * never instead).
 */
export class SarahNostrTurnService {
  private readonly claims = new SarahTurnClaimStore();
  private seqByTurn = new Map<string, number>();

  constructor(
    private readonly signer: SarahNostrSigner,
    private readonly cipher: SarahNostrCipher,
    private readonly conversation: SarahTurnConversation,
  ) {
    if (conversation.sarahPubkey !== signer.getPublicKey()) {
      throw new Error(
        "sarah_nostr_turn: conversation.sarahPubkey must match signer",
      );
    }
  }

  /**
   * Claim the turn and publish durable `turn.started`.
   * Returns null if another claim already holds the turnRef.
   */
  startTurn(input: {
    readonly turnRef: string;
    readonly parents?: ReadonlyArray<SarahTurnParent>;
    readonly payload?: Record<string, unknown>;
  }): SarahNostrTurnPublishResult | null {
    const claim = this.claims.tryClaim({
      turnRef: input.turnRef,
      conversation: this.conversation.conversation,
    });
    if (!claim) {
      return null;
    }
    return this.publishDurable({
      entry: "turn.started",
      turnRef: input.turnRef,
      claimHeld: true,
      ...(input.parents !== undefined ? { parents: input.parents } : {}),
      ...(input.payload !== undefined ? { payload: input.payload } : {}),
    });
  }

  publishToolActivity(input: {
    readonly turnRef: string;
    readonly entry: "tool.call" | "tool.result" | "tool.error";
    readonly payload: Record<string, unknown>;
    readonly parents?: ReadonlyArray<SarahTurnParent>;
  }): SarahNostrTurnPublishResult {
    this.requireClaim(input.turnRef);
    const durable = this.publishDurable({
      entry: input.entry,
      turnRef: input.turnRef,
      payload: input.payload,
      claimHeld: true,
      ...(input.parents !== undefined ? { parents: input.parents } : {}),
    });
    const seq = durable.seq;
    const liveTemplate = buildLiveAoFrameTemplate({
      conversation: this.conversation,
      turnRef: input.turnRef,
      seq,
      frameType: input.entry,
      body: input.payload,
    });
    const live = this.signer.signEvent(liveTemplate);
    assertSarahNostrPublicSafe(live);
    return { ...durable, live };
  }

  finishTurn(input: {
    readonly turnRef: string;
    readonly entry: "turn.finished" | "turn.interrupted";
    readonly payload?: Record<string, unknown>;
    readonly parents?: ReadonlyArray<SarahTurnParent>;
  }): SarahNostrTurnPublishResult {
    this.requireClaim(input.turnRef);
    const result = this.publishDurable({
      entry: input.entry,
      turnRef: input.turnRef,
      claimHeld: true,
      ...(input.parents !== undefined ? { parents: input.parents } : {}),
      ...(input.payload !== undefined ? { payload: input.payload } : {}),
    });
    this.claims.release(input.turnRef);
    return result;
  }

  /** Interrupt control frame (NIP-AO cancel_turn). Does not finish the turn alone. */
  publishCancelTurn(turnRef: string): SarahNostrTurnPublishResult {
    this.requireClaim(turnRef);
    const seq = (this.seqByTurn.get(turnRef) ?? 0) + 1;
    this.seqByTurn.set(turnRef, seq);
    const live = this.signer.signEvent(
      buildLiveAoFrameTemplate({
        conversation: this.conversation,
        turnRef,
        seq,
        frameType: "cancel_turn",
        body: { action: "cancel_turn" },
      }),
    );
    assertSarahNostrPublicSafe(live);
    return {
      claimHeld: true,
      live,
      entry: "cancel_turn",
      seq,
    };
  }

  private requireClaim(turnRef: string): void {
    if (!this.claims.get(turnRef)) {
      throw new Error(`sarah_nostr_turn: turn not claimed: ${turnRef}`);
    }
  }

  private publishDurable(input: {
    readonly entry: SarahTurnEntry;
    readonly turnRef: string;
    readonly parents?: ReadonlyArray<SarahTurnParent>;
    readonly payload?: Record<string, unknown>;
    readonly claimHeld: boolean;
  }): SarahNostrTurnPublishResult {
    const seq = (this.seqByTurn.get(input.turnRef) ?? 0) + 1;
    this.seqByTurn.set(input.turnRef, seq);
    const { template } = buildDurableTurnRecordTemplate({
      conversation: this.conversation,
      entry: input.entry,
      turnRef: input.turnRef,
      seq,
      cipher: this.cipher,
      ...(input.parents !== undefined ? { parents: input.parents } : {}),
      ...(input.payload !== undefined ? { payload: input.payload } : {}),
    });
    const durable = this.signer.signEvent(template);
    assertSarahNostrPublicSafe(durable);
    // Wire content must not be plaintext JSON payload
    if (durable.content.includes('"schema":"openagents.sarah.turn_record.v1"')) {
      throw new Error("sarah_nostr_turn: durable content leaked plaintext");
    }
    return {
      claimHeld: input.claimHeld,
      durable,
      entry: input.entry,
      seq,
    };
  }
}

/** Test cipher: not NIP-44, but never leaves plaintext in wire content. */
export const testSarahNostrCipher = (): SarahNostrCipher => ({
  encryptToOwner: (plaintext: string) => {
    const bytes = new TextEncoder().encode(plaintext);
    let bin = "";
    for (const b of bytes) bin += String.fromCharCode(b);
    const b64 = btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    return `nip44:v2:test:${b64}`;
  },
});
