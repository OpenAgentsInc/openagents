import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { ToolCallBase, parseToolArgs } from "./ToolCallBase";
import { FileIcon } from "lucide-react";

export const WriteTool: ToolCallMessagePartComponent = ({
  toolName,
  argsText,
  result,
  status,
}) => {
  const args = parseToolArgs(argsText);
  const filePath = args.file_path || args.path || "unknown";
  const fileName = filePath.split('/').pop() || filePath;

  const title = (
    <div className="flex items-center gap-2">
      <FileIcon className="size-3" />
      <span className="text-muted-foreground">Write</span>
      <span className="text-foreground">(</span>
      <span className="text-green-400">{fileName}</span>
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
      <div className="px-4">
        <p className="text-xs text-muted-foreground">
          {filePath}
        </p>
      </div>
    </ToolCallBase>
  );
};
