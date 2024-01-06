import { AgentShowcase } from "@/Components/builder/AgentShowcase";
import { SidebarLayout } from "@/Layouts/SidebarLayout";

function Showcase() {
  return <AgentShowcase />
}

Showcase.layout = (page) => <SidebarLayout children={page} />

export default Showcase
