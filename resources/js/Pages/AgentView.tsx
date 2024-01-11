import { usePage } from "@inertiajs/react"
import { Agent } from "@/types/agents"
import { NavLayout } from "@/Layouts/NavLayout"
import { AgentView } from "@/Components/agent/AgentView"
import { useEffect } from "react"

function AgentViewPage() {
  const props = usePage().props as any
  const agent = props.agent as Agent
  const conversation = props.conversation as any
  const files = props.files as any
  const owner = props.owner as string
  const user = props.auth?.user as any

  useEffect(() => {
    if (import.meta.env.VITE_ENV === "local") return
    if (!agent) return
    // @ts-ignore
    window.Echo.channel(`Agent.${agent.id}`)
      .listen('EmbeddingCreated', (e) => {
        // console.log("Received vector embedding from one page of a PDF")
      });
  }, [agent.id]);

  return (
    <AgentView agent={agent} conversation={conversation} files={files} owner={owner} user={user} />
  )
}

AgentViewPage.layout = (page) => <NavLayout children={page} />

export default AgentViewPage
