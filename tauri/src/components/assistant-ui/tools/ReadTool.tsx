import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { ToolCallBase, parseToolArgs } from "./ToolCallBase";

export const ReadTool: ToolCallMessagePartComponent = (props) => {
  const { argsText, result, status } = props;
  const argsFromRuntime = (props as any).args as Record<string, any> | undefined;
  const args = argsFromRuntime && Object.keys(argsFromRuntime).length > 0 ? argsFromRuntime : parseToolArgs(argsText);
  const filePath = args.file_path || args.path || "unknown";

  // Extract just the filename from the full path
  const fileName = filePath.split('/').pop() || filePath;

  const title = (
    <>
      <span className="text-muted-foreground">Read</span>
      <span className="text-foreground">(</span>
      <span className="text-blue-400">{fileName}</span>
      <span className="text-foreground">)</span>
    </>
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
      <div className="px-4">
        <p className="text-xs text-muted-foreground">
          {filePath}
        </p>
      </div>
    </ToolCallBase>
  );
};
