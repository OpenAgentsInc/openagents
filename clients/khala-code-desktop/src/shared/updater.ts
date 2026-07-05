import {
  KHALA_CODE_DESKTOP_GITHUB_TAG_PREFIX,
  KHALA_CODE_DESKTOP_UPDATE_FEED_BASE_URL,
} from "./release-lane.js"

/**
 * Khala Code desktop in-app updater plumbing (#8440).
 *
 * Electrobun ships a built-in `Updater` (see `electrobun/bun`) that fetches a
 * flat `{channel}-{os}-{arch}-update.json` file from the app's configured
 * `release.baseUrl` (see `electrobun.config.ts`). This module defines the
 * shared, backend-agnostic types plus pure URL/state helpers that both the
 * bun-side controller (`src/bun/khala-code-updater-controller.ts`) and the
 * renderer (`src/ui/khala-code-updater-settings-section.ts`) rely on.
 *
 * Known gap: `src/shared/release-lane.ts` still exposes a
 * `updateFeedUrl`/`updateFeedBucketPrefix` shaped like
 * `{baseUrl}/{channel}/feed.json`, which does not match Electrobun's actual
 * flat `{baseUrl}/{channel}-{os}-{arch}-update.json` naming. This module
 * intentionally uses `KHALA_CODE_DESKTOP_UPDATE_FEED_BASE_URL` directly (the
 * same value as `electrobun.config.ts`'s `release.baseUrl`) rather than the
 * `feed.json` construct, since that is the contract Electrobun's CLI and
 * `Updater` actually implement. Reconciling `release-lane.ts` is out of scope
 * for #8440 (in-app updater plumbing only) and is left as a known follow-up.
 */

export const KHALA_CODE_DESKTOP_UPDATER_DEV_CHANNEL = "dev"

export type KhalaCodeDesktopUpdaterOs = "linux" | "macos" | "win"
export type KhalaCodeDesktopUpdaterArch = "arm64" | "x64"

export type KhalaCodeDesktopUpdaterPlatform = {
  readonly arch: KhalaCodeDesktopUpdaterArch
  readonly os: KhalaCodeDesktopUpdaterOs
}

/** Local build identity, mirroring Electrobun's `Updater.getLocalInfo()` shape. */
export type KhalaCodeDesktopUpdaterLocalInfo = {
  readonly baseUrl: string
  readonly channel: string
  readonly hash: string
  readonly identifier: string
  readonly name: string
  readonly version: string
}

export type KhalaCodeDesktopUpdaterFeedInfo = {
  readonly error: string
  readonly hash: string
  readonly updateAvailable: boolean
  readonly version: string
}

// The wire-facing `KhalaCodeDesktopUpdaterState` / `...Status` / `...ActionResult`
// types are defined once, alongside their Effect Schemas, in `./rpc.js`
// (matching the rest of this codebase's convention of deriving RPC projection
// types from their schema rather than duplicating a hand-rolled shape here).

/** Matches Electrobun's real `getPlatformPrefix(channel, os, arch)` convention. */
export const khalaCodeDesktopUpdaterPlatformPrefix = (
  channel: string,
  platform: KhalaCodeDesktopUpdaterPlatform,
): string => `${channel}-${platform.os}-${platform.arch}`

/** Matches Electrobun's real `getUpdateInfoUrl(baseUrl, platformPrefix)` convention. */
export const khalaCodeDesktopUpdaterFeedUrl = (
  baseUrl: string,
  channel: string,
  platform: KhalaCodeDesktopUpdaterPlatform,
): string => {
  const prefix = khalaCodeDesktopUpdaterPlatformPrefix(channel, platform)
  return `${baseUrl.replace(/\/+$/, "")}/${prefix}-update.json`
}

export const khalaCodeDesktopUpdaterReleaseNotesUrl = (version: string): string =>
  `https://github.com/OpenAgentsInc/openagents/releases/tag/${KHALA_CODE_DESKTOP_GITHUB_TAG_PREFIX}${version}`

export const khalaCodeDesktopUpdaterDisabledLocalInfo = (
  version: string,
): KhalaCodeDesktopUpdaterLocalInfo => ({
  baseUrl: KHALA_CODE_DESKTOP_UPDATE_FEED_BASE_URL,
  channel: KHALA_CODE_DESKTOP_UPDATER_DEV_CHANNEL,
  hash: "",
  identifier: "com.openagents.khala.code.desktop",
  name: "Khala Code",
  version,
})

export const khalaCodeDesktopUpdaterCurrentPlatform = (
  input: { readonly platform?: string; readonly arch?: string } = {
    platform: process.platform,
    arch: process.arch,
  },
): KhalaCodeDesktopUpdaterPlatform => {
  const os: KhalaCodeDesktopUpdaterOs =
    input.platform === "win32" ? "win" : input.platform === "linux" ? "linux" : "macos"
  const arch: KhalaCodeDesktopUpdaterArch = input.arch === "x64" ? "x64" : "arm64"
  return { arch, os }
}

/**
 * Fetch-based update feed client. Mirrors Electrobun's real
 * `Updater.checkForUpdate()` wire contract (cache-busted GET, hash
 * comparison, `dev` channel short-circuit, and the same error taxonomy) so a
 * fixture HTTP server can stand in for the real `updates.openagents.com`
 * feed in tests.
 */
export async function fetchKhalaCodeDesktopUpdateFeedInfo(input: {
  readonly fetchImpl?: typeof fetch
  readonly localInfo: KhalaCodeDesktopUpdaterLocalInfo
  readonly platform: KhalaCodeDesktopUpdaterPlatform
}): Promise<KhalaCodeDesktopUpdaterFeedInfo> {
  const { localInfo } = input
  if (localInfo.channel === KHALA_CODE_DESKTOP_UPDATER_DEV_CHANNEL) {
    return {
      error: "",
      hash: localInfo.hash,
      updateAvailable: false,
      version: localInfo.version,
    }
  }

  const fetchFn = input.fetchImpl ?? fetch
  const url = khalaCodeDesktopUpdaterFeedUrl(localInfo.baseUrl, localInfo.channel, input.platform)
  const cacheBuster = Math.random().toString(36).slice(2)

  let response: Response
  try {
    response = await fetchFn(`${url}?${cacheBuster}`)
  } catch {
    return { error: `Failed to fetch update info from ${url}`, hash: "", updateAvailable: false, version: "" }
  }

  if (!response.ok) {
    return {
      error: `Failed to fetch update info (HTTP ${response.status})`,
      hash: "",
      updateAvailable: false,
      version: "",
    }
  }

  const text = await response.text()
  let parsed: unknown
  try {
    parsed = JSON.parse(text)
  } catch {
    return {
      error: "Invalid update.json: failed to parse JSON",
      hash: "",
      updateAvailable: false,
      version: "",
    }
  }

  if (
    typeof parsed !== "object" ||
    parsed === null ||
    typeof (parsed as Record<string, unknown>).hash !== "string" ||
    (parsed as Record<string, unknown>).hash === ""
  ) {
    return {
      error: "Invalid update.json: missing hash",
      hash: "",
      updateAvailable: false,
      version: "",
    }
  }

  const feed = parsed as { readonly hash: string; readonly version?: unknown }
  const version = typeof feed.version === "string" ? feed.version : ""
  return {
    error: "",
    hash: feed.hash,
    updateAvailable: feed.hash !== localInfo.hash,
    version,
  }
}
