/**
 * SARAH-NR-05 / SARAH-NR-08 — fail-soft dual-publish bridge from hosted Sarah
 * turns to Nostr.
 *
 * While Khala Sync remains the record authority (mode khala|shadow), this bridge
 * mirrors the ladder onto the Nostr turn service (durable kind 44300 + live
 * NIP-AO). Failures never affect the hosted turn outcome under shadow.
 *
 * Enable with env (SARAH-NR-08):
 *   SARAH_NOSTR_RECORD_MODE=khala|shadow|nostr
 * Legacy (SARAH-NR-05), still honored when RECORD_MODE is unset:
 *   SARAH_NOSTR_SHADOW_PUBLISH=1  → treated as mode=shadow
 * Also required:
 *   SARAH_NOSTR_IDENTITY_SECRET=<mounted Secret Manager value>
 *   SARAH_NOSTR_OWNER_PUBKEY=<64 hex owner npub-derived pubkey>
 *
 * Owner pubkey is public identity material for tags; it is not a secret.
 * Production default stays khala (or shadow when the legacy flag is set).
 * Actual prod cutover (mode=nostr) is operator-gated and not the package default.
 *
 * @see docs/omega/2026-07-24-sarah-nostr-cutover.md
 */
import {
  SARAH_NIP_AM_KIND,
  SarahNostrTurnService,
  conversationTagFromThreadRef,
  generateSarahNostrSigner,
  isSarahThreadRef,
  loadSarahNostrSignerFromSecretManagerMount,
  resolveSarahNostrRecordMode,
  shouldPublishSarahNostrFromMode,
  testSarahNostrCipher,
  type SarahNostrCipher,
  type SarahNostrRecordMode,
  type SarahNostrSigner,
  type SarahNostrSignedEvent,
  type SarahNostrTurnPublishResult,
  type SarahTurnConversation,
} from '@openagentsinc/sarah'

/** @deprecated Prefer SARAH_NOSTR_RECORD_MODE. Kept for SARAH-NR-05 deploys. */
export const SARAH_NOSTR_SHADOW_PUBLISH_ENV = 'SARAH_NOSTR_SHADOW_PUBLISH' as const
/** SARAH-NR-08 primary flag: khala | shadow | nostr */
export const SARAH_NOSTR_RECORD_MODE_ENV = 'SARAH_NOSTR_RECORD_MODE' as const
export const SARAH_NOSTR_OWNER_PUBKEY_ENV = 'SARAH_NOSTR_OWNER_PUBKEY' as const

export const isSarahOwnerThread = (threadId: string): boolean =>
  isSarahThreadRef(threadId)

/** Map legacy thread.sarah.<digest> → conversation tag sarah.<digest>. */
export const conversationTagFromSarahThread = (threadId: string): string | null =>
  conversationTagFromThreadRef(threadId)

/** Resolve publish enablement (RECORD_MODE or legacy SHADOW_PUBLISH). */
export const resolveSarahNostrBridgeMode = (
  env: NodeJS.ProcessEnv = process.env,
): SarahNostrRecordMode => resolveSarahNostrRecordMode(env)

export type SarahNostrBridgeLogFn = (
  event: string,
  fields: Record<string, unknown>,
) => void

export type SarahNostrTurnBridge = Readonly<{
  readonly enabled: true
  readonly conversation: SarahTurnConversation
  startTurn: (turnRef: string) => SarahNostrTurnPublishResult | null
  publishToolActivity: (input: {
    readonly turnRef: string
    readonly entry: 'tool.call' | 'tool.result' | 'tool.error'
    readonly payload: Record<string, unknown>
  }) => SarahNostrTurnPublishResult
  finishTurn: (input: {
    readonly turnRef: string
    readonly entry: 'turn.finished' | 'turn.interrupted'
    readonly payload?: Record<string, unknown>
  }) => SarahNostrTurnPublishResult
  /** Additive NIP-AM kind 44200 metric event (signed). Never replaces Cloud SQL. */
  publishUsageMetric: (input: {
    readonly turnRef: string
    readonly totalTokens: number
    readonly inputTokens: number
    readonly outputTokens: number
  }) => SarahNostrSignedEvent
}>

export const createSarahNostrTurnBridge = (input: {
  readonly signer: SarahNostrSigner
  readonly ownerPubkey: string
  readonly conversationTag: string
  readonly cipher?: SarahNostrCipher
}): SarahNostrTurnBridge => {
  if (!/^[0-9a-f]{64}$/.test(input.ownerPubkey)) {
    throw new Error('sarah_nostr_bridge: ownerPubkey must be 64 lowercase hex')
  }
  if (!/^sarah\.[0-9a-f]{24}$/.test(input.conversationTag)) {
    throw new Error('sarah_nostr_bridge: conversation tag form is sarah.<24 hex>')
  }
  const conversation: SarahTurnConversation = {
    ownerPubkey: input.ownerPubkey,
    sarahPubkey: input.signer.getPublicKey(),
    conversation: input.conversationTag,
  }
  const service = new SarahNostrTurnService(
    input.signer,
    input.cipher ?? testSarahNostrCipher(),
    conversation,
  )

  return {
    enabled: true,
    conversation,
    startTurn: turnRef => service.startTurn({ turnRef }),
    publishToolActivity: args => service.publishToolActivity(args),
    finishTurn: args =>
      service.finishTurn({
        turnRef: args.turnRef,
        entry: args.entry,
        ...(args.payload !== undefined ? { payload: args.payload } : {}),
      }),
    publishUsageMetric: args => {
      const template = {
        kind: SARAH_NIP_AM_KIND,
        created_at: Math.floor(Date.now() / 1000),
        tags: [
          ['p', conversation.ownerPubkey],
          ['conversation', conversation.conversation],
          ['turn', args.turnRef],
          ['metric', 'token_usage'],
          ['alt', 'OpenAgents Sarah usage metric (encrypted to owner later)'],
        ] as string[][],
        // Wire still public-safe: counts only, no prompts.
        content: JSON.stringify({
          schema: 'openagents.sarah.usage_metric.v1',
          turnRef: args.turnRef,
          totalTokens: args.totalTokens,
          inputTokens: args.inputTokens,
          outputTokens: args.outputTokens,
        }),
      }
      return input.signer.signEvent(template)
    },
  }
}

/**
 * Build a bridge from process env for Cloud Run / local dogfood.
 * Returns null when publish is off (mode khala) or config is incomplete.
 *
 * Modes (SARAH_NOSTR_RECORD_MODE, with legacy SARAH_NOSTR_SHADOW_PUBLISH):
 * - khala  — no bridge
 * - shadow — dual-publish; Khala remains record authority
 * - nostr  — publish; relay is record (cutover path; still fail-soft here so
 *            a bridge error never drops the hosted turn until a later packet
 *            makes Nostr-primary fail-closed)
 */
export const tryCreateSarahNostrTurnBridgeFromEnv = (input: {
  readonly threadId: string
  readonly log?: SarahNostrBridgeLogFn
}): SarahNostrTurnBridge | null => {
  const log = input.log ?? (() => undefined)
  const mode = resolveSarahNostrBridgeMode()
  if (!shouldPublishSarahNostrFromMode(mode)) {
    return null
  }
  const conversationTag = conversationTagFromSarahThread(input.threadId)
  if (conversationTag === null) {
    return null
  }
  const ownerPubkey = process.env[SARAH_NOSTR_OWNER_PUBKEY_ENV]?.trim().toLowerCase()
  if (ownerPubkey === undefined || !/^[0-9a-f]{64}$/.test(ownerPubkey)) {
    log('sarah_nostr_bridge_skipped', {
      reason: 'missing_or_invalid_owner_pubkey',
      threadId: input.threadId,
      mode,
    })
    return null
  }

  try {
    let signer: SarahNostrSigner
    try {
      signer = loadSarahNostrSignerFromSecretManagerMount()
    } catch {
      // Local/dev fallback: ephemeral signer so unit tests and dogfood can
      // exercise the path without Secret Manager. Production must mount the secret.
      if (process.env.NODE_ENV === 'production') {
        log('sarah_nostr_bridge_skipped', {
          reason: 'identity_secret_missing',
          threadId: input.threadId,
          mode,
        })
        return null
      }
      signer = generateSarahNostrSigner()
      log('sarah_nostr_bridge_ephemeral_signer', {
        threadId: input.threadId,
        pubkey: signer.getPublicKey(),
        mode,
      })
    }
    log('sarah_nostr_bridge_enabled', {
      threadId: input.threadId,
      mode,
      conversation: conversationTag,
    })
    return createSarahNostrTurnBridge({
      signer,
      ownerPubkey,
      conversationTag,
    })
  } catch (error) {
    log('sarah_nostr_bridge_init_failed', {
      detail: error instanceof Error ? error.message : 'unknown',
      threadId: input.threadId,
      mode,
    })
    return null
  }
}

/** Run a bridge action fail-soft; never throws. */
export const bridgeFailSoft = (
  log: SarahNostrBridgeLogFn,
  event: string,
  fields: Record<string, unknown>,
  action: () => void,
): void => {
  try {
    action()
  } catch (error) {
    log(event, {
      ...fields,
      detail: error instanceof Error ? error.message : 'unknown',
    })
  }
}
