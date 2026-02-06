import { FetchHttpClient } from '@effect/platform';
import { RpcClient, RpcSerialization } from '@effect/rpc';
import { Context, Effect, Layer } from 'effect';
import { AgentRpcs } from './agentRpc';

export type AgentRpcClient = RpcClient.FromGroup<typeof AgentRpcs>;

export class AgentRpcClientService extends Context.Tag('@openagents/web/AgentRpcClient')<
  AgentRpcClientService,
  AgentRpcClient
>() {}

const ProtocolLive = RpcClient.layerProtocolHttp({ url: '/api/rpc' }).pipe(
  Layer.provide(FetchHttpClient.layer),
  Layer.provide(RpcSerialization.layerNdjson),
);

export const AgentRpcClientLive = Layer.scoped(
  AgentRpcClientService,
  RpcClient.make(AgentRpcs).pipe(
    Effect.map((client) => AgentRpcClientService.of(client)),
    Effect.provide(ProtocolLive),
  ),
);

