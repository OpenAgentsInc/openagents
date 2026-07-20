/**
 * Pylon compatibility surface for the neutral Apple FM runtime (AFS-02, #9080).
 *
 * AFS-02 extracted the portable Apple FM wire contract, loopback client,
 * supervisor contract, Phase-1 recommendation decoder, and inference provider
 * adapter into the neutral `@openagentsinc/apple-fm-runtime` package, so Desktop
 * no longer depends on this nested Pylon runtime for the Apple FM
 * implementation. Pylon becomes a CONSUMER of the same neutral package through
 * this thin compat re-export.
 *
 * This is additive: Pylon's existing Apple FM backend (Blueprint tools,
 * assignment receipts, fleet/wallet data, workspace tools, CLI presentation,
 * and the deeper client) is unchanged and stays in Pylon. The neutral surface
 * is re-exported under a namespace so it never collides with Pylon's existing
 * `contract`/`wire`/`client` exports. New Pylon code that needs the shared,
 * cross-surface Apple FM contract or client should import it from here.
 */
export * as AppleFmNeutralRuntime from "@openagentsinc/apple-fm-runtime";
