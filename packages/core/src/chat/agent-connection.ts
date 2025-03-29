import { Message, UIMessage } from './types';

// Model the AgentClient interface without directly importing it
// This avoids TypeScript module resolution issues
export interface AgentClientOptions {
  /** Name of the agent to connect to */
  agent: string;
  /** Name of the specific Agent instance */
  name?: string;
  /** Called when the Agent's state is updated */
  onStateUpdate?: (state: any, source: "server" | "client") => void;
  /** Host URL for the agent server */
  host?: string;
  /** Headers for authentication */
  headers?: Record<string, string>;
}

// Simplified AgentClient interface for type checking
export interface AgentClient {
  agent: string;
  name: string;
  setState(state: any): void;
  call<T = unknown>(method: string, args?: unknown[]): Promise<T>;
  close(): void;
}

// Runtime implementation helper
export const getAgentClient = (): any => {
  // This function will be used to dynamically import the real AgentClient at runtime
  // and avoid TypeScript module resolution issues during compilation
  try {
    // @ts-ignore - We know this will be available at runtime
    return require('agents/client').AgentClient;
  } catch (error) {
    console.error('Failed to load AgentClient:', error);
    throw new Error('Agents SDK not available. Make sure agents package is installed.');
  }
};

/**
 * Options for connecting to a Cloudflare Agent
 */
export interface AgentConnectionOptions {
  /**
   * The ID of the agent to connect to (e.g., 'coder-agent')
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
    // Get the AgentClient constructor dynamically at runtime
    const AgentClientClass = getAgentClient();
    
    // Create the agent client
    const client = new AgentClientClass(clientOptions);
    
    return client as AgentClient;
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
        return await client.call('executeCommand', [command]);
      } catch (error) {
        console.error('Failed to execute command on agent:', error);
        throw new Error(`Command execution failed: ${error instanceof Error ? error.message : String(error)}`);
      }
    },
    setProjectContext: async (context: any) => {
      try {
        return await client.call('setProjectContext', [context]);
      } catch (error) {
        console.error('Failed to set project context:', error);
        return false;
      }
    },
    getProjectContext: async () => {
      try {
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