import { Effect, Layer } from "effect"

import type { PolicyCheckInput, SpendPolicy } from "../contracts/policy.js"
import { BudgetExceededError, DomainNotAllowedError } from "../errors/lightningErrors.js"
import { SpendPolicyService } from "../services/spendPolicy.js"

export const defaultSpendPolicy: SpendPolicy = {
  defaultMaxSpendMsats: 100_000,
  allowedHosts: [],
  blockedHosts: [],
}

const normalizeHost = (host: string): string => host.trim().toLowerCase()

export const makeSpendPolicyLayer = (input?: Partial<SpendPolicy>) => {
  const policy: SpendPolicy = {
    defaultMaxSpendMsats: input?.defaultMaxSpendMsats ?? defaultSpendPolicy.defaultMaxSpendMsats,
    allowedHosts: (input?.allowedHosts ?? defaultSpendPolicy.allowedHosts).map(normalizeHost),
    blockedHosts: (input?.blockedHosts ?? defaultSpendPolicy.blockedHosts).map(normalizeHost),
  }

  return Layer.succeed(
    SpendPolicyService,
    SpendPolicyService.of({
      policy,
      ensureRequestAllowed: (request: PolicyCheckInput) => {
        const host = normalizeHost(request.host)
        const isBlocked = policy.blockedHosts.includes(host)
        if (isBlocked) {
          return Effect.fail(
            DomainNotAllowedError.make({
              host,
              reasonCode: "host_blocked",
              reason: "Host is blocked by spend policy",
            }),
          )
        }

        const allowlistEnabled = policy.allowedHosts.length > 0
        if (allowlistEnabled && !policy.allowedHosts.includes(host)) {
          return Effect.fail(
            DomainNotAllowedError.make({
              host,
              reasonCode: "host_not_allowlisted",
              reason: "Host is not present in allowlist",
            }),
          )
        }

        const effectiveMax = Math.min(policy.defaultMaxSpendMsats, request.maxSpendMsats)
        if (request.quotedAmountMsats > effectiveMax) {
          return Effect.fail(
            BudgetExceededError.make({
              maxSpendMsats: effectiveMax,
              quotedAmountMsats: request.quotedAmountMsats,
              reasonCode: "amount_over_cap",
              reason: "Quoted invoice amount exceeds configured spend cap",
            }),
          )
        }

        return Effect.void
      },
    }),
  )
}
