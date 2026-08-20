import { Layer } from "effect";
import * as Context from "effect/Context";

export interface TerminalSessionValues {
  readonly interactive: boolean;
}

export class TerminalSession extends Context.Service<TerminalSession, TerminalSessionValues>()(
  "@openagentsinc/cli/TerminalSession",
) {}

export const terminalSessionNodeLayer = Layer.succeed(TerminalSession, {
  interactive: process.stdin.isTTY === true && process.stdout.isTTY === true,
});

export const terminalSessionTestLayer = (interactive: boolean) =>
  Layer.succeed(TerminalSession, { interactive });
