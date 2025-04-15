import type { Route } from "./+types/home";
import MainLayout from '@/components/layout/main-layout';
import Header from '@/components/layout/headers/projects/header';
import Projects from '@/components/common/projects/projects';

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "OpenAgents" },
    { name: "description", content: "Your agent dealer" },
  ];
}

export default function ProjectsPage() {
  return (
    <MainLayout header={<Header />}>
      <Projects />
    </MainLayout>
  )
}
