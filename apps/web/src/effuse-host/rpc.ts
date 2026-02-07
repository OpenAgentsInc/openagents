import { Etag, FileSystem, HttpPlatform, Path } from "@effect/platform"
import { RpcSerialization, RpcServer } from "@effect/rpc"
import { Context, Layer } from "effect"

import { AgentRpcs } from "../effect/api/agentRpc"
import { AgentRpcsLive } from "../effect/api/agentRpcHandlers"
import { RequestContextService, makeServerRequestContext } from "../effect/requestContext"

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
    return new Response("Method not allowed", {
      status: 405,
      headers: { "cache-control": "no-store" },
    })
  }

  const { handler } = getRpcWebHandler(env)
  const requestContext = Context.make(RequestContextService, makeServerRequestContext(request))
  const response = await handler(request, requestContext)

  // RPC responses must never be cached.
  const headers = new Headers(response.headers)
  headers.set("cache-control", "no-store")

  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  })
}
