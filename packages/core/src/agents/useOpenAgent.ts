// Import React directly from the root node_modules
import React, { useState, useCallback, useEffect } from "react";
import { generateId, type UIMessage } from "ai";
// Explicitly import from the package
import { useAgent } from "agents/react";

// Define agent types
type AgentType = 'coder' | 'solver';

// Define a Message type that matches the Message type in UI package
export type Message = {
  id: string;
  role: 'user' | 'assistant' | 'system' | 'data';
  content: string;
  parts?: Array<{ type: string; text: string }>;
};

export type OpenAgent = {
  state: AgentState;
  messages: Message[];
  setMessages: (messages: Message[]) => void;
  handleSubmit: (message: string) => Promise<void>; // Changed to async for compatibility
  infer: (token?: string) => Promise<any>;
  setGithubToken: (token: string) => Promise<void>;
  getGithubToken: () => Promise<string>;
  setCurrentIssue?: (issue: any) => Promise<void>;
  setRepositoryContext?: (owner: string, repo: string, branch?: string) => Promise<void>;
  addAgentObservation: (observation: string) => Promise<void>; // Add method to send observations
  sendRawMessage: (message: any) => void; // Add method to send raw WebSocket messages
  getSystemPrompt: () => Promise<string>; // Method to get the agent's system prompt
  connectionStatus: 'connecting' | 'connected' | 'disconnected' | 'error';
  disconnect: () => void; // Method to properly disconnect
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
    
    const handleSocketMessage = (event: MessageEvent) => {
      try {
        // Parse the message
        const data = JSON.parse(event.data);
        
        // Don't log state messages as they generate too much noise
        if (data.type !== 'cf_agent_state') {
          console.log(`[useOpenAgent ${agentName}] Received WebSocket message:`, data);
        }
        
        // Handle different message types
        if (data.type === 'prompt_response' && data.requestId) {
          // Create the event name for this response
          const eventName = `${agentName}:prompt_response:${data.requestId}`;
          
          // Create a custom event with the response data
          const responseEvent = new CustomEvent(eventName, {
            detail: data
          });
          
          // Dispatch the event for the promise to handle
          window.dispatchEvent(responseEvent);
          console.log(`[useOpenAgent ${agentName}] Dispatched prompt response event for request ${data.requestId}`);
        }
      } catch (error) {
        console.error(`[useOpenAgent ${agentName}] Error handling WebSocket message:`, error);
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
    cloudflareAgent.addEventListener('message', handleSocketMessage);

    return () => {
      // Clean up listeners
      cloudflareAgent.removeEventListener('open', handleSocketOpen);
      cloudflareAgent.removeEventListener('close', handleSocketClose);
      cloudflareAgent.removeEventListener('error', handleSocketError);
      cloudflareAgent.removeEventListener('message', handleSocketMessage);
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
  const handleSubmit = useCallback(async (message: string): Promise<void> => {
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
      
      // Re-throw the error for the caller to handle
      throw error;
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

  // Convert UIMessages to Messages to satisfy the interface
  const convertUIMessagesToMessages = (uiMessages: UIMessage[]): Message[] => {
    return uiMessages.map(uiMsg => ({
      id: uiMsg.id,
      role: uiMsg.role,
      content: uiMsg.content,
      parts: uiMsg.parts?.map(part => ({
        type: part.type,
        text: 'text' in part ? part.text : JSON.stringify(part)
      }))
    }));
  };

  // Create message adapter function for setMessages
  const adaptSetMessages = (messages: Message[]) => {
    // Convert Messages to UIMessages (simplified conversion assuming compatible structure)
    const uiMessages = messages as unknown as UIMessage[];
    setMessages(uiMessages);
  };

  /**
   * Adds an observation to the agent's state using direct message
   */
  const addAgentObservation = useCallback(async (observation: string): Promise<void> => {
    try {
      console.log(`[useOpenAgent ${agentName}] Adding agent observation: ${observation}`);
      
      // Instead of calling a method, send a direct message with a specific format
      // that the agent's onMessage handler can recognize
      if (cloudflareAgent && typeof cloudflareAgent.send === 'function') {
        const observationMessage = {
          type: 'agent_observation',
          content: observation,
          timestamp: new Date().toISOString()
        };
        
        cloudflareAgent.send(JSON.stringify(observationMessage));
        console.log(`[useOpenAgent ${agentName}] Agent observation sent successfully`);
        return Promise.resolve();
      } else {
        console.error(`[useOpenAgent ${agentName}] Cannot add observation: Agent not available or send not a function`);
        throw new Error('Agent not available');
      }
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Failed to add agent observation:`, error);
      // Mark connection as error if needed
      if (connectionStatus !== 'error') {
        setConnectionStatus('error');
      }
      throw error;
    }
  }, [cloudflareAgent, agentName, connectionStatus]);
  
  /**
   * Sends a raw WebSocket message to the agent
   */
  const sendRawMessage = useCallback((message: any): void => {
    try {
      console.log(`[useOpenAgent ${agentName}] Sending raw message:`, message);
      
      if (cloudflareAgent && typeof cloudflareAgent.send === 'function') {
        // Send the raw message as JSON string
        cloudflareAgent.send(typeof message === 'string' ? message : JSON.stringify(message));
        console.log(`[useOpenAgent ${agentName}] Raw message sent successfully`);
      } else {
        console.error(`[useOpenAgent ${agentName}] Cannot send raw message: Agent not available or send not a function`);
      }
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Failed to send raw message:`, error);
    }
  }, [cloudflareAgent, agentName]);
  
  /**
   * Gets the system prompt from the agent using WebSocket messaging
   * This uses a special message type to request the system prompt
   * and returns a Promise that resolves when the response is received
   */
  const getSystemPrompt = useCallback(async (): Promise<string> => {
    try {
      console.log(`[useOpenAgent ${agentName}] Getting system prompt via WebSocket message...`);
      
      if (cloudflareAgent && typeof cloudflareAgent.send === 'function') {
        // Create a unique request ID for this system prompt request
        const requestId = `prompt_req_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;
        const eventName = `${agentName}:prompt_response:${requestId}`;
        
        // Create a promise that will be resolved when we get the response
        const responsePromise = new Promise<string>((resolve, reject) => {
          // Set a timeout to ensure we don't wait forever
          const timeoutId = setTimeout(() => {
            reject(new Error("Timeout waiting for system prompt response"));
            // Remove the event listener after timeout
            window.removeEventListener(eventName, handleResponse);
          }, 10000); // 10 second timeout
          
          // Function to handle the response - properly typed for EventListener
          const handleResponse = (event: Event) => {
            clearTimeout(timeoutId);
            // Cast to CustomEvent for type safety
            const customEvent = event as CustomEvent;
            const promptData = customEvent.detail;
            resolve(promptData.prompt || "No system prompt received");
            // Clean up by removing the event listener
            window.removeEventListener(eventName, handleResponse);
          };
          
          // Listen for the response event
          window.addEventListener(eventName, handleResponse);
          
          // Send the request message
          const message = {
            type: "get_system_prompt",
            requestId,
            timestamp: new Date().toISOString()
          };
          
          cloudflareAgent.send(JSON.stringify(message));
          console.log(`[useOpenAgent ${agentName}] System prompt request sent with ID ${requestId}`);
        });
        
        return await responsePromise;
      } else {
        console.error(`[useOpenAgent ${agentName}] Cannot get system prompt: Agent not available or send not a function`);
        throw new Error('Agent not available');
      }
    } catch (error) {
      console.error(`[useOpenAgent ${agentName}] Failed to get system prompt:`, error);
      
      // If we can't get the actual system prompt, return a fallback message
      return `Failed to retrieve system prompt for ${type} agent. The agent may not be connected or the method is not implemented.`;
    }
  }, [cloudflareAgent, agentName, type]);

  // Return the OpenAgent interface with appropriate methods
  return {
    state,
    messages: convertUIMessagesToMessages(state?.messages || []),
    setMessages: adaptSetMessages,
    handleSubmit,
    infer,
    setGithubToken,
    getGithubToken,
    addAgentObservation,
    sendRawMessage,        // Expose direct WebSocket messaging
    getSystemPrompt,       // Method to get the agent's system prompt
    ...(type === 'solver' ? { setCurrentIssue } : {}),
    setRepositoryContext,  // Available for both agent types
    connectionStatus,      // Expose connection status to consumer components
    disconnect             // Method to properly disconnect the WebSocket
  };
}
