/**
 * NIP-90 provider lane refs, split out of `apps/pylon/src/provider-nip90.ts`
 * (issue #8578, PY-1 presence extraction).
 *
 * `presence.ts`'s heartbeat needs exactly these four symbols to publish
 * provider-discovery fields (`PYLON_NIP90_PROVIDER_CAPABILITY_REF`,
 * `providerNip90LaneRefs`, `relaysFromEnv`) and their supporting constant
 * (`OPENAGENTS_MARKET_RELAY_URL`). The rest of `provider-nip90.ts` — job
 * request/result handling, the labor-market bridge, and the Apple FM client
 * used to *serve* NIP-90 work — depends on `wallet.ts`, `labor-market.ts`,
 * and `@openagentsinc/pylon-runtime` and stays in `apps/pylon`. This module
 * depends only on the public `@openagentsinc/nip90` job-kind constants, so
 * it is a clean leaf for presence to sit on top of. `apps/pylon`'s
 * `provider-nip90.ts` re-exports these symbols (rather than redefining them)
 * so there is a single source of truth.
 */
import {
  KIND_JOB_LABOR_CODE_TASK,
  KIND_JOB_LABOR_DOCUMENT_WORK,
  KIND_JOB_LABOR_REVIEW,
  KIND_JOB_TEXT_GENERATION,
} from "@openagentsinc/nip90"

export const OPENAGENTS_MARKET_RELAY_URL = "wss://relay.openagents.com"
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
