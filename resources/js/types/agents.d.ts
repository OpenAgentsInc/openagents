export interface Step {
  agent_id: number
  category: string
  created_at: string
  description: string
  entry_type: string
  error_message: string
  id: number
  name: string
  order: number
  params: any
  success_action: string
  task_id: number
  updated_at: string
}

export interface Task {
  description: string
  steps: Step[]
}

export interface Agent {
  tasks: Task[]
}
