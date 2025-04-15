import { auth } from "@/lib/auth";
import { type ActionFunctionArgs, type LoaderFunctionArgs } from "react-router-dom";
import { json, redirect } from "react-router";
import { 
  getAllIssues, 
  getWorkflowStates, 
  getIssueLabels,
  createIssue,
  updateIssue 
} from "../lib/db/issue-helpers.server";
import { getUsers, getProjects, getTeams } from "../lib/db/project-helpers.server";
import AllIssues from "../components/common/issues/all-issues";
import { useLoaderData, useSubmit } from "react-router-dom";
import { useIssuesStore } from "../store/issues-store";
import { useEffect } from "react";
import { Button } from "../components/ui/button";
import { HeaderIssues } from "../components/layout/headers/issues/header";

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
      teams,
      users
    ] = await Promise.all([
      getAllIssues(),
      getWorkflowStates(),
      getIssueLabels(),
      getProjects(),
      getTeams(),
      getUsers()
    ]);
    
    return json({
      issues,
      workflowStates,
      labels,
      projects,
      teams,
      users,
      user
    });
  } catch (error) {
    console.error("Error loading issues:", error);
    return json({ 
      issues: [], 
      workflowStates: [], 
      labels: [], 
      projects: [], 
      teams: [], 
      users: [],
      user: null,
      error: "Failed to load issues"
    });
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
      const priority = parseInt(formData.get("priority") as string || "0");
      const assigneeId = formData.get("assigneeId") as string || null;
      const projectId = formData.get("projectId") as string || null;
      const labelIds = formData.getAll("labelIds") as string[];
      
      if (!title || !teamId || !stateId) {
        return json({ error: "Missing required fields" }, { status: 400 });
      }
      
      const issueId = await createIssue({
        title,
        description,
        teamId,
        stateId,
        priority,
        assigneeId: assigneeId || undefined,
        projectId: projectId || undefined,
        creatorId: session.userId,
        labelIds
      });
      
      return json({ success: true, issueId });
    }
    
    // Handle issue updates
    if (action === "update") {
      const id = formData.get("id") as string;
      const title = formData.get("title") as string;
      const description = formData.get("description") as string;
      const stateId = formData.get("stateId") as string;
      const priority = formData.get("priority") ? parseInt(formData.get("priority") as string) : undefined;
      const assigneeId = formData.get("assigneeId") as string;
      const projectId = formData.get("projectId") as string;
      const labelIds = formData.getAll("labelIds") as string[];
      
      if (!id) {
        return json({ error: "Missing issue ID" }, { status: 400 });
      }
      
      await updateIssue(id, {
        title,
        description,
        stateId,
        priority,
        assigneeId: assigneeId || null,
        projectId: projectId || null,
        labelIds
      });
      
      return json({ success: true });
    }
    
    return json({ error: "Invalid action" }, { status: 400 });
  } catch (error) {
    console.error("Error in issues action:", error);
    return json({ error: "Failed to process request" }, { status: 500 });
  }
}

export default function IssuesRoute() {
  const { 
    issues, 
    workflowStates, 
    error 
  } = useLoaderData<typeof loader>();
  const { setIssues, isLoaded } = useIssuesStore();
  const submit = useSubmit();
  
  // Set issues in store when loaded from the server
  useEffect(() => {
    if (issues && issues.length > 0) {
      setIssues(issues);
    }
  }, [issues, setIssues]);
  
  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-full">
        <h2 className="text-xl font-bold mb-4">Error loading issues</h2>
        <p className="text-muted-foreground mb-4">{error}</p>
        <Button onClick={() => window.location.reload()}>Retry</Button>
      </div>
    );
  }
  
  if (!isLoaded && (!issues || issues.length === 0)) {
    return (
      <div className="flex flex-col items-center justify-center h-full">
        <h2 className="text-xl font-bold mb-4">No issues found</h2>
        <p className="text-muted-foreground mb-4">Create your first issue to get started</p>
        <Button 
          onClick={() => {
            // Open create issue modal
            // This would typically use the create-issue-modal-provider.tsx
            // but that component would need to be updated to work with the DB
          }}
        >
          Create Issue
        </Button>
      </div>
    );
  }
  
  return (
    <div className="min-h-screen bg-background">
      <HeaderIssues />
      <main className="container mx-auto p-4">
        <AllIssues />
      </main>
    </div>
  );
}