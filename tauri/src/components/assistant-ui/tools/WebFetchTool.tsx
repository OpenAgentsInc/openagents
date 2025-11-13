import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { ToolCallBase, parseToolArgs } from "./ToolCallBase";
import { GlobeIcon } from "lucide-react";

export const WebFetchTool: ToolCallMessagePartComponent = ({
  toolName,
  argsText,
  result,
  status,
}) => {
  const args = parseToolArgs(argsText);
  const url = args.url || "unknown";
  const prompt = args.prompt;

  const title = (
    <div className="flex items-center gap-2">
      <GlobeIcon className="size-3" />
      <span className="text-muted-foreground">WebFetch</span>
      <span className="text-foreground">(</span>
      <span className="text-cyan-400">{url}</span>
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
      {prompt && (
        <div className="px-4">
          <p className="text-xs text-muted-foreground">
            <span className="font-semibold">Prompt:</span> {prompt}
          </p>
        </div>
      )}
    </ToolCallBase>
  );
};
