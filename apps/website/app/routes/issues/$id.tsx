import { useLoaderData, useParams, useSubmit } from "react-router";
import type { Route } from "../+types/issues";
import type { ActionFunctionArgs } from "react-router";
import MainLayout from '@/components/layout/main-layout';
import { HeaderIssues } from '@/components/layout/headers/issues/header';
import { redirect } from "react-router";
import { getIssueById, getWorkflowStates, getIssueLabels } from "@/lib/db/issue-helpers.server";
import { getTeamsForUser } from "@/lib/db/team-helpers.server";
import { getUsers, getProjects } from "@/lib/db/project-helpers.server";
import { useEffect, useState, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card, CardHeader, CardTitle, CardContent, CardFooter } from "@/components/ui/card";
import { formatDistanceToNow } from "date-fns";
import { useIssuesStore, type Status, type User } from "@/store/issues-store";
import { Avatar, AvatarImage, AvatarFallback } from "@/components/ui/avatar";
import { CalendarIcon, Clock, Edit, LinkIcon, Tag, User as UserIcon } from "lucide-react";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { type Priority } from "@/mock-data/priorities";
import { type LabelInterface } from "@/mock-data/labels";
import { useChat } from "@ai-sdk/react";
import { SolverConnector } from "@/components/agent/solver-connector-updated";

export function meta({ params, location, data }: Route.MetaArgs) {
  const loaderData = data as Route.IssueLoaderData;

  // Use the issue identifier from the loader data if available
  const issueName = loaderData?.issue?.identifier || "Issue Details";

  return [
    { title: `${issueName} - ${loaderData?.issue?.title || "Issue"} - OpenAgents` },
    { name: "description", content: `View details for issue ${issueName}` },
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

    const issue = await getIssueById(id as string);

    if (!issue) {
      throw new Error("Issue not found");
    }

    // Load data needed for the issue detail view
    const [workflowStates, labels, teams, users, projects] = await Promise.all([
      getWorkflowStates(),
      getIssueLabels(),
      getTeamsForUser(user.id),
      getUsers(),
      getProjects()
    ]);

    return {
      issue,
      options: {
        workflowStates,
        labels,
        teams,
        users,
        projects
      },
      user
    };
  } catch (error) {
    console.error("Error loading issue:", error);
    return {
      error: "Failed to load issue",
      options: {
        workflowStates: [],
        labels: [],
        teams: [],
        users: [],
        projects: []
      }
    };
  }
}

export async function action({ request }: Route.ActionArgs) {
  // Import auth only within action (server-side only)
  const { auth, requireAuth } = await import('@/lib/auth.server');
  try {
    // Check authentication with requireAuth helper
    const authResult = await requireAuth(request);

    if (authResult.redirect) {
      return redirect(authResult.redirect);
    }

    const formData = await request.formData();
    const action = formData.get("_action") as string;

    // Forward to the main issues action handler
    // Import dynamically to avoid circular imports
    const { action: issuesAction } = await import("../issues");
    return issuesAction({ request, params: {} } as ActionFunctionArgs);
  } catch (error) {
    console.error("Error handling issue action:", error);
    return {
      success: false,
      error: "Failed to process issue action"
    };
  }
}

function StatusBadge({ status }: { status: Status }) {
  return (
    <Badge
      style={{ backgroundColor: `${status.color}20`, color: status.color, borderColor: status.color }}
      variant="outline"
      className="py-1 px-2"
    >
      {status.name}
    </Badge>
  );
}

function PriorityBadge({ priority }: { priority: Priority }) {
  return (
    <Badge
      style={{ backgroundColor: `${priority.color}20`, color: priority.color, borderColor: priority.color }}
      variant="outline"
      className="py-1 px-2"
    >
      {priority.name}
    </Badge>
  );
}

function LabelBadge({ label }: { label: LabelInterface }) {
  return (
    <Badge
      style={{ backgroundColor: `${label.color}20`, color: label.color, borderColor: label.color }}
      variant="outline"
      className="py-1 px-2 text-xs"
    >
      {label.name}
    </Badge>
  );
}

function ItemSection({ title, children }: { title: string, children: React.ReactNode }) {
  return (
    <div className="border-b border-border pb-2 mb-2 last:border-0">
      <h3 className="text-xs font-medium text-muted-foreground mb-1">{title}</h3>
      <div>{children}</div>
    </div>
  );
}

function EditableDescription({ issue }: { issue: Issue }) {
  const [isEditing, setIsEditing] = useState(false);
  const [description, setDescription] = useState(issue.description || '');
  const [isSaving, setIsSaving] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const submit = useSubmit();

  // Focus the textarea when entering edit mode
  useEffect(() => {
    if (isEditing && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [isEditing]);

  const handleDoubleClick = () => {
    setIsEditing(true);
  };

  const handleSave = () => {
    setIsSaving(true);

    // Create form data for the update
    const formData = new FormData();
    formData.append('_action', 'update');
    formData.append('id', issue.id);
    formData.append('description', description);

    console.log('Saving description update for issue:', issue.id);

    // Submit the form using React Router's submit
    submit(formData, {
      method: 'post',
      action: '/issues',
      replace: true,
      navigate: false
    });

    // Set a flag in sessionStorage to reload after update (only in browser)
    if (typeof window !== 'undefined' && window.sessionStorage) {
      sessionStorage.setItem('pendingDescriptionUpdate', issue.id);
    }

    // End editing mode
    setIsEditing(false);
    setIsSaving(false);
  };

  const handleCancel = () => {
    // Reset to original description and exit edit mode
    setDescription(issue.description || '');
    setIsEditing(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // Save on Ctrl+Enter or Cmd+Enter
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault(); // Prevent default to avoid submitting other forms
      handleSave();
    }
    // Cancel on Escape
    if (e.key === 'Escape') {
      e.preventDefault();
      handleCancel();
    }
  };

  if (isEditing) {
    return (
      <div className="prose prose-sm dark:prose-invert max-w-none">
        <textarea
          ref={textareaRef}
          className="w-full min-h-[150px] p-2 border rounded-md focus:outline-none focus:ring-2 focus:ring-primary"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Add a description..."
        />
        <div className="flex gap-2 mt-2">
          <Button
            size="sm"
            onClick={handleSave}
            disabled={isSaving}
          >
            {isSaving ? 'Saving...' : 'Save'}
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={handleCancel}
            disabled={isSaving}
          >
            Cancel
          </Button>
        </div>
        <div className="text-xs text-muted-foreground mt-1">
          Tip: Press Ctrl+Enter to save, Esc to cancel
        </div>
      </div>
    );
  }

  return (
    <div
      className="prose prose-sm dark:prose-invert max-w-none cursor-pointer hover:bg-secondary/10 p-2 rounded-md transition-colors"
      onDoubleClick={handleDoubleClick}
    >
      {description ? (
        <p>{description}</p>
      ) : (
        <p className="text-muted-foreground italic">Double-click to add a description</p>
      )}
    </div>
  );
}

export default function IssueDetails() {
  const { id } = useParams();
  const data = useLoaderData() as Route.IssueLoaderData;
  const { issue, options } = data;
  const submit = useSubmit();
  const [activeTab, setActiveTab] = useState("details");
  const { updateIssueStatus } = useIssuesStore();

  // Prepare issue details for the system prompt
  const issueDetails = `
Current Issue Details:
- Identifier: ${issue.identifier}
- Title: ${issue.title}
- Description: ${issue.description || 'No description provided'}
- Status: ${issue.status.name} (${issue.status.type})
- Priority: ${issue.priority.name}
${issue.assignee ? `- Assigned to: ${issue.assignee.name}` : '- Unassigned'}
${issue.dueDate ? `- Due Date: ${new Date(issue.dueDate).toLocaleDateString()}` : '- No due date'}
${issue.labels && issue.labels.length > 0 ? `- Labels: ${issue.labels.map(l => l.name).join(', ')}` : '- No labels'}
${issue.project ? `
Project Details:
- Project: ${issue.project.name}
- Color: ${issue.project.color}
` : '- Not assigned to any project'}
${issue.team ? `
Team Details:
- Team: ${issue.team.name}
- Team Key: ${issue.team.key}
` : ''}
${issue.subissues && issue.subissues.length > 0 ? `- Has ${issue.subissues.length} subtasks` : '- No subtasks'}
${issue.parentId ? `- Is a subtask of issue: ${issue.parentId}` : '- Is a top-level issue'}
- Created: ${new Date(issue.createdAt).toLocaleString()}
${issue.creator ? `- Created by: ${issue.creator.name}` : '- Creator unknown'}
`;

  // Safe way to access localStorage with a check for browser environment
  const getGithubToken = () => {
    if (typeof window !== 'undefined' && window.localStorage) {
      return localStorage.getItem('github_token') || '';
    }
    return '';
  };

  const { messages, input, handleInputChange, handleSubmit, isLoading, stop } = useChat({
    api: `https://chat.openagents.com/chat`,
    headers: {
      'X-GitHub-Token': getGithubToken()
    },
    maxSteps: 25,
    initialMessages: [{
      id: '12309123',
      role: 'system',
      content: `You are an AI assistant integrated into OpenAgents - a comprehensive project management and issue tracking system.

Project Context:
- OpenAgents is a platform for AI agents using open protocols
- The system includes web, mobile, and desktop applications
- You're currently in the issue tracking module, similar to Linear or Jira

Issue Tracking System:
- Issues belong to Teams and can be assigned to Projects
- Issues have workflow states: Triage, Backlog, Todo, In Progress, Done, Canceled
- Issues have properties: title, description, priority, labels, assignees, due dates
- Issues can have parent-child relationships and can reference other issues

Team & Project Structure:
- Teams are organizational units that own issues and define workflows
- Projects group related issues and can belong to teams
- Users can be members of multiple teams and projects with different roles
- Teams can customize their workflow states and issue numbering

${issueDetails}

Your Role:
- Help users understand this specific issue's details and context
- Assist with improving the issue description if needed
- Suggest appropriate status updates, labels, or priority changes
- Provide technical guidance related to this issue's implementation
- Answer questions about this issue, its project, and its team
- Maintain a helpful, professional tone focused on productivity

You're currently viewing the issue page where users can see all details about this issue, edit the description, change status, and discuss the issue through this chat interface. Be helpful and concise in your responses.`
    }],
  });

  // Log first 14 characters of GitHub token (only in browser environment)
  useEffect(() => {
    if (typeof window !== 'undefined' && window.localStorage) {
      console.log('GitHub token (first 14 chars):', (localStorage.getItem('github_token') || '').slice(0, 14));
    }
  }, []);

  // Check for pending updates from sessionStorage on load
  useEffect(() => {
    if (typeof window !== 'undefined' && window.sessionStorage) {
      const pendingUpdate = sessionStorage.getItem('pendingDescriptionUpdate');
      if (pendingUpdate === id) {
        // Clear the flag
        sessionStorage.removeItem('pendingDescriptionUpdate');
        // Reload once to get fresh data
        window.location.reload();
      }
    }
  }, [id]);

  // Listen for fetch responses to update the UI
  useEffect(() => {
    const handleFetchResponse = (event: any) => {
      // Check if this is an update to our issue
      if (event.detail?.data?.success && event.detail?.formData) {
        const formData = event.detail.formData;
        if (formData.get('_action') === 'update' && formData.get('id') === id) {
          // Only reload for our specific issue updates
          setTimeout(() => {
            window.location.reload();
          }, 200);
        }
      }
    };

    // Listen for action response events
    window.addEventListener('fetchresponse', handleFetchResponse);

    return () => {
      window.removeEventListener('fetchresponse', handleFetchResponse);
    };
  }, [id]);

  // Handle status change
  const handleStatusChange = (statusId: string) => {
    const newStatus = options?.workflowStates?.find(state => state.id === statusId);
    if (newStatus) {
      const formData = new FormData();
      formData.append("_action", "update");
      formData.append("id", issue.id);
      formData.append("stateId", statusId);

      // For Done status, add extra data
      if (newStatus.type === 'done') {
        formData.append("completedAt", new Date().toISOString());
        formData.append("isDone", "true");
        if (issue.project?.id) {
          formData.append("projectId", issue.project.id);
        }
        if (issue.team?.id) {
          formData.append("teamId", issue.team.id);
        }
      }

      // Update the UI optimistically
      updateIssueStatus(issue.id, newStatus);

      // Submit the form to save to the database
      submit(formData, { method: "post", replace: true });
    }
  };

  // Handle assignee change - commented out until needed
  // This function is currently unused but kept for future reference
  /*
  const handleAssigneeChange = (userId: string | null) => {
    const formData = new FormData();
    formData.append("_action", "update");
    formData.append("id", issue.id);
    formData.append("assigneeId", userId || "");

    // Submit the form to save to the database
    submit(formData, { method: "post", replace: true });
  };
  */

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

  if (!issue) {
    return (
      <MainLayout>
        <div className="container mx-auto p-6">
          <h1 className="text-2xl font-bold">Loading...</h1>
        </div>
      </MainLayout>
    );
  }

  // Get creator and assignee details
  const creator = issue.creator || null;
  const assignee = issue.assignee || null;

  return (
    <MainLayout header={<HeaderIssues />}>
      <div className="container mx-auto px-6 pt-2">
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {/* Main content */}
          <div className="md:col-span-2 space-y-4">
            <Card className="shadow-sm">
              <CardHeader className="py-2 px-4">
                <div className="flex justify-between items-center">
                  <div className="flex flex-col">
                    <CardTitle className="text-base font-medium">{issue.title}</CardTitle>
                    <div className="flex items-center gap-2 mt-1">
                      {issue.project && (
                        <Badge variant="secondary" className="font-normal text-xs px-1 py-0">
                          {issue.project.name}
                        </Badge>
                      )}
                      <StatusBadge status={issue.status} />
                    </div>
                  </div>
                </div>
              </CardHeader>
              <CardContent className="p-3">
                <Tabs defaultValue="details" value={activeTab} onValueChange={setActiveTab}>
                  <TabsList className="mb-2 h-8">
                    <TabsTrigger value="details" className="text-xs px-2 py-1">Details</TabsTrigger>
                    <TabsTrigger value="activity" className="text-xs px-2 py-1">Activity</TabsTrigger>
                    {issue.subissues && issue.subissues.length > 0 && (
                      <TabsTrigger value="subtasks" className="text-xs px-2 py-1">Subtasks ({issue.subissues.length})</TabsTrigger>
                    )}
                  </TabsList>

                  <TabsContent value="details" className="space-y-3">
                    {/* Description section */}
                    <div className="border rounded-md p-2 bg-background">
                      <h2 className="text-xs font-medium text-muted-foreground mb-1">Description</h2>
                      <EditableDescription issue={issue} />
                    </div>

                    {/* Other details as needed */}
                    {issue.subissues && issue.subissues.length > 0 && (
                      <div>
                        <h2 className="text-xs font-medium text-muted-foreground mb-1">Subtasks</h2>
                        <ul className="list-disc pl-4 text-xs">
                          {issue.subissues.map(subissueId => (
                            <li key={subissueId}>
                              <a href={`/issues/${subissueId}`} className="text-zinc-500 hover:underline">
                                {subissueId}
                              </a>
                            </li>
                          ))}
                        </ul>
                      </div>
                    )}
                  </TabsContent>

                  <TabsContent value="activity">
                    <div className="space-y-2">
                      <div className="flex items-start gap-2 py-2 border-b border-border">
                        <Avatar className="h-6 w-6">
                          {creator?.image ? (
                            <AvatarImage src={creator.image} alt={creator.name} />
                          ) : (
                            <AvatarFallback className="text-xs">{creator?.name?.charAt(0) || '?'}</AvatarFallback>
                          )}
                        </Avatar>
                        <div className="flex-1">
                          <div className="flex justify-between items-center">
                            <p className="text-xs font-medium">{creator?.name || 'Unknown'} created this issue</p>
                            <span className="text-xs text-muted-foreground">
                              {issue.createdAt ? formatDistanceToNow(new Date(issue.createdAt), { addSuffix: true }) : ''}
                            </span>
                          </div>
                        </div>
                      </div>

                      {/* You could add more activity items here from a real activity log */}
                    </div>
                  </TabsContent>

                  {issue.subissues && issue.subissues.length > 0 && (
                    <TabsContent value="subtasks">
                      <div className="space-y-1">
                        {issue.subissues.map(subissueId => (
                          <Card key={subissueId} className="p-2">
                            <div className="flex items-center justify-between">
                              <div>
                                <a href={`/issues/${subissueId}`} className="text-xs font-medium hover:underline">
                                  {subissueId}
                                </a>
                              </div>
                              <Button size="sm" variant="ghost" className="h-6 text-xs">View</Button>
                            </div>
                          </Card>
                        ))}
                      </div>
                    </TabsContent>
                  )}
                </Tabs>
              </CardContent>
            </Card>

            {/* Solver Agent Card */}
            <SolverConnector issue={issue} githubToken={getGithubToken()} />

            {/* Chat Card - Commented out as requested
            <Card>
              <CardContent>
                <Chat
                  messages={messages}
                  input={input}
                  handleInputChange={handleInputChange}
                  handleSubmit={handleSubmit}
                  isGenerating={isLoading}
                  stop={stop}
                />
              </CardContent>
            </Card>
            */}
          </div>

          {/* Sidebar */}
          <div>
            <Card className="shadow-sm">
              <CardHeader className="py-2 px-3">
                <CardTitle className="text-sm">Details</CardTitle>
              </CardHeader>
              <CardContent className="p-3 space-y-2">
                <ItemSection title="Status">
                  <div className="flex items-center gap-1">
                    <StatusBadge status={issue.status} />
                  </div>
                </ItemSection>

                <ItemSection title="Priority">
                  <PriorityBadge priority={issue.priority} />
                </ItemSection>

                <ItemSection title="Assignee">
                  <div className="flex items-center gap-2">
                    {assignee ? (
                      <div className="flex items-center gap-1">
                        <Avatar className="h-5 w-5">
                          {assignee.image ? (
                            <AvatarImage src={assignee.image} alt={assignee.name} />
                          ) : (
                            <AvatarFallback className="text-xs">{assignee.name.charAt(0)}</AvatarFallback>
                          )}
                        </Avatar>
                        <span className="text-xs">{assignee.name}</span>
                      </div>
                    ) : (
                      <span className="text-xs text-muted-foreground">Unassigned</span>
                    )}
                  </div>
                </ItemSection>

                {issue.project && (
                  <ItemSection title="Project">
                    <div className="flex items-center gap-1">
                      <div
                        className="h-2 w-2 rounded-full"
                        style={{ backgroundColor: issue.project.color }}
                      />
                      <a
                        href={`/projects/${issue.project.id}`}
                        className="text-xs text-zinc-500 hover:underline"
                      >
                        {issue.project.name}
                      </a>
                    </div>
                  </ItemSection>
                )}

                {issue.team && (
                  <ItemSection title="Team">
                    <div className="flex items-center gap-1">
                      <span className="text-xs">{issue.team.name}</span>
                      <span className="text-xs text-muted-foreground">({issue.team.key})</span>
                    </div>
                  </ItemSection>
                )}

                {issue.labels && issue.labels.length > 0 && (
                  <ItemSection title="Labels">
                    <div className="flex flex-wrap gap-1">
                      {issue.labels.map(label => (
                        <LabelBadge key={label.id} label={label} />
                      ))}
                    </div>
                  </ItemSection>
                )}

                <ItemSection title="Created">
                  <div className="flex items-center gap-1 text-xs">
                    <Clock className="h-3 w-3 text-muted-foreground" />
                    <span>
                      {issue.createdAt
                        ? formatDistanceToNow(new Date(issue.createdAt), { addSuffix: true })
                        : 'Unknown'}
                    </span>
                  </div>
                </ItemSection>

                {issue.dueDate && (
                  <ItemSection title="Due Date">
                    <div className="flex items-center gap-1 text-xs">
                      <CalendarIcon className="h-3 w-3 text-muted-foreground" />
                      <span>{new Date(issue.dueDate).toLocaleDateString()}</span>
                    </div>
                  </ItemSection>
                )}
              </CardContent>
            </Card>
          </div>
        </div>
      </div>
    </MainLayout>
  );
}
