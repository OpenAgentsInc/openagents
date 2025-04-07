/**
 * Base interface for Coder agent plugins
 * Plugins provide tools and functionality that can be used by the agent
 */
import { AIChatAgent } from "agents/ai-chat-agent";

export interface AgentPlugin {
  /**
   * Initialize the plugin
   * This is called when the agent is created and should set up any necessary resources
   * @param agent The agent instance the plugin is being initialized for
   */
  initialize(agent: AIChatAgent<any>): Promise<void>;
  
  /**
   * Get the tools provided by this plugin
   * Tools should be compatible with the AI SDK tool format
   */
  getTools(): Record<string, any>;
  
  /**
   * Name of the plugin
   * Used for identification and logging
   */
  readonly name: string;
}