import * as Device from "expo-device"

import {
  discoverKhalaMobilePairingCredentials,
  khalaMobilePairingTargets,
  KHALA_MOBILE_PAIRING_PATH,
  type KhalaMobilePairingCredentials,
  type KhalaMobilePairingProbeOutcome,
} from "./khala-mobile-pairing-core"

export {
  KHALA_MOBILE_PAIRING_PATH,
  type KhalaMobilePairingCredentials,
  type KhalaMobilePairingProbeOutcome,
} from "./khala-mobile-pairing-core"

/**
 * On the iOS Simulator (same machine as the desktop app), localhost is
 * reachable directly. On a physical device, the desktop is only reachable
 * over the Tailnet — via its MagicDNS hostname, on the same health-beacon
 * port the desktop app's connectivity dot already probes (see
 * clients/khala-code-desktop/src/bun/index.ts startTailnetHealthBeacon /
 * tailnetMobilePairingFetch).
 */
export const discoverKhalaMobilePairing = async (): Promise<KhalaMobilePairingProbeOutcome> =>
  discoverKhalaMobilePairingCredentials(
    khalaMobilePairingTargets(Device.isDevice),
    fetch,
  )
