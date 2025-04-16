// Server-side imports moved to loader and action
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
  // Import auth only within loader (server-side only)
  const { auth, requireAuth } = await import('@/lib/auth.server');
  try {
    // Check authentication with requireAuth helper
    const authResult = await requireAuth(request);
    
    if (authResult.redirect) {
      return redirect(authResult.redirect);
    }
    
    // Get user and session from the auth result
    const { user, session } = authResult;

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

    // No debug logs in production

    // Get teams that the current user is a member of
    const teams = await getTeamsForUser(user.id);

    // No debug logs in production

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
  // Import auth only within action (server-side only)
  const { auth, requireAuth } = await import('@/lib/auth.server');
  try {
    // Check authentication with requireAuth helper
    const authResult = await requireAuth(request);
    
    if (authResult.redirect) {
      return redirect(authResult.redirect);
    }
    
    // Get session from the auth result
    const { session } = authResult;

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

      const issueId = await createIssue(issueData);

      // Get the updated issues list
      const newIssues = await getAllIssues();

      // Find our new issue in the list
      const createdIssue = newIssues.find(issue => issue.id === issueId);

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
      
      // Basic validation - we need at least an ID
      if (!id) {
        return {
          success: false,
          error: "Issue ID is required for updates"
        };
      }
      
      // Check if this is a partial update (e.g., just updating the assignee)
      const isPartialUpdate = !formData.has("title");
      
      if (isPartialUpdate) {
        // Handle partial updates (like just updating the assignee)
        const updateData: Record<string, any> = {};
        
        // Add fields that were provided in the request
        if (formData.has("assigneeId")) {
          const assigneeId = formData.get("assigneeId") as string;
          updateData.assigneeId = assigneeId || null;
        }
        
        if (formData.has("stateId")) {
          updateData.stateId = formData.get("stateId") as string;
        }
        
        if (formData.has("priority")) {
          const priorityStr = formData.get("priority") as string;
          updateData.priority = parseInt(priorityStr, 10);
        }
        
        if (formData.has("projectId")) {
          updateData.projectId = formData.get("projectId") as string || null;
        }
        
        // Perform the update with just the fields that were provided
        await updateIssue(id, updateData);
        
        // Get the updated issue
        const newIssues = await getAllIssues();
        const updatedIssue = newIssues.find(issue => issue.id === id);
        
        return { 
          success: true,
          issue: updatedIssue
        };
      } else {
        // Handle full updates with all required fields
        const title = formData.get("title") as string;
        const description = formData.get("description") as string;
        const teamId = formData.get("teamId") as string;
        const stateId = formData.get("stateId") as string;
        const priorityStr = formData.get("priority") as string;
        const priority = parseInt(priorityStr, 10);
        const assigneeId = formData.get("assigneeId") as string;
        const projectId = formData.get("projectId") as string;

        // Validate required fields for full updates
        if (!title || !teamId || !stateId) {
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
      // console.log('[DEBUG] IssuesRoute - Setting issues:', loaderData.issues.length);
      setIssues(loaderData.issues);
    } else {
      // console.log('[DEBUG] IssuesRoute - No issues in loader data');
    }

    if (loaderData.options?.workflowStates) {
      // console.log('[DEBUG] IssuesRoute - Setting workflow states:', loaderData.options.workflowStates.length);
      setWorkflowStates(loaderData.options.workflowStates);
    } else {
      // console.log('[DEBUG] IssuesRoute - No workflow states in loader data');
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
