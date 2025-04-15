import { useLoaderData, useParams } from "react-router";
import type { Route } from "../+types/projects";
import MainLayout from '@/components/layout/main-layout';
import { HeaderIssues } from '@/components/layout/headers/issues/header';
import AllIssues from '@/components/common/issues/all-issues';
import { auth } from "@/lib/auth";
import { redirect } from "react-router";
import { getProjectById } from "@/lib/db/project-helpers.server";
import { getWorkflowStates, getIssueLabels } from "@/lib/db/issue-helpers.server";
import { getTeamsForUser } from "@/lib/db/team-helpers.server";
import { getUsers } from "@/lib/db/project-helpers.server";
import { CreateIssueModalProvider } from "@/components/common/issues/create-issue-modal-provider";

export function meta({ params, location, data }: Route.MetaArgs) {
  return [
    { title: `Project: ${params.id}` },
    { name: "description", content: "View project details" },
  ];
}

export async function loader({ params, request }: Route.LoaderArgs) {
  try {
    const { id } = params;
    
    // Get current session with better-auth
    const { user, session } = await auth.api.getSession(request);
    
    if (!session) {
      return redirect("/login");
    }
    
    const project = await getProjectById(id as string);
    
    if (!project) {
      throw new Error("Project not found");
    }
    
    // Load data needed for the issue creation modal
    const [workflowStates, labels, teams, users] = await Promise.all([
      getWorkflowStates(),
      getIssueLabels(),
      getTeamsForUser(user.id),
      getUsers()
    ]);
    
    // --- START DEBUG LOG ---
    console.log(`[DEBUG] /projects/$id Loader - Fetched ${workflowStates?.length ?? 0} workflow states:`, JSON.stringify(workflowStates));
    console.log(`[DEBUG] /projects/$id Loader - Fetched ${teams?.length ?? 0} teams for user ${user.id}:`, JSON.stringify(teams));
    // --- END DEBUG LOG ---
    
    return { 
      project,
      options: {
        workflowStates,
        labels,
        teams,
        users
      },
      user
    };
  } catch (error) {
    console.error("Error loading project:", error);
    return { 
      error: "Failed to load project",
      options: {
        workflowStates: [],
        labels: [],
        teams: [],
        users: []
      }
    };
  }
}

export default function ProjectDetails() {
  const { id } = useParams();
  const data = useLoaderData();
  const project = data.project;
  
  if (data.error) {
    return (
      <MainLayout>
        <div className="container mx-auto p-6">
          <h1 className="text-2xl font-bold">Error</h1>
          <p className="text-red-500">{data.error}</p>
        </div>
      </MainLayout>
    );
  }
  
  return (
    <MainLayout header={<HeaderIssues />}>
      <div className="container mx-auto p-6">
        <h1 className="text-2xl font-bold mb-6">{project.name}</h1>
        <AllIssues />
      </div>
      <CreateIssueModalProvider />
    </MainLayout>
  );
}