import { Rpc, RpcGroup } from '@effect/rpc';
import { Schema } from 'effect';

export class AgentRpcError extends Schema.TaggedError<AgentRpcError>()('AgentRpcError', {
  operation: Schema.String,
  status: Schema.optional(Schema.Number),
  message: Schema.String,
}) {}

export const AgentToolContractSchema = Schema.Struct({
  name: Schema.String,
  description: Schema.String,
  usage: Schema.optional(Schema.String),
  inputSchemaJson: Schema.Unknown,
  outputSchemaJson: Schema.NullOr(Schema.Unknown),
});

export const DseSignatureContractSchema = Schema.Struct({
  format: Schema.String,
  formatVersion: Schema.Number,
  signatureId: Schema.String,
  inputSchemaJson: Schema.Unknown,
  outputSchemaJson: Schema.Unknown,
  promptIr: Schema.Unknown,
  defaultParams: Schema.Unknown,
  defaultConstraints: Schema.Unknown,
});

export const DseModuleContractSchema = Schema.Struct({
  format: Schema.String,
  formatVersion: Schema.Number,
  moduleId: Schema.String,
  description: Schema.String,
  signatureIds: Schema.Array(Schema.String),
});

export class AgentRpcs extends RpcGroup.make(
  Rpc.make('agent.getBlueprint', {
    payload: {
      chatId: Schema.String,
    },
    success: Schema.Unknown,
    error: AgentRpcError,
  }),
  Rpc.make('agent.getMessages', {
    payload: {
      chatId: Schema.String,
    },
    success: Schema.Array(Schema.Unknown),
    error: AgentRpcError,
  }),
  Rpc.make('agent.getToolContracts', {
    payload: {
      chatId: Schema.String,
    },
    success: Schema.Array(AgentToolContractSchema),
    error: AgentRpcError,
  }),
  Rpc.make('agent.getSignatureContracts', {
    payload: {
      chatId: Schema.String,
    },
    success: Schema.Array(DseSignatureContractSchema),
    error: AgentRpcError,
  }),
  Rpc.make('agent.getModuleContracts', {
    payload: {
      chatId: Schema.String,
    },
    success: Schema.Array(DseModuleContractSchema),
    error: AgentRpcError,
  }),
  Rpc.make('agent.resetAgent', {
    payload: {
      chatId: Schema.String,
    },
    success: Schema.Void,
    error: AgentRpcError,
  }),
  Rpc.make('agent.importBlueprint', {
    payload: {
      chatId: Schema.String,
      blueprint: Schema.Unknown,
    },
    success: Schema.Void,
    error: AgentRpcError,
  }),
) {}

