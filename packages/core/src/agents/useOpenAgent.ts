import { useState, useCallback, useEffect } from "react";
import { generateId, UIMessage } from "ai";

// Create a mock useAgent implementation for development/testing
// This will be replaced with the actual import in production
const createMockAgent = (config: any) => {
  const { name, agent, onStateUpdate } = config;
  const state: any = { messages: [] };
  
  // Initial state update
  setTimeout(() => {
    if (onStateUpdate) onStateUpdate(state);
  }, 0);
  
  return {
    setState: (newState: any) => {
      Object.assign(state, newState);
      if (onStateUpdate) onStateUpdate({...state});
    },
    call: async (method: string, args: any[]) => {
      console.log(`Mock agent call: ${method}`, args);
      
      // Simulate responses for different methods
      if (method === 'getGithubToken') {
        return localStorage.getItem('githubToken') || '';
      }
      
      if (method === 'setGithubToken') {
        const token = args[0];
        localStorage.setItem('githubToken', token);
        return;
      }
      
      if (method === 'infer') {
        // Simulate a response
        return {
          content: `This is a simulated response from the ${agent} agent.`,
          toolCalls: []
        };
      }
      
      return null;
    }
  };
};

// Define agent types
type AgentType = 'coder' | 'solver';

export type OpenAgent = {
  state: AgentState;
  messages: UIMessage[];
  setMessages: (messages: UIMessage[]) => void;
  handleSubmit: (message: string) => void;
  infer: (token: string) => Promise<any>;
  setGithubToken: (token: string) => Promise<void>;
  getGithubToken: () => Promise<string>;
  setCurrentIssue?: (issue: any) => Promise<void>;
  setRepositoryContext?: (owner: string, repo: string, branch?: string) => Promise<void>;
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

  // Try to dynamically import the Cloudflare Agents SDK
  // If it fails, use the mock implementation
  const mockAgent = createMockAgent({
    name: `${type}-${id}`,
    agent: type,
    onStateUpdate: (newState: AgentState) => {
      setAgentState(newState);
    }
  });

  // Use the mock agent for now
  // This can be replaced with the real Cloudflare agent when deployed
  const cloudflareAgent = mockAgent;

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
    
    // Update the agent's state with the new message
    cloudflareAgent.setState({
      messages: [...(state?.messages || []), newMessage]
    });
  }, [cloudflareAgent, state?.messages]);

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
    // Call the method on the agent
    await cloudflareAgent.call('setGithubToken', [token]);
    return;
  }, [cloudflareAgent]);

  /**
   * Gets the GitHub token from the agent
   */
  const getGithubToken = useCallback(async (): Promise<string> => {
    const result = await cloudflareAgent.call('getGithubToken', []);
    return result as string;
  }, [cloudflareAgent]);

  /**
   * Sets the current issue for the Solver agent
   */
  const setCurrentIssue = useCallback(async (issue: any): Promise<void> => {
    if (type === 'solver') {
      await cloudflareAgent.call('setCurrentIssue', [issue]);
    }
    return;
  }, [cloudflareAgent, type]);

  /**
   * Sets the repository context for the agent
   */
  const setRepositoryContext = useCallback(async (owner: string, repo: string, branch: string = 'main'): Promise<void> => {
    await cloudflareAgent.call('setRepositoryContext', [owner, repo, branch]);
    return;
  }, [cloudflareAgent]);

  /**
   * Triggers the agent to generate a response
   */
  const infer = useCallback(async (token: string): Promise<any> => {
    return await cloudflareAgent.call('infer', [token]);
  }, [cloudflareAgent]);

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
    setRepositoryContext  // Available for both agent types
  };
}