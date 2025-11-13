import { useModelStore } from "@/lib/model-store";
import type { ModelKind } from "@/lib/model-store";
import { useWorkingDirStore } from "@/lib/working-dir-store";
import { pickDirectory, validateDirectory } from "@/lib/tauri-acp";
import type { FC } from "react";
import { useState, useEffect } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { FolderIcon, AlertTriangleIcon } from "lucide-react";

// Toolbar that lives to the right of the sidebar, at the very top of the chat area
// (separate from the draggable window header).
export const ModelToolbar: FC = () => {
  const model = useModelStore((s) => s.selected);
  const setModel = useModelStore((s) => s.setSelected);
  const workingDir = useWorkingDirStore((s) => s.defaultCwd);
  const warning = useWorkingDirStore((s) => s.warning);
  const setDefaultCwd = useWorkingDirStore((s) => s.setDefaultCwd);
  const clearWarning = useWorkingDirStore((s) => s.clearWarning);

  const [localWorkingDir, setLocalWorkingDir] = useState(workingDir);
  const [isValidating, setIsValidating] = useState(false);
  const [isEditing, setIsEditing] = useState(false);

  useEffect(() => {
    setLocalWorkingDir(workingDir);
  }, [workingDir]);

  const handleModelChange = (v: string) => {
    setModel(v as ModelKind);
    // Focus the composer after changing model
    setTimeout(() => {
      window.dispatchEvent(new Event("openagents:focus-composer"));
    }, 80);
  };

  const handleWorkingDirChange = async (value: string) => {
    setLocalWorkingDir(value);

    // Validate and update if valid
    setIsValidating(true);
    try {
      const isValid = await validateDirectory(value);
      if (isValid) {
        setDefaultCwd(value);
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
      setDefaultCwd(selected);
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

        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
              <div className="flex items-center gap-1 w-auto">
                <Input
                  value={shownValue}
                  onChange={(e) => handleWorkingDirChange(e.target.value)}
                  placeholder="Working directory..."
                  className="h-7 text-xs font-mono w-auto"
                  size={Math.max(1, shownValue.length || 0)}
                  onFocus={() => setIsEditing(true)}
                  onBlur={() => setIsEditing(false)}
                  disabled={isValidating}
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
