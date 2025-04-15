import type { Route } from "./+types/home";
import MainLayout from '@/components/layout/main-layout';
import Header from '@/components/layout/headers/projects/header';
import Projects from '@/components/common/projects/projects';
import { getProjects } from '@/lib/db/project-helpers';

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "Projects - OpenAgents" },
    { name: "description", content: "Manage your projects" },
  ];
}

export async function loader({ context }: Route.LoaderArgs) {
  try {
    const projects = await getProjects();
    return { projects };
  } catch (error) {
    console.error('Error loading projects:', error);
    return { projects: [], error: 'Failed to load projects' };
  }
}

export default function ProjectsPage() {
  return (
    <MainLayout header={<Header />}>
      <Projects />
    </MainLayout>
  )
}