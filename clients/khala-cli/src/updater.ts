import { changelogTeaser, firstWords } from "./changelog.js"
import { spawnProcess } from "./proc.js"

const KHALA_PACKAGE_NAME = "@openagentsinc/khala"
const NPM_REGISTRY_LATEST_URL = "https://registry.npmjs.org/@openagentsinc%2Fkhala/latest"

type SpawnInstall = (command: ReadonlyArray<string>) => { readonly exited: Promise<number> }

export interface KhalaAutoUpdateOptions {
  readonly currentVersion: string
  readonly env?: Record<string, string | undefined> | undefined
  readonly fetch?: typeof fetch | undefined
  readonly notify?: ((line: string) => void) | undefined
  readonly notifyMode?: "defer" | "immediate" | undefined
  readonly spawnInstall?: SpawnInstall | undefined
}

interface NpmLatestMetadata {
  readonly version?: unknown
  readonly readme?: unknown
}

export type KhalaAutoUpdateResult =
  | { readonly kind: "disabled" }
  | { readonly kind: "no-update"; readonly latestVersion: string }
  | { readonly kind: "installed"; readonly latestVersion: string; readonly summary: string }
  | { readonly kind: "failed" }

export interface KhalaAutoUpdateHandle {
  readonly done: Promise<KhalaAutoUpdateResult>
  readonly pendingNotificationCount: number
  readonly flushNotifications: () => number
}

export function startKhalaAutoUpdate(options: KhalaAutoUpdateOptions): KhalaAutoUpdateHandle {
  const pendingNotifications: string[] = []
  const emit = (line: string): void => {
    if (options.notifyMode === "defer") {
      pendingNotifications.push(line)
      return
    }
    options.notify?.(line)
  }
  const handle: KhalaAutoUpdateHandle = {
    done: runKhalaAutoUpdate(options).then(result => {
      if (result.kind !== "installed") return result
      emit(`update added - ${firstWords(result.summary)} - restart to apply`)
      return result
    }),
    get pendingNotificationCount() {
      return pendingNotifications.length
    },
    flushNotifications: () => {
      const count = pendingNotifications.length
      for (const line of pendingNotifications.splice(0)) {
        options.notify?.(line)
      }
      return count
    },
  }
  void handle.done.catch(() => {
    // runKhalaAutoUpdate is fail-soft, but keep the background task fire-and-forget
    // if a future implementation accidentally rejects.
  })
  return handle
}

export async function awaitSettledKhalaAutoUpdate(
  handle: KhalaAutoUpdateHandle,
  timeoutMs = 0,
): Promise<boolean> {
  let timeout: ReturnType<typeof setTimeout> | undefined
  try {
    return await Promise.race([
      handle.done.then(() => true),
      new Promise<boolean>(resolve => {
        timeout = setTimeout(() => resolve(false), timeoutMs)
      }),
    ])
  } finally {
    if (timeout !== undefined) clearTimeout(timeout)
  }
}

export async function runKhalaAutoUpdate(options: KhalaAutoUpdateOptions): Promise<KhalaAutoUpdateResult> {
  if (autoUpdateDisabled(options.env ?? {})) {
    return { kind: "disabled" }
  }

  try {
    const metadata = await fetchLatestMetadata(options.fetch ?? fetch)
    const latestVersion = typeof metadata.version === "string" ? metadata.version : undefined
    if (latestVersion === undefined) {
      return { kind: "failed" }
    }
    if (compareVersions(latestVersion, options.currentVersion) <= 0) {
      return { kind: "no-update", latestVersion }
    }

    const exitCode = await installLatestVersion(options.spawnInstall ?? spawnInstall)
    if (exitCode !== 0) {
      return { kind: "failed" }
    }

    const summary =
      changelogTeaser(latestVersion, typeof metadata.readme === "string" ? metadata.readme : undefined) ??
      `Khala ${latestVersion} is available.`
    return { kind: "installed", latestVersion, summary }
  } catch {
    return { kind: "failed" }
  }
}

export function compareVersions(left: string, right: string): number {
  const leftParts = parseVersion(left)
  const rightParts = parseVersion(right)
  for (let index = 0; index < Math.max(leftParts.length, rightParts.length); index += 1) {
    const leftPart = leftParts[index] ?? 0
    const rightPart = rightParts[index] ?? 0
    if (leftPart > rightPart) return 1
    if (leftPart < rightPart) return -1
  }
  return 0
}

function parseVersion(version: string): ReadonlyArray<number> {
  return version
    .split(/[.-]/)
    .slice(0, 3)
    .map(part => Number.parseInt(part, 10))
    .map(part => Number.isFinite(part) ? part : 0)
}

function autoUpdateDisabled(env: Record<string, string | undefined>): boolean {
  return env.KHALA_NO_AUTO_UPDATE === "1" || env.CI === "true"
}

async function fetchLatestMetadata(fetchImpl: typeof fetch): Promise<NpmLatestMetadata> {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), 1_500)
  try {
    const response = await fetchImpl(NPM_REGISTRY_LATEST_URL, {
      headers: { accept: "application/json" },
      signal: controller.signal,
    })
    if (!response.ok) {
      throw new Error(`npm registry returned HTTP ${response.status}`)
    }
    return await response.json() as NpmLatestMetadata
  } finally {
    clearTimeout(timeout)
  }
}

async function installLatestVersion(spawn: SpawnInstall): Promise<number> {
  const process = spawn(["npm", "i", "-g", `${KHALA_PACKAGE_NAME}@latest`])
  return await process.exited
}

const spawnInstall: SpawnInstall = command =>
  spawnProcess([...command], {
    stdin: "ignore",
    stdout: "ignore",
    stderr: "ignore",
  })
