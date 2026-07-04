#!/usr/bin/env bun
import { basename } from "node:path"

import {
  buildKhalaCodeDesktopReleasePlan,
  type KhalaCodeDesktopReleaseChannel,
} from "../src/shared/release-lane"

type Args = {
  readonly version: string
  readonly channel: KhalaCodeDesktopReleaseChannel
  readonly artifact: string
}

const args = parseArgs(Bun.argv.slice(2))
const plan = buildKhalaCodeDesktopReleasePlan({
  version: args.version,
  channel: args.channel,
  artifactFileName: basename(args.artifact),
})

console.log(JSON.stringify(plan, null, 2))

function parseArgs(argv: string[]): Args {
  const values = new Map<string, string>()

  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index]
    const value = argv[index + 1]

    if (!key?.startsWith("--") || value === undefined || value.startsWith("--")) {
      throw new Error(usage())
    }

    values.set(key.slice(2), value)
    index += 1
  }

  const version = values.get("version")
  const channel = values.get("channel")
  const artifact = values.get("artifact")

  if (version === undefined || channel === undefined || artifact === undefined) {
    throw new Error(usage())
  }

  if (channel !== "stable" && channel !== "rc") {
    throw new Error("Khala Code Desktop release channel must be stable or rc")
  }

  return {
    version,
    channel,
    artifact,
  }
}

function usage(): string {
  return [
    "Usage:",
    "  bun scripts/release-plan.ts --version 0.1.0-rc.1 --channel rc --artifact ./Khala-Code.dmg",
  ].join("\n")
}
