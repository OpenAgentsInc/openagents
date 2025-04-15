import { useLoaderData, useParams } from "react-router";
import type { Route } from "../+types/projects";
import MainLayout from '@/components/layout/main-layout';
import Header from '@/components/layout/headers/projects/header';

interface Project {
  id: string;
  name: string;
  description: string;
  // Add other project fields as needed
}

export function meta({ params, location, data }: Route.MetaArgs) {
  return [
    { title: `Project: ${params.id}` },
    { name: "description", content: "View project details" },
  ];
}

export async function loader({ params }: Route.LoaderArgs) {
  const { id } = params;

  // TODO: Implement actual project data fetching
  // For now, just return the ID
  return { id };
}

export default function ProjectDetails() {
  const { id } = useParams();
  const data = useLoaderData() as { id: string };

  return (
    <MainLayout header={<Header />}>
      <div className="container mx-auto py-8">
        <h1 className="text-3xl font-bold mb-6">Project {id}</h1>
        {/* Add your project details UI components here */}
      </div>
    </MainLayout>
  );
}
