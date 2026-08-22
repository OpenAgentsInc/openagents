import {
  CLOUD_COMPUTER_COMMAND_CURSOR_SCHEMA,
  CloudComputerCommandError,
  type CloudComputerCommandCursor,
  type CommandDigest,
} from "./cloud-computer-command.js";
import {
  CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA,
  type CloudComputerCommandStreamCursor,
} from "./cloud-computer-command-stream.js";

/** Converts next-sequence runtime cursors to last-observed Phoenix stream cursors. */
export const cloudComputerCommandCursorToStream = (
  cursor: CloudComputerCommandCursor,
): CloudComputerCommandStreamCursor => {
  if (cursor.nextSequence < 1)
    throw new CloudComputerCommandError("invalid", "cursor.nextSequence");
  return Object.freeze({
    schema: CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA,
    sessionRef: cursor.sessionRef,
    commandRef: cursor.commandRef,
    runtimeGeneration: cursor.runtimeGeneration,
    sequence: cursor.nextSequence - 1,
    retentionEpoch: cursor.retentionEpoch,
  });
};

/** Restores the execution bindings omitted from the public stream cursor. */
export const cloudComputerCommandCursorFromStream = (
  cursor: CloudComputerCommandStreamCursor,
  binding: Readonly<{
    requestDigest: CommandDigest;
    providerExecutionRef: string;
    runtimeRef: string;
    retentionWatermark: number;
  }>,
): CloudComputerCommandCursor => {
  if (
    cursor.schema !== CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA ||
    !Number.isSafeInteger(cursor.sequence) ||
    cursor.sequence < binding.retentionWatermark ||
    !Number.isSafeInteger(cursor.retentionEpoch) ||
    cursor.retentionEpoch < 0
  ) {
    throw new CloudComputerCommandError("cursor_expired", "streamCursor");
  }
  return Object.freeze({
    schema: CLOUD_COMPUTER_COMMAND_CURSOR_SCHEMA,
    sessionRef: cursor.sessionRef,
    commandRef: cursor.commandRef,
    requestDigest: binding.requestDigest,
    providerExecutionRef: binding.providerExecutionRef,
    runtimeRef: binding.runtimeRef,
    runtimeGeneration: cursor.runtimeGeneration,
    nextSequence: cursor.sequence + 1,
    retentionEpoch: cursor.retentionEpoch,
    retainedThrough: binding.retentionWatermark,
  });
};
