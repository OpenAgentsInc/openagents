interface Datapoint {
  data: string
  embedding?: number[]
}

export interface Brain {
  created_at: string
  id: number
  datapoints: Datapoint[]
}

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
  params?: any
  success_action: string
  task_id: number
  updated_at: string
}

export interface Task {
  agent_id: number
  created_at: string
  description: string
  id: number
  output: any
  steps: Step[]
  updated_at: string
}

export interface Agent {
  brains: Brain[]
  created_at: string
  description: string
  id: number
  instructions: string
  name: string
  balance?: number
  tasks: Task[]
  updated_at: string
  user_id: number
  welcome_message: string
}
