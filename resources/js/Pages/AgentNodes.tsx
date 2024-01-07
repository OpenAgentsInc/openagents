import { usePage } from "@inertiajs/react"
import { Canvas, Node } from 'agentgraph'
import { SidebarLayout } from "@/Layouts/SidebarLayout"
import { Agent, Step, Task } from "@/types/agents"
import { NavLayout } from "@/Layouts/NavLayout"

// We show all relevant agent nodes, starting with steps.
function AgentNodes() {
  const props = usePage().props as any
  const agent = props.agent as Agent
  const task = agent.tasks[0] as Task
  const steps = task.steps as Step[]
  // Show a Node per Step
  return (
    <Canvas>
      <Node agent={agent} position={{ x: 480, y: 50 }} />
      <Node brain={agent.brains[0]} position={{ x: 715, y: 405 }} />
      {steps.map((step, index) => (
        <Node key={index} step={step} position={{ x: 20 + 325 * index, y: 180 }} />
      ))}
    </Canvas>
  )
}

AgentNodes.layout = (page) => <NavLayout children={page} />

export default AgentNodes
