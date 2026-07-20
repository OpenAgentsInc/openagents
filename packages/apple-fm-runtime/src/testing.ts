import { APPLE_FM_DEFAULT_MODEL_ID } from "./index.js";

/**
 * `@openagentsinc/apple-fm-runtime/testing` — fixtures and a fake transport
 * (AFS-00 reservation).
 *
 * Packet AFS-02 moves the wire fixtures and the fake transport here. AFS-00
 * reserves the subpath and seeds the health fixture that pins the drift value.
 */
export const APPLE_FM_RUNTIME_TESTING_RESERVED = true as const;

/**
 * The captured helper health fixture at the AFS-00 snapshot. The live bridge
 * fixtures assert `0.1.1` while the Swift source declares `0.1.3`. AFS-02
 * regenerates this from one source; AFS-00 records it as evidence of the drift.
 */
export const appleFmHealthFixtureAtSnapshot = {
  ready: true,
  model: APPLE_FM_DEFAULT_MODEL_ID,
  modelId: APPLE_FM_DEFAULT_MODEL_ID,
  version: "0.1.1",
} as const;
