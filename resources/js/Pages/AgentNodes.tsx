import { SidebarLayout } from "@/Layouts/SidebarLayout"
import { usePage } from "@inertiajs/react"

function AgentNodes() {
  const { agent } = usePage().props
  return (
    <div className="flex flex-col justify-center items-center h-screen">
    </div>
  )
}

AgentNodes.layout = (page) => <SidebarLayout children={page} grid={true} />

export default AgentNodes
