#!/usr/bin/env bun

import * as Path from "node:path"

import { Command, Options } from "@effect/cli"
import { BunContext, BunRuntime } from "@effect/platform-bun"
import { Effect, Option } from "effect"

import { run } from "./runner/runner.ts"

const project = Options.text("project").pipe(
  Options.withAlias("p"),
  Options.withDescription("Project directory (currently only apps/web is supported)"),
  Options.withDefault("../../apps/web"),
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

const runCommand = Command.make(
  "run",
  { project, serverPort, baseUrl, viewerPort, watch, headed, headless, grep, tag },
  ({ project, serverPort, baseUrl, viewerPort, watch, headed, headless, grep, tag }) => {
    if (headed && headless) {
      return Effect.fail(new Error("Cannot set both --headed and --headless"))
    }

    const resolvedProject = Path.resolve(process.cwd(), project)
    const shouldWatch = watch
    const finalHeadless = shouldWatch ? false : headed ? false : headless ? true : true
    const resolvedBaseUrl = Option.getOrUndefined(baseUrl)
    const tagsRaw = Option.getOrUndefined(tag)
    const tags = tagsRaw ? tagsRaw.split(",").map((s) => s.trim()).filter(Boolean) : undefined

    return run({
      projectDir: resolvedProject,
      serverPort,
      baseUrl: resolvedBaseUrl,
      viewerPort,
      headless: finalHeadless,
      watch: shouldWatch,
      grep: Option.getOrUndefined(grep),
      tags,
    })
  },
).pipe(Command.withDescription("Run Effuse browser / integration tests"))

const root = Command.make("effuse-test", {}).pipe(Command.withSubcommands([runCommand]))

const cli = Command.run(root, { name: "effuse-test", version: "0.0.1" })

cli(process.argv).pipe(Effect.provide(BunContext.layer), BunRuntime.runMain)
