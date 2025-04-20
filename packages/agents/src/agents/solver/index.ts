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
  Layer,
  pipe
} from "effect";
import type { BaseIssue, BaseProject, BaseTeam } from "@openagents/core";
import { createHandleMessageEffect } from "./handleMessage";
import { AnthropicConfig } from "./types";
import { solverContext } from "./tools";
import { SolverToolsImplementationLayer } from "./effect-tools";

// Define the state to hold context
export type SolverState = {
  messages: UIMessage[];
  currentIssue?: BaseIssue;
  currentProject?: BaseProject;
  currentTeam?: BaseTeam;
  githubToken?: string;
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
    currentTeam: undefined,
    githubToken: undefined
  };

  /**
   * Handles incoming WebSocket messages using Effect
   */
  async onMessage(connection: Connection, message: WSMessage) {
    // Create a basic default Anthropic config for use in the context
    const defaultAnthropicConfig = {
      apiKey: process.env.ANTHROPIC_API_KEY || "",
      fetch: globalThis.fetch,
      model: "claude-3-5-sonnet-latest"
    };
    
    // Set the solver instance in AsyncLocalStorage context for tools
    return await solverContext.run(this, async () => {
      // Create the Anthropic config layer
      const anthropicConfigLayer = Layer.succeed(
        AnthropicConfig,
        defaultAnthropicConfig as AnthropicConfig
      );
      
      // Create the message handling effect
      const handleMessageEffect = createHandleMessageEffect(this, message as string);
      
      // Run with the customized runtime and provide the Anthropic config layer
      const exit = await Runtime.runPromiseExit(customizedRuntime)(
        handleMessageEffect.pipe(
          Effect.provide(anthropicConfigLayer)
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
    });
  }
  
  /**
   * Set GitHub token for API access
   */
  setGitHubToken(token: string): void {
    this.setState({ 
      ...this.state, 
      githubToken: token 
    });
  }
}
