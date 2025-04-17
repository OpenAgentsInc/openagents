import { useState, useCallback, useEffect } from "react";
import { generateId, type UIMessage } from "ai";
import { useAgent } from "agents/react";

// Define agent types
type AgentType = 'coder' | 'solver';

export type OpenAgent = {
  state: AgentState;
  messages: UIMessage[];
  setMessages: (messages: UIMessage[]) => void;
  handleSubmit: (message: string) => void;
  infer: (token?: string) => Promise<any>;
  setGithubToken: (token: string) => Promise<void>;
  getGithubToken: () => Promise<string>;
  setCurrentIssue?: (issue: any) => Promise<void>;
  setRepositoryContext?: (owner: string, repo: string, branch?: string) => Promise<void>;
  connectionStatus: 'connecting' | 'connected' | 'disconnected' | 'error';
  disconnect: () => void; // New method to properly disconnect
}

type AgentState = {
  messages: UIMessage[];
  githubToken?: string;
  currentIssue?: any;
  currentRepoOwner?: string;
  currentRepoName?: string;
  currentBranch?: string;
  [key: string]: any;
}

/**
 * Hook to connect to and interact with OpenAgents agents
 * @param id Unique identifier for this agent instance
 * @param type Type of agent to connect to ('coder' or 'solver')
 * @returns An OpenAgent interface for interacting with the agent
 */
export function useOpenAgent(id: string, type: AgentType = "coder"): OpenAgent {
  // Setup local state that will be synchronized with the agent
  const [state, setAgentState] = useState<AgentState>({
    messages: []
  });

  // Connect to the agent using the Cloudflare Agents SDK
  // Make sure we don't duplicate the agent type prefix in the name
  const agentName = id.startsWith(`${type}-`) ? id : `${type}-${id}`;

  // Add more robust logging and connection debugging with auto-reconnect logic
  const [connectionAttempts, setConnectionAttempts] = useState(0);
  const maxReconnectAttempts = 3;

  const onOpen = () => {
    console.log(`[useOpenAgent ${agentName}] WebSocket connection opened successfully`);
    // Reset connection attempts on successful connection
    setConnectionAttempts(0);
  };

  const onClose = (event: any) => {
    console.log(`[useOpenAgent ${agentName}] WebSocket connection closed`, event);
  };

  const onError = (error: any) => {
    console.error(`[useOpenAgent ${agentName}] WebSocket connection error:`, error);

    // Only attempt to reconnect if we haven't exceeded max attempts
    if (connectionAttempts < maxReconnectAttempts) {
      setConnectionAttempts(prev => prev + 1);
      console.log(`[useOpenAgent ${agentName}] Auto-reconnect attempt ${connectionAttempts + 1}/${maxReconnectAttempts}`);
    } else {
      console.error(`[useOpenAgent ${agentName}] Max reconnection attempts (${maxReconnectAttempts}) reached. Giving up.`);
    }
  };

  // Use the Cloudflare Agents SDK hook to connect to the agent
  const cloudflareAgent = useAgent({
    name: agentName, // Unique name for this agent instance
    agent: type,     // Agent type maps to URL path/DO binding name
    onStateUpdate: (newState: AgentState) => {
      // Update local state whenever agent state changes
      console.log(`[useOpenAgent ${agentName}] State updated from agent:`, newState);
      setAgentState(newState);
    },
    host: "agents.openagents.com"
  });

  // Add listeners manually since we can't pass them to useAgent directly
  useEffect(() => {
    // Add listeners to the socket instance after it's created
    if (cloudflareAgent) {
      cloudflareAgent.addEventListener('open', onOpen);
      cloudflareAgent.addEventListener('close', onClose);
      cloudflareAgent.addEventListener('error', onError);
    }

    return () => {
      // Clean up listeners
      if (cloudflareAgent) {
        cloudflareAgent.removeEventListener('open', onOpen);
        cloudflareAgent.removeEventListener('close', onClose);
        cloudflareAgent.removeEventListener('error', onError);
      }
    };
  }, [cloudflareAgent, onOpen, onClose, onError]);


  // Track connection status for logging purposes
  const [connectionStatus, setConnectionStatus] = useState<'connecting' | 'connected' | 'disconnected' | 'error'>('disconnected');

  // We'll use custom event names with prefixes to avoid recursion
  const connectedEventName = `agent:${agentName}:connected`;
  const disconnectedEventName = `agent:${agentName}:disconnected`;
  const errorEventName = `agent:${agentName}:error`;

  // Update connection status based on actual WebSocket events (not our custom events)
  useEffect(() => {
    const handleSocketOpen = () => {
      console.log(`[useOpenAgent ${agentName}] WebSocket connection opened successfully`);
      setConnectionStatus('connected');

      // Dispatch a custom event that components can listen for
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent(connectedEventName));
        // Also dispatch a generic event for legacy listeners
        window.dispatchEvent(new CustomEvent('agent:connected', {
          detail: { agentName, type }
        }));
      }
    };

    const handleSocketClose = () => {
      console.log(`[useOpenAgent ${agentName}] WebSocket connection closed`);
      setConnectionStatus('disconnected');

      // Dispatch a custom event that components can listen for
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent(disconnectedEventName));
        // Also dispatch a generic event for legacy listeners
        window.dispatchEvent(new CustomEvent('agent:disconnected', {
          detail: { agentName, type }
        }));
      }
    };

    const handleSocketError = () => {
      console.error(`[useOpenAgent ${agentName}] WebSocket connection error`);
      setConnectionStatus('error');

      // Dispatch a custom event that components can listen for
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent(errorEventName));
        // Also dispatch a generic event for legacy listeners
        window.dispatchEvent(new CustomEvent('agent:error', {
          detail: { agentName, type }
        }));
      }
    };

    // If we already received a state update, we're already connected
    if (state && Object.keys(state).length > 0 && connectionStatus === 'disconnected') {
      console.log(`[useOpenAgent ${agentName}] Already connected with state:`, state);
      setConnectionStatus('connected');

      // Dispatch a connected event
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent(connectedEventName));
        // Also dispatch a generic event for legacy listeners
        window.dispatchEvent(new CustomEvent('agent:connected', {
          detail: { agentName, type }
        }));
      }
    }

    // Connect the actual WebSocket events to our handlers
    if (cloudflareAgent) {
      cloudflareAgent.addEventListener('open', handleSocketOpen);
      cloudflareAgent.addEventListener('close', handleSocketClose);
      cloudflareAgent.addEventListener('error', handleSocketError);
    }

    return () => {
      // Cleanup socket listeners
      if (cloudflareAgent) {
        cloudflareAgent.removeEventListener('open', handleSocketOpen);
        cloudflareAgent.removeEventListener('close', handleSocketClose);
        cloudflareAgent.removeEventListener('error', handleSocketError);
      }
    };
  }, [agentName, type, cloudflareAgent, state, connectionStatus]);

  // console.log(`[useOpenAgent ${agentName}] Agent initialized:`, {
  //   type,
  //   agentName,
  //   connectionStatus: cloudflareAgent ? 'created' : 'failed'
  // });

  /**
   * Submits a new user message to the agent
   */
  const handleSubmit = useCallback((message: string) => {
    // First create the new message
    const newMessage: UIMessage = {
      id: generateId(),
      role: 'user',
      content: message,
      parts: [{
        type: 'text',
        text: message
      }]
    };

    try {
      // Log that we're submitting a message
      console.log(`[useOpenAgent ${agentName}] Submitting message to agent:`, message);

      // Update the agent's state with the new message
      cloudflareAgent.setState({
        messages: [...(state?.messages || []), newMessage]
      });
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Failed to submit message:`, error);

      // Mark connection as error if it wasn't already
      if (connectionStatus !== 'error') {
        setConnectionStatus('error');
      }

      // Still update local state so the message appears in the UI
      setAgentState(prevState => ({
        ...prevState,
        messages: [...(prevState.messages || []), newMessage]
      }));
    }
  }, [cloudflareAgent, state?.messages, agentName, connectionStatus]);

  /**
   * Sets the messages in the agent's state
   */
  const setMessages = useCallback((messages: UIMessage[]) => {
    cloudflareAgent.setState({ messages });
  }, [cloudflareAgent]);

  /**
   * Sets the GitHub token in the agent's state and calls the setGithubToken method
   */
  const setGithubToken = useCallback(async (token: string): Promise<void> => {
    try {
      console.log(`[useOpenAgent ${agentName}] Setting GitHub token...`);
      // Call the method on the agent
      await cloudflareAgent.call('setGithubToken', [token]);
      console.log(`[useOpenAgent ${agentName}] GitHub token set successfully`);
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Failed to set GitHub token:`, error);
      // Re-throw to let caller handle error
      throw error;
    }
    return;
  }, [cloudflareAgent, agentName]);

  /**
   * Gets the GitHub token from the agent
   */
  const getGithubToken = useCallback(async (): Promise<string> => {
    try {
      console.log(`[useOpenAgent ${agentName}] Getting GitHub token...`);
      const result = await cloudflareAgent.call('getGithubToken', []);
      console.log(`[useOpenAgent ${agentName}] GitHub token retrieved successfully`);
      return result as string;
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Failed to get GitHub token:`, error);
      throw error;
    }
  }, [cloudflareAgent, agentName]);

  /**
   * Sets the current issue for the Solver agent
   */
  const setCurrentIssue = useCallback(async (issue: any): Promise<void> => {
    if (type === 'solver') {
      try {
        console.log(`[useOpenAgent ${agentName}] Setting current issue:`, issue.id);
        await cloudflareAgent.call('setCurrentIssue', [issue]);
        console.log(`[useOpenAgent ${agentName}] Current issue set successfully`);
      } catch (error) {
        console.error(`[useOpenAgent ${agentName}] Failed to set current issue:`, error);
        throw error;
      }
    }
    return;
  }, [cloudflareAgent, type, agentName]);

  /**
   * Sets the repository context for the agent
   */
  const setRepositoryContext = useCallback(async (owner: string, repo: string, branch: string = 'main'): Promise<void> => {
    try {
      console.log(`[useOpenAgent ${agentName}] Setting repository context: ${owner}/${repo}:${branch}`);
      await cloudflareAgent.call('setRepositoryContext', [owner, repo, branch]);
      console.log(`[useOpenAgent ${agentName}] Repository context set successfully`);
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Failed to set repository context:`, error);
      throw error;
    }
    return;
  }, [cloudflareAgent, agentName]);

  /**
   * Triggers the agent to generate a response
   */
  const infer = useCallback(async (token?: string): Promise<any> => {
    try {
      // Pass token in args array if needed by the agent's infer method
      const args = token ? [token] : [];
      console.log(`[useOpenAgent ${agentName}] Calling infer method...`);

      // Start a timeout to detect if the call is taking too long
      const timeoutId = setTimeout(() => {
        console.warn(`[useOpenAgent ${agentName}] Infer call is taking longer than expected`);
      }, 5000);

      const response = await cloudflareAgent.call('infer', args);
      clearTimeout(timeoutId);

      console.log(`[useOpenAgent ${agentName}] Infer response received:`, response);
      return response;
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Failed to call infer method:`, error);

      // Update connection status to error
      setConnectionStatus('error');

      // Throw the error so the caller can handle it
      throw error;
    }
  }, [cloudflareAgent, agentName, connectionStatus]);

  /**
   * Disconnects the WebSocket connection to the agent
   */
  const disconnect = useCallback(() => {
    try {
      console.log(`[useOpenAgent ${agentName}] Disconnecting WebSocket connection...`);

      // Close the WebSocket connection if it exists
      if (cloudflareAgent && cloudflareAgent.readyState !== 3) { // 3 = CLOSED
        cloudflareAgent.close(1000, "User initiated disconnect");
      }

      // Reset connection status
      setConnectionStatus('disconnected');

      // Reset agent state
      setAgentState({
        messages: []
      });

      console.log(`[useOpenAgent ${agentName}] WebSocket connection closed successfully`);
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Error disconnecting:`, error);
    }
  }, [cloudflareAgent, agentName]);

  // Return the OpenAgent interface with appropriate methods
  return {
    state,
    messages: state?.messages || [],
    setMessages,
    handleSubmit,
    infer,
    setGithubToken,
    getGithubToken,
    ...(type === 'solver' ? { setCurrentIssue } : {}),
    setRepositoryContext,  // Available for both agent types
    connectionStatus,      // Expose connection status to consumer components
    disconnect             // Method to properly disconnect the WebSocket
  };
}
