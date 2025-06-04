#!/usr/bin/env node

import { NodeContext, NodeHttpClient, NodeRuntime } from "@effect/platform-node"
import { Effect, Layer } from "effect"
import { cli } from "./Cli.js"
import { TodosClient } from "./TodosClient.js"

const MainLive = TodosClient.Default.pipe(
  Layer.provide(NodeHttpClient.layerUndici),
  Layer.merge(NodeContext.layer)
)

cli(process.argv).pipe(
  Effect.provide(MainLive),
  NodeRuntime.runMain
)
