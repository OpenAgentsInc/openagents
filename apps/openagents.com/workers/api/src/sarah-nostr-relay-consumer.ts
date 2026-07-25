/**
 * SARAH-NR-05 — API wiring for the relay-primary Sarah turn consumer.
 *
 * Uses @openagentsinc/sarah relay consumer + injectable agent runner.
 * Default agent runner is a thin adapter over runSarahAgentTurn when an
 * inference adapter is provided; tests inject a stub.
 */
import { Effect } from "effect";
import {
  SarahRelayTurnConsumer,
  createWebSocketRelayPublisher,
  loadSealedSarahNostrStackFromSecretManagerMount,
  verifyOwnerAuthTag,
  type SarahNostrCipher,
  type SarahNostrSigner,
  type SarahOwnerAuthTag,
  type SarahRelayAgentRunner,
  type SarahRelayPublisher,
  type SarahRelayTurnConsumerResult,
  type SarahTurnConversation,
} from "@openagentsinc/sarah";

import type { InferenceProviderAdapter } from "./inference/provider-adapter";
import {
  runSarahAgentTurn,
  type SarahAgentTool,
  type SarahAgentToolActivity,
} from "./sarah-agent-runtime";

export const SARAH_NOSTR_RELAY_PRIMARY_ENV = "SARAH_NOSTR_RELAY_PRIMARY" as const;
export const SARAH_NOSTR_RELAY_URL_ENV = "SARAH_NOSTR_RELAY_URL" as const;
export const SARAH_NOSTR_OWNER_AUTH_TAG_ENV = "SARAH_NOSTR_OWNER_AUTH_TAG_JSON" as const;

export type SarahNostrRelayConsumerDeps = Readonly<{
  readonly conversation: SarahTurnConversation;
  readonly signer?: SarahNostrSigner;
  readonly cipher?: SarahNostrCipher;
  readonly publish?: SarahRelayPublisher;
  readonly runAgent?: SarahRelayAgentRunner;
  /** When set with tools, builds default runAgent via runSarahAgentTurn. */
  readonly inference?: {
    readonly adapter: InferenceProviderAdapter;
    readonly model: string;
    readonly system: string;
    readonly tools: ReadonlyArray<SarahAgentTool>;
  };
}>;

const mapToolActivity = (
  activity: SarahAgentToolActivity,
): {
  entry: "tool.call" | "tool.result" | "tool.error";
  payload: Record<string, unknown>;
} => {
  if (activity.phase === "started") {
    return {
      entry: "tool.call",
      payload: {
        toolName: activity.toolName,
        toolCallId: activity.toolCallId,
      },
    };
  }
  if (activity.phase === "succeeded") {
    return {
      entry: "tool.result",
      payload: {
        toolName: activity.toolName,
        toolCallId: activity.toolCallId,
        summary: activity.summary.slice(0, 500),
      },
    };
  }
  return {
    entry: "tool.error",
    payload: {
      toolName: activity.toolName,
      toolCallId: activity.toolCallId,
      summary: activity.summary.slice(0, 500),
    },
  };
};

export const makeSarahRelayAgentRunner = (input: {
  readonly adapter: InferenceProviderAdapter;
  readonly model: string;
  readonly system: string;
  readonly tools: ReadonlyArray<SarahAgentTool>;
}): SarahRelayAgentRunner => {
  return async ({ prompt, signal, onToolActivity }) => {
    try {
      const result = await Effect.runPromise(
        runSarahAgentTurn({
          adapter: input.adapter,
          model: input.model,
          system: input.system,
          prompt,
          tools: input.tools,
          onToolActivity: (activity) =>
            Effect.sync(() => {
              const mapped = mapToolActivity(activity);
              onToolActivity(mapped);
            }),
        }),
        { signal },
      );
      return {
        ok: true as const,
        text: result.text,
        usage: {
          totalTokens: result.usage.totalTokens,
          inputTokens: result.usage.promptTokens,
          outputTokens: result.usage.completionTokens,
        },
      };
    } catch (error) {
      return {
        ok: false as const,
        detail: error instanceof Error ? error.message : "agent_failed",
      };
    }
  };
};

export const createSarahNostrRelayConsumer = (
  deps: SarahNostrRelayConsumerDeps,
): SarahRelayTurnConsumer => {
  const hasAnyInjectedTransport =
    deps.signer !== undefined || deps.cipher !== undefined || deps.publish !== undefined;
  const hasCompleteInjectedTransport =
    deps.signer !== undefined && deps.cipher !== undefined && deps.publish !== undefined;
  if (hasAnyInjectedTransport && !hasCompleteInjectedTransport) {
    throw new Error("sarah_nostr_relay: tests must inject signer, cipher, and publisher together");
  }

  let signer: SarahNostrSigner;
  let cipher: SarahNostrCipher;
  let publish: SarahRelayPublisher;
  let closeTransport: () => Promise<void> = async () => undefined;
  if (deps.signer !== undefined && deps.cipher !== undefined && deps.publish !== undefined) {
    signer = deps.signer;
    cipher = deps.cipher;
    publish = deps.publish;
  } else {
    const stack = loadSealedSarahNostrStackFromSecretManagerMount({
      ownerPubkeyHex: deps.conversation.ownerPubkey,
      expectedSarahPubkeyHex: deps.conversation.sarahPubkey,
    });
    signer = stack.signer;
    cipher = stack.cipher;
    const relayUrl = requireRelayUrl();
    const ownerAuthTag = requireOwnerAuthTag(signer);
    const relay = createWebSocketRelayPublisher({
      url: relayUrl,
      signer,
      ownerAuthTag,
    });
    publish = relay.publish;
    closeTransport = relay.close;
  }

  if (deps.conversation.sarahPubkey !== signer.getPublicKey()) {
    throw new Error("sarah_nostr_relay: signer does not match admitted Sarah identity");
  }

  const runAgent =
    deps.runAgent ??
    (deps.inference !== undefined ? makeSarahRelayAgentRunner(deps.inference) : undefined);
  if (runAgent === undefined) {
    throw new Error("sarah_nostr_relay: missing admitted agent runner");
  }

  return new SarahRelayTurnConsumer(
    signer,
    cipher,
    deps.conversation,
    runAgent,
    publish,
    closeTransport,
  );
};

const requireRelayUrl = (): string => {
  const value = process.env[SARAH_NOSTR_RELAY_URL_ENV]?.trim();
  if (value === undefined || value === "") {
    throw new Error(`sarah_nostr_relay: missing ${SARAH_NOSTR_RELAY_URL_ENV}`);
  }
  const url = new URL(value);
  const localhost = url.hostname === "127.0.0.1" || url.hostname === "localhost";
  if (url.protocol !== "wss:" && !(localhost && url.protocol === "ws:")) {
    throw new Error("sarah_nostr_relay: relay URL must use wss");
  }
  return url.toString();
};

const requireOwnerAuthTag = (signer: SarahNostrSigner): SarahOwnerAuthTag => {
  const encoded = process.env[SARAH_NOSTR_OWNER_AUTH_TAG_ENV];
  if (encoded === undefined || encoded.trim() === "") {
    throw new Error(`sarah_nostr_relay: missing ${SARAH_NOSTR_OWNER_AUTH_TAG_ENV}`);
  }
  let value: unknown;
  try {
    value = JSON.parse(encoded) as unknown;
  } catch {
    throw new Error("sarah_nostr_relay: malformed owner auth tag");
  }
  if (
    !Array.isArray(value) ||
    value.length !== 4 ||
    value[0] !== "auth" ||
    typeof value[1] !== "string" ||
    typeof value[2] !== "string" ||
    typeof value[3] !== "string"
  ) {
    throw new Error("sarah_nostr_relay: malformed owner auth tag");
  }
  const authTag: SarahOwnerAuthTag = ["auth", value[1], value[2], value[3]];
  if (!verifyOwnerAuthTag(authTag, signer.getPublicKey())) {
    throw new Error("sarah_nostr_relay: owner auth tag is not admitted");
  }
  return authTag;
};

export const isSarahNostrRelayPrimaryEnabled = (): boolean =>
  process.env[SARAH_NOSTR_RELAY_PRIMARY_ENV] === "1";

/** Convenience one-shot for cron/local smoke. */
export const handleSarahRelayOwnerMessage = async (input: {
  readonly deps: SarahNostrRelayConsumerDeps;
  readonly turnRef: string;
  readonly plaintext: string;
  readonly promptEventId?: string;
}): Promise<SarahRelayTurnConsumerResult> => {
  const consumer = createSarahNostrRelayConsumer(input.deps);
  try {
    return await consumer.handleOwnerMessage({
      turnRef: input.turnRef,
      plaintext: input.plaintext,
      ...(input.promptEventId !== undefined ? { promptEventId: input.promptEventId } : {}),
    });
  } finally {
    await consumer.close();
  }
};
