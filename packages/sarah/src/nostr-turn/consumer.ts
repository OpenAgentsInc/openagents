/**
 * SARAH-NR-05 relay-primary turn consumer.
 *
 * Drives one owner message → claim → agent loop → durable/live ladder → answer.
 * Transport is injected (local Node relay or mock). Khala Sync is not required.
 */
import type {
  SarahNostrEventTemplate,
  SarahNostrSignedEvent,
  SarahNostrSigner,
} from "../nostr-identity/types.ts";
import { assertSarahNostrPublicSafe } from "../nostr-identity/redaction.ts";
import { SarahNostrTurnService } from "./service.ts";
import type {
  SarahNostrCipher,
  SarahTurnConversation,
  SarahTurnParent,
} from "./types.ts";

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
  readonly onToolActivity: (activity: {
    readonly entry: "tool.call" | "tool.result" | "tool.error";
    readonly payload: Record<string, unknown>;
  }) => void;
}) => Promise<SarahRelayAgentOutcome>;

/** Publishes signed events to a relay (or records them in tests). */
export type SarahRelayPublisher = (
  event: SarahNostrSignedEvent,
) => Promise<void> | void;

export interface SarahRelayTurnConsumerResult {
  readonly status: "answered" | "failed" | "skipped";
  readonly turnRef: string;
  readonly durableEvents: ReadonlyArray<SarahNostrSignedEvent>;
  readonly liveEvents: ReadonlyArray<SarahNostrSignedEvent>;
  readonly answerEvent?: SarahNostrSignedEvent;
  readonly usageMetric?: SarahNostrSignedEvent;
  readonly detail?: string;
}

export class SarahRelayTurnConsumer {
  private readonly service: SarahNostrTurnService;

  constructor(
    private readonly signer: SarahNostrSigner,
    cipher: SarahNostrCipher,
    private readonly conversation: SarahTurnConversation,
    private readonly runAgent: SarahRelayAgentRunner,
    private readonly publish: SarahRelayPublisher,
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
      await this.publish(started.durable);
    }

    const outcome = await this.runAgent({
      prompt: message.plaintext,
      turnRef: message.turnRef,
      onToolActivity: (activity) => {
        const published = this.service.publishToolActivity({
          turnRef: message.turnRef,
          entry: activity.entry,
          payload: activity.payload,
        });
        if (published.durable) {
          durableEvents.push(published.durable);
          void this.publish(published.durable);
        }
        if (published.live) {
          liveEvents.push(published.live);
          void this.publish(published.live);
        }
      },
    });

    if (!outcome.ok) {
      const finished = this.service.finishTurn({
        turnRef: message.turnRef,
        entry: "turn.interrupted",
        payload: { reason: outcome.detail },
      });
      if (finished.durable) {
        durableEvents.push(finished.durable);
        await this.publish(finished.durable);
      }
      return {
        status: "failed",
        turnRef: message.turnRef,
        durableEvents,
        liveEvents,
        detail: outcome.detail,
      };
    }

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
          await this.publish(published.durable);
        }
        if (published.live) {
          liveEvents.push(published.live);
          await this.publish(published.live);
        }
      }
    }

    // NIP-17 style answer: kind 14 rumor template (plaintext only in tests;
    // production wraps with NIP-44/NIP-59 before publish).
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
    const answerEvent = this.signer.signEvent(answerTemplate);
    assertSarahNostrPublicSafe({
      id: answerEvent.id,
      pubkey: answerEvent.pubkey,
      kind: answerEvent.kind,
      tags: answerEvent.tags,
      // content is owner-bound conversation; still redact secret field names
      contentLength: answerEvent.content.length,
    });
    await this.publish(answerEvent);

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
      await this.publish(usageMetric);
    }

    const finished = this.service.finishTurn({
      turnRef: message.turnRef,
      entry: "turn.finished",
      payload: { finishReason: "stop" },
    });
    if (finished.durable) {
      durableEvents.push(finished.durable);
      await this.publish(finished.durable);
    }

    return {
      status: "answered",
      turnRef: message.turnRef,
      durableEvents,
      liveEvents,
      answerEvent,
      ...(usageMetric !== undefined ? { usageMetric } : {}),
    };
  }
}

/** In-memory publisher for tests and local dogfood without a live relay. */
export const createMemoryRelayPublisher = (): {
  readonly publish: SarahRelayPublisher;
  readonly events: SarahNostrSignedEvent[];
} => {
  const events: SarahNostrSignedEvent[] = [];
  return {
    events,
    publish: (event) => {
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
