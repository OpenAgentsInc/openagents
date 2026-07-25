/**
 * SARAH-NR-05 relay-primary turn consumer.
 *
 * Drives one owner message → claim → agent loop → durable/live ladder → answer.
 * Transport is injected. Khala Sync is not required.
 */
import { LocalKeySigner } from "nostr-effect/identity";

import type {
  SarahNostrEventTemplate,
  SarahNostrSignedEvent,
  SarahNostrSigner,
} from "../nostr-identity/types.ts";
import { eventIdOf, generateSecretKeyBytes } from "../nostr-identity/crypto.ts";
import { assertSarahNostrPublicSafe } from "../nostr-identity/redaction.ts";
import { SarahNostrTurnService } from "./service.ts";
import type { SarahNostrCipher, SarahTurnConversation, SarahTurnParent } from "./types.ts";

const NIP59_MAX_TIMESTAMP_SKEW_SECONDS = 2 * 24 * 60 * 60;

const privateEnvelopeTimestamp = (createdAt: number): number => {
  const random = new Uint32Array(1);
  crypto.getRandomValues(random);
  const value = random[0];
  if (value === undefined) throw new Error("sarah_relay_consumer: random timestamp unavailable");
  return createdAt - (value % (Math.min(createdAt, NIP59_MAX_TIMESTAMP_SKEW_SECONDS) + 1));
};

class SarahRelayPublishUnavailable extends Error {}

/** Inbound owner message already decrypted to plaintext. */
export interface SarahRelayInboundMessage {
  readonly turnRef: string;
  readonly plaintext: string;
  /** Optional prompt event id for causal parent tag. */
  readonly promptEventId?: string;
}

export interface SarahRelayAgentResult {
  readonly ok: true;
  readonly text: string;
  readonly usage?: {
    readonly totalTokens: number;
    readonly inputTokens: number;
    readonly outputTokens: number;
  };
  readonly toolActivities?: ReadonlyArray<{
    readonly entry: "tool.call" | "tool.result" | "tool.error";
    readonly payload: Record<string, unknown>;
  }>;
}

export interface SarahRelayAgentFailure {
  readonly ok: false;
  readonly detail: string;
}

export type SarahRelayAgentOutcome = SarahRelayAgentResult | SarahRelayAgentFailure;

/** Runs the bounded agent loop. Inject runSarahAgentTurn wrapper in the API. */
export type SarahRelayAgentRunner = (input: {
  readonly prompt: string;
  readonly turnRef: string;
  readonly signal: AbortSignal;
  readonly onToolActivity: (activity: {
    readonly entry: "tool.call" | "tool.result" | "tool.error";
    readonly payload: Record<string, unknown>;
  }) => void;
}) => Promise<SarahRelayAgentOutcome>;

/** Publishes signed events to a relay (or records them in tests). */
export type SarahRelayPublisher = (event: SarahNostrSignedEvent) => Promise<void>;

export interface SarahRelayTurnConsumerResult {
  readonly status: "answered" | "failed" | "service_unavailable" | "skipped";
  readonly turnRef: string;
  readonly durableEvents: ReadonlyArray<SarahNostrSignedEvent>;
  readonly liveEvents: ReadonlyArray<SarahNostrSignedEvent>;
  readonly answerEvent?: SarahNostrSignedEvent;
  readonly usageMetric?: SarahNostrSignedEvent;
  readonly detail?: string;
}

export class SarahRelayTurnConsumer {
  private readonly service: SarahNostrTurnService;
  private readonly activeTurns = new Map<string, AbortController>();

  constructor(
    private readonly signer: SarahNostrSigner,
    private readonly cipher: SarahNostrCipher,
    private readonly conversation: SarahTurnConversation,
    private readonly runAgent: SarahRelayAgentRunner,
    private readonly publish: SarahRelayPublisher,
    private readonly closeTransport: () => Promise<void> = async () => undefined,
  ) {
    if (conversation.sarahPubkey !== signer.getPublicKey()) {
      throw new Error("sarah_relay_consumer: sarahPubkey must match signer");
    }
    this.service = new SarahNostrTurnService(signer, cipher, conversation);
  }

  /**
   * Process one inbound owner message end-to-end.
   * Claim is exactly-one on turnRef; duplicate returns skipped.
   */
  async handleOwnerMessage(
    message: SarahRelayInboundMessage,
  ): Promise<SarahRelayTurnConsumerResult> {
    const durableEvents: SarahNostrSignedEvent[] = [];
    const liveEvents: SarahNostrSignedEvent[] = [];

    const parents: SarahTurnParent[] | undefined =
      message.promptEventId !== undefined
        ? [{ eventId: message.promptEventId, marker: "prompt" }]
        : undefined;

    const started = this.service.startTurn({
      turnRef: message.turnRef,
      ...(parents !== undefined ? { parents } : {}),
      payload: { source: "relay_primary" },
    });
    if (started === null) {
      return {
        status: "skipped",
        turnRef: message.turnRef,
        durableEvents,
        liveEvents,
        detail: "claim_held",
      };
    }
    if (started.durable) {
      durableEvents.push(started.durable);
      try {
        await this.publish(started.durable);
      } catch {
        this.service.abandonUnconfirmedTurn(message.turnRef);
        return {
          status: "service_unavailable",
          turnRef: message.turnRef,
          durableEvents,
          liveEvents,
          detail: "relay_publish_unavailable",
        };
      }
    }

    const abortController = new AbortController();
    this.activeTurns.set(message.turnRef, abortController);
    let activityPublish = Promise.resolve();
    let outcome: SarahRelayAgentOutcome;
    try {
      outcome = await this.runAgent({
        prompt: message.plaintext,
        turnRef: message.turnRef,
        signal: abortController.signal,
        onToolActivity: (activity) => {
          const published = this.service.publishToolActivity({
            turnRef: message.turnRef,
            entry: activity.entry,
            payload: activity.payload,
          });
          if (published.durable) {
            const durable = published.durable;
            durableEvents.push(durable);
            activityPublish = activityPublish.then(() => this.publishRequired(durable));
          }
          if (published.live) {
            const live = published.live;
            liveEvents.push(live);
            activityPublish = activityPublish.then(() => this.publishRequired(live));
          }
        },
      });
    } catch (error) {
      outcome = {
        ok: false,
        detail: error instanceof Error ? error.message : "agent_failed",
      };
    }

    try {
      await activityPublish;
    } catch {
      this.activeTurns.delete(message.turnRef);
      return {
        status: "service_unavailable",
        turnRef: message.turnRef,
        durableEvents,
        liveEvents,
        detail: "relay_publish_unavailable",
      };
    }

    if (!outcome.ok) {
      const finished = this.service.finishTurn({
        turnRef: message.turnRef,
        entry: "turn.interrupted",
        payload: { reason: outcome.detail },
      });
      if (finished.durable) {
        durableEvents.push(finished.durable);
        try {
          await this.publish(finished.durable);
        } catch {
          this.activeTurns.delete(message.turnRef);
          return {
            status: "service_unavailable",
            turnRef: message.turnRef,
            durableEvents,
            liveEvents,
            detail: "relay_publish_unavailable",
          };
        }
      }
      this.activeTurns.delete(message.turnRef);
      return {
        status: "failed",
        turnRef: message.turnRef,
        durableEvents,
        liveEvents,
        detail: outcome.detail,
      };
    }

    try {
      // Optional pre-recorded tool activities (when runner batches them)
      if (outcome.toolActivities !== undefined) {
        for (const activity of outcome.toolActivities) {
          const published = this.service.publishToolActivity({
            turnRef: message.turnRef,
            entry: activity.entry,
            payload: activity.payload,
          });
          if (published.durable) {
            durableEvents.push(published.durable);
            await this.publishRequired(published.durable);
          }
          if (published.live) {
            liveEvents.push(published.live);
            await this.publishRequired(published.live);
          }
        }
      }

      // The kind-14 NIP-17 rumor is never published directly. It is sealed with
      // Sarah's key and then gift-wrapped by an unlinkable ephemeral key.
      const answerTemplate: SarahNostrEventTemplate = {
        kind: 14,
        created_at: Math.floor(Date.now() / 1000),
        tags: [
          ["p", this.conversation.ownerPubkey],
          ["conversation", this.conversation.conversation],
          ["turn", message.turnRef],
          ["alt", "OpenAgents Sarah answer"],
        ],
        content: outcome.text,
      };
      const rumor = {
        id: eventIdOf(this.signer.getPublicKey(), answerTemplate),
        pubkey: this.signer.getPublicKey(),
        ...answerTemplate,
      };
      const seal = this.signer.signEvent({
        kind: 13,
        created_at: privateEnvelopeTimestamp(answerTemplate.created_at),
        tags: [],
        content: this.cipher.encryptToOwner(JSON.stringify(rumor)),
      });
      const answerEvent = await this.createPrivateAnswerGiftWrap(answerTemplate, seal);
      assertSarahNostrPublicSafe({
        id: answerEvent.id,
        pubkey: answerEvent.pubkey,
        kind: answerEvent.kind,
        tags: answerEvent.tags,
        contentLength: answerEvent.content.length,
      });
      await this.publishRequired(answerEvent);

      let usageMetric: SarahNostrSignedEvent | undefined;
      if (outcome.usage !== undefined) {
        usageMetric = this.signer.signEvent({
          kind: 44200,
          created_at: Math.floor(Date.now() / 1000),
          tags: [
            ["p", this.conversation.ownerPubkey],
            ["conversation", this.conversation.conversation],
            ["turn", message.turnRef],
            ["metric", "token_usage"],
            ["alt", "OpenAgents Sarah usage metric"],
          ],
          content: JSON.stringify({
            schema: "openagents.sarah.usage_metric.v1",
            turnRef: message.turnRef,
            ...outcome.usage,
          }),
        });
        await this.publishRequired(usageMetric);
      }

      const finished = this.service.finishTurn({
        turnRef: message.turnRef,
        entry: "turn.finished",
        payload: { finishReason: "stop" },
      });
      if (finished.durable) {
        durableEvents.push(finished.durable);
        await this.publishRequired(finished.durable);
      }

      this.activeTurns.delete(message.turnRef);
      return {
        status: "answered",
        turnRef: message.turnRef,
        durableEvents,
        liveEvents,
        answerEvent,
        ...(usageMetric !== undefined ? { usageMetric } : {}),
      };
    } catch (error) {
      if (!(error instanceof SarahRelayPublishUnavailable)) throw error;
      this.activeTurns.delete(message.turnRef);
      return {
        status: "service_unavailable",
        turnRef: message.turnRef,
        durableEvents,
        liveEvents,
        detail: "relay_publish_unavailable",
      };
    }
  }

  private async createPrivateAnswerGiftWrap(
    answerTemplate: SarahNostrEventTemplate,
    seal: SarahNostrSignedEvent,
  ): Promise<SarahNostrSignedEvent> {
    const ephemeralSecret = generateSecretKeyBytes();
    let ephemeralSigner: LocalKeySigner | null = null;
    try {
      ephemeralSigner = LocalKeySigner.fromPrivateKey(ephemeralSecret);
      ephemeralSecret.fill(0);
      const giftWrapContent = await ephemeralSigner.nip44Encrypt(
        this.conversation.ownerPubkey,
        JSON.stringify(seal),
      );
      return await ephemeralSigner.signEvent({
        kind: 1059,
        created_at: privateEnvelopeTimestamp(answerTemplate.created_at),
        tags: [["p", this.conversation.ownerPubkey]],
        content: giftWrapContent,
      });
    } finally {
      ephemeralSecret.fill(0);
      ephemeralSigner?.dispose();
    }
  }

  private async publishRequired(event: SarahNostrSignedEvent): Promise<void> {
    try {
      await this.publish(event);
    } catch {
      throw new SarahRelayPublishUnavailable("relay publication unavailable");
    }
  }

  /** Request interruption. The caller must still wait for the terminal record. */
  async interrupt(turnRef: string): Promise<"pending" | "not_running"> {
    const active = this.activeTurns.get(turnRef);
    if (active === undefined) return "not_running";
    active.abort();
    const cancellation = this.service.publishCancelTurn(turnRef);
    if (cancellation.live !== undefined) await this.publish(cancellation.live);
    return "pending";
  }

  async close(): Promise<void> {
    for (const controller of this.activeTurns.values()) controller.abort();
    this.activeTurns.clear();
    await this.closeTransport();
  }
}

/** In-memory publisher for deterministic tests only. */
export const createMemoryRelayPublisher = (): {
  readonly publish: SarahRelayPublisher;
  readonly events: SarahNostrSignedEvent[];
} => {
  const events: SarahNostrSignedEvent[] = [];
  return {
    events,
    publish: async (event) => {
      assertSarahNostrPublicSafe({
        id: event.id,
        pubkey: event.pubkey,
        kind: event.kind,
        tags: event.tags,
      });
      events.push(event);
    },
  };
};
