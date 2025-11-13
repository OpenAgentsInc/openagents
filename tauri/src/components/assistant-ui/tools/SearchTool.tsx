import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { ToolCallBase, parseToolArgs } from "./ToolCallBase";
import { SearchIcon } from "lucide-react";

export const SearchTool: ToolCallMessagePartComponent = (props) => {
  const { argsText, result, status } = props;
  const argsFromRuntime = (props as any).args as Record<string, any> | undefined;
  const args = argsFromRuntime && Object.keys(argsFromRuntime).length > 0 ? argsFromRuntime : parseToolArgs(argsText);

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
      debugData={props}
      debugLabel="ToolCall"
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
