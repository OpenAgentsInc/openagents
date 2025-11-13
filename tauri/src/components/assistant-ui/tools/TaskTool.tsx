import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { ToolCallBase, parseToolArgs } from "./ToolCallBase";
import { BrainCircuitIcon } from "lucide-react";

export const TaskTool: ToolCallMessagePartComponent = ({
  toolName,
  argsText,
  result,
  status,
}) => {
  const args = parseToolArgs(argsText);
  const description = args.description || "Task";
  const subagentType = args.subagent_type;

  const title = (
    <div className="flex items-center gap-2">
      <BrainCircuitIcon className="size-3" />
      <span className="text-muted-foreground">Task</span>
      <span className="text-foreground">: </span>
      <span className="text-orange-400">{description}</span>
      {subagentType && (
        <span className="text-xs text-muted-foreground">({subagentType})</span>
      )}
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
