import type { ControlPlanePaywall } from "../src/contracts.js";

export const makePaywall = (
  paywallId: string,
  options?: {
    readonly ownerId?: string;
    readonly hostPattern?: string;
    readonly pathPattern?: string;
    readonly upstreamUrl?: string;
    readonly protocol?: "http" | "https";
    readonly priority?: number;
    readonly timeoutMs?: number;
    readonly fixedAmountMsats?: number;
    readonly status?: "active" | "paused" | "archived";
  },
): ControlPlanePaywall => ({
  paywallId,
  ownerId: options?.ownerId ?? "owner_1",
  name: `Paywall ${paywallId}`,
  status: options?.status ?? "active",
  createdAtMs: 1_730_000_000_000,
  updatedAtMs: 1_730_000_000_500,
  policy: {
    paywallId,
    ownerId: options?.ownerId ?? "owner_1",
    pricingMode: "fixed",
    fixedAmountMsats: options?.fixedAmountMsats ?? 2_000,
    maxPerRequestMsats: 5_000,
    allowedHosts: ["openagents.com"],
    blockedHosts: [],
    quotaPerMinute: 120,
    quotaPerDay: 10_000,
    killSwitch: false,
    createdAtMs: 1_730_000_000_000,
    updatedAtMs: 1_730_000_000_500,
  },
  routes: [
    {
      routeId: `route_${paywallId}`,
      paywallId,
      ownerId: options?.ownerId ?? "owner_1",
      hostPattern: options?.hostPattern ?? "openagents.com",
      pathPattern: options?.pathPattern ?? `/api/${paywallId}`,
      upstreamUrl: options?.upstreamUrl ?? `https://upstream.example.com/${paywallId}`,
      protocol: options?.protocol ?? "https",
      timeoutMs: options?.timeoutMs ?? 6_000,
      priority: options?.priority ?? 10,
      createdAtMs: 1_730_000_000_000,
      updatedAtMs: 1_730_000_000_500,
    },
  ],
});
