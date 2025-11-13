import type { FC, FormEvent } from "react";
import { useMemo, useState } from "react";
import { useUiStore } from "@/lib/ui-store";
import { useProjectStore } from "@/lib/project-store";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { FolderOpenIcon } from "lucide-react";
import { useAssistantRuntime } from "@openagentsinc/assistant-ui-runtime";

export const ProjectPanel: FC = () => {
  const route = useUiStore((s) => s.route);
  const getProject = useProjectStore((s) => s.getProject);
  const setActiveProject = useProjectStore((s) => s.setActiveProject);
  const clearProjectView = useUiStore((s) => s.clearProjectView);
  const runtime = useAssistantRuntime();

  const projectId = route.kind === "project" ? route.projectId : null;
  const project = projectId ? getProject(projectId) : undefined;

  const [prompt, setPrompt] = useState("");

  const startChat = async (e?: FormEvent) => {
    if (e) e.preventDefault();
    if (!project) return;
    const text = prompt.trim();
    // Scope to this project
    setActiveProject(project.id);
    clearProjectView();
    if (runtime.switchToNewThread) {
      runtime.switchToNewThread();
    }
    // Prefill the composer and focus
    if (text.length > 0) {
      const ev = new CustomEvent("openagents:prefill-composer", { detail: { text } });
      window.dispatchEvent(ev);
    }
    const focusEv = new Event("openagents:focus-composer");
    window.dispatchEvent(focusEv);
    setPrompt("");
  };

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
      <div className="flex items-center gap-2 border-b border-zinc-800 px-4 py-3">
        <Badge variant="secondary" className="flex items-center gap-1">
          <FolderOpenIcon className="h-3 w-3" />
          {project.name}
        </Badge>
        <span className="text-xs text-muted-foreground">Project</span>
      </div>

      <div className="flex-1 overflow-y-auto p-6">
        <div className="space-y-4">
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

      <form onSubmit={startChat} className="border-t border-zinc-800 p-4">
        <div className="flex items-center gap-2">
          <Input
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder={`Start a chat in ${project.name}...`}
            className="font-normal"
          />
          <Button type="submit" disabled={prompt.trim().length === 0}>Start</Button>
        </div>
      </form>
    </div>
  );
};

export default ProjectPanel;

