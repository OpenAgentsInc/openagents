import { usePage } from "@inertiajs/react"

export default function AgentNodes() {
  const { agent } = usePage().props
  console.log(agent)
  return (
    <div className="flex flex-col justify-center items-center h-screen">
      <h1>Agent Nodes</h1>
    </div>
  )
}
