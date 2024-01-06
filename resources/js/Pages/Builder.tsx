import { AgentBuilder } from "@/Components/builder/AgentBuilder";
import { SidebarLayout } from "@/Layouts/SidebarLayout";

function Builder() {
  return <AgentBuilder />
}

Builder.layout = (page) => <SidebarLayout children={page} />

export default Builder
