// Import React directly from the root node_modules
import React, { useState, useCallback, useEffect } from "react";
import { generateId, type UIMessage } from "ai";
// Explicitly import from the package
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

  // Track connection status for logging purposes
  const [connectionStatus, setConnectionStatus] = useState<'connecting' | 'connected' | 'disconnected' | 'error'>('disconnected');

  // We'll use custom event names with prefixes to avoid recursion
  const connectedEventName = `agent:${agentName}:connected`;
  const disconnectedEventName = `agent:${agentName}:disconnected`;
  const errorEventName = `agent:${agentName}:error`;

  // Single event handling effect to avoid duplication
  useEffect(() => {
    if (!cloudflareAgent) {
      console.warn(`[useOpenAgent ${agentName}] Agent not available for event listeners`);
      return;
    }

    const handleSocketOpen = () => {
      console.log(`[useOpenAgent ${agentName}] WebSocket connection opened successfully`);
      setConnectionStatus('connected');
      setConnectionAttempts(0); // Reset connection attempts on successful connection

      // Dispatch a custom event that components can listen for
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent(connectedEventName));
        window.dispatchEvent(new CustomEvent('agent:connected', {
          detail: { agentName, type }
        }));
      }
    };

    const handleSocketClose = () => {
      console.log(`[useOpenAgent ${agentName}] WebSocket connection closed`);
      setConnectionStatus('disconnected');

      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent(disconnectedEventName));
        window.dispatchEvent(new CustomEvent('agent:disconnected', {
          detail: { agentName, type }
        }));
      }
    };

    const handleSocketError = (error: any) => {
      console.error(`[useOpenAgent ${agentName}] WebSocket connection error:`, error);
      setConnectionStatus('error');

      // Only attempt to reconnect if we haven't exceeded max attempts
      if (connectionAttempts < maxReconnectAttempts) {
        setConnectionAttempts(prev => prev + 1);
        console.log(`[useOpenAgent ${agentName}] Auto-reconnect attempt ${connectionAttempts + 1}/${maxReconnectAttempts}`);
      } else {
        console.error(`[useOpenAgent ${agentName}] Max reconnection attempts (${maxReconnectAttempts}) reached. Giving up.`);
      }

      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent(errorEventName));
        window.dispatchEvent(new CustomEvent('agent:error', {
          detail: { agentName, type }
        }));
      }
    };

    // If we already received a state update, we're already connected
    if (state && Object.keys(state).length > 0 && connectionStatus === 'disconnected') {
      console.log(`[useOpenAgent ${agentName}] Already connected with state:`, state);
      setConnectionStatus('connected');
    }

    // Add event listeners
    cloudflareAgent.addEventListener('open', handleSocketOpen);
    cloudflareAgent.addEventListener('close', handleSocketClose);
    cloudflareAgent.addEventListener('error', handleSocketError);

    return () => {
      // Clean up listeners
      cloudflareAgent.removeEventListener('open', handleSocketOpen);
      cloudflareAgent.removeEventListener('close', handleSocketClose);
      cloudflareAgent.removeEventListener('error', handleSocketError);
    };
  }, [cloudflareAgent, agentName, type, maxReconnectAttempts, connectionAttempts, state, connectionStatus]);

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
      if (cloudflareAgent && typeof cloudflareAgent.setState === 'function') {
        cloudflareAgent.setState({
          messages: [...(state?.messages || []), newMessage]
        });
      } else {
        console.error(`[useOpenAgent ${agentName}] Cannot submit message: Agent not available or setState not a function`);
        throw new Error('Agent not available');
      }
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
    if (cloudflareAgent && typeof cloudflareAgent.setState === 'function') {
      try {
        cloudflareAgent.setState({ messages });
      } catch (error) {
        console.error(`[useOpenAgent ${agentName}] Failed to set messages:`, error);
        setConnectionStatus('error');
      }
    } else {
      console.warn(`[useOpenAgent ${agentName}] Cannot set messages: Agent not available`);
      setConnectionStatus('error');
    }
  }, [cloudflareAgent, agentName, setConnectionStatus]);

  /**
   * Sets the GitHub token in the agent's state and calls the setGithubToken method
   */
  const setGithubToken = useCallback(async (token: string): Promise<void> => {
    try {
      console.log(`[useOpenAgent ${agentName}] Setting GitHub token...`);
      
      if (cloudflareAgent && typeof cloudflareAgent.call === 'function') {
        // Call the method on the agent
        await cloudflareAgent.call('setGithubToken', [token]);
        console.log(`[useOpenAgent ${agentName}] GitHub token set successfully`);
      } else {
        console.error(`[useOpenAgent ${agentName}] Cannot set GitHub token: Agent not available or call not a function`);
        throw new Error('Agent not available');
      }
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Failed to set GitHub token:`, error);
      // Mark connection as error
      if (connectionStatus !== 'error') {
        setConnectionStatus('error');
      }
      // Re-throw to let caller handle error
      throw error;
    }
  }, [cloudflareAgent, agentName, connectionStatus]);

  /**
   * Gets the GitHub token from the agent
   */
  const getGithubToken = useCallback(async (): Promise<string> => {
    try {
      console.log(`[useOpenAgent ${agentName}] Getting GitHub token...`);
      
      if (cloudflareAgent && typeof cloudflareAgent.call === 'function') {
        const result = await cloudflareAgent.call('getGithubToken', []);
        console.log(`[useOpenAgent ${agentName}] GitHub token retrieved successfully`);
        return result as string;
      } else {
        console.error(`[useOpenAgent ${agentName}] Cannot get GitHub token: Agent not available or call not a function`);
        throw new Error('Agent not available');
      }
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Failed to get GitHub token:`, error);
      // Mark connection as error
      if (connectionStatus !== 'error') {
        setConnectionStatus('error');
      }
      throw error;
    }
  }, [cloudflareAgent, agentName, connectionStatus]);

  /**
   * Sets the current issue for the Solver agent
   */
  const setCurrentIssue = useCallback(async (issue: any): Promise<void> => {
    if (type === 'solver') {
      try {
        console.log(`[useOpenAgent ${agentName}] Setting current issue:`, issue.id);
        
        if (cloudflareAgent && typeof cloudflareAgent.call === 'function') {
          await cloudflareAgent.call('setCurrentIssue', [issue]);
          console.log(`[useOpenAgent ${agentName}] Current issue set successfully`);
        } else {
          console.error(`[useOpenAgent ${agentName}] Cannot set current issue: Agent not available or call not a function`);
          throw new Error('Agent not available');
        }
      } catch (error) {
        console.error(`[useOpenAgent ${agentName}] Failed to set current issue:`, error);
        // Mark connection as error
        if (connectionStatus !== 'error') {
          setConnectionStatus('error');
        }
        throw error;
      }
    }
  }, [cloudflareAgent, type, agentName, connectionStatus]);

  /**
   * Sets the repository context for the agent
   */
  const setRepositoryContext = useCallback(async (owner: string, repo: string, branch: string = 'main'): Promise<void> => {
    try {
      console.log(`[useOpenAgent ${agentName}] Setting repository context: ${owner}/${repo}:${branch}`);
      
      if (cloudflareAgent && typeof cloudflareAgent.call === 'function') {
        await cloudflareAgent.call('setRepositoryContext', [owner, repo, branch]);
        console.log(`[useOpenAgent ${agentName}] Repository context set successfully`);
      } else {
        console.error(`[useOpenAgent ${agentName}] Cannot set repository context: Agent not available or call not a function`);
        throw new Error('Agent not available');
      }
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Failed to set repository context:`, error);
      // Mark connection as error
      if (connectionStatus !== 'error') {
        setConnectionStatus('error');
      }
      throw error;
    }
  }, [cloudflareAgent, agentName, connectionStatus]);

  /**
   * Triggers the agent to generate a response
   */
  const infer = useCallback(async (token?: string): Promise<any> => {
    try {
      // Pass token in args array if needed by the agent's infer method
      const args = token ? [token] : [];
      console.log(`[useOpenAgent ${agentName}] Calling infer method...`);

      if (cloudflareAgent && typeof cloudflareAgent.call === 'function') {
        // Start a timeout to detect if the call is taking too long
        const timeoutId = setTimeout(() => {
          console.warn(`[useOpenAgent ${agentName}] Infer call is taking longer than expected`);
        }, 5000);

        const response = await cloudflareAgent.call('infer', args);
        clearTimeout(timeoutId);

        console.log(`[useOpenAgent ${agentName}] Infer response received:`, response);
        return response;
      } else {
        console.error(`[useOpenAgent ${agentName}] Cannot call infer: Agent not available or call not a function`);
        throw new Error('Agent not available');
      }
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

      // Close the WebSocket connection if it exists and not already closed
      if (cloudflareAgent && typeof cloudflareAgent.close === 'function' && cloudflareAgent.readyState !== 3) { // 3 = CLOSED
        cloudflareAgent.close(1000, "User initiated disconnect");
      } else if (cloudflareAgent && cloudflareAgent.readyState !== 3) {
        console.warn(`[useOpenAgent ${agentName}] Cannot close connection: close method not available`);
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
