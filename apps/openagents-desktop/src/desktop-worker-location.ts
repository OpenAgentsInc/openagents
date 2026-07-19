import path from "node:path"
import { fileURLToPath, pathToFileURL } from "node:url"

export type DesktopWorkerBasename =
  | "codex-history-worker.js"
  | "language-utility-worker.js"
  | "workspace-search-worker.js"

/**
 * Resolve one package-owned worker entry. Electron's ASAR filesystem can read
 * an unpacked entry through its virtual `app.asar` path, but Node Worker must
 * receive the real `app.asar.unpacked` file URL on Electron 43.
 */
export const desktopWorkerUrl = (
  baseUrl: string,
  basename: DesktopWorkerBasename,
): URL => {
  const candidate = new URL(`./workers/${basename}`, baseUrl)
  if (candidate.protocol !== "file:") return candidate
  const virtualPath = fileURLToPath(candidate)
  const marker = `${path.sep}app.asar${path.sep}`
  const markerIndex = virtualPath.indexOf(marker)
  if (markerIndex < 0) return candidate
  return new URL(pathToFileURL(
    `${virtualPath.slice(0, markerIndex)}${path.sep}app.asar.unpacked${path.sep}${virtualPath.slice(markerIndex + marker.length)}`,
  ).href)
}
