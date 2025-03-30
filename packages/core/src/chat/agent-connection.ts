// @ts-nocheck
/**
 * Agent Connection - Thin wrapper around the official Cloudflare Agents SDK
 *
 * This module provides types and re-exports from the Cloudflare Agents SDK.
 * It's a compatibility layer for existing code that uses the custom bridge.
 */
import { Message } from './types';
import type { AgentClient as SDKAgentClient, AgentClientOptions as SDKAgentClientOptions } from 'agents/dist/client';

// Import for re-export - these are the official hooks from the Agents SDK
import { useAgent } from 'agents/dist/react';
import { useAgentChat } from 'agents/dist/ai-react';

// Define local interfaces that match the SDK types for compatibility with useChat
export interface AgentClient<T = unknown> extends SDKAgentClient<T> { }
export interface AgentClientOptions extends SDKAgentClientOptions { }

/**
 * Options for connecting to a Cloudflare Agent
 */
export interface AgentConnectionOptions {
  /**
   * The ID of the agent to connect to (e.g., 'coderagent')
   */
  agentId: string;

  /**
   * The name of the specific agent instance
   * This allows connecting to different instances of the same agent type
   */
  agentName?: string;

  /**
   * The base URL for the agent server
   * @default 'https://agents.openagents.com'
   */
  serverUrl?: string;

  /**
   * Path pattern for WebSocket endpoint
   * Not needed for official SDK
   * @deprecated
   */
  pathPattern?: string;

  /**
   * Optional callback when the agent's state is updated
   */
  onStateUpdate?: (state: any, source: 'server' | 'client') => void;

  /**
   * Optional token for authentication
   */
  token?: string;
}

// Export the official SDK hooks for direct use
export { useAgent, useAgentChat };
