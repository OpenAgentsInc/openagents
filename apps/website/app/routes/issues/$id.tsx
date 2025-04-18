// IssueDetails.tsx
import { useLoaderData, useParams, useSubmit } from "react-router";
import type { Route } from "../+types/issues";
import type { ActionFunctionArgs } from "react-router";
import MainLayout from '@/components/layout/main-layout';
import { HeaderIssues } from '@/components/layout/headers/issues/header';
import { useOpenAgent } from "@openagents/core";
import { redirect } from "react-router";
import { getIssueById, getWorkflowStates, getIssueLabels } from "@/lib/db/issue-helpers.server";
import { getTeamsForUser } from "@/lib/db/team-helpers.server";
import { getUsers, getProjects } from "@/lib/db/project-helpers.server";
import { useEffect, useState, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardHeader,
  CardTitle,
  CardContent,
} from "@/components/ui/card";
import { formatDistanceToNow } from "date-fns";
import { useIssuesStore, type Status, type User } from "@/store/issues-store";
import {
  Avatar,
  AvatarImage,
  AvatarFallback,
} from "@/components/ui/avatar";
import { CalendarIcon, Clock, CheckCircle, AlertTriangle, BotIcon } from "lucide-react";
import { type Priority } from "@/mock-data/priorities";
import { type LabelInterface } from "@/mock-data/labels";
import { useChat } from "@ai-sdk/react";
import { SolverConnector } from "@/components/agent/solver-connector";
import { SolverControls } from "@/components/agent/solver-controls";
import { Spinner } from "@/components/ui/spinner";

export function meta({ params, location, data }: Route.MetaArgs) {
  const loaderData = data as Route.IssueLoaderData;
  const issueName = loaderData?.issue?.identifier || "Issue Details";

  return [
    { title: `${issueName} - ${loaderData?.issue?.title || "Issue"} - OpenAgents` },
    { name: "description", content: `View details for issue ${issueName}` },
  ];
}

export async function loader({ params, request }: Route.LoaderArgs) {
  // Import auth only within loader (server-side only)
  const { requireAuth } = await import('@/lib/auth.server');
  try {
    const { id } = params;
    const authResult = await requireAuth(request);

    if (authResult.redirect) {
      return redirect(authResult.redirect);
    }

    const { user } = authResult;
    const issue = await getIssueById(id as string);

    if (!issue) {
      throw new Error("Issue not found");
    }

    const [workflowStates, labels, teams, users, projects] = await Promise.all([
      getWorkflowStates(),
      getIssueLabels(),
      getTeamsForUser(user.id),
      getUsers(),
      getProjects(),
    ]);

    return {
      issue,
      options: {
        workflowStates,
        labels,
        teams,
        users,
        projects,
      },
      user,
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
        projects: [],
      },
    };
  }
}

export async function action({ request }: Route.ActionArgs) {
  const { requireAuth } = await import('@/lib/auth.server');
  try {
    const authResult = await requireAuth(request);

    if (authResult.redirect) {
      return redirect(authResult.redirect);
    }

    const { action: issuesAction } = await import("../issues");
    return issuesAction({ request, params: {} } as ActionFunctionArgs);
  } catch (error) {
    console.error("Error handling issue action:", error);
    return {
      success: false,
      error: "Failed to process issue action",
    };
  }
}

/* ---------- UI helper components ---------- */

function StatusBadge({ status }: { status: Status }) {
  return (
    <Badge
      style={{
        backgroundColor: `${status.color}20`,
        color: status.color,
        borderColor: status.color,
      }}
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
      style={{
        backgroundColor: `${priority.color}20`,
        color: priority.color,
        borderColor: priority.color,
      }}
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
      style={{
        backgroundColor: `${label.color}20`,
        color: label.color,
        borderColor: label.color,
      }}
      variant="outline"
      className="py-1 px-2 text-xs"
    >
      {label.name}
    </Badge>
  );
}

function ItemSection({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="border-b border-border pb-2 mb-2 last:border-0">
      <h3 className="text-xs font-medium text-muted-foreground mb-1">
        {title}
      </h3>
      <div>{children}</div>
    </div>
  );
}

function EditableDescription({ issue }: { issue: Issue }) {
  const [isEditing, setIsEditing] = useState(false);
  const [description, setDescription] = useState(issue.description || "");
  const [isSaving, setIsSaving] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const submit = useSubmit();

  useEffect(() => {
    if (isEditing && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [isEditing]);

  const handleSave = () => {
    setIsSaving(true);
    const formData = new FormData();
    formData.append("_action", "update");
    formData.append("id", issue.id);
    formData.append("description", description);
    submit(formData, {
      method: "post",
      action: "/issues",
      replace: true,
      navigate: false,
    });
    if (typeof window !== "undefined" && window.sessionStorage) {
      sessionStorage.setItem("pendingDescriptionUpdate", issue.id);
    }
    setIsEditing(false);
    setIsSaving(false);
  };

  const handleCancel = () => {
    setDescription(issue.description || "");
    setIsEditing(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
      e.preventDefault();
      handleSave();
    }
    if (e.key === "Escape") {
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
          <Button size="sm" onClick={handleSave} disabled={isSaving}>
            {isSaving ? "Saving..." : "Save"}
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
      onDoubleClick={() => setIsEditing(true)}
    >
      {description ? (
        <p>{description}</p>
      ) : (
        <p className="text-muted-foreground italic">
          Double-click to add a description
        </p>
      )}
    </div>
  );
}

/* ---------- Main component ---------- */

export default function IssueDetails() {
  const { id } = useParams();
  const data = useLoaderData() as Route.IssueLoaderData;
  const { issue, options } = data;
  const submit = useSubmit();
  const { updateIssueStatus } = useIssuesStore();

  /* ---- GitHub token helper ---- */
  const getGithubToken = () =>
    typeof window !== "undefined" && window.localStorage
      ? localStorage.getItem("github_token") || ""
      : "";
      
  /* ---- Solver agent setup (single instance to be shared) ---- */
  const agent = useOpenAgent(issue.id, "solver");
  
  /* ---- Ensure agent has context when connected ---- */
  useEffect(() => {
    if (agent.connectionStatus === 'connected' && 
        (!agent.state?.currentIssue || !agent.state?.currentProject || !agent.state?.currentTeam)) {
      
      console.log("Setting agent context from parent component...");
      
      // Create formatted issue object
      const formattedIssue = {
        id: issue.id,
        number: parseInt(issue.identifier.replace(/[^\d]/g, '')),
        title: issue.title,
        description: issue.description || "",
        source: "openagents",
        status: issue.status.type === 'done' ? 'closed' : 'open',
        labels: issue.labels?.map((label: any) => label.name) || [],
        assignee: issue.assignee?.name,
        created: new Date(issue.createdAt),
        updated: issue.updatedAt ? new Date(issue.updatedAt) : undefined
      };
      
      // Create formatted project object
      const formattedProject = issue.project ? {
        id: issue.project.id,
        name: issue.project.name,
        color: issue.project.color,
        icon: issue.project.icon
      } : undefined;
      
      // Create formatted team object
      const formattedTeam = issue.team ? {
        id: issue.team.id,
        name: issue.team.name,
        key: issue.team.key || 'default'
      } : undefined;
      
      // Send context to agent
      try {
        const contextMessage = {
          type: "set_context",
          issue: formattedIssue,
          project: formattedProject,
          team: formattedTeam,
          timestamp: new Date().toISOString()
        };
        
        agent.sendRawMessage(contextMessage);
        console.log("✓ Agent context set successfully from parent component");
      } catch (error) {
        console.error("Failed to set agent context:", error);
      }
    }
  }, [agent.connectionStatus, agent.state, issue]);

  /* ---- Chat setup (unchanged) ---- */
  const { messages, input, handleInputChange, handleSubmit, isLoading, stop } =
    useChat({
      api: `https://chat.openagents.com/chat`,
      headers: { "X-GitHub-Token": getGithubToken() },
      maxSteps: 25,
      initialMessages: [
        {
          id: "12309123",
          role: "system",
          content: "You are an AI assistant integrated into OpenAgents...",
        },
      ],
    });

  /* ---- Effect hooks to refresh on updates ---- */
  useEffect(() => {
    if (
      typeof window !== "undefined" &&
      window.sessionStorage.getItem("pendingDescriptionUpdate") === id
    ) {
      window.sessionStorage.removeItem("pendingDescriptionUpdate");
      window.location.reload();
    }
  }, [id]);

  /* ---- Status update handler ---- */
  const handleStatusChange = (statusId: string) => {
    const newStatus = options?.workflowStates?.find(
      (state) => state.id === statusId
    );
    if (!newStatus) return;

    const formData = new FormData();
    formData.append("_action", "update");
    formData.append("id", issue.id);
    formData.append("stateId", statusId);

    if (newStatus.type === "done") {
      formData.append("completedAt", new Date().toISOString());
      formData.append("isDone", "true");
      if (issue.project?.id) formData.append("projectId", issue.project.id);
      if (issue.team?.id) formData.append("teamId", issue.team.id);
    }

    updateIssueStatus(issue.id, newStatus);
    submit(formData, { method: "post", replace: true });
  };

  /* ---- Error / Loading states ---- */
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

  const creator = issue.creator || null;
  const assignee = issue.assignee || null;

  return (
    <MainLayout header={<HeaderIssues />}>
      <div className="container mx-auto px-6 pt-2">
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 flex-1">
          {/* ---------- Main column: SolverConnector full height ---------- */}
          <div className="md:col-span-2 flex flex-col h-[calc(100vh-4rem)]">
            <div className="flex-1 overflow-hidden">
              <SolverConnector
                issue={issue}
                agent={agent}
                githubToken={getGithubToken()}
                className="w-full"
              />
            </div>
          </div>

          {/* ---------- Sidebar ---------- */}
          <div className="flex flex-col h-[calc(100vh-4rem)] overflow-y-auto">
            {/* Agent Controls Card */}
            <SolverControls 
              issue={issue}
              agent={agent}
              githubToken={getGithubToken()}
            />
            
            {/* Issue Details Card */}
            <Card className="shadow-sm flex flex-col h-full">
              {/* Header with title & quick badges */}
              <CardHeader className="py-2 px-3">
                <CardTitle className="text-sm">{issue.title}</CardTitle>
                <div className="flex items-center gap-2 mt-1">
                  {issue.project && (
                    <Badge
                      variant="secondary"
                      className="font-normal text-xs px-1 py-0"
                    >
                      {issue.project.name}
                    </Badge>
                  )}
                  <StatusBadge status={issue.status} />
                </div>
              </CardHeader>

              {/* Consolidated content */}
              <CardContent className="p-3 space-y-2 overflow-y-auto">
                {/* Description */}
                <ItemSection title="Description">
                  <EditableDescription issue={issue} />
                </ItemSection>

                {/* Subtasks */}
                {issue.subissues && issue.subissues.length > 0 && (
                  <ItemSection title="Subtasks">
                    <ul className="list-disc pl-4 text-xs space-y-1">
                      {issue.subissues.map((subId) => (
                        <li key={subId}>
                          <a
                            href={`/issues/${subId}`}
                            className="text-zinc-500 hover:underline"
                          >
                            {subId}
                          </a>
                        </li>
                      ))}
                    </ul>
                  </ItemSection>
                )}

                {/* Status */}
                <ItemSection title="Status">
                  <StatusBadge status={issue.status} />
                </ItemSection>

                {/* Priority */}
                <ItemSection title="Priority">
                  <PriorityBadge priority={issue.priority} />
                </ItemSection>

                {/* Assignee */}
                <ItemSection title="Assignee">
                  <div className="flex items-center gap-2">
                    {assignee ? (
                      <>
                        <Avatar className="h-5 w-5">
                          {assignee.image ? (
                            <AvatarImage
                              src={assignee.image}
                              alt={assignee.name}
                            />
                          ) : (
                            <AvatarFallback className="text-xs">
                              {assignee.name.charAt(0)}
                            </AvatarFallback>
                          )}
                        </Avatar>
                        <span className="text-xs">{assignee.name}</span>
                      </>
                    ) : (
                      <span className="text-xs text-muted-foreground">
                        Unassigned
                      </span>
                    )}
                  </div>
                </ItemSection>

                {/* Project */}
                {issue.project && (
                  <ItemSection title="Project">
                    <div className="flex items-center gap-1">
                      <span
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

                {/* Team */}
                {issue.team && (
                  <ItemSection title="Team">
                    <div className="flex items-center gap-1">
                      <span className="text-xs">{issue.team.name}</span>
                      <span className="text-xs text-muted-foreground">
                        ({issue.team.key})
                      </span>
                    </div>
                  </ItemSection>
                )}

                {/* Labels */}
                {issue.labels && issue.labels.length > 0 && (
                  <ItemSection title="Labels">
                    <div className="flex flex-wrap gap-1">
                      {issue.labels.map((label) => (
                        <LabelBadge key={label.id} label={label} />
                      ))}
                    </div>
                  </ItemSection>
                )}

                {/* Created */}
                <ItemSection title="Created">
                  <div className="flex items-center gap-1 text-xs">
                    <Clock className="h-3 w-3 text-muted-foreground" />
                    <span>
                      {issue.createdAt
                        ? formatDistanceToNow(new Date(issue.createdAt), {
                          addSuffix: true,
                        })
                        : "Unknown"}
                    </span>
                  </div>
                </ItemSection>

                {/* Due Date */}
                {issue.dueDate && (
                  <ItemSection title="Due Date">
                    <div className="flex items-center gap-1 text-xs">
                      <CalendarIcon className="h-3 w-3 text-muted-foreground" />
                      <span>
                        {new Date(issue.dueDate).toLocaleDateString()}
                      </span>
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
