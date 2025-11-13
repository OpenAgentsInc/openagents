import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { ToolCallBase, parseToolArgs } from "./ToolCallBase";
import { FolderSearchIcon } from "lucide-react";

export const GlobTool: ToolCallMessagePartComponent = (props) => {
  const { argsText, result, status } = props;
  const args = parseToolArgs(argsText);
  const pattern = args.pattern || "unknown";
  const path = args.path;

  const title = (
    <div className="flex items-center gap-2">
      <FolderSearchIcon className="size-3" />
      <span className="text-muted-foreground">Glob</span>
      <span className="text-foreground">(</span>
      <span className="text-purple-400">"{pattern}"</span>
      <span className="text-foreground">)</span>
    </div>
  );

  return (
    <ToolCallBase
      title={title}
      status={status?.type}
      argsText={argsText}
      result={result}
      debugData={props}
      debugLabel="ToolCall"
    >
      {path && (
        <div className="px-4">
          <p className="text-xs text-muted-foreground">
            <span className="font-semibold">Path:</span> {path}
          </p>
        </div>
      )}
    </ToolCallBase>
  );
};
