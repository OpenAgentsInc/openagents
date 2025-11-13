import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { ToolCallBase, parseToolArgs } from "./ToolCallBase";
import { SearchIcon } from "lucide-react";

export const WebSearchTool: ToolCallMessagePartComponent = ({
  toolName,
  argsText,
  result,
  status,
}) => {
  const args = parseToolArgs(argsText);
  const query = args.query || "unknown";

  const title = (
    <div className="flex items-center gap-2">
      <SearchIcon className="size-3" />
      <span className="text-muted-foreground">WebSearch</span>
      <span className="text-foreground">(</span>
      <span className="text-cyan-400">"{query}"</span>
      <span className="text-foreground">)</span>
    </div>
  );

  return (
    <ToolCallBase
      title={title}
      status={status?.type}
      argsText={argsText}
      result={result}
    />
  );
};
