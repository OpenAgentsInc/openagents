import type { Route } from "./+types/home";
import MainLayout from '@/components/layout/main-layout';
import Header from '@/components/layout/headers/projects/header';
import Projects from '@/components/common/projects/projects';
import { getProjects, createProject } from '@/lib/db/project-helpers';

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

export async function action({ request }: Route.ActionArgs) {
  // Get the current logged-in user (in a real app, this would come from the auth system)
  // For now, we'll use a hardcoded test user ID
  const testUserId = "test-user-123"; // This should be replaced with a real user ID from auth
  
  const formData = await request.formData();
  const action = formData.get('action');
  
  if (action === 'createProject') {
    try {
      const projectDataStr = formData.get('project') as string;
      if (!projectDataStr) {
        return { success: false, error: 'Project data is required' };
      }
      
      const projectData = JSON.parse(projectDataStr);
      
      // Add the current user as the creator
      projectData.creatorId = testUserId;
      
      // Create the project
      const projectId = await createProject(projectData);
      
      return { 
        success: true, 
        projectId,
        message: 'Project created successfully' 
      };
    } catch (error) {
      console.error('Error creating project:', error);
      return { 
        success: false, 
        error: 'Failed to create project' 
      };
    }
  }
  
  return { success: false, error: 'Unknown action' };
}

export default function ProjectsPage() {
  return (
    <MainLayout header={<Header />}>
      <Projects />
    </MainLayout>
  )
}