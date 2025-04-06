import { AGENTS, AgentName } from './AGENTS';
import { generateObject } from 'ai';
import { z } from 'zod';
import { AgentRouterProvider } from './provider';

interface RoutedPrompt {
  agent_name: AgentName
  why: string
  instructions_for_agent: string
  user_prompt: string
}

const routingSchema = z.object({
  agent_name: z.custom<AgentName>((val) =>
    AGENTS.some(agent => agent.name === val),
    "Must be a valid agent name"
  ),
  why: z.string().describe("Explanation for why this agent was selected"),
  instructions_for_agent: z.string().describe("Instructions for the selected agent")
}).required();

export const inferRouted = async (
  provider: AgentRouterProvider,
  prompt: string
): Promise<RoutedPrompt | null> => {
  try {
    const { object } = await generateObject({
      model: provider.model,
      schema: routingSchema,
      schemaDescription: "Select the most appropriate agent based on the user's prompt",
      prompt: `Given these available agents:
${AGENTS.map(agent => `- ${agent.name}: ${agent.description}`).join('\n')}

User prompt: "${prompt}"

Select the most appropriate agent to handle this request.`,
      temperature: 0.1, // Low temperature for more deterministic routing
      headers: provider.headers
    });

    return {
      ...object,
      user_prompt: prompt
    } as RoutedPrompt;
  } catch (error) {
    console.error('Error in agent routing:', error);
    return null;
  }
}
