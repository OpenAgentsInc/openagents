#!/usr/bin/env node

import { NodeContext, NodeRuntime } from "@effect/platform-node"
import { Effect } from "effect"
import { cli } from "./Cli.js"

const MainLive = NodeContext.layer

const main = cli(process.argv).pipe(
  Effect.provide(MainLive),
  Effect.catchAll((error) => Effect.die(error))
)

NodeRuntime.runMain(main)
