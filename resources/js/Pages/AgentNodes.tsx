import { usePage } from "@inertiajs/react"
import { Canvas, Node } from 'agentgraph'
import { SidebarLayout } from "@/Layouts/SidebarLayout"
import { Agent, Step, Task } from "@/types/agents"

// We show all relevant agent nodes, starting with steps.
function AgentNodes() {
  const props = usePage().props as any
  const agent = props.agent as Agent
  const task = agent.tasks[0] as Task
  const steps = task.steps as Step[]
  // Show a Node per Step
  return (
    <Canvas>
      {steps.map((step, index) => (
        <Node key={index} step={step} position={{ x: 20 + 325 * index, y: 50 }} />
      ))}
    </Canvas>
  )
}

AgentNodes.layout = (page) => <SidebarLayout children={page} />

export default AgentNodes
