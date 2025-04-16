import { useLoaderData, useParams } from "react-router";
import type { Route } from "../+types/projects";
import MainLayout from '@/components/layout/main-layout';
import { HeaderIssues } from '@/components/layout/headers/issues/header';
import AllIssues from '@/components/common/issues/all-issues';
// Server-side imports moved to loader and action
import { redirect } from "react-router";
import { getProjectById } from "@/lib/db/project-helpers.server";
import { getWorkflowStates, getIssueLabels } from "@/lib/db/issue-helpers.server";
import { getTeamsForUser } from "@/lib/db/team-helpers.server";
import { getUsers } from "@/lib/db/project-helpers.server";
import { CreateIssueModalProvider } from "@/components/common/issues/create-issue-modal-provider";
import { useIssuesStore } from "@/store/issues-store";
import { useEffect } from "react";

export function meta({ params, location, data }: Route.MetaArgs) {
  // Use type assertion to access project data
  const loaderData = data as Route.ProjectLoaderData;

  // Use the project name from the loader data if available
  const projectName = loaderData?.project?.name || "Project Details";

  return [
    { title: `${projectName} - OpenAgents` },
    { name: "description", content: `View details for ${projectName}` },
  ];
}

export async function loader({ params, request }: Route.LoaderArgs) {
  // Import auth only within loader (server-side only)
  const { auth, requireAuth } = await import('@/lib/auth.server');
  try {
    const { id } = params;

    // Check authentication with requireAuth helper
    const authResult = await requireAuth(request);

    if (authResult.redirect) {
      return redirect(authResult.redirect);
    }

    // Get user and session from the auth result
    const { user, session } = authResult;

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
    // console.log(`[DEBUG] /projects/$id Loader - Fetched ${workflowStates?.length ?? 0} workflow states:`, JSON.stringify(workflowStates));
    // console.log(`[DEBUG] /projects/$id Loader - Fetched ${teams?.length ?? 0} teams for user ${user.id}:`, JSON.stringify(teams));
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
  const { setIssues, setWorkflowStates } = useIssuesStore();

  // Update issues store with project-specific issues
  useEffect(() => {
    if (project?.issues) {
      console.log('[DEBUG] ProjectDetails - Setting issues from project data:', project.issues.length);
      setIssues(project.issues);
    }

    if (data.options?.workflowStates) {
      console.log('[DEBUG] ProjectDetails - Setting workflow states:', data.options.workflowStates.length);
      setWorkflowStates(data.options.workflowStates);
    }
  }, [project, data.options, setIssues, setWorkflowStates]);
  
  // Also listen for fetch responses from actions
  useEffect(() => {
    const handleFetchResponse = (event: any) => {
      // Check if the event is from an issue update action
      if (event.detail?.data?.issues && Array.isArray(event.detail.data.issues)) {
        console.log('[DEBUG] ProjectDetails - Received new issues from fetch response:', event.detail.data.issues.length);
        
        // Filter to only include issues for this project
        const projectIssues = event.detail.data.issues.filter((issue: any) => 
          issue.project && issue.project.id === id
        );
        
        if (projectIssues.length > 0) {
          console.log('[DEBUG] ProjectDetails - Setting filtered issues for project:', projectIssues.length);
          setIssues(projectIssues);
        }
      }
    };
    
    // Listen for action response events
    window.addEventListener('fetchresponse', handleFetchResponse);
    
    return () => {
      window.removeEventListener('fetchresponse', handleFetchResponse);
    };
  }, [id, setIssues]);

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
      <div className="flex-1 container mx-auto p-6">
        <AllIssues />
      </div>
      <CreateIssueModalProvider />
    </MainLayout>
  );
}
