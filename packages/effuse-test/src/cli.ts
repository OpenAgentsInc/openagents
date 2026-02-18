#!/usr/bin/env bun

import * as Path from "node:path"

import { Command, Options } from "@effect/cli"
import { BunContext, BunRuntime } from "@effect/platform-bun"
import { Effect, Option } from "effect"

import { run } from "./runner/runner.ts"

const project = Options.text("project").pipe(
  Options.withAlias("p"),
  Options.withDescription("Project directory (e.g. ../../apps/openagents.com)"),
  Options.withDefault("../../apps/openagents.com"),
)

const serverPort = Options.integer("server-port").pipe(
  Options.withDescription("Port for wrangler dev"),
  Options.withDefault(3010),
)

const baseUrl = Options.text("base-url").pipe(
  Options.withAlias("u"),
  Options.optional,
  Options.withDescription(
    "Use an existing server base URL instead of starting wrangler dev (e.g. https://autopilot-web.openagents.workers.dev)",
  ),
)

const viewerPort = Options.integer("viewer-port").pipe(
  Options.withDescription("Port for the live viewer UI (only used in --watch)"),
  Options.withDefault(3020),
)

const watch = Options.boolean("watch").pipe(
  Options.withAlias("w"),
  Options.withDescription("Headed browser + live viewer UI"),
)

const headed = Options.boolean("headed").pipe(Options.withDescription("Run headed (non-headless)"))
const headless = Options.boolean("headless").pipe(Options.withDescription("Run headless (default)"))

const grep = Options.text("grep").pipe(Options.optional, Options.withDescription("Regex to filter test ids"))

const tag = Options.text("tag").pipe(
  Options.optional,
  Options.withDescription("Comma-separated tags to filter (OR semantics)"),
)

const chromePath = Options.text("chrome-path").pipe(
  Options.optional,
  Options.withDescription("Override Chromium executable path (equivalent to EFFUSE_TEST_CHROME_PATH)"),
)

const updateSnapshots = Options.text("update-snapshots").pipe(
  Options.optional,
  Options.withDescription("Override snapshot update mode: true|false (or 1|0, yes|no)"),
)

const e2eBypassSecret = Options.text("e2e-bypass-secret").pipe(
  Options.optional,
  Options.withDescription("Override E2E bypass secret (equivalent to EFFUSE_TEST_E2E_BYPASS_SECRET)"),
)

const magicEmail = Options.text("magic-email").pipe(
  Options.optional,
  Options.withDescription("Override magic-login email (must be set with --magic-code)"),
)

const magicCode = Options.text("magic-code").pipe(
  Options.optional,
  Options.withDescription("Override magic-login code (must be set with --magic-email)"),
)

const parseBooleanFlag = (
  flag: string,
  raw: string | undefined,
): Effect.Effect<boolean | undefined, Error> =>
  Effect.gen(function* () {
    if (raw === undefined) return undefined
    const value = raw.trim().toLowerCase()
    if (value === "1" || value === "true" || value === "yes") return true
    if (value === "0" || value === "false" || value === "no") return false
    return yield* Effect.fail(
      new Error(`Invalid --${flag} value "${raw}". Expected one of: 1,true,yes,0,false,no`),
    )
  })

const runCommand = Command.make(
  "run",
  {
    project,
    serverPort,
    baseUrl,
    viewerPort,
    watch,
    headed,
    headless,
    grep,
    tag,
    chromePath,
    updateSnapshots,
    e2eBypassSecret,
    magicEmail,
    magicCode,
  },
  ({
    project,
    serverPort,
    baseUrl,
    viewerPort,
    watch,
    headed,
    headless,
    grep,
    tag,
    chromePath,
    updateSnapshots,
    e2eBypassSecret,
    magicEmail,
    magicCode,
  }) => {
    if (headed && headless) {
      return Effect.fail(new Error("Cannot set both --headed and --headless"))
    }

    return Effect.gen(function* () {
      const resolvedProject = Path.resolve(process.cwd(), project)
      const shouldWatch = watch
      const finalHeadless = shouldWatch ? false : headed ? false : headless ? true : true
      const resolvedBaseUrl = Option.getOrUndefined(baseUrl)
      const tagsRaw = Option.getOrUndefined(tag)
      const tags = tagsRaw ? tagsRaw.split(",").map((s) => s.trim()).filter(Boolean) : undefined
      const resolvedUpdateSnapshots = yield* parseBooleanFlag(
        "update-snapshots",
        Option.getOrUndefined(updateSnapshots),
      )

      return yield* run({
        projectDir: resolvedProject,
        serverPort,
        baseUrl: resolvedBaseUrl,
        viewerPort,
        headless: finalHeadless,
        watch: shouldWatch,
        grep: Option.getOrUndefined(grep),
        tags,
        configOverrides: {
          chromePath: Option.getOrUndefined(chromePath),
          updateSnapshots: resolvedUpdateSnapshots,
          e2eBypassSecret: Option.getOrUndefined(e2eBypassSecret),
          magicEmail: Option.getOrUndefined(magicEmail),
          magicCode: Option.getOrUndefined(magicCode),
        },
      })
    })
  },
).pipe(Command.withDescription("Run Effuse browser / integration tests"))

const root = Command.make("effuse-test", {}).pipe(Command.withSubcommands([runCommand]))

const cli = Command.run(root, { name: "effuse-test", version: "0.0.1" })

cli(process.argv).pipe(Effect.provide(BunContext.layer), BunRuntime.runMain)
