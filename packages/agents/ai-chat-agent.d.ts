import { Agent, AgentContext } from "./index.js";
import { Message, StreamTextOnFinishCallback, ToolSet } from "ai";
import { Connection, WSMessage } from "partyserver";
import "cloudflare:workers";

/**
 * Extension of Agent with built-in chat capabilities
 * @template Env Environment type containing bindings
 */
declare class AIChatAgent<Env = unknown, State = unknown> extends Agent<
  Env,
  State
> {
  #private;
  /** Array of chat messages for the current conversation */
  messages: Message[];
  constructor(ctx: AgentContext, env: Env);
  onMessage(connection: Connection, message: WSMessage): Promise<void>;
  onRequest(request: Request): Promise<Response>;
  /**
   * Handle incoming chat messages and generate a response
   * @param onFinish Callback to be called when the response is finished
   * @returns Response to send to the client or undefined
   */
  onChatMessage(
    onFinish: StreamTextOnFinishCallback<ToolSet>
  ): Promise<Response | undefined>;
  /**
   * Save messages on the server side and trigger AI response
   * @param messages Chat messages to save
   */
  saveMessages(messages: Message[]): Promise<void>;
}

export { AIChatAgent };
