import { AgentShowcase } from "@/Components/builder/AgentShowcase";
import { NavLayout } from "@/Layouts/NavLayout";

function Showcase() {
  return <AgentShowcase />
}

Showcase.layout = (page) => <NavLayout children={page} />

export default Showcase
