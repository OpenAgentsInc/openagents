import { Effect, Layer } from "effect";

import type { CommandReceipt, QuarantinedCommand, QueuedCommand } from "../src/model.ts";
import { ClientOutboxStore } from "../src/services.ts";

export interface MemoryOutboxState {
  readonly commands: Map<string, QueuedCommand>;
  readonly receipts: Array<CommandReceipt>;
  readonly quarantine: Array<QuarantinedCommand>;
}

export const makeMemoryOutbox = (): Readonly<{
  state: MemoryOutboxState;
  layer: Layer.Layer<ClientOutboxStore>;
}> => {
  const state: MemoryOutboxState = {
    commands: new Map(),
    receipts: [],
    quarantine: [],
  };

  return {
    state,
    layer: Layer.succeed(ClientOutboxStore, {
      put: (command) => Effect.sync(() => void state.commands.set(command.commandId, command)),
      list: () => Effect.sync(() => [...state.commands.values()]),
      remove: (commandId) => Effect.sync(() => void state.commands.delete(commandId)),
      recordReceipt: (receipt) => Effect.sync(() => void state.receipts.push(receipt)),
      quarantine: (entry) => Effect.sync(() => void state.quarantine.push(entry)),
    }),
  };
};
