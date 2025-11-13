import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { ToolCallBase, parseToolArgs } from "./ToolCallBase";
import { EditIcon } from "lucide-react";

export const EditTool: ToolCallMessagePartComponent = ({
  toolName,
  argsText,
  result,
  status,
}) => {
  const args = parseToolArgs(argsText);
  const filePath = args.file_path || args.path || "unknown";
  const fileName = filePath.split('/').pop() || filePath;
  const oldString = args.old_string;
  const newString = args.new_string;

  const title = (
    <div className="flex items-center gap-2">
      <EditIcon className="size-3" />
      <span className="text-muted-foreground">Edit</span>
      <span className="text-foreground">(</span>
      <span className="text-yellow-400">{fileName}</span>
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
      <div className="px-4 space-y-2">
        <p className="text-xs text-muted-foreground">
          {filePath}
        </p>
        {oldString && newString && (
          <div className="border rounded p-2 space-y-1 text-xs">
            <div className="text-red-400">
              <span className="text-muted-foreground">- </span>
              {oldString.length > 100 ? oldString.substring(0, 100) + "..." : oldString}
            </div>
            <div className="text-green-400">
              <span className="text-muted-foreground">+ </span>
              {newString.length > 100 ? newString.substring(0, 100) + "..." : newString}
            </div>
          </div>
        )}
      </div>
    </ToolCallBase>
  );
};
