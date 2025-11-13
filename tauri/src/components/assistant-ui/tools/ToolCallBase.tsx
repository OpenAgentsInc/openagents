import type { ToolCallMessagePartComponent } from "@openagentsinc/assistant-ui-runtime";
import { CheckIcon, ChevronDownIcon, ChevronUpIcon, XIcon, LoaderIcon } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";

interface ToolCallBaseProps {
  title: ReactNode;
  status?: string;
  argsText?: string;
  result?: unknown;
  children?: ReactNode;
  showArgs?: boolean;
}

export const ToolCallBase = ({
  title,
  status,
  argsText,
  result,
  children,
  showArgs = false,
}: ToolCallBaseProps) => {
  const [isCollapsed, setIsCollapsed] = useState(true);

  const getStatusIcon = () => {
    switch (status) {
      case "completed":
        return <CheckIcon className="size-4 text-green-500" />;
      case "failed":
        return <XIcon className="size-4 text-red-500" />;
      case "in_progress":
        return <LoaderIcon className="size-4 animate-spin text-blue-500" />;
      default:
        return <CheckIcon className="size-4" />;
    }
  };

  const hasContent = children || showArgs || result !== undefined;

  return (
    <div className="mb-4 flex w-full flex-col gap-3 rounded-[var(--radius-lg)] border py-1.5">
      <div className="flex items-center gap-2 px-4">
        {getStatusIcon()}
        <div className="flex-grow font-mono text-sm">{title}</div>
        {hasContent && (
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setIsCollapsed(!isCollapsed)}
          >
            {isCollapsed ? <ChevronUpIcon className="size-4" /> : <ChevronDownIcon className="size-4" />}
          </Button>
        )}
      </div>
      {!isCollapsed && hasContent && (
        <div className="flex flex-col gap-2 border-t pt-2">
          {children}
          {showArgs && argsText && (
            <div className="px-4">
              <p className="mb-1 text-xs font-semibold uppercase text-muted-foreground">
                Arguments
              </p>
              <pre className="whitespace-pre-wrap text-xs">
                {argsText}
              </pre>
            </div>
          )}
          {result !== undefined && (
            <div className="border-t border-dashed px-4 pt-2">
              <p className="mb-1 text-xs font-semibold uppercase text-muted-foreground">
                Result
              </p>
              <pre className="whitespace-pre-wrap text-xs">
                {typeof result === "string"
                  ? result
                  : JSON.stringify(result, null, 2)}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export const parseToolArgs = (argsText: string): Record<string, any> => {
  try {
    return JSON.parse(argsText);
  } catch {
    return {};
  }
};
