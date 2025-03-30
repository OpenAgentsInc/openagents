/**
 * Agent Connection - Wrapper around the official Cloudflare Agents SDK
 * 
 * This module provides a simplified interface to the Cloudflare Agents SDK
 * for use in the OpenAgents application.
 */
import { Message, UIMessage } from './types';
import type { AgentClient as SDKAgentClient, AgentClientOptions as SDKAgentClientOptions } from 'agents/client';
import type { UseAgentOptions } from 'agents/react';
// Import for re-export
import { useAgent } from 'agents/react';
import { useAgentChat } from 'agents/ai-react';

// Define local interfaces that match the SDK types
export interface AgentClient extends SDKAgentClient {}
export interface AgentClientOptions extends SDKAgentClientOptions {}

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

/**
 * Connection state for an agent
 */
export interface AgentConnectionState {
  isConnected: boolean;
  agentId: string;
  agentName: string;
  client: AgentClient | null;
  messages: UIMessage[];
  error?: Error;
}

/**
 * Creates a connection to a Cloudflare Agent using the official SDK
 */
export const createAgentClient = (options: AgentClientOptions): AgentClient => {
  // This now uses the official SDK implementation
  if (typeof window !== 'undefined') {
    console.warn('createAgentClient should be used with useAgent hook in React components');
  }
  
  // Import dynamically to avoid SSR issues
  // In practice, you should use the useAgent hook in React components
  const { AgentClient } = require('agents/client');
  return new AgentClient(options) as AgentClient;
};

/**
 * Creates a connection to a Cloudflare Agent
 * This is a compatibility wrapper around the official SDK
 */
export const createAgentConnection = async (options: AgentConnectionOptions): Promise<AgentClient> => {
  const { 
    agentId, 
    agentName = 'default', 
    serverUrl = 'https://agents.openagents.com',
    onStateUpdate,
    token
  } = options;
  
  // Set up the client options with authentication if provided
  const clientOptions: AgentClientOptions = {
    agent: agentId,
    name: agentName,
    host: serverUrl,
    onStateUpdate,
  };
  
  // Add authentication headers if token is provided
  if (token) {
    clientOptions.headers = {
      Authorization: `Bearer ${token}`
    };
  }
  
  try {
    // Create the agent client using the official SDK implementation
    console.log(`🔌 Creating agent client for ${agentId}/${agentName} using official SDK`);
    return createAgentClient(clientOptions);
  } catch (error) {
    console.error('Failed to connect to agent:', error);
    throw new Error(`Failed to connect to agent: ${error instanceof Error ? error.message : String(error)}`);
  }
};

/**
 * Fetches initial messages from an agent
 */
export const fetchAgentMessages = async (client: AgentClient): Promise<UIMessage[]> => {
  try {
    // Call the agent's getMessages method to fetch chat history
    const messages = await client.call<Message[]>('getMessages');
    
    // Convert to UIMessages with proper typing
    return messages.map((msg: Message) => ({
      ...msg,
      parts: msg.parts || [{
        type: 'text',
        text: msg.content
      }]
    })) as UIMessage[];
  } catch (error) {
    console.error('Failed to fetch agent messages:', error);
    return [];
  }
};

/**
 * Sends a message to the agent
 */
export const sendMessageToAgent = async (
  client: AgentClient, 
  message: Message
): Promise<string | null> => {
  try {
    // Call the agent's sendMessage method
    const result = await client.call<{id: string}>('sendMessage', [message]);
    return result.id;
  } catch (error) {
    console.error('Failed to send message to agent:', error);
    throw new Error(`Failed to send message to agent: ${error instanceof Error ? error.message : String(error)}`);
  }
};

/**
 * Creates a utility object for interacting with an agent
 */
export const createAgentUtils = (client: AgentClient) => {
  return {
    fetchMessages: () => fetchAgentMessages(client),
    sendMessage: (message: Message) => sendMessageToAgent(client, message),
    executeCommand: async (command: string) => {
      try {
        // Check if client is connected
        if (!client) {
          const notConnectedError = new Error('Cannot execute command: client not connected');
          console.error('⚠️ Agent command execution failed:', notConnectedError);
          throw notConnectedError;
        }
        
        return await client.call('executeCommand', [command]);
      } catch (error) {
        console.error('Failed to execute command on agent:', error);
        throw new Error(`Command execution failed: ${error instanceof Error ? error.message : String(error)}`);
      }
    },
    setProjectContext: async (context: any) => {
      try {
        if (!client) {
          console.warn('Cannot set project context: client not connected');
          return false;
        }
        
        return await client.call('setProjectContext', [context]);
      } catch (error) {
        console.error('Failed to set project context:', error);
        return false;
      }
    },
    getProjectContext: async () => {
      try {
        if (!client) {
          console.warn('Cannot get project context: client not connected');
          return {};
        }
        
        return await client.call('getProjectContext');
      } catch (error) {
        console.error('Failed to get project context:', error);
        return {};
      }
    },
    disconnect: () => {
      try {
        client.close();
      } catch (error) {
        console.error('Error closing agent connection:', error);
      }
    }
  };
};

// Export the official SDK hooks for direct use
export { useAgent, useAgentChat };