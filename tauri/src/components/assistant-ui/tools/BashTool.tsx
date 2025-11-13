import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { ToolCallBase, parseToolArgs } from "./ToolCallBase";
import { TerminalIcon } from "lucide-react";

export const BashTool: ToolCallMessagePartComponent = (props) => {
  const { argsText, result, status } = props;
  const argsFromRuntime = (props as any).args as Record<string, any> | undefined;
  const args = argsFromRuntime && Object.keys(argsFromRuntime).length > 0 ? argsFromRuntime : parseToolArgs(argsText);
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
      debugData={props}
      debugLabel="ToolCall"
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
