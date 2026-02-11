import { Clock, Effect, Layer, Ref } from "effect"

import type {
  GatewayCompiledRoute,
  GatewayConfigCompileResult,
  L402AuthorizationVerificationResult,
  L402ChallengeIssueRequest,
  L402ChallengeIssueResult,
  PaywallDefinition,
  PaywallStatus,
  SettlementRecord,
} from "../contracts/seller.js"
import { SellerPolicyViolationError } from "../errors/lightningErrors.js"
import { GatewayConfigCompilerService } from "../services/gatewayConfigCompiler.js"
import { InvoiceIssuerService } from "../services/invoiceIssuer.js"
import { PaywallRegistryService } from "../services/paywallRegistry.js"
import { SellerPolicyService } from "../services/sellerPolicy.js"
import { SettlementIngestService } from "../services/settlementIngest.js"

const normalizeHost = (host: string): string => host.trim().toLowerCase()

const deterministicHex = (input: string): string => {
  let acc = 0
  for (let i = 0; i < input.length; i += 1) {
    acc = (acc + input.charCodeAt(i) * (i + 37)) % 0xffffffff
  }

  const chunks: Array<string> = []
  for (let i = 0; i < 8; i += 1) {
    chunks.push(((acc + i * 2654435761) >>> 0).toString(16).padStart(8, "0"))
  }

  return chunks.join("").slice(0, 64)
}

const deterministicId = (prefix: string, input: string, size = 20): string =>
  `${prefix}_${deterministicHex(input).slice(0, size)}`

export const makePaywallRegistryInMemoryLayer = (input?: {
  readonly seedPaywalls?: ReadonlyArray<PaywallDefinition>
}) =>
  Layer.effect(
    PaywallRegistryService,
    Effect.gen(function* () {
      const seed = input?.seedPaywalls ?? []
      const initialEntries = seed.map((paywall) => [paywall.paywallId, paywall] as const)
      const storeRef = yield* Ref.make<ReadonlyMap<string, PaywallDefinition>>(new Map(initialEntries))

      const upsert = Effect.fn("PaywallRegistryInMemory.upsert")(function* (
        definition: PaywallDefinition,
      ) {
        const nowMs = yield* Clock.currentTimeMillis
        let nextDefinition = definition
        yield* Ref.update(storeRef, (store) => {
          const existing = store.get(definition.paywallId)
          nextDefinition = {
            ...definition,
            createdAtMs: existing?.createdAtMs ?? definition.createdAtMs,
            updatedAtMs: nowMs,
          }
          const next = new Map(store)
          next.set(definition.paywallId, nextDefinition)
          return next
        })
        return nextDefinition
      })

      const getById = (paywallId: string) =>
        Ref.get(storeRef).pipe(
          Effect.map((store) => store.get(paywallId) ?? null),
        )

      const listByOwner = (ownerId: string) =>
        Ref.get(storeRef).pipe(
          Effect.map((store) =>
            [...store.values()]
              .filter((paywall) => paywall.ownerId === ownerId)
              .sort((a, b) => a.createdAtMs - b.createdAtMs),
          ),
        )

      const setStatus = Effect.fn("PaywallRegistryInMemory.setStatus")(function* (
        paywallId: string,
        status: PaywallStatus,
      ) {
        const nowMs = yield* Clock.currentTimeMillis
        let updated: PaywallDefinition | null = null
        yield* Ref.update(storeRef, (store) => {
          const current = store.get(paywallId)
          if (!current) return store
          updated = { ...current, status, updatedAtMs: nowMs }
          const next = new Map(store)
          next.set(paywallId, updated)
          return next
        })
        return updated
      })

      return PaywallRegistryService.of({
        upsert,
        getById,
        listByOwner,
        setStatus,
      })
    }),
  )

const compileRoutes = (paywalls: ReadonlyArray<PaywallDefinition>): ReadonlyArray<GatewayCompiledRoute> =>
  [...paywalls]
    .sort((a, b) => {
      if (a.route.priority !== b.route.priority) return a.route.priority - b.route.priority
      if (a.route.hostPattern !== b.route.hostPattern) {
        return a.route.hostPattern.localeCompare(b.route.hostPattern)
      }
      if (a.route.pathPattern !== b.route.pathPattern) {
        return a.route.pathPattern.localeCompare(b.route.pathPattern)
      }
      return a.paywallId.localeCompare(b.paywallId)
    })
    .map((paywall) => ({
      paywallId: paywall.paywallId,
      ownerId: paywall.ownerId,
      hostPattern: paywall.route.hostPattern,
      pathPattern: paywall.route.pathPattern,
      upstreamUrl: paywall.route.upstreamUrl,
      priority: paywall.route.priority,
      status: paywall.status,
      priceMsats: paywall.policy.fixedAmountMsats ?? paywall.policy.maxAmountMsats ?? 1_000,
    }))

export const makeGatewayConfigCompilerDeterministicLayer = () =>
  Layer.succeed(
    GatewayConfigCompilerService,
    GatewayConfigCompilerService.of({
      compilePaywalls: (paywalls: ReadonlyArray<PaywallDefinition>) =>
        Effect.sync(() => {
          const routes = compileRoutes(paywalls)

          const diagnostics: Array<string> = []
          const keySet = new Set<string>()
          for (const route of routes) {
            const routeKey = `${route.hostPattern}::${route.pathPattern}`
            if (keySet.has(routeKey)) diagnostics.push(`duplicate_route:${routeKey}`)
            keySet.add(routeKey)
          }

          const compiledConfig = routes
            .map((route) =>
              [
                "route",
                route.paywallId,
                route.ownerId,
                route.hostPattern,
                route.pathPattern,
                route.upstreamUrl,
                String(route.priority),
                String(route.priceMsats),
                route.status,
              ].join("|"),
            )
            .join("\n")

          const result: GatewayConfigCompileResult = {
            configHash: deterministicId("cfg", compiledConfig || "empty", 24),
            compiledConfig,
            routes,
            diagnostics,
            valid: diagnostics.length === 0,
          }
          return result
        }),
    }),
  )

export const makeInvoiceIssuerDeterministicLayer = (options?: {
  readonly defaultAmountMsats?: number
  readonly issuer?: string
}) =>
  Layer.succeed(
    InvoiceIssuerService,
    InvoiceIssuerService.of({
      issueChallenge: (request: L402ChallengeIssueRequest, paywall: PaywallDefinition) =>
        Effect.gen(function* () {
          const issuedAtMs = yield* Clock.currentTimeMillis
          const amountMsats =
            request.quotedAmountMsats ??
            paywall.policy.fixedAmountMsats ??
            paywall.policy.maxAmountMsats ??
            Math.max(1, Math.floor(options?.defaultAmountMsats ?? 1_000))

          const seed = `${request.paywallId}:${request.requestId}:${request.host}:${request.path}:${amountMsats}`
          const challenge: L402ChallengeIssueResult = {
            paywallId: request.paywallId,
            requestId: request.requestId,
            status: "challenge_issued",
            challenge: {
              invoice: deterministicId("lnoa", seed, 28),
              macaroon: deterministicId("mac", seed, 24),
              amountMsats,
              issuer: options?.issuer ?? "openagents",
            },
            issuedAtMs,
          }
          return challenge
        }),
    }),
  )

export const makeSettlementIngestInMemoryLayer = (input?: {
  readonly seedSettlements?: ReadonlyArray<SettlementRecord>
}) =>
  Layer.effect(
    SettlementIngestService,
    Effect.gen(function* () {
      const seed = input?.seedSettlements ?? []
      const initialEntries = seed.map((record) => [record.settlementId, record] as const)
      const storeRef = yield* Ref.make<ReadonlyMap<string, SettlementRecord>>(new Map(initialEntries))

      const ingest = (record: SettlementRecord) =>
        Ref.modify(storeRef, (store) => {
          const existing = store.get(record.settlementId)
          if (existing) return [existing, store] as const
          const next = new Map(store)
          next.set(record.settlementId, record)
          return [record, next] as const
        })

      const getBySettlementId = (settlementId: string) =>
        Ref.get(storeRef).pipe(Effect.map((store) => store.get(settlementId) ?? null))

      const listByPaywall = (paywallId: string) =>
        Ref.get(storeRef).pipe(
          Effect.map((store) =>
            [...store.values()]
              .filter((record) => record.paywallId === paywallId)
              .sort((a, b) => a.createdAtMs - b.createdAtMs),
          ),
        )

      return SettlementIngestService.of({
        ingest,
        getBySettlementId,
        listByPaywall,
      })
    }),
  )

const deny = (paywallId: string, code: string, reason: string) =>
  SellerPolicyViolationError.make({ paywallId, code, reason })

export const makeSellerPolicyDeterministicLayer = () =>
  Layer.succeed(
    SellerPolicyService,
    SellerPolicyService.of({
      ensureChallengeAllowed: (paywall: PaywallDefinition, request: L402ChallengeIssueRequest) =>
        Effect.gen(function* () {
          if (paywall.status !== "active") {
            return yield* deny(paywall.paywallId, "paywall_inactive", "Paywall is not active")
          }

          if (paywall.policy.killSwitch) {
            return yield* deny(
              paywall.paywallId,
              "kill_switch_active",
              "Paywall kill switch is active",
            )
          }

          const host = normalizeHost(request.host)
          const blockedHosts = paywall.policy.blockedBuyerHosts.map(normalizeHost)
          if (blockedHosts.includes(host)) {
            return yield* deny(
              paywall.paywallId,
              "host_blocked",
              "Buyer host is blocked by policy",
            )
          }

          const allowedHosts = paywall.policy.allowedBuyerHosts.map(normalizeHost)
          if (allowedHosts.length > 0 && !allowedHosts.includes(host)) {
            return yield* deny(
              paywall.paywallId,
              "host_not_allowlisted",
              "Buyer host is not in allowlist",
            )
          }

          const effectiveMax =
            paywall.policy.maxAmountMsats ?? paywall.policy.fixedAmountMsats
          if (typeof effectiveMax === "number") {
            const quoted =
              request.quotedAmountMsats ?? paywall.policy.fixedAmountMsats ?? 0
            if (quoted > effectiveMax) {
              return yield* deny(
                paywall.paywallId,
                "amount_over_cap",
                "Quoted amount exceeds paywall max",
              )
            }
          }

          return yield* Effect.void
        }),
      ensureAuthorizationAllowed: (
        paywall: PaywallDefinition,
        verification: L402AuthorizationVerificationResult,
      ) =>
        Effect.gen(function* () {
          if (verification.status === "denied") {
            return yield* deny(
              paywall.paywallId,
              "authorization_denied",
              verification.denyReason ?? "Authorization denied",
            )
          }

          if (!verification.proofReference) {
            return yield* deny(
              paywall.paywallId,
              "missing_proof",
              "Authorization missing proof reference",
            )
          }

          return yield* Effect.void
        }),
    }),
  )

export const makeSellerDeterministicLayer = (input?: {
  readonly seedPaywalls?: ReadonlyArray<PaywallDefinition>
  readonly seedSettlements?: ReadonlyArray<SettlementRecord>
  readonly defaultAmountMsats?: number
  readonly issuer?: string
}) =>
  Layer.mergeAll(
    makePaywallRegistryInMemoryLayer(
      input?.seedPaywalls ? { seedPaywalls: input.seedPaywalls } : undefined,
    ),
    makeGatewayConfigCompilerDeterministicLayer(),
    makeInvoiceIssuerDeterministicLayer({
      ...(input?.defaultAmountMsats !== undefined
        ? { defaultAmountMsats: input.defaultAmountMsats }
        : {}),
      ...(input?.issuer !== undefined ? { issuer: input.issuer } : {}),
    }),
    makeSettlementIngestInMemoryLayer(
      input?.seedSettlements ? { seedSettlements: input.seedSettlements } : undefined,
    ),
    makeSellerPolicyDeterministicLayer(),
  )
