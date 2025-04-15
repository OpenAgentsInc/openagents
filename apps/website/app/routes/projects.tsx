import type { Route } from "./+types/home";

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "OpenAgents" },
    { name: "description", content: "Your agent dealer" },
  ];
}

export default function Projects() {
  return (
    <MainLayout header={<Header />}>
      <Projects />
    </MainLayout>
  )
}
