import { NodeHttpServer, NodeRuntime } from "@effect/platform-node"
import { Layer } from "effect"
import { createServer } from "node:http"

// Simple HTTP server setup - will be expanded with API routes later
const HttpLive = NodeHttpServer.layer(createServer, { port: 3000 })

NodeRuntime.runMain(Layer.launch(HttpLive))
