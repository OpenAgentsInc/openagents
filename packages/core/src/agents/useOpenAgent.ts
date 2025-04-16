import { useState, useCallback } from "react";
import { generateId, UIMessage } from "ai";
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
  const cloudflareAgent = useAgent({
    name: `${type}-${id}`, // Unique name combining type and ID
    agent: type,          // Agent type maps to URL path/DO binding name
    onStateUpdate: (newState: AgentState) => {
      // Update local state whenever agent state changes
      console.log(`[useOpenAgent ${type}-${id}] State updated from agent:`, newState);
      setAgentState(newState);
    }
  });

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
  const infer = useCallback(async (token?: string): Promise<any> => {
    // Pass token in args array if needed by the agent's infer method
    const args = token ? [token] : [];
    return await cloudflareAgent.call('infer', args);
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