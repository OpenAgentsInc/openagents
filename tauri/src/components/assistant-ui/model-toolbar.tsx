import { useModelStore } from "@/lib/model-store";
import type { ModelKind } from "@/lib/model-store";
import { useWorkingDirStore } from "@/lib/working-dir-store";
import { useProjectStore } from "@/lib/project-store";
import { pickDirectory, validateDirectory } from "@/lib/tauri-acp";
import type { FC } from "react";
import { useState, useEffect } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@openagentsinc/ui";
import { Input } from "@openagentsinc/ui";
import { Button } from "@openagentsinc/ui";
import { Badge } from "@openagentsinc/ui";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@openagentsinc/ui";
import { FolderIcon, AlertTriangleIcon, FolderOpenIcon } from "lucide-react";
import { useAssistantState } from "@openagentsinc/assistant-ui-runtime";
import { useUiStore } from "@/lib/ui-store";

// Toolbar that lives to the right of the sidebar, at the very top of the chat area
// (separate from the draggable window header).
export const ModelToolbar: FC = () => {
  const model = useModelStore((s) => s.selected);
  const setModel = useModelStore((s) => s.setSelected);
  const currentThreadId = useAssistantState((s) => s.threads?.mainThreadId);
  const route = useUiStore((s) => s.route);
  // Subscribe to the derived working directory so changes re-render
  const perThreadWorkingDir = useWorkingDirStore((s) => s.getThreadCwd(currentThreadId));
  const defaultCwd = useWorkingDirStore((s) => s.defaultCwd);
  const warning = useWorkingDirStore((s) => s.warning);
  const setThreadCwd = useWorkingDirStore((s) => s.setThreadCwd);
  const setDefaultCwd = useWorkingDirStore((s) => s.setDefaultCwd);
  const clearWarning = useWorkingDirStore((s) => s.clearWarning);
  const activeProject = useProjectStore((s) => s.getActiveProject());

  // Choose display context: project panel overrides everything; otherwise thread cwd; otherwise default
  const isProjectPanel = route.kind === "project" && !!activeProject;
  const workingDir = isProjectPanel
    ? activeProject!.path
    : currentThreadId
      ? perThreadWorkingDir
      : activeProject
        ? activeProject.path
        : defaultCwd;

  const [localWorkingDir, setLocalWorkingDir] = useState(workingDir);
  const [isValidating, setIsValidating] = useState(false);
  const [isEditing, setIsEditing] = useState(false);

  useEffect(() => {
    // Do not clobber typed text while editing
    if (!isEditing) setLocalWorkingDir(workingDir);
  }, [workingDir, isEditing]);

  const handleModelChange = (v: string) => {
    setModel(v as ModelKind);
    // Focus the composer after changing model
    setTimeout(() => {
      window.dispatchEvent(new Event("openagents:focus-composer"));
    }, 80);
  };

  const handleWorkingDirChange = (value: string) => {
    // Only update local state while typing; commit on blur/enter
    setLocalWorkingDir(value);
  };

  const commitWorkingDir = async () => {
    const next = localWorkingDir;
    if (!next || next === workingDir) return;
    setIsValidating(true);
    try {
      const isValid = await validateDirectory(next);
      if (isValid) {
        if (currentThreadId) {
          setThreadCwd(currentThreadId, next);
        } else {
          setDefaultCwd(next);
        }
        clearWarning();
      }
    } finally {
      setIsValidating(false);
    }
  };

  const handlePickDirectory = async () => {
    const selected = await pickDirectory();
    if (selected) {
      setLocalWorkingDir(selected);
      // If we have a current thread, set thread-specific cwd
      if (currentThreadId) {
        setThreadCwd(currentThreadId, selected);
      } else {
        // Otherwise set default
        setDefaultCwd(selected);
      }
      clearWarning();
    }
  };

  // Show only the folder name when not editing; on focus show full path
  const displayPath = (path: string) => {
    if (!path) return "";
    // Remove trailing separators and split on POSIX/Windows separators
    const trimmed = path.replace(/[\\\/]+$/, "");
    const parts = trimmed.split(/[\\\/]/).filter(Boolean);
    return parts.length ? parts[parts.length - 1] : trimmed;
  };

  const shownValue = isEditing ? localWorkingDir : displayPath(localWorkingDir);

  return (
    <div className="sticky top-0 z-10 bg-background border-b border-zinc-800">
      <div className="flex h-10 items-center gap-3 px-3">
        <Select value={model} onValueChange={handleModelChange}>
          <SelectTrigger size="sm" aria-label="Model">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="codex">Codex</SelectItem>
            <SelectItem value="claude-code">Claude Code</SelectItem>
          </SelectContent>
        </Select>

        {activeProject && (
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Badge variant="secondary" className="flex items-center gap-1 text-xs">
                  <FolderOpenIcon className="h-3 w-3" />
                  {activeProject.name}
                </Badge>
              </TooltipTrigger>
              <TooltipContent side="bottom">
                <p className="text-xs font-mono">{activeProject.path}</p>
                {activeProject.description && (
                  <p className="text-xs text-muted-foreground mt-1">{activeProject.description}</p>
                )}
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        )}

        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
              <div className="flex items-center gap-1 w-auto">
                <Input
                  value={shownValue}
                  onChange={(e) => handleWorkingDirChange(e.target.value)}
                  placeholder="Working directory..."
                  className="h-7 text-xs font-mono w-auto"
                  onFocus={() => setIsEditing(true)}
                  onBlur={() => {
                    void commitWorkingDir();
                    setIsEditing(false);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.currentTarget.blur();
                    }
                  }}
                  disabled={isProjectPanel}
                />
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={handlePickDirectory}
                  className="h-7 w-7 p-0"
                  title="Pick directory"
                >
                  <FolderIcon className="h-4 w-4" />
                </Button>
                {warning && (
                  <AlertTriangleIcon className="h-4 w-4 text-yellow-500" />
                )}
              </div>
            </TooltipTrigger>
            <TooltipContent side="bottom">
              <p className="text-xs font-mono">
                {warning ? warning.message : workingDir}
              </p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>
    </div>
  );
};

export default ModelToolbar;
