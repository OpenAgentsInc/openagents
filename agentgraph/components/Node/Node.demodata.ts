import { Agent, Brain, Step, Task } from '@/types/agents';

export const demoBrain: Brain = {
  created_at: "2024-01-02T23:50:33.000000Z",
  id: 1,
  datapoints: [ // We're not going to render the embeddings client-side so don't need them here
    { data: "OpenAgents is an open platform for AI agents." },
    { data: "Marketing copy: Soon every person and company will have multiple AI agents working on their behalf. Who will own those agents? A closed-source megacorp with a history of monopolization and regulatory capture? Or an open cloud built on open models and open data?" },
    { data: "Do not mention OpenAI or other companies. Do not ever say 'real estate', these are AI agents."},
    { data: "Supercharge your productivity. How many agents will you want working for you?"},
    { data: "OpenAgents benefit #1: Configurable. Configure your agent with a large selection of open models, customizable prompts, and third-party integrations."},
    { data: "OpenAgents benefit #2: Deploy to our cloud. Put them in the open compute network - we handle the hosting for you. No code or difficult setup required."},
    { data: "OpenAgents benefit #3: Infinite work. Why stop? These are long-running processes that will keep working as long as compute is paid for."},
    { data: "OpenAgents benefit #4: Earn and spend. Agents can earn and spend on your behalf using the native currency of the internet: Bitcoin."}
  ]
}

export const demoSteps: Step[] = [
  {
    agent_id: 1,
    category: 'validation',
    created_at: '2023-08-31T15:00:00.000Z',
    description: "Ensure input is a valid chat message",
    entry_type: 'input',
    error_message: "Could not validate input",
    id: 1,
    name: "Validate Input",
    order: 1,
    success_action: "next_node",
    task_id: 1,
    updated_at: '2023-08-31T15:00:00.000Z',
  },
  {
    agent_id: 1,
    category: 'embedding',
    created_at: '2023-08-31T15:00:00.000Z',
    description: "Convert input to vector embedding",
    entry_type: 'node',
    error_message: "Could not generate embedding",
    id: 2,
    name: "Embed Input",
    order: 2,
    success_action: "next_node",
    task_id: 1,
    updated_at: '2023-08-31T15:00:00.000Z',
  },
  {
    agent_id: 1,
    category: 'similarity_search',
    created_at: '2023-08-31T15:00:00.000Z',
    description: "Compare input to knowledge base",
    entry_type: 'node',
    error_message: "Could not run similarity search",
    id: 3,
    name: "Similarity Search",
    order: 3,
    success_action: "next_node",
    task_id: 1,
    updated_at: '2023-08-31T15:00:00.000Z',
  },
  {
    agent_id: 1,
    category: 'inference',
    created_at: '2023-08-31T15:00:00.000Z',
    description: "Call to LLM to generate response",
    entry_type: 'node',
    error_message: "Could not call to LLM",
    id: 4,
    name: "Call LLM",
    order: 4,
    success_action: "json_response",
    task_id: 1,
    updated_at: '2023-08-31T15:00:00.000Z',
  }
];

export const demoStep: Step = demoSteps[0]

export const demoTask: Task = {
  agent_id: 1,
  created_at: "2024-01-02T16:35:26.000000Z",
  description: "Respond to user chat message after consulting knowledge base",
  id: 49,
  output: null,
  steps: demoSteps,
  updated_at: "2024-01-02T16:35:26.000000Z"
}

export const demoAgent: Agent = {
  brains: [demoBrain],
  created_at: "2024-01-02T16:35:26.000000Z",
  id: 1,
  name: "The Concierge",
  balance: 42,
  tasks: [demoTask],
  updated_at: "2024-01-02T16:35:26.000000Z",
  user_id: 1
}
