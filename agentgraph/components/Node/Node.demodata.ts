import { Step } from '@/types/agents';

export const demoStep: Step = {
  agent_id: 1,
  category: 'validation',
  created_at: '2021-08-31T15:00:00.000Z',
  description: "Ensure input is a valid chat message",
  entry_type: 'input',
  error_message: "Could not validate input",
  id: 1,
  name: "Validate Input",
  order: 1,
  success_action: "next_node",
  task_id: 1,
  updated_at: '2021-08-31T15:00:00.000Z',
}
