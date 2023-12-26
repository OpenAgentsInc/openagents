import { SidebarLayout } from "@/Layouts/SidebarLayout"
import { usePage } from "@inertiajs/react"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/Components/ui/card';

// We show all relevant agent nodes, starting with steps.

interface Agent {
  tasks: Task[]
}

interface Task {
  description: string
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
  // Show a Card per Step
  return (
    <div className="flex flex-col justify-center items-center h-screen">
      <div className="pt-6 px-8 rounded-lg">
        <Card>
          <CardHeader>
            <CardTitle>Task</CardTitle>
          </CardHeader>
          <CardContent>
            <p>{task.description}</p>
          </CardContent>
        </Card>
        <div className="my-6">
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            {steps.map((step, index) => (
              <Card key={index}
                onClick={() => {
                  // router.get(`/step/${step.id}`)
                }}

                style={{ cursor: 'pointer' }}
              >
                <CardHeader>
                  <CardTitle>Step {index + 1}: {step.name}</CardTitle>
                  <CardDescription>{step.description}</CardDescription>
                </CardHeader>
                {/* <CardContent>
                  <div className="flex justify-between items-center">
                    <span className={`text-${step.status === 'success' ? 'green' : 'red'}-400`}>{step.status}</span>
                  </div>
                </CardContent> */}
              </Card>
            ))}
          </div>
        </div>
      </div>

    </div>
  )
}

AgentNodes.layout = (page) => <SidebarLayout children={page} grid={true} />

export default AgentNodes
