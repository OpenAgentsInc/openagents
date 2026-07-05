import * as Device from "expo-device"

import {
  candidateTargets,
  resolveKhalaCodeConnectivity,
  type KhalaCodeConnectivityStatus
} from "./khala-code-connectivity-core"

export {
  KHALA_CODE_TAILNET_HEALTH_PORT,
  KHALA_CODE_TAILNET_CANDIDATE_HOSTS,
  candidateTargets,
  resolveKhalaCodeConnectivity,
  type FetchLike,
  type KhalaCodeConnectivityStatus
} from "./khala-code-connectivity-core"

/**
 * On the iOS Simulator (same machine as the desktop app), localhost is
 * reachable directly. On a physical device, the desktop is only reachable
 * over the Tailnet — via its MagicDNS hostname or Tailscale IP, on the
 * dedicated health-beacon port the desktop app runs (see
 * clients/khala-code-desktop/src/bun/index.ts startTailnetHealthBeacon).
 */
export const checkKhalaCodeConnectivity =
  async (): Promise<KhalaCodeConnectivityStatus> =>
    resolveKhalaCodeConnectivity(candidateTargets(Device.isDevice), fetch)
