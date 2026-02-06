import { Etag, FileSystem, HttpPlatform, Path } from '@effect/platform';
import { RpcSerialization, RpcServer } from '@effect/rpc';
import { createFileRoute } from '@tanstack/react-router';
import { Layer } from 'effect';
import { AgentRpcs } from '../effect/api/agentRpc';
import { AgentRpcsLive } from '../effect/api/agentRpcHandlers';
import { getAppConfig } from '../effect/config';
import { getServerRuntime } from '../effect/serverRuntime';
import { getAppLayer } from '../effect/runtime';

const FileSystemLive = FileSystem.layerNoop({});
const DefaultHttpServicesLive = Layer.mergeAll(
  FileSystemLive,
  Path.layer,
  Etag.layerWeak,
  HttpPlatform.layer.pipe(Layer.provide(FileSystemLive)),
);

let rpcWebHandler: ReturnType<typeof RpcServer.toWebHandler> | null = null;

function getRpcWebHandler(): ReturnType<typeof RpcServer.toWebHandler> {
  if (rpcWebHandler) return rpcWebHandler;

  const { memoMap } = getServerRuntime();
  const appLayer = getAppLayer(getAppConfig());

  rpcWebHandler = RpcServer.toWebHandler(AgentRpcs, {
    disableFatalDefects: true,
    memoMap,
    layer: Layer.mergeAll(
      AgentRpcsLive.pipe(Layer.provide(appLayer)),
      RpcSerialization.layerNdjson,
      DefaultHttpServicesLive,
    ),
  });

  // Ensure hot reload doesn't leak fibers/resources.
  if (import.meta.hot) {
    import.meta.hot.dispose(() => {
      const current = rpcWebHandler;
      rpcWebHandler = null;
      return current?.dispose();
    });
  }

  return rpcWebHandler;
}

export const Route = createFileRoute('/api/rpc')({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const { handler } = getRpcWebHandler();
        return handler(request);
      },
    },
  },
});
