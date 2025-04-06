import { AGENTS, AgentName } from './AGENTS';

interface RoutedPrompt {
  agent_name: AgentName
  why: string
  instructions_for_agent: string
}

export const inferRouted = (prompt: string): RoutedPrompt | null => {
  const agent = AGENTS.find((agent) => prompt.includes(agent.name));
  if (!agent) {
    return null;
  }
  return {
    agent_name: agent.name,
    why: `User's prompt mentions ${agent.name}, who ${agent.description.toLowerCase()}`,
    instructions_for_agent: prompt
  };
}
