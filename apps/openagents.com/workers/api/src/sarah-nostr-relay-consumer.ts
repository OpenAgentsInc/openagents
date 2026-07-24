/**
 * SARAH-NR-05 — API wiring for the relay-primary Sarah turn consumer.
 *
 * Uses @openagentsinc/sarah relay consumer + injectable agent runner.
 * Default agent runner is a thin adapter over runSarahAgentTurn when an
 * inference adapter is provided; tests inject a stub.
 */
import { Effect } from 'effect'
import {
  SarahRelayTurnConsumer,
  createMemoryRelayPublisher,
  generateSarahNostrSigner,
  loadSarahNostrSignerFromSecretManagerMount,
  testSarahNostrCipher,
  type SarahNostrSigner,
  type SarahRelayAgentRunner,
  type SarahRelayPublisher,
  type SarahRelayTurnConsumerResult,
  type SarahTurnConversation,
} from '@openagentsinc/sarah'

import type { InferenceProviderAdapter } from './inference/provider-adapter'
import {
  runSarahAgentTurn,
  type SarahAgentTool,
  type SarahAgentToolActivity,
} from './sarah-agent-runtime'

export const SARAH_NOSTR_RELAY_PRIMARY_ENV = 'SARAH_NOSTR_RELAY_PRIMARY' as const

export type SarahNostrRelayConsumerDeps = Readonly<{
  readonly conversation: SarahTurnConversation
  readonly signer?: SarahNostrSigner
  readonly publish?: SarahRelayPublisher
  readonly runAgent?: SarahRelayAgentRunner
  /** When set with tools, builds default runAgent via runSarahAgentTurn. */
  readonly inference?: {
    readonly adapter: InferenceProviderAdapter
    readonly model: string
    readonly system: string
    readonly tools: ReadonlyArray<SarahAgentTool>
  }
}>

const mapToolActivity = (
  activity: SarahAgentToolActivity,
): {
  entry: 'tool.call' | 'tool.result' | 'tool.error'
  payload: Record<string, unknown>
} => {
  if (activity.phase === 'started') {
    return {
      entry: 'tool.call',
      payload: {
        toolName: activity.toolName,
        toolCallId: activity.toolCallId,
      },
    }
  }
  if (activity.phase === 'succeeded') {
    return {
      entry: 'tool.result',
      payload: {
        toolName: activity.toolName,
        toolCallId: activity.toolCallId,
        summary: activity.summary.slice(0, 500),
      },
    }
  }
  return {
    entry: 'tool.error',
    payload: {
      toolName: activity.toolName,
      toolCallId: activity.toolCallId,
      summary: activity.summary.slice(0, 500),
    },
  }
}

export const makeSarahRelayAgentRunner = (input: {
  readonly adapter: InferenceProviderAdapter
  readonly model: string
  readonly system: string
  readonly tools: ReadonlyArray<SarahAgentTool>
}): SarahRelayAgentRunner => {
  return async ({ prompt, onToolActivity }) => {
    try {
      const result = await Effect.runPromise(
        runSarahAgentTurn({
          adapter: input.adapter,
          model: input.model,
          system: input.system,
          prompt,
          tools: input.tools,
          onToolActivity: activity =>
            Effect.sync(() => {
              const mapped = mapToolActivity(activity)
              onToolActivity(mapped)
            }),
        }),
      )
      return {
        ok: true as const,
        text: result.text,
        usage: {
          totalTokens: result.usage.totalTokens,
          inputTokens: result.usage.promptTokens,
          outputTokens: result.usage.completionTokens,
        },
      }
    } catch (error) {
      return {
        ok: false as const,
        detail: error instanceof Error ? error.message : 'agent_failed',
      }
    }
  }
}

export const createSarahNostrRelayConsumer = (
  deps: SarahNostrRelayConsumerDeps,
): SarahRelayTurnConsumer => {
  const signer =
    deps.signer ??
    (() => {
      try {
        return loadSarahNostrSignerFromSecretManagerMount()
      } catch {
        return generateSarahNostrSigner()
      }
    })()

  const conversation: SarahTurnConversation = {
    ...deps.conversation,
    sarahPubkey: signer.getPublicKey(),
  }

  const runAgent =
    deps.runAgent ??
    (deps.inference !== undefined
      ? makeSarahRelayAgentRunner(deps.inference)
      : async () => ({
          ok: false as const,
          detail: 'no_agent_runner',
        }))

  const publish = deps.publish ?? createMemoryRelayPublisher().publish

  return new SarahRelayTurnConsumer(
    signer,
    testSarahNostrCipher(),
    conversation,
    runAgent,
    publish,
  )
}

export const isSarahNostrRelayPrimaryEnabled = (): boolean =>
  process.env[SARAH_NOSTR_RELAY_PRIMARY_ENV] === '1'

/** Convenience one-shot for cron/local smoke. */
export const handleSarahRelayOwnerMessage = async (input: {
  readonly deps: SarahNostrRelayConsumerDeps
  readonly turnRef: string
  readonly plaintext: string
  readonly promptEventId?: string
}): Promise<SarahRelayTurnConsumerResult> => {
  const consumer = createSarahNostrRelayConsumer(input.deps)
  return consumer.handleOwnerMessage({
    turnRef: input.turnRef,
    plaintext: input.plaintext,
    ...(input.promptEventId !== undefined
      ? { promptEventId: input.promptEventId }
      : {}),
  })
}
