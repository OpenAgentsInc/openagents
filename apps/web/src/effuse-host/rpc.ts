import { Etag, FileSystem, HttpPlatform, Path } from "@effect/platform"
import { RpcSerialization, RpcServer } from "@effect/rpc"
import { Layer } from "effect"

import { AgentRpcs } from "../effect/api/agentRpc"
import { AgentRpcsLive } from "../effect/api/agentRpcHandlers"

import { getWorkerRuntime } from "./runtime"
import type { WorkerEnv } from "./env"

const FileSystemLive = FileSystem.layerNoop({})
const DefaultHttpServicesLive = Layer.mergeAll(
  FileSystemLive,
  Path.layer,
  Etag.layerWeak,
  HttpPlatform.layer.pipe(Layer.provide(FileSystemLive)),
)

let rpcWebHandler: ReturnType<typeof RpcServer.toWebHandler> | null = null

const getRpcWebHandler = (env: WorkerEnv): ReturnType<typeof RpcServer.toWebHandler> => {
  if (rpcWebHandler) return rpcWebHandler

  const { memoMap, layer: appLayer } = getWorkerRuntime(env)

  rpcWebHandler = RpcServer.toWebHandler(AgentRpcs, {
    disableFatalDefects: true,
    memoMap,
    layer: Layer.mergeAll(
      AgentRpcsLive.pipe(Layer.provide(appLayer)),
      RpcSerialization.layerNdjson,
      DefaultHttpServicesLive,
    ),
  })

  return rpcWebHandler
}

export const handleRpcRequest = async (request: Request, env: WorkerEnv): Promise<Response> => {
  if (request.method !== "POST") {
    return new Response("Method not allowed", { status: 405 })
  }
  const { handler } = getRpcWebHandler(env)
  return handler(request)
}

