import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { ToolCallBase, parseToolArgs } from "./ToolCallBase";
import { SearchIcon } from "lucide-react";

export const GrepTool: ToolCallMessagePartComponent = ({
  toolName,
  argsText,
  result,
  status,
}) => {
  const args = parseToolArgs(argsText);
  const pattern = args.pattern || "unknown";
  const path = args.path;
  const glob = args.glob;
  const outputMode = args.output_mode || "files_with_matches";

  const title = (
    <div className="flex items-center gap-2">
      <SearchIcon className="size-3" />
      <span className="text-muted-foreground">Grep</span>
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
    >
      <div className="px-4 space-y-1 text-xs">
        {path && (
          <p className="text-muted-foreground">
            <span className="font-semibold">Path:</span> {path}
          </p>
        )}
        {glob && (
          <p className="text-muted-foreground">
            <span className="font-semibold">Glob:</span> {glob}
          </p>
        )}
        <p className="text-muted-foreground">
          <span className="font-semibold">Mode:</span> {outputMode}
        </p>
      </div>
    </ToolCallBase>
  );
};
