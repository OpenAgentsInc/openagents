// Deep-link buffering coordinator (#8442). Cold launches can receive a
// `khala-code://` URL (via the OS `open-url` event or an argv URL) before the
// renderer has finished booting and can act on it; warm launches (a second
// instance being forwarded a link, or a `reopen`/`open-url` event on an
// already-running app) can route immediately. This module holds the pure
// state machine for that behavior so it is testable without Electrobun,
// BrowserWindow, or any real IPC transport.
//
// Parsing itself lives in `../shared/deep-links.ts` (the existing #8442
// parser already wired into the renderer's boot-time view resolution); this
// coordinator only adds the cold/warm buffering and OS-launch plumbing that
// was still missing around it.

import { parseKhalaCodeDeepLink, type KhalaCodeDeepLinkTarget } from "../shared/deep-links.js"

export type KhalaCodeDeepLinkCoordinatorEvent =
  | Readonly<{ type: "buffered"; raw: string }>
  | Readonly<{ type: "invalid"; raw: string; error: string }>
  | Readonly<{ type: "routed"; raw: string; target: KhalaCodeDeepLinkTarget }>

export type KhalaCodeDeepLinkCoordinator = Readonly<{
  /** Handle an incoming deep-link URL from any source (open-url, argv, a
   * forwarded second-instance link). Parses it, and either routes it
   * immediately (renderer ready) or buffers it (renderer not ready yet). */
  handleUrl: (raw: string) => void
  /** Mark the renderer ready and flush any buffered links, in arrival order. */
  markRendererReady: () => void
  /** True once `markRendererReady` has been called. */
  isRendererReady: () => boolean
  /** Number of links currently buffered, waiting on renderer readiness. */
  pendingCount: () => number
}>

export type KhalaCodeDeepLinkCoordinatorOptions = Readonly<{
  /** Called for every observable transition: buffered, routed, or rejected
   * as invalid. Never throws on invalid input -- invalid links are always
   * harmless no-ops that only ever reach this diagnostic callback. */
  onEvent?: (event: KhalaCodeDeepLinkCoordinatorEvent) => void
  /** Called once a link has been resolved to a typed target and the
   * renderer is (or becomes) ready to receive it. */
  onRoute: (target: KhalaCodeDeepLinkTarget, raw: string) => void
  /** Bound to avoid unbounded memory growth if a launch storm arrives before
   * the renderer is ready. Oldest links are dropped first.
   * @default 32 */
  maxBuffered?: number
}>

export const createKhalaCodeDeepLinkCoordinator = (
  options: KhalaCodeDeepLinkCoordinatorOptions,
): KhalaCodeDeepLinkCoordinator => {
  const maxBuffered = Math.max(1, Math.trunc(options.maxBuffered ?? 32))
  const buffered: Array<{ readonly raw: string; readonly target: KhalaCodeDeepLinkTarget }> = []
  let rendererReady = false

  const route = (raw: string, target: KhalaCodeDeepLinkTarget): void => {
    options.onEvent?.({ type: "routed", raw, target })
    options.onRoute(target, raw)
  }

  const handleUrl = (raw: string): void => {
    const parsed = parseKhalaCodeDeepLink(raw)
    if (!parsed.ok) {
      options.onEvent?.({ type: "invalid", raw, error: parsed.error })
      return
    }
    if (rendererReady) {
      route(raw, parsed.target)
      return
    }
    if (buffered.length >= maxBuffered) buffered.shift()
    buffered.push({ raw, target: parsed.target })
    options.onEvent?.({ type: "buffered", raw })
  }

  const markRendererReady = (): void => {
    if (rendererReady) return
    rendererReady = true
    const toFlush = buffered.splice(0, buffered.length)
    for (const entry of toFlush) route(entry.raw, entry.target)
  }

  return {
    handleUrl,
    isRendererReady: () => rendererReady,
    markRendererReady,
    pendingCount: () => buffered.length,
  }
}
