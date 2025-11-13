import type { FC, FormEvent } from "react";
import { useMemo, useState } from "react";
import { useUiStore } from "@/lib/ui-store";
import { useProjectStore } from "@/lib/project-store";
import { ThreadComposer } from "@/components/assistant-ui/thread";
import { Badge } from "@/components/ui/badge";
import { FolderOpenIcon } from "lucide-react";
import { useAssistantRuntime, useAssistantApi } from "@openagentsinc/assistant-ui-runtime";

export const ProjectPanel: FC = () => {
  const route = useUiStore((s) => s.route);
  const getProject = useProjectStore((s) => s.getProject);
  const setActiveProject = useProjectStore((s) => s.setActiveProject);
  const clearProjectView = useUiStore((s) => s.clearProjectView);
  const runtime = useAssistantRuntime();
  const api = useAssistantApi();

  const projectId = route.kind === "project" ? route.projectId : null;
  const project = projectId ? getProject(projectId) : undefined;

  // When the composer submits from the project page, we want the runtime to
  // associate the new thread with this project. Clicking a project already
  // sets activeProject, so the runtime's onNew handler will attach projectId
  // and use the project's path as cwd. No additional logic needed here.

  if (!project) {
    return (
      <div className="p-6 text-sm text-muted-foreground">Select a project to view details.</div>
    );
  }

  const displayFolderName = (path: string) => {
    const trimmed = path.replace(/[\\\/]+$/, "");
    const parts = trimmed.split(/[\\\/]/).filter(Boolean);
    return parts.length ? parts[parts.length - 1] : trimmed;
  };

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-zinc-800">
        <div className="mx-auto w-full max-w-[48rem] flex items-center gap-2 px-4 py-3">
          <Badge variant="secondary" className="flex items-center gap-1">
            <FolderOpenIcon className="h-3 w-3" />
            {project.name}
          </Badge>
          <span className="text-xs text-muted-foreground">Project</span>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-[48rem] p-6 space-y-4">
          {project.description && (
            <p className="text-sm text-muted-foreground">{project.description}</p>
          )}

          <div>
            <div className="text-xs text-muted-foreground mb-1">Default working directory</div>
            <div className="rounded-md border border-zinc-800 bg-zinc-900/40 p-3">
              <div className="text-sm font-mono">{project.path}</div>
              <div className="text-xs text-muted-foreground">{displayFolderName(project.path)}</div>
            </div>
          </div>
        </div>
      </div>

      <div className="border-t border-zinc-800">
        <div className="mx-auto w-full max-w-[48rem]">
          <ThreadComposer />
        </div>
      </div>
    </div>
  );
};

export default ProjectPanel;
