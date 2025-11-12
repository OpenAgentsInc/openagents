import { useModelStore } from "@/lib/model-store";
import type { ModelKind } from "@/lib/model-store";
import type { FC } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

// Toolbar that lives to the right of the sidebar, at the very top of the chat area
// (separate from the draggable window header).
export const ModelToolbar: FC = () => {
  const model = useModelStore((s) => s.selected);
  const setModel = useModelStore((s) => s.setSelected);

  const handleModelChange = (v: string) => {
    setModel(v as ModelKind);
    // Focus the composer after changing model
    setTimeout(() => {
      window.dispatchEvent(new Event("openagents:focus-composer"));
    }, 80);
  };

  return (
    <div className="sticky top-0 z-10 bg-background border-b border-zinc-800">
      <div className="flex h-10 items-center gap-2 px-3">
        <Select value={model} onValueChange={handleModelChange}>
          <SelectTrigger size="sm" aria-label="Model">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="codex">Codex</SelectItem>
            <SelectItem value="claude-code">Claude Code</SelectItem>
            <SelectItem value="ollama">GLM-4.6</SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>
  );
};

export default ModelToolbar;
