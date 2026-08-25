/**
 * NIP-90 provider lane refs, split out of `apps/pylon/src/provider-nip90.ts`
 * (issue #8578, PY-1 presence extraction).
 *
 * `presence.ts`'s heartbeat needs exactly these four symbols to publish
 * provider-discovery fields (`PYLON_NIP90_PROVIDER_CAPABILITY_REF`,
 * `providerNip90LaneRefs`, `relaysFromEnv`) and their supporting constant
 * (`OPENAGENTS_MARKET_RELAY_URL`). This module depends only on the public
 * `@openagentsinc/nip90` job-kind constants, so it is a clean leaf for
 * presence to sit on top of.
 *
 * The serving half is gone. `apps/pylon/src/provider-nip90.ts`, which used
 * to re-export these symbols and handle job requests and results, was
 * deleted on 2026-07-14 by `21e82ce829` ("retire money sites and wallet
 * authority"), so this file is now the only definition. Nothing adds
 * `PYLON_NIP90_PROVIDER_CAPABILITY_REF` to a runtime's capability refs any
 * more, which means `providerDiscoveryFields` returns nothing in practice.
 * These refs are kept as the port source for issue #30. See
 * `apps/pylon/docs/nip90-provider-loop.md`.
 */
import {
  KIND_JOB_LABOR_CODE_TASK,
  KIND_JOB_LABOR_DOCUMENT_WORK,
  KIND_JOB_LABOR_REVIEW,
  KIND_JOB_TEXT_GENERATION,
} from "@openagentsinc/nip90"

// Public Nostr transport is external protocol infrastructure, not an owned
// OpenAgents service. Override explicitly for another public relay.
export const OPENAGENTS_MARKET_RELAY_URL = "wss://nos.lol"
export const PYLON_NIP90_PROVIDER_CAPABILITY_REF = "capability.public.pylon.nip90.text_inference.v0.3"

export function providerSupportedKinds() {
  return [
    KIND_JOB_TEXT_GENERATION,
    KIND_JOB_LABOR_CODE_TASK,
    KIND_JOB_LABOR_REVIEW,
    KIND_JOB_LABOR_DOCUMENT_WORK,
  ] as const
}

const providerNip90LaneLabels: Record<number, string> = {
  [KIND_JOB_TEXT_GENERATION]: "text_generation",
  [KIND_JOB_LABOR_CODE_TASK]: "labor_code_task",
  [KIND_JOB_LABOR_REVIEW]: "labor_review",
  [KIND_JOB_LABOR_DOCUMENT_WORK]: "labor_document_work",
}

// #4864: the public-safe lane refs this provider declares in registration
// and heartbeat presence writes, matching the NIP-90 kinds the provider
// loop subscribes to and announces via NIP-89 handler info.
export function providerNip90LaneRefs(): string[] {
  return providerSupportedKinds().map(
    (kind) => `lane.public.nip90.${kind}.${providerNip90LaneLabels[kind]}`,
  )
}

export function relaysFromEnv(env: NodeJS.ProcessEnv = process.env): string[] {
  const raw = env.PYLON_NIP90_RELAYS ?? env.OPENAGENTS_MARKET_RELAY_URL ?? OPENAGENTS_MARKET_RELAY_URL
  return raw.split(",").map((relay) => relay.trim()).filter(Boolean)
}
