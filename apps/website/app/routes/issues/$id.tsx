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
import { Form } from "@/components/ui/form";
import { Avatar, AvatarImage, AvatarFallback } from "@/components/ui/avatar";
import { CalendarIcon, CheckCircle, Clock, Edit, LinkIcon, Tag, User as UserIcon } from "lucide-react";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Breadcrumb, BreadcrumbItem, BreadcrumbLink } from "@/components/ui/breadcrumb";
import { type Priority } from "@/mock-data/priorities";
import { type LabelInterface } from "@/mock-data/labels";

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
    <div className="border-b border-border pb-4 mb-4 last:border-0">
      <h3 className="text-sm font-medium text-muted-foreground mb-2">{title}</h3>
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
    
    // Set a flag in sessionStorage to reload after update
    sessionStorage.setItem('pendingDescriptionUpdate', issue.id);
    
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
  
  // Check for pending updates from sessionStorage on load
  useEffect(() => {
    const pendingUpdate = sessionStorage.getItem('pendingDescriptionUpdate');
    if (pendingUpdate === id) {
      // Clear the flag
      sessionStorage.removeItem('pendingDescriptionUpdate');
      // Reload once to get fresh data
      window.location.reload();
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
      <div className="container mx-auto p-6">
        {/* <Breadcrumb className="mb-4">
          <BreadcrumbItem>
            <BreadcrumbLink href="/issues">Issues</BreadcrumbLink>
          </BreadcrumbItem>
          <BreadcrumbItem>
            <BreadcrumbLink href={`/issues/${issue.id}`} isCurrentPage>
              {issue.identifier}
            </BreadcrumbLink>
          </BreadcrumbItem>
        </Breadcrumb> */}

        <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
          {/* Main content */}
          <div className="md:col-span-2">
            <Card>
              <CardHeader className="pb-2">
                <div className="flex justify-between items-start">
                  <div className="flex flex-col">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-sm font-mono text-muted-foreground">
                        {issue.identifier}
                      </span>
                      {issue.project && (
                        <Badge variant="secondary" className="font-normal">
                          {issue.project.name}
                        </Badge>
                      )}
                    </div>
                    <CardTitle className="text-xl font-semibold">{issue.title}</CardTitle>
                  </div>
                  {/* Edit button commented out until functionality is implemented
                  <Button size="sm" variant="outline">
                    <Edit className="h-4 w-4 mr-1" /> Edit
                  </Button>
                  */}
                </div>
              </CardHeader>
              <CardContent>
                <Tabs defaultValue="details" value={activeTab} onValueChange={setActiveTab}>
                  <TabsList className="mb-4">
                    <TabsTrigger value="details">Details</TabsTrigger>
                    <TabsTrigger value="activity">Activity</TabsTrigger>
                    {issue.subissues && issue.subissues.length > 0 && (
                      <TabsTrigger value="subtasks">Subtasks ({issue.subissues.length})</TabsTrigger>
                    )}
                  </TabsList>

                  <TabsContent value="details" className="space-y-6">
                    {/* Description section */}
                    <div className="border rounded-md p-4 bg-background">
                      <h2 className="text-sm font-medium text-muted-foreground mb-2">Description</h2>
                      <EditableDescription issue={issue} />
                    </div>

                    {/* Other details as needed */}
                    {issue.subissues && issue.subissues.length > 0 && (
                      <div>
                        <h2 className="text-sm font-medium text-muted-foreground mb-2">Subtasks</h2>
                        <ul className="list-disc pl-5">
                          {issue.subissues.map(subissueId => (
                            <li key={subissueId}>
                              <a href={`/issues/${subissueId}`} className="text-blue-500 hover:underline">
                                {subissueId}
                              </a>
                            </li>
                          ))}
                        </ul>
                      </div>
                    )}
                  </TabsContent>

                  <TabsContent value="activity">
                    <div className="space-y-4">
                      <div className="flex items-start gap-3 p-3 border-b border-border">
                        <Avatar className="h-8 w-8">
                          {creator?.image ? (
                            <AvatarImage src={creator.image} alt={creator.name} />
                          ) : (
                            <AvatarFallback>{creator?.name?.charAt(0) || '?'}</AvatarFallback>
                          )}
                        </Avatar>
                        <div className="flex-1">
                          <div className="flex justify-between items-center">
                            <p className="text-sm font-medium">{creator?.name || 'Unknown'} created this issue</p>
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
                      <div className="space-y-2">
                        {issue.subissues.map(subissueId => (
                          <Card key={subissueId} className="p-3">
                            <div className="flex items-center justify-between">
                              <div>
                                <a href={`/issues/${subissueId}`} className="font-medium hover:underline">
                                  {subissueId}
                                </a>
                              </div>
                              <Button size="sm" variant="ghost">View</Button>
                            </div>
                          </Card>
                        ))}
                      </div>
                    </TabsContent>
                  )}
                </Tabs>
              </CardContent>
            </Card>
          </div>

          {/* Sidebar */}
          <div>
            <Card>
              <CardHeader>
                <CardTitle className="text-lg">Issue details</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <ItemSection title="Status">
                  <div className="flex items-center gap-2">
                    <StatusBadge status={issue.status} />

                    {/* Status change dropdown could go here */}
                    {/* This would be a more advanced component in a real app */}
                  </div>
                </ItemSection>

                <ItemSection title="Priority">
                  <PriorityBadge priority={issue.priority} />
                </ItemSection>

                <ItemSection title="Assignee">
                  <div className="flex items-center gap-2">
                    {assignee ? (
                      <div className="flex items-center gap-2">
                        <Avatar className="h-6 w-6">
                          {assignee.image ? (
                            <AvatarImage src={assignee.image} alt={assignee.name} />
                          ) : (
                            <AvatarFallback>{assignee.name.charAt(0)}</AvatarFallback>
                          )}
                        </Avatar>
                        <span>{assignee.name}</span>
                      </div>
                    ) : (
                      <span className="text-muted-foreground">Unassigned</span>
                    )}
                  </div>
                </ItemSection>

                {issue.project && (
                  <ItemSection title="Project">
                    <div className="flex items-center gap-2">
                      <div
                        className="h-3 w-3 rounded-full"
                        style={{ backgroundColor: issue.project.color }}
                      />
                      <a
                        href={`/projects/${issue.project.id}`}
                        className="text-blue-500 hover:underline"
                      >
                        {issue.project.name}
                      </a>
                    </div>
                  </ItemSection>
                )}

                {issue.team && (
                  <ItemSection title="Team">
                    <div className="flex items-center gap-2">
                      <span>{issue.team.name}</span>
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
                  <div className="flex items-center gap-2 text-sm">
                    <Clock className="h-4 w-4 text-muted-foreground" />
                    <span>
                      {issue.createdAt
                        ? formatDistanceToNow(new Date(issue.createdAt), { addSuffix: true })
                        : 'Unknown'}
                    </span>
                  </div>
                </ItemSection>

                {issue.dueDate && (
                  <ItemSection title="Due Date">
                    <div className="flex items-center gap-2 text-sm">
                      <CalendarIcon className="h-4 w-4 text-muted-foreground" />
                      <span>{new Date(issue.dueDate).toLocaleDateString()}</span>
                    </div>
                  </ItemSection>
                )}
              </CardContent>
              <CardFooter className="flex justify-between">
                {/* Copy link button commented out until functionality is implemented
                <Button variant="outline" size="sm">
                  <LinkIcon className="h-4 w-4 mr-1" /> Copy Link
                </Button>
                */}
                {/* Mark Done button kept functional as it has proper implementation */}
                {/* {issue.status.type !== 'done' && (
                  <Button variant="default" size="sm" onClick={() => {
                    const doneState = options?.workflowStates?.find(state => state.type === 'done');
                    if (doneState) {
                      handleStatusChange(doneState.id);
                    }
                  }}>
                    <CheckCircle className="h-4 w-4 mr-1" /> Mark Done
                  </Button>
                )} */}
              </CardFooter>
            </Card>
          </div>
        </div>
      </div>
    </MainLayout>
  );
}
