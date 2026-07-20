/**
 * `@openagentsinc/apple-fm-runtime` — neutral Apple Foundation Models provider
 * package (AFS-02, root export).
 *
 * The root export is portable and bundle-safe. It owns the frozen wire-version
 * source, the portable Apple FM wire contract, the loopback client, the
 * supervisor contract + neutral state machine, the Phase-1 JSON recommendation
 * decoder, and the inference provider adapter. It must not import Node. The
 * `./node` subpath owns helper discovery, signature/digest verification, spawn,
 * readiness, and shutdown, so a browser or mobile bundle cannot import the Node
 * host by accident. The `./testing` subpath owns fixtures and a fake transport.
 *
 * AFS-02 moved the portable wire schemas, client, capability probe, provider
 * adapter, supervisor contract, and receipt verification here from the nested
 * Pylon runtime and the Desktop app, and copied the Swift `foundation-bridge`
 * source into `native/foundation-bridge/`. Pylon keeps its Blueprint tools,
 * assignment receipts, fleet/wallet data, and CLI presentation; Desktop keeps
 * Electron IPC, `process.resourcesPath`, packaged-app staging, signing, ASAR,
 * and notarization.
 */
export const APPLE_FM_RUNTIME_PACKAGE = "@openagentsinc/apple-fm-runtime" as const;

export * from "./identity.js";
export * from "./wire.js";
export * from "./client.js";
export * from "./supervisor.js";
export * from "./recommendation.js";
export * from "./provider.js";
export * from "./ambient-task.js";
export * from "./ambient-task-corpus.js";
