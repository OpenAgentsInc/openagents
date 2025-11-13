import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { ToolCallBase, parseToolArgs } from "./ToolCallBase";
import { TerminalIcon } from "lucide-react";

export const BashTool: ToolCallMessagePartComponent = ({
  toolName,
  argsText,
  result,
  status,
}) => {
  const args = parseToolArgs(argsText);
  const command = args.command || "unknown command";
  const description = args.description;

  const title = (
    <div className="flex items-center gap-2">
      <TerminalIcon className="size-3" />
      <span className="text-muted-foreground">$</span>
      <span className="text-foreground">{command}</span>
    </div>
  );

  return (
    <ToolCallBase
      title={title}
      status={status?.type}
      argsText={argsText}
      result={result}
    >
      {description && (
        <div className="px-4">
          <p className="text-xs text-muted-foreground italic">
            {description}
          </p>
        </div>
      )}
    </ToolCallBase>
  );
};
