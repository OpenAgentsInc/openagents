import { useAgent } from "agents/react";
import { useState } from "react";
import { generateId, UIMessage } from "ai";

type AgentType = 'coder';

export type OpenAgent = {
  messages: UIMessage[];
  setMessages: (messages: UIMessage[]) => void;
  handleSubmit: (message: string) => void;
  infer: (token: string) => Promise<any>;
  setGithubToken: (token: string) => Promise<void>;
  getGithubToken: () => Promise<string>;
}

// later get this from the agents package
type AgentState = {
  messages: UIMessage[];
}

export function useOpenAgent(agentType: AgentType): OpenAgent {
  // const [messages, setMessages] = useState<UIMessage[]>(demoMessages);

  const [agentState, setAgentState] = useState<AgentState>({ messages: [] })

  const cloudflareAgent = useAgent({
    name: `${agentType}1234`,
    agent: agentType,
    onStateUpdate: (state: AgentState) => {
      // update local state
      setAgentState(state)
    }
  })

  const handleSubmit = (message: string) => {
    cloudflareAgent.setState({
      messages: [...(agentState?.messages || []), {
        id: generateId(),
        role: 'user',
        content: message,
        parts: [{
          type: 'text',
          text: message
        }]
      }]
    })
  }

  const infer = async (token: string) => {
    console.log('infer', token)
    return await cloudflareAgent.call('infer', [token])
  }

  const setGithubToken = async (token: string): Promise<void> => {
    await cloudflareAgent.call('setGithubToken', [token])
    return
  }

  const getGithubToken = async (): Promise<string> => {
    const result = await cloudflareAgent.call('getGithubToken')
    return result as string
  }

  return {
    messages: agentState?.messages || [],
    setMessages: (messages) => cloudflareAgent.setState({ messages }),
    handleSubmit,
    infer,
    setGithubToken,
    getGithubToken
  };
}
