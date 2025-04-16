import { useState, useCallback } from "react";
import { generateId, UIMessage } from "ai";

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
 * Fallback implementation for when the Cloudflare Agents SDK is not available
 * @param id Unique identifier for this agent instance
 * @param type Type of agent to connect to ('coder' or 'solver')
 * @returns An OpenAgent interface for interacting with the agent
 */
export function useOpenAgent(id: string, type: AgentType = "coder"): OpenAgent {
  // Setup local state
  const [state, setState] = useState<AgentState>({ 
    messages: [],
    githubToken: undefined,
    currentIssue: undefined,
    currentRepoOwner: undefined,
    currentRepoName: undefined,
    currentBranch: undefined
  });

  /**
   * Submits a new user message
   */
  const handleSubmit = useCallback((message: string) => {
    const newMessage: UIMessage = {
      id: generateId(),
      role: 'user',
      content: message,
      parts: [{
        type: 'text',
        text: message
      }]
    };
    
    setState(prevState => ({
      ...prevState,
      messages: [...prevState.messages, newMessage]
    }));
  }, []);

  /**
   * Sets the messages
   */
  const setMessages = useCallback((messages: UIMessage[]) => {
    setState(prevState => ({
      ...prevState,
      messages
    }));
  }, []);

  /**
   * Sets the GitHub token
   */
  const setGithubToken = useCallback(async (token: string): Promise<void> => {
    setState(prevState => ({
      ...prevState,
      githubToken: token
    }));
    
    return Promise.resolve();
  }, []);

  /**
   * Gets the GitHub token
   */
  const getGithubToken = useCallback(async (): Promise<string> => {
    return state.githubToken || '';
  }, [state.githubToken]);

  /**
   * Sets the current issue
   */
  const setCurrentIssue = useCallback(async (issue: any): Promise<void> => {
    if (type === 'solver') {
      setState(prevState => ({
        ...prevState,
        currentIssue: issue
      }));
    }
    return Promise.resolve();
  }, [type]);

  /**
   * Sets the repository context
   */
  const setRepositoryContext = useCallback(async (owner: string, repo: string, branch: string = 'main'): Promise<void> => {
    setState(prevState => ({
      ...prevState,
      currentRepoOwner: owner,
      currentRepoName: repo,
      currentBranch: branch
    }));
    return Promise.resolve();
  }, []);

  /**
   * Simulates inference
   */
  const infer = useCallback(async (token: string) => {
    // Simulate a response after a short delay
    return new Promise(resolve => {
      setTimeout(() => {
        const response = {
          id: generateId(),
          role: 'assistant',
          content: `This is a simulated response from the ${type} agent.`,
          parts: [{
            type: 'text',
            text: `This is a simulated response from the ${type} agent.`
          }]
        };
        
        setState(prevState => ({
          ...prevState,
          messages: [...prevState.messages, response]
        }));
        
        resolve({
          content: response.content,
          toolCalls: []
        });
      }, 1000);
    });
  }, [type]);

  // Return the OpenAgent interface with appropriate methods
  return {
    state,
    messages: state.messages,
    setMessages,
    handleSubmit,
    infer,
    setGithubToken,
    getGithubToken,
    ...(type === 'solver' ? { setCurrentIssue } : {}),
    setRepositoryContext
  };
}