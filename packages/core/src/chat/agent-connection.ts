import { Message, UIMessage } from './types';
import { BaseAgentClient, AgentClientOptions as BridgeClientOptions, createAgentClient, AgentClient as AgentClientImpl } from '../mcp/agent-sdk-bridge';

// Re-export the agent client interfaces
export type AgentClientOptions = BridgeClientOptions;
export type AgentClient = BaseAgentClient;

// Ensure exports for IDE intellisense
export { createAgentClient } from '../mcp/agent-sdk-bridge';

/**
 * Options for connecting to a Cloudflare Agent
 */
export interface AgentConnectionOptions {
  /**
   * The ID of the agent to connect to (e.g., 'CoderAgent')
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
   * @default 'api/agents'
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
 * Creates a connection to a Cloudflare Agent
 */
export const createAgentConnection = async (options: AgentConnectionOptions): Promise<AgentClient> => {
  const { 
    agentId, 
    agentName = 'default', 
    serverUrl = 'https://agents.openagents.com',
    pathPattern,
    onStateUpdate,
    token
  } = options;
  
  // Set up the client options with authentication if provided
  const clientOptions: AgentClientOptions = {
    agent: agentId,
    name: agentName,
    host: serverUrl,
    pathPattern,
    onStateUpdate,
  };
  
  // Add authentication headers if token is provided
  if (token) {
    clientOptions.headers = {
      Authorization: `Bearer ${token}`
    };
  }
  
  try {
    // Create the agent client using our WebSocket implementation
    console.log(`🔌 USECHAT: Creating agent client for ${agentId}/${agentName}`);
    const client = createAgentClient(clientOptions);
    
    return client;
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
        // Check if client is actually connected before attempting to execute command
        // We need to cast to the implementation type to access the connected property
        const agentClient = client as AgentClientImpl;
        if (agentClient.connected === false) {
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
        // Check if client is actually connected before attempting to set context
        const agentClient = client as AgentClientImpl;
        if (agentClient.connected === false) {
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
        // Check if client is actually connected before attempting to get context
        const agentClient = client as AgentClientImpl;
        if (agentClient.connected === false) {
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