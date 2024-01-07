import { AgentBuilder } from "@/Components/builder/AgentBuilder";
import { NavLayout } from "@/Layouts/NavLayout";

function Builder() {
  return <AgentBuilder />
}

Builder.layout = (page) => <NavLayout children={page} />

export default Builder
