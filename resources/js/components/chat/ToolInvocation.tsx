import { CheckCircle2, FileText, Loader2, Settings } from "lucide-react"
import { useState } from "react"
import { Button } from "../ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card"
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger
} from "../ui/dialog"

interface JSONValue {
  [key: string]: any;
}

export interface ToolInvocation {
  id?: string;
  toolCallId?: string;
  tool_name?: string;
  toolName?: string;
  input?: JSONValue;
  args?: JSONValue;
  output?: JSONValue;
  result?: JSONValue;
  status?: 'pending' | 'completed' | 'failed';
  state?: 'call' | 'result';
}

const ensureObject = (value: JSONValue): Record<string, any> => {
  if (typeof value === 'object' && value !== null) {
    return value as Record<string, any>;
  }
  if (typeof value === 'string') {
    try {
      return JSON.parse(value);
    } catch (error) {
      console.error("Failed to parse as JSON:", error);
    }
  }
  return {};
};

export function ToolInvocation({ toolInvocation }: { toolInvocation: ToolInvocation }) {
  const [isFileContentDialogOpen, setIsFileContentDialogOpen] = useState(false);
  const [isInputParamsDialogOpen, setIsInputParamsDialogOpen] = useState(false);

  if (!toolInvocation || typeof toolInvocation !== 'object') {
    console.error("Invalid toolInvocation prop:", toolInvocation);
    return <div>Error: Invalid tool invocation data</div>;
  }

  const {
    id,
    toolCallId,
    tool_name,
    toolName,
    input,
    args,
    output,
    result,
    status,
    state
  } = toolInvocation;

  const displayId = id || toolCallId;
  const displayName = tool_name || toolName;
  const displayInput = input || args as JSONValue;
  const displayOutput = output || result;
  const displayStatus = status || (state === 'result' ? 'completed' : 'pending');

  const inputObject = ensureObject(displayInput);
  const outputObject = displayOutput ? ensureObject(displayOutput) : null;

  const { owner, repo, branch } = inputObject;

  const repoInfo = owner && repo && branch
    ? `${owner}/${repo} (${branch})`
    : null;

  const renderStateIcon = () => {
    if (displayStatus === 'pending') {
      return <Loader2 className="animate-spin w-5 h-5 text-foreground" />;
    } else if (displayStatus === 'completed') {
      return <CheckCircle2 className="text-foreground w-6 h-6" />;
    } else if (displayStatus === 'failed') {
      return <span className="text-red-500">Failed</span>;
    }
    return null;
  };


  const summary = outputObject?.summary || outputObject?.value?.result?.summary || outputObject?.value?.result?.details || "---";

  const fileContent = outputObject?.content;

  return (
    <Card className="text-xs mb-2">
      <CardHeader className="p-3">
        <div className="flex justify-between items-start">
          <div className="flex items-center space-x-2">
            <CardTitle>{displayName}</CardTitle>
            {repoInfo && (
              <span className="text-xs text-zinc-500">
                {repoInfo}
              </span>
            )}
          </div>
          <div>{renderStateIcon()}</div>
        </div>
      </CardHeader>
      <CardContent>
        {summary && <p className="mb-2">{summary}</p>}
        <div className="flex space-x-2">
          <Dialog open={isInputParamsDialogOpen} onOpenChange={setIsInputParamsDialogOpen}>
            <DialogTrigger asChild>
              <Button variant="outline" size="sm">
                <Settings className="w-4 h-4 mr-2" />
                View Input Params
              </Button>
            </DialogTrigger>
            <DialogContent className="max-w-3xl max-h-[80vh] overflow-y-auto">
              <DialogHeader>
                <DialogTitle>Input Parameters</DialogTitle>
              </DialogHeader>
              <pre className="text-xs whitespace-pre-wrap break-all">
                {JSON.stringify(inputObject, null, 2)}
              </pre>
            </DialogContent>
          </Dialog>
          {fileContent && (
            <Dialog open={isFileContentDialogOpen} onOpenChange={setIsFileContentDialogOpen}>
              <DialogTrigger asChild>
                <Button variant="outline" size="sm">
                  <FileText className="w-4 h-4 mr-2" />
                  View File Content
                </Button>
              </DialogTrigger>
              <DialogContent className="max-w-3xl max-h-[80vh] overflow-y-auto">
                <DialogHeader>
                  <DialogTitle>File Content</DialogTitle>
                </DialogHeader>
                <pre className="text-xs whitespace-pre-wrap break-all">
                  {fileContent}
                </pre>
              </DialogContent>
            </Dialog>
          )}
        </div>
      </CardContent>
    </Card>
  )
}
