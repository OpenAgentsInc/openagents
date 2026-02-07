import { Effect } from 'effect';
import { AgentApiService } from '../agentApi';
import { TelemetryService } from '../telemetry';
import { AgentRpcError, AgentRpcs } from './agentRpc';

import type { AgentApiError } from '../agentApi';

const toRpcError = (e: AgentApiError): AgentRpcError =>
  AgentRpcError.make({
    operation: e.operation,
    status: e.status,
    message: e.error instanceof Error ? e.error.message : String(e.error),
  });

export const AgentRpcsLive = AgentRpcs.toLayer(
  Effect.gen(function* () {
    const api = yield* AgentApiService;
    const telemetry = yield* TelemetryService;
    const t = telemetry.withNamespace('rpc.agent');

    return {
      'agent.getBlueprint': ({ chatId }) =>
        api.getBlueprint(chatId).pipe(
          Effect.tap(() => t.event('getBlueprint', { ok: true })),
          Effect.mapError(toRpcError),
        ),

      'agent.getMessages': ({ chatId }) =>
        api.getMessages(chatId).pipe(
          Effect.tap(() => t.event('getMessages', { ok: true })),
          Effect.mapError(toRpcError),
        ),

      'agent.getToolContracts': ({ chatId }) =>
        api.getToolContracts(chatId).pipe(
          Effect.tap(() => t.event('getToolContracts', { ok: true })),
          Effect.mapError(toRpcError),
        ),

      'agent.getSignatureContracts': ({ chatId }) =>
        api.getSignatureContracts(chatId).pipe(
          Effect.tap(() => t.event('getSignatureContracts', { ok: true })),
          Effect.mapError(toRpcError),
        ),

      'agent.getModuleContracts': ({ chatId }) =>
        api.getModuleContracts(chatId).pipe(
          Effect.tap(() => t.event('getModuleContracts', { ok: true })),
          Effect.mapError(toRpcError),
        ),

      'agent.resetAgent': ({ chatId }) =>
        api.resetAgent(chatId).pipe(
          Effect.tap(() => t.event('resetAgent', { ok: true })),
          Effect.mapError(toRpcError),
        ),

      'agent.importBlueprint': ({ chatId, blueprint }) =>
        api.importBlueprint(chatId, blueprint).pipe(
          Effect.tap(() => t.event('importBlueprint', { ok: true })),
          Effect.mapError(toRpcError),
        ),
    };
  }),
);
