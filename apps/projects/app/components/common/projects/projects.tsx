import { useLoaderData } from 'react-router';
import ProjectLine from '@/components/common/projects/project-line';

interface Status {
  id: string;
  name: string;
  color: string;
}

interface Priority {
  id: string;
  name: string;
  color: string;
}

interface Health {
  id: string;
  name: string;
  color: string;
  description: string;
}

interface User {
  id: string;
  name: string;
  image: string | null;
}

interface Project {
  id: string;
  name: string;
  icon: string;
  color: string;
  percentComplete: number;
  startDate: string | null;
  targetDate: string | null;
  status: Status;
  priority: Priority;
  health: Health;
  lead: User | null;
}

interface LoaderData {
  projects: Project[];
  error?: string;
}

export default function Projects() {
  const { projects, error } = useLoaderData() as LoaderData;

  if (error) {
    return (
      <div className="w-full p-8 text-center">
        <p className="text-red-500">Error loading projects: {error}</p>
      </div>
    );
  }

  if (!projects || projects.length === 0) {
    return (
      <div className="w-full p-8 text-center">
        <p className="text-muted-foreground">No projects found. Create a project to get started.</p>
      </div>
    );
  }

  return (
    <div className="w-full">
      <div className="bg-container px-6 py-1.5 text-sm flex items-center text-muted-foreground border-b sticky top-0 z-10">
        <div className="w-[60%] sm:w-[70%] xl:w-[46%]">Title</div>
        <div className="w-[20%] sm:w-[10%] xl:w-[13%] pl-2.5">Health</div>
        <div className="hidden w-[10%] sm:block pl-2">Priority</div>
        <div className="hidden xl:block xl:w-[13%] pl-2">Lead</div>
        <div className="hidden xl:block xl:w-[13%] pl-2.5">Target date</div>
        <div className="w-[20%] sm:w-[10%] pl-2">Status</div>
      </div>

      <div className="w-full">
        {projects.map((project) => (
          <ProjectLine key={project.id} project={project} />
        ))}
      </div>
    </div>
  );
}