import {
  fetchKhalaCodeDesktopUpdateFeedInfo,
  khalaCodeDesktopUpdaterCurrentPlatform,
  khalaCodeDesktopUpdaterDisabledLocalInfo,
  type KhalaCodeDesktopUpdaterLocalInfo,
} from "../shared/updater.js"
import type { KhalaCodeDesktopUpdaterBackend } from "./khala-code-updater-controller.js"

/**
 * Real Electrobun-backed updater backend for #8440. Only the subset of
 * `electrobun/bun`'s `Updater` namespace we actually call is typed here so
 * this module never needs to import Electrobun's native bindings directly —
 * `src/bun/index.ts` passes the already-imported `Updater` object in.
 */
export type KhalaCodeDesktopElectrobunUpdaterLike = {
  readonly applyUpdate: () => Promise<void>
  readonly downloadUpdate: () => Promise<void>
  readonly getLocalInfo: () => Promise<KhalaCodeDesktopUpdaterLocalInfo>
}

export function createKhalaCodeDesktopElectrobunUpdaterBackend(input: {
  readonly currentVersion: string
  readonly fetchImpl?: typeof fetch
  readonly updater: KhalaCodeDesktopElectrobunUpdaterLike
}): KhalaCodeDesktopUpdaterBackend {
  const localInfo = async (): Promise<KhalaCodeDesktopUpdaterLocalInfo> => {
    try {
      const info = await input.updater.getLocalInfo()
      if (info.baseUrl.length === 0) {
        return khalaCodeDesktopUpdaterDisabledLocalInfo(input.currentVersion)
      }
      return info
    } catch {
      return khalaCodeDesktopUpdaterDisabledLocalInfo(input.currentVersion)
    }
  }

  return {
    async checkForUpdates() {
      const info = await localInfo()
      const feed = await fetchKhalaCodeDesktopUpdateFeedInfo({
        ...(input.fetchImpl === undefined ? {} : { fetchImpl: input.fetchImpl }),
        localInfo: info,
        platform: khalaCodeDesktopUpdaterCurrentPlatform(),
      })
      return { error: feed.error, updateAvailable: feed.updateAvailable, version: feed.version }
    },
    async downloadUpdate() {
      // Electrobun's real Updater.downloadUpdate() re-derives the latest
      // hash from checkForUpdate() internally and stages the patched or
      // full tarball on disk; it throws on failure.
      try {
        await input.updater.downloadUpdate()
        return { ok: true }
      } catch (error) {
        return { error: error instanceof Error ? error.message : String(error), ok: false }
      }
    },
    async install() {
      // Electrobun's real Updater.applyUpdate() quits and relaunches the
      // app; it never resolves normally in a packaged build.
      await input.updater.applyUpdate()
    },
  }
}
