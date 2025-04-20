import { Agent, type Connection, type WSMessage } from "agents";
import type { UIMessage } from "ai";
import {
  Exit,
  Runtime,
  LogLevel,
  FiberRefs,
  FiberRef,
  FiberId,
  Cause,
  Effect,
  Layer
} from "effect";
import type { BaseIssue, BaseProject, BaseTeam } from "@openagents/core";
import { createHandleMessageEffect } from "./handleMessage";
import { AnthropicConfig } from "./types";

// Define the state to hold context
export type SolverState = {
  messages: UIMessage[];
  currentIssue?: BaseIssue;
  currentProject?: BaseProject;
  currentTeam?: BaseTeam;
};

// Configure the runtime with Debug level logging only
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

    // Create a basic default Anthropic config for use in the context
    const defaultAnthropicConfig = {
      apiKey: process.env.ANTHROPIC_API_KEY || "",
      fetch: globalThis.fetch,
      model: "claude-3-sonnet-20240229"
    };

    // Run with the customized runtime and provide the AnthropicConfig
    const exit = await Runtime.runPromiseExit(customizedRuntime)(
      handleMessageEffect.pipe(
        Effect.provideService(AnthropicConfig, defaultAnthropicConfig as AnthropicConfig)
      )
    );

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
