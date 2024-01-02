import { Agent, Step, Task } from '@/types/agents';

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
  created_at: "2024-01-02T16:35:26.000000Z",
  id: 1,
  name: "The Concierge",
  tasks: [demoTask],
  updated_at: "2024-01-02T16:35:26.000000Z",
  user_id: 1
}
