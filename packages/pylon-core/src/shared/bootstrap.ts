import { mkdir, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { join } from "node:path"
import { homedir, platform } from "node:os"
import { PYLON_VERSION, type PylonVersion } from "./version.js"
import { detectWslHost } from "./wsl-host-detect.js"

export type SupportedPlatform = "darwin" | "linux"
export type BootstrapOptions = {
  registerOpenAgents: boolean
  setupMdkWallet: boolean
  pylonRef?: string
  displayName?: string
  resourceMode: string
  capabilityRefs: string[]
  json: boolean
}

export type BootstrapSummary = {
  packageName: "@openagentsinc/pylon"
  version: PylonVersion
  bin: "pylon"
  platform: {
    current: NodeJS.Platform
    supported: boolean
    supportedTargets: SupportedPlatform[]
    // True when the host is running under WSL. WSL reports `current === "linux"`,
    // so `supported` alone cannot reveal it; this is the public-safe WSL signal.
    wsl: boolean
    // The authoritative self-serve gate: the host is a proven target AND not WSL.
    // `supported` is the raw platform check; `inScope` is what the install path
    // must gate on so a WSL host (a `linux`-reporting but out-of-scope host) is
    // not silently treated as supported.
    inScope: boolean
  }
  paths: {
    home: string
    config: string
    cache: string
    releases: string
  }
  updatePolicy: {
    dashboardPolling: boolean
    channel: "github-releases"
    sourceBuildFallback: "disabled"
  }
  bootstrap: {
    registerOpenAgents: boolean
    setupMdkWallet: boolean
    pylonRef: string | null
    displayName: string | null
    resourceMode: string
    capabilityRefs: string[]
  }
}

const supportedTargets: SupportedPlatform[] = ["darwin", "linux"]
export const PYLON_DEFAULT_CAPABILITY_REFS = [] as const

const withDefaultCapabilityRefs = (refs: string[]) =>
  [...new Set([...refs, ...PYLON_DEFAULT_CAPABILITY_REFS])]

// The marker a candidate home must hold to count as a REAL node home: the
// NIP-06 identity seed. We never read or print the seed — we only test for the
// file's presence. (`identity.json` is the public projection; the seed file is
// what makes a home authoritative.)
const HOME_SEED_MARKER = "identity.mnemonic"

export type PylonHomeResolutionSource =
  | "explicit_pylon_home"
  | "discovered_openagents_pylon"
  | "discovered_dot_pylon"
  | "legacy_default"

export type PylonHomeResolution = {
  home: string
  config: string
  cache: string
  releases: string
  // Public-safe label describing WHY this home was selected (never the seed).
  source: PylonHomeResolutionSource
}

function homePathsFor(home: string, source: PylonHomeResolutionSource): PylonHomeResolution {
  return {
    home,
    config: join(home, "config.json"),
    cache: join(home, "cache"),
    releases: join(home, "cache", "releases"),
    source,
  }
}

function homeHasSeed(home: string): boolean {
  try {
    return existsSync(join(home, HOME_SEED_MARKER))
  } catch {
    return false
  }
}

// Auto-resolve the node home when PYLON_HOME is unset.
//
// Bug (the Orwell report on v1.0.x): with no PYLON_HOME, the CLI silently used
// `~/.pylon`, which on his machine was a SEEDLESS home → seedPresent:false →
// daemonOnline:false → balanceSats:null. His real node home was
// `~/.openagents/pylon` (the historical identity home the running node used),
// holding the seed + 5,672 sats.
//
// We discover the home that ACTUALLY has a seed instead of blindly defaulting:
//   1. an explicit PYLON_HOME always wins (never break overrides);
//   2. otherwise prefer `~/.openagents/pylon` when it holds a seed (the
//      historical-config identity home the live node uses);
//   3. then `~/.pylon` when IT holds a seed;
//   4. else fall back to `~/.openagents/pylon` (the same place
//      `resolveNostrIdentityPath` writes/reads a fresh seed for a bare unset
//      PYLON_HOME — keeps the home and the identity path consistent).
//
// `selectPylonHomeResolution` returns the public-safe `source` label so callers
// can log WHICH home was selected (path label, never the seed).
export function selectPylonHomeResolution(
  env: NodeJS.ProcessEnv = process.env,
  home: string = homedir(),
): PylonHomeResolution {
  const explicit = env.PYLON_HOME?.trim()
  if (explicit) {
    return homePathsFor(explicit, "explicit_pylon_home")
  }

  const openagentsPylon = join(home, ".openagents", "pylon")
  const dotPylon = join(home, ".pylon")

  // Prefer the seed-bearing home. `~/.openagents/pylon` is the historical
  // identity home a live node uses, so it wins ties over a bare `~/.pylon`.
  if (homeHasSeed(openagentsPylon)) {
    return homePathsFor(openagentsPylon, "discovered_openagents_pylon")
  }
  if (homeHasSeed(dotPylon)) {
    return homePathsFor(dotPylon, "discovered_dot_pylon")
  }

  // No seed anywhere yet (fresh machine): default to `~/.openagents/pylon` so a
  // brand-new node creates its seed in the SAME home `resolveNostrIdentityPath`
  // uses for the unset-PYLON_HOME legacy default. This keeps a fresh node's home
  // and its identity seed colocated instead of split across two directories.
  return homePathsFor(openagentsPylon, "legacy_default")
}

export function resolvePylonHome(env: NodeJS.ProcessEnv = process.env) {
  const resolution = selectPylonHomeResolution(env)
  return {
    home: resolution.home,
    config: resolution.config,
    cache: resolution.cache,
    releases: resolution.releases,
  }
}

export function isSupportedPlatform(current: NodeJS.Platform = platform()) {
  return supportedTargets.includes(current as SupportedPlatform)
}

export function parseBootstrapArgs(args: string[]): BootstrapOptions {
  const options: BootstrapOptions = {
    registerOpenAgents: false,
    setupMdkWallet: false,
    resourceMode: "background_20",
    capabilityRefs: [],
    json: false,
  }

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    const readValue = () => {
      const value = args[index + 1]
      if (!value || value.startsWith("--")) {
        throw new Error(`${arg} requires a value`)
      }
      index += 1
      return value
    }

    if (arg === "--register-openagents") {
      options.registerOpenAgents = true
    } else if (arg === "--setup-mdk-wallet") {
      options.setupMdkWallet = true
    } else if (arg === "--pylon-ref") {
      options.pylonRef = readValue()
    } else if (arg === "--display-name") {
      options.displayName = readValue()
    } else if (arg === "--resource-mode") {
      options.resourceMode = readValue()
    } else if (arg === "--capability-ref") {
      options.capabilityRefs.push(readValue())
    } else if (arg === "--json") {
      options.json = true
    } else if (arg === "--help" || arg === "-h") {
      continue
    } else {
      throw new Error(`Unknown bootstrap option: ${arg}`)
    }
  }

  return options
}

export function createBootstrapSummary(
  options: BootstrapOptions,
  env: NodeJS.ProcessEnv = process.env,
  currentPlatform: NodeJS.Platform = platform(),
): BootstrapSummary {
  const supported = isSupportedPlatform(currentPlatform)
  // WSL reports `platform === "linux"`. Detect it from public-safe env signals so
  // a WSL host is held out of scope rather than passing the `linux` supported
  // check. The WSL signal is only meaningful on linux.
  const wsl = currentPlatform === "linux" && detectWslHost(env)
  return {
    packageName: "@openagentsinc/pylon",
    version: PYLON_VERSION,
    bin: "pylon",
    platform: {
      current: currentPlatform,
      supported,
      supportedTargets,
      wsl,
      inScope: supported && !wsl,
    },
    paths: resolvePylonHome(env),
    updatePolicy: {
      dashboardPolling: true,
      channel: "github-releases",
      sourceBuildFallback: "disabled",
    },
    bootstrap: {
      registerOpenAgents: options.registerOpenAgents,
      setupMdkWallet: options.setupMdkWallet,
      pylonRef: options.pylonRef ?? null,
      displayName: options.displayName ?? null,
      resourceMode: options.resourceMode,
      capabilityRefs: withDefaultCapabilityRefs(options.capabilityRefs),
    },
  }
}

export async function writeBootstrapFiles(summary: BootstrapSummary) {
  await mkdir(summary.paths.cache, { recursive: true })
  await mkdir(summary.paths.releases, { recursive: true })
  await writeFile(
    summary.paths.config,
    `${JSON.stringify(
      {
        packageName: summary.packageName,
        version: summary.version,
        pylonRef: summary.bootstrap.pylonRef,
        displayName: summary.bootstrap.displayName,
        resourceMode: summary.bootstrap.resourceMode,
        capabilityRefs: summary.bootstrap.capabilityRefs,
      },
      null,
      2,
    )}\n`,
  )
}

export function formatBootstrapText(summary: BootstrapSummary) {
  const lines = [
    `Pylon ${summary.version} bootstrap summary`,
    `Package: ${summary.packageName}`,
    `Binary: ${summary.bin}`,
    `Platform: ${summary.platform.current} (${summary.platform.supported ? "supported" : "unsupported"})`,
    `Home: ${summary.paths.home}`,
    `Cache: ${summary.paths.cache}`,
    `Register OpenAgents: ${summary.bootstrap.registerOpenAgents ? "requested" : "not requested"}`,
    `Setup MDK wallet: ${summary.bootstrap.setupMdkWallet ? "requested" : "not requested"}`,
    `Resource mode: ${summary.bootstrap.resourceMode}`,
    `Source build fallback: ${summary.updatePolicy.sourceBuildFallback}`,
  ]

  if (summary.bootstrap.pylonRef) lines.push(`Pylon ref: ${summary.bootstrap.pylonRef}`)
  if (summary.bootstrap.displayName) lines.push(`Display name: ${summary.bootstrap.displayName}`)
  if (summary.bootstrap.capabilityRefs.length > 0) {
    lines.push(`Capability refs: ${summary.bootstrap.capabilityRefs.join(", ")}`)
  }

  return `${lines.join("\n")}\n`
}
