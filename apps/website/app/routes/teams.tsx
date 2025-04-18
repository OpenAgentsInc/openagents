import type { Route } from "./+types/teams";
import Teams from '@/components/common/teams/teams';
import MainLayout from '@/components/layout/main-layout';
import Header from '@/components/layout/headers/teams/header';
import { getTeamsForUser } from '@/lib/db/team-helpers.server';
import { createTeam } from '@/lib/db/team-helpers.server';
// Server-side imports moved to loader and action
import { CreateTeam } from '@/components/layout/modals/create-team';
import { redirect } from 'react-router';

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "Teams - OpenAgents" },
    { name: "description", content: "Manage your teams" },
  ];
}

export async function loader({ request }: Route.LoaderArgs) {
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
    
    // Fetch teams the current user is a member of
    const teams = await getTeamsForUser(user.id);
    
    return { 
      teams,
      user
    };
  } catch (error) {
    console.error('Error loading teams:', error);
    return { 
      teams: [], 
      user: null,
      error: 'Failed to load teams'
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
  const action = formData.get('_action');
  
  if (action === 'createTeam') {
    try {
      // Extract team data from form
      const name = formData.get('name') as string;
      const description = formData.get('description') as string;
      const icon = formData.get('icon') as string;
      const color = formData.get('color') as string;
      const private_ = formData.get('private') === '1';
      const cyclesEnabled = formData.get('cyclesEnabled') === '1';
      
      // Validate required fields
      if (!name) {
        return { success: false, error: 'Team name is required' };
      }
      
      // Create the team
      const teamId = await createTeam({
        name,
        description: description || '',
        icon: icon || '👥',
        color: color || '#6366F1',
        private: private_,
        cyclesEnabled,
      }, user.id);
      
      return { 
        success: true, 
        teamId,
        message: 'Team created successfully' 
      };
    } catch (error) {
      console.error('Error creating team:', error);
      return { 
        success: false, 
        error: 'Failed to create team'
      };
    }
  }
  
  return { success: false, error: 'Unknown action' };
}

export default function TeamsPage() {
  return (
    <MainLayout header={<Header />}>
      <Teams />
      <CreateTeam />
    </MainLayout>
  );
}