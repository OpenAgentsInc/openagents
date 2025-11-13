import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { ToolCallBase, parseToolArgs } from "./ToolCallBase";
import { SearchIcon } from "lucide-react";

export const SearchTool: ToolCallMessagePartComponent = ({
  toolName,
  argsText,
  result,
  status,
}) => {
  const args = parseToolArgs(argsText);

  // Try to extract search context from args
  const pattern = args.pattern || args.query || args.search;
  const target = args.path || args.file || args.directory;

  const title = (
    <div className="flex items-center gap-2">
      <SearchIcon className="size-3" />
      <span className="text-muted-foreground">Search</span>
      {pattern && (
        <>
          <span className="text-foreground">(</span>
          <span className="text-purple-400">"{pattern}"</span>
          <span className="text-foreground">)</span>
        </>
      )}
    </div>
  );

  return (
    <ToolCallBase
      title={title}
      status={status?.type}
      argsText={argsText}
      result={result}
    >
      {target && (
        <div className="px-4">
          <p className="text-xs text-muted-foreground">
            <span className="font-semibold">Target:</span> {target}
          </p>
        </div>
      )}
    </ToolCallBase>
  );
};
