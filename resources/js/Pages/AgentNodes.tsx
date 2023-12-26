import { SidebarLayout } from "@/Layouts/SidebarLayout"
import { usePage } from "@inertiajs/react"

// We show all relevant agent nodes, starting with steps.

interface Agent {
  tasks: Task[]
}

interface Task {
  steps: Step[]
}

interface Step {
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

function AgentNodes() {
  const props = usePage().props as any
  const agent = props.agent as Agent
  const task = agent.tasks[0] as Task
  const steps = task.steps as Step[]
  console.log(steps)
  return (
    <div className="flex flex-col justify-center items-center h-screen">
    </div>
  )
}

AgentNodes.layout = (page) => <SidebarLayout children={page} grid={true} />

export default AgentNodes
