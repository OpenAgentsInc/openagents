// Server-side imports moved to loader and action
import { type ActionFunctionArgs, type LoaderFunctionArgs } from "react-router";
import { redirect } from "react-router";
import {
  getAllIssues,
  getWorkflowStates,
  getIssueLabels,
  createIssue,
  updateIssue,
  getDb
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

          // Special handling for Done status
          const isDone = formData.get("isDone") === "true";
          const isDoneStatus =
            updateData.stateId.includes('done') ||
            updateData.stateId.includes('completed') ||
            updateData.stateId === 'default-done';

          if (isDone || isDoneStatus) {
            console.log(`Setting issue ${id} to Done status: ${updateData.stateId}`);

            // If completedAt was provided, use it
            if (formData.has("completedAt")) {
              updateData.completedAt = formData.get("completedAt") as string;
            } else {
              // Otherwise set it to now
              updateData.completedAt = new Date().toISOString();
            }

            // Ensure we have a teamId for this issue update
            if (formData.has("teamId")) {
              updateData.teamId = formData.get("teamId") as string;
              console.log(`Using provided teamId: ${updateData.teamId} for Done update`);
            } else {
              // Try to get teamId from the database
              try {
                const db = getDb();
                const issue = await db
                  .selectFrom('issue')
                  .select(['teamId'])
                  .where('id', '=', id)
                  .executeTakeFirst();

                if (issue && issue.teamId) {
                  updateData.teamId = issue.teamId;
                  console.log(`Found issue teamId: ${updateData.teamId} for Done update`);
                } else {
                  console.log(`Could not find teamId for issue ${id}, trying to find a team`);

                  // Find any team if we can't get the issue's teamId
                  const anyTeam = await db
                    .selectFrom('team')
                    .select(['id'])
                    .limit(1)
                    .executeTakeFirst();

                  if (anyTeam) {
                    updateData.teamId = anyTeam.id;
                    console.log(`Using first available teamId: ${updateData.teamId} for Done update`);
                  } else {
                    // Hard-coded fallback if all else fails - find a working status ID
                    console.log('No teams found. Finding a valid workflow state instead');

                    // Look for a non-done workflow state to use
                    const validState = await db
                      .selectFrom('workflow_state')
                      .select(['id'])
                      .where('type', '!=', 'done')
                      .limit(1)
                      .executeTakeFirst();

                    if (validState) {
                      // Override the stateId to use a known working state
                      updateData.stateId = validState.id;
                      console.log(`Fallback: Using workflow state ${validState.id} instead of done`);
                      updateData.completedAt = null; // Clear completed since we're not using done
                    }
                  }
                }
              } catch (error) {
                console.error(`Error getting teamId for issue ${id}:`, error);

                // If we failed to get a team ID, try a final fallback - don't use done status
                try {
                  // Find any non-done workflow state as a fallback
                  const db = getDb();
                  const validState = await db
                    .selectFrom('workflow_state')
                    .select(['id', 'type'])
                    .limit(1)
                    .executeTakeFirst();

                  if (validState) {
                    // Override the stateId to use a known working state
                    updateData.stateId = validState.id;
                    console.log(`Emergency fallback: Using workflow state ${validState.id} (${validState.type}) instead`);

                    // Only clear completedAt if it's not a "done" type
                    if (validState.type !== 'done') {
                      updateData.completedAt = null;
                    }
                  }
                } catch (innerError) {
                  console.error('Critical error in fallback:', innerError);
                  // At this point we have no more fallbacks - let the action continue and potentially fail
                }
              }
            }
          } else {
            // If not Done, ensure completedAt is null
            updateData.completedAt = null;
          }

          console.log(`Setting issue ${id} stateId to ${updateData.stateId} (completedAt: ${updateData.completedAt})`);
        }

        if (formData.has("priority")) {
          const priorityStr = formData.get("priority") as string;
          updateData.priority = parseInt(priorityStr, 10);
        }

        if (formData.has("projectId")) {
          updateData.projectId = formData.get("projectId") as string || null;
        }

        // Handle description updates
        if (formData.has("description")) {
          updateData.description = formData.get("description") as string;
          updateData.updatedAt = new Date().toISOString(); // Explicitly update the timestamp
          console.log(`Updating description for issue ${id}: ${updateData.description?.substring(0, 50)}...`);
        }

        console.log(`Updating issue ${id} with data:`, updateData);

        try {
          // Perform the update with just the fields that were provided
          await updateIssue(id, updateData);
          console.log(`Successfully updated issue ${id}`);
        } catch (error) {
          console.error(`Error updating issue ${id}:`, error);

          // Check if this was a workflow state related error
          const errorString = String(error);
          const isWorkflowStateError =
            errorString.includes('FOREIGN KEY constraint') ||
            errorString.includes('workflow_state');

          if (isWorkflowStateError && updateData.stateId?.includes('done')) {
            console.log('Detected workflow state error for Done status, trying fallback...');

            try {
              // Find a valid workflow state that works
              const db = getDb();
              const existingState = await db
                .selectFrom('workflow_state')
                .select(['id'])
                .limit(1)
                .executeTakeFirst();

              if (existingState) {
                console.log(`Using fallback workflow state: ${existingState.id}`);

                // Update with the valid workflow state instead
                updateData.stateId = existingState.id;
                await updateIssue(id, updateData);

                // Get the updated issue list
                const newIssues = await getAllIssues();

                return {
                  success: true,
                  issues: newIssues,
                  message: 'Used fallback workflow state due to constraint error'
                };
              }
            } catch (fallbackError) {
              console.error('Error in workflow state fallback:', fallbackError);
            }
          }

          return {
            success: false,
            error: `Failed to update issue: ${error instanceof Error ? error.message : String(error)}`
          };
        }

        // Get the updated issue list for the frontend
        const newIssues = await getAllIssues();
        const updatedIssue = newIssues.find(issue => issue.id === id);

        // Return the full issue list to update the UI
        return {
          success: true,
          issues: newIssues,
          issue: updatedIssue,
          id: id // Include the issue ID in the response for easier identification
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
      console.log('[DEBUG] IssuesRoute - Setting issues from loader data:', loaderData.issues.length);
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

  // Also listen for fetch responses from actions
  useEffect(() => {
    const handleFetchResponse = (event: any) => {
      // Check if the event is from an issue update action
      if (event.detail?.data?.issues && Array.isArray(event.detail.data.issues)) {
        console.log('[DEBUG] IssuesRoute - Received new issues from fetch response:', event.detail.data.issues.length);
        setIssues(event.detail.data.issues);
      } else if (event.detail?.formData?.get('_action') === 'update') {
        // If this was an update action but we didn't get a full issues list in the response,
        // force a refresh to get the latest data
        console.log('[DEBUG] IssuesRoute - Status update detected, fetching latest issues');

        // Use a small timeout to not interfere with the current action response
        setTimeout(() => {
          window.location.reload();
        }, 300);
      }
    };

    // Listen for action response events
    window.addEventListener('fetchresponse', handleFetchResponse);

    return () => {
      window.removeEventListener('fetchresponse', handleFetchResponse);
    };
  }, [setIssues]);

  return (
    <div className="w-full min-h-screen flex flex-col">
      <HeaderIssues />
      <div className="flex-1 container mx-auto">
        <AllIssues />
      </div>
      <CreateIssueModalProvider />
    </div>
  );
}
