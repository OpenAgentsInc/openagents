import type { Route } from "./+types/home";
import MainLayout from '@/components/layout/main-layout';
import Header from '@/components/layout/headers/projects/header';
import Projects from '@/components/common/projects/projects';
import { getProjects, createProject, getProjectStatuses, getUsers } from '@/lib/db/project-helpers.server';
import { getTeamsForUser } from '@/lib/db/team-helpers.server';
import { getDb } from '@/lib/db/project-helpers.server';
import { redirect } from 'react-router';

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "Projects - OpenAgents" },
    { name: "description", content: "Manage your projects" },
  ];
}

export async function loader({ context, request }: Route.LoaderArgs) {
  // Import auth only within loader (server-side only)
  const { auth, requireAuth } = await import('@/lib/auth.server');
  try {
    // Check authentication with requireAuth helper
    const authResult = await requireAuth(request);
    
    if (authResult.redirect) {
      return redirect(authResult.redirect);
    }
    
    // Get user from the auth result
    const { user } = authResult;
    
    const projects = await getProjects();
    
    // Get additional data for modal selectors
    const statuses = await getProjectStatuses();
    const users = await getUsers();
    
    // Get teams that the current user is a member of
    const teams = await getTeamsForUser(user.id);
    
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
  // Import auth only within action (server-side only)
  const { auth, requireAuth } = await import('@/lib/auth.server');
  // Check authentication with requireAuth helper
  const authResult = await requireAuth(request);
  
  if (authResult.redirect) {
    return redirect(authResult.redirect);
  }
  
  // Get user from the auth result
  const { user } = authResult;
  
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
      
      // Check if we're using a default status ID
      if (projectData.statusId && projectData.statusId.startsWith('default-')) {
        // Create the status in the database first
        const db = getDb();
        const statusId = crypto.randomUUID();
        const statusType = projectData.statusId.replace('default-', '');
        
        let statusName = 'Unknown';
        let statusColor = '#808080';
        
        switch (statusType) {
          case 'backlog':
            statusName = 'Backlog';
            statusColor = '#95A5A6';
            break;
          case 'planned':
            statusName = 'Planned';
            statusColor = '#3498DB';
            break;
          case 'started':
            statusName = 'In Progress';
            statusColor = '#F1C40F';
            break;
          case 'completed':
            statusName = 'Completed';
            statusColor = '#2ECC71';
            break;
          case 'canceled':
            statusName = 'Canceled';
            statusColor = '#E74C3C';
            break;
        }
        
        try {
          await db
            .insertInto('project_status')
            .values({
              id: statusId,
              name: statusName,
              description: `Projects in ${statusName.toLowerCase()} state`,
              color: statusColor,
              type: statusType,
              position: getStatusPosition(statusType),
              indefinite: statusType === 'backlog' || statusType === 'planned' ? 1 : 0,
              createdAt: new Date().toISOString(),
              updatedAt: new Date().toISOString(),
            })
            .execute();
            
          // Update the project data with the real status ID
          projectData.statusId = statusId;
        } catch (error) {
          console.error('Error creating status:', error);
          // Continue with the default status ID
        }
      }
      
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

// Helper function to get status position
function getStatusPosition(type: string): number {
  switch (type) {
    case 'backlog': return 0;
    case 'planned': return 1;
    case 'started': return 2;
    case 'paused': return 3;
    case 'completed': return 4;
    case 'canceled': return 5;
    default: return 0;
  }
}

export default function ProjectsPage() {
  return (
    <MainLayout header={<Header />}>
      <Projects />
    </MainLayout>
  )
}