import { auth } from "@/lib/auth";
import { type ActionFunctionArgs, type LoaderFunctionArgs } from "react-router";
import { redirect } from "react-router";
import {
  getAllIssues,
  getWorkflowStates,
  getIssueLabels,
  createIssue,
  updateIssue
} from "../lib/db/issue-helpers.server";
import { getUsers, getProjects } from "../lib/db/project-helpers.server";
import { getTeamsForUser } from "../lib/db/team-helpers.server";
import AllIssues from "../components/common/issues/all-issues";
import { useLoaderData, useSubmit } from "react-router";
import { useIssuesStore } from "../store/issues-store";
import { useEffect } from "react";
import { Button } from "../components/ui/button";
import { HeaderIssues } from "../components/layout/headers/issues/header";
import { CreateIssueModalProvider } from "../components/common/issues/create-issue-modal-provider";
import { useCreateIssueStore } from "../store/create-issue-store";

// Load issues and all related data
export async function loader({ request }: LoaderFunctionArgs) {
  try {
    // Get current session with better-auth
    const { user, session } = await auth.api.getSession(request);

    if (!session) {
      return redirect("/login");
    }

    // Concurrently fetch data
    const [
      issues,
      workflowStates,
      labels,
      projects,
      users
    ] = await Promise.all([
      getAllIssues(),
      getWorkflowStates(),
      getIssueLabels(),
      getProjects(),
      getUsers()
    ]);
    
    console.log('[DEBUG] /issues loader - Fetched issues:', issues.length);
    console.log('[DEBUG] /issues loader - First issue sample:', issues.length > 0 ? JSON.stringify(issues[0]).substring(0, 300) : 'No issues');

    // Get teams that the current user is a member of
    const teams = await getTeamsForUser(user.id);
    
    // --- START DEBUG LOG ---
    console.log(`[DEBUG] /issues Loader - Fetched ${workflowStates?.length ?? 0} workflow states:`, JSON.stringify(workflowStates));
    console.log(`[DEBUG] /issues Loader - Fetched ${teams?.length ?? 0} teams for user ${user.id}:`, JSON.stringify(teams));
    // --- END DEBUG LOG ---

    // Return simple object instead of json
    return {
      issues,
      options: {
        workflowStates,
        labels,
        projects,
        teams,
        users
      },
      user
    };
  } catch (error) {
    console.error("Error loading issues:", error);
    return {
      issues: [],
      options: {
        workflowStates: [],
        labels: [],
        projects: [],
        teams: [],
        users: []
      },
      user: null,
      error: "Failed to load issues"
    };
  }
}

// Handle forms for creating or updating issues
export async function action({ request }: ActionFunctionArgs) {
  try {
    const { session } = await auth.api.getSession(request);
    
    if (!session) {
      return redirect("/login");
    }
    
    const formData = await request.formData();
    const action = formData.get("_action") as string;
    
    // Handle issue creation
    if (action === "create") {
      const title = formData.get("title") as string;
      const description = formData.get("description") as string;
      const teamId = formData.get("teamId") as string;
      const stateId = formData.get("stateId") as string;
      const priorityStr = formData.get("priority") as string;
      const priority = parseInt(priorityStr, 10);
      const assigneeId = formData.get("assigneeId") as string;
      const projectId = formData.get("projectId") as string;
      
      // Convert label IDs from FormData (might be multiple entries)
      let labelIds: string[] = [];
      formData.getAll("labelIds").forEach(labelId => {
        if (typeof labelId === 'string') {
          labelIds.push(labelId);
        }
      });
      
      // Validate required fields
      if (!title || !teamId || !stateId) {
        return {
          success: false,
          error: "Required fields are missing"
        };
      }
      
      const issueData = {
        title,
        description,
        teamId,
        stateId,
        priority,
        assigneeId: assigneeId || undefined,
        projectId: projectId || undefined,
        creatorId: session.userId,
        labelIds
      };
      
      console.log('[DEBUG] Creating issue with data:', JSON.stringify(issueData));
      const issueId = await createIssue(issueData);
      console.log('[DEBUG] Issue created with ID:', issueId);
      
      // Instead of just returning the ID, let's get the complete created issue
      // Force-refresh the issues list by requesting again
      const newIssues = await getAllIssues();
      console.log('[DEBUG] Reloaded issues after creation, found:', newIssues.length);
      
      // Find our new issue in the list so we can return it directly
      const createdIssue = newIssues.find(issue => issue.id === issueId);
      console.log('[DEBUG] Created issue details:', createdIssue);
      
      return { 
        success: true, 
        issueId,
        issues: newIssues,
        createdIssue 
      };
    }
    
    // Handle issue update
    if (action === "update") {
      const id = formData.get("id") as string;
      const title = formData.get("title") as string;
      const description = formData.get("description") as string;
      const teamId = formData.get("teamId") as string;
      const stateId = formData.get("stateId") as string;
      const priorityStr = formData.get("priority") as string;
      const priority = parseInt(priorityStr, 10);
      const assigneeId = formData.get("assigneeId") as string;
      const projectId = formData.get("projectId") as string;
      
      // Validate required fields
      if (!id || !title || !teamId || !stateId) {
        return {
          success: false,
          error: "Required fields are missing"
        };
      }
      
      await updateIssue(id, {
        title,
        description,
        teamId,
        stateId,
        priority,
        assigneeId: assigneeId || null,
        projectId: projectId || null
      });
      
      return { success: true };
    }
    
    return { success: false, error: "Unknown action" };
  } catch (error) {
    console.error("Error handling issue action:", error);
    return {
      success: false,
      error: "Failed to process issue action"
    };
  }
}

export default function IssuesRoute() {
  const loaderData = useLoaderData();
  const { setIssues, setWorkflowStates } = useIssuesStore();
  const submit = useSubmit();
  const { openModal } = useCreateIssueStore();

  // Update store with issues and workflow states from loader data
  useEffect(() => {
    if (loaderData.issues) {
      console.log('[DEBUG] IssuesRoute - Setting issues:', loaderData.issues.length);
      setIssues(loaderData.issues);
    } else {
      console.log('[DEBUG] IssuesRoute - No issues in loader data');
    }
    
    if (loaderData.options?.workflowStates) {
      console.log('[DEBUG] IssuesRoute - Setting workflow states:', loaderData.options.workflowStates.length);
      setWorkflowStates(loaderData.options.workflowStates);
    } else {
      console.log('[DEBUG] IssuesRoute - No workflow states in loader data');
    }
  }, [loaderData.issues, loaderData.options?.workflowStates, setIssues, setWorkflowStates]);

  return (
    <div className="w-full min-h-screen flex flex-col">
      <HeaderIssues />
      <div className="flex-1 container mx-auto p-6">
        <AllIssues />
      </div>
      <CreateIssueModalProvider />
    </div>
  );
}