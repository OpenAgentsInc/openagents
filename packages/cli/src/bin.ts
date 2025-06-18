#!/usr/bin/env node

import { NodeContext, NodeRuntime } from "@effect/platform-node"
import { Effect } from "effect"
import { cli } from "./Cli.js"

const MainLive = NodeContext.layer

const main = Effect.provide(
  cli(process.argv).pipe(
    Effect.catchAll((error) => Effect.die(error))
  ),
  MainLive
)

NodeRuntime.runMain(main as Effect.Effect<void, never, never>)
