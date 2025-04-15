import type { Route } from "./+types/home";
import MainLayout from '@/components/layout/main-layout';
import Header from '@/components/layout/headers/projects/header';
import Projects from '@/components/common/projects/projects';
import { getProjects, createProject, getProjectStatuses, getUsers, getTeams } from '@/lib/db/project-helpers.server';
import { auth } from '@/lib/auth';

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "Projects - OpenAgents" },
    { name: "description", content: "Manage your projects" },
  ];
}

export async function loader({ context, request }: Route.LoaderArgs) {
  try {
    // Get current session with better-auth
    const { user } = await auth.api.getSession(request);
    
    const projects = await getProjects();
    
    // Get additional data for modal selectors
    const statuses = await getProjectStatuses();
    const users = await getUsers();
    const teams = await getTeams();
    
    return { 
      projects,
      options: {
        statuses,
        users,
        teams
      },
      user
    };
  } catch (error) {
    console.error('Error loading projects:', error);
    return { 
      projects: [], 
      options: {
        statuses: [],
        users: [],
        teams: []
      },
      user: null,
      error: 'Failed to load projects' 
    };
  }
}

export async function action({ request }: Route.ActionArgs) {
  // Get the currently authenticated user
  const { user } = await auth.api.getSession(request);
  
  // Check if user is authenticated
  if (!user) {
    return { success: false, error: 'Authentication required' };
  }
  
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
      projectData.creatorId = user.id;
      
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