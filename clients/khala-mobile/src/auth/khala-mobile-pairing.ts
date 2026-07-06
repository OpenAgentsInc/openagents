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
 * Diagnostic wrapper for the retired Tailnet pairing path. The mobile-only MVP
 * auth provider does not call this on cold start; GitHub PKCE is the default
 * signed-out path.
 */
export const discoverKhalaMobilePairing = async (): Promise<KhalaMobilePairingProbeOutcome> =>
  discoverKhalaMobilePairingCredentials(
    khalaMobilePairingTargets(Device.isDevice),
    fetch,
  )
