import { mkdir, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { homedir, platform } from "node:os"
import { TASSADAR_EXECUTOR_CAPABILITY_REF } from "@openagentsinc/tassadar-executor"
import { PYLON_VERSION, type PylonVersion } from "./version"

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
export const PYLON_DEFAULT_CAPABILITY_REFS = [
  TASSADAR_EXECUTOR_CAPABILITY_REF,
] as const

const withDefaultCapabilityRefs = (refs: string[]) =>
  [...new Set([...refs, ...PYLON_DEFAULT_CAPABILITY_REFS])]

export function resolvePylonHome(env: NodeJS.ProcessEnv = process.env) {
  const home = env.PYLON_HOME || join(homedir(), ".pylon")
  return {
    home,
    config: join(home, "config.json"),
    cache: join(home, "cache"),
    releases: join(home, "cache", "releases"),
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
  return {
    packageName: "@openagentsinc/pylon",
    version: PYLON_VERSION,
    bin: "pylon",
    platform: {
      current: currentPlatform,
      supported: isSupportedPlatform(currentPlatform),
      supportedTargets,
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
