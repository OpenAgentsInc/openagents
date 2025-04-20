import { Agent, type Connection, type WSMessage } from "agents";
import {
  Exit,
  Runtime,
  LogLevel,
  FiberRefs,
  FiberRef,
  FiberId,
  Cause,
  Effect,
} from "effect";
import { createHandleMessageEffect, type SolverState } from "./handleMessage";

// Configure the runtime with Debug level logging
const customizedRuntime = Runtime.make({
  context: Runtime.defaultRuntime.context,
  runtimeFlags: Runtime.defaultRuntime.runtimeFlags,
  fiberRefs: FiberRefs.updateAs(
    Runtime.defaultRuntime.fiberRefs,
    {
      fiberId: FiberId.none,
      fiberRef: FiberRef.currentMinimumLogLevel,
      value: LogLevel.Debug
    }
  )
});

export class Solver extends Agent<Env, SolverState> {

  initialState: SolverState = {
    messages: [],
    currentIssue: undefined,
    currentProject: undefined,
    currentTeam: undefined
  };

  /**
   * Handles incoming WebSocket messages using Effect
   */
  async onMessage(connection: Connection, message: WSMessage) {
    const handleMessageEffect = createHandleMessageEffect(this, message as string);

    // Run with the customized runtime
    const exit = await Runtime.runPromiseExit(customizedRuntime)(handleMessageEffect);

    if (Exit.isFailure(exit)) {
      await Runtime.runPromise(customizedRuntime)(
        Effect.logError(`❌ Message Processing Failed`).pipe(
          Effect.annotateLogs({
            failureCause: Cause.pretty(exit.cause)
              .replace(/\\"/g, '"')  // Remove escaped quotes
              .split('\n')
              .map(line => `│ ${line}`)
              .join('\n')
          })
        )
      );
    }
  }
}
