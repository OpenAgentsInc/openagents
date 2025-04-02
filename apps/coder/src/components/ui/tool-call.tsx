import React, { useState } from "react"
import { CheckCircle2, Code2, FileText, GitBranch, Loader2, Terminal } from "lucide-react"
import { Button } from "../ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle, DialogTrigger } from "../ui/dialog"
import { cn } from "../../utils/tailwind"

interface Attachment {
  name?: string
  contentType?: string
  url: string
}

interface PartialToolCall {
  state: "partial-call"
  toolName: string
}

interface ToolCall {
  state: "call"
  toolName: string
  args?: Record<string, any>
}

interface ToolResult {
  state: "result"
  toolName: string
  args?: Record<string, any>
  result: {
    __cancelled?: boolean
    input?: Record<string, any>
    parameters?: Record<string, any>
    args?: Record<string, any>
    content?: string
    [key: string]: any
  }
}

type ToolInvocation = PartialToolCall | ToolCall | ToolResult

export interface ToolCallProps {
  toolInvocations?: ToolInvocation[]
}

// Helper function to get parameters from any invocation state
function getToolParameters(invocation: ToolInvocation) {
  if (invocation.state === "result") {
    // Try multiple possible locations for parameters
    const params = invocation.result?.input ||
      invocation.result?.parameters ||
      invocation.result?.args ||
      (invocation as any).args;
    return params;
  }
  // For calls in progress or partial calls, we don't have parameters yet
  return null;
}

// Helper function to extract repo info from tool invocation
function getRepoInfo(invocation: ToolInvocation) {
  // Get parameters based on invocation state
  const params = invocation.state === "result"
    ? (invocation.result?.input || invocation.result?.parameters || invocation.result?.args || invocation.args)
    : invocation.state === "call"
      ? invocation.args
      : undefined;

  if (!params) return null;

  // Handle grep tool's different parameter names
  if ('repoOwner' in params && 'repoName' in params) {
    return {
      owner: params.repoOwner as string,
      repo: params.repoName as string,
      branch: 'branch' in params ? params.branch as string : undefined
    };
  }

  // Handle standard GitHub tool parameter names
  if ('owner' in params && 'repo' in params) {
    return {
      owner: params.owner as string,
      repo: params.repo as string,
      branch: 'branch' in params ? params.branch as string : undefined
    };
  }

  return null;
}

export function ToolCall({ toolInvocations }: ToolCallProps) {
  if (!toolInvocations?.length) return null;

  // Lift state up - create arrays of state for each invocation
  const [resultDialogStates, setResultDialogStates] = useState<boolean[]>(
    new Array(toolInvocations.length).fill(false)
  );
  const [paramsDialogStates, setParamsDialogStates] = useState<boolean[]>(
    new Array(toolInvocations.length).fill(false)
  );

  return (
    <div className="flex flex-col items-start gap-2">
      {toolInvocations.map((invocation, index) => {
        // Use the state arrays instead of individual useState hooks
        const isResultDialogOpen = resultDialogStates[index];
        const isParamsDialogOpen = paramsDialogStates[index];

        const setResultDialogOpen = (open: boolean) => {
          const newStates = [...resultDialogStates];
          newStates[index] = open;
          setResultDialogStates(newStates);
        };

        const setParamsDialogOpen = (open: boolean) => {
          const newStates = [...paramsDialogStates];
          newStates[index] = open;
          setParamsDialogStates(newStates);
        };

        // Extract repo info from the invocation
        const repoInfo = getRepoInfo(invocation);

        switch (invocation.state) {
          case "partial-call":
          case "call":
            return (
              <Card key={index} className="bg-card text-card-foreground shadow-sm text-xs my-1 p-1 gap-1 w-full overflow-hidden">
                <CardHeader className="p-2 pb-0">
                  <div className="flex justify-between items-start">
                    <div className="flex flex-col space-y-1 overflow-hidden">
                      <div className="flex items-center space-x-2 flex-wrap">
                        <CardTitle className="text-xs">{invocation.toolName}</CardTitle>
                        {repoInfo && (
                          <div className="flex items-center text-muted-foreground overflow-hidden text-ellipsis">
                            <GitBranch className="w-3 h-3 mx-1 flex-shrink-0" />
                            <span className="overflow-hidden text-ellipsis whitespace-nowrap">{repoInfo.owner}/{repoInfo.repo}</span>
                            {repoInfo.branch && (
                              <span className="ml-1 overflow-hidden text-ellipsis whitespace-nowrap">({repoInfo.branch})</span>
                            )}
                          </div>
                        )}
                      </div>
                    </div>
                    <div className="flex-shrink-0">
                      <Loader2 className="animate-spin w-4 h-4 text-foreground" />
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="p-2">
                  <div className="flex items-center gap-2 text-sm text-muted-foreground">
                    <Terminal className="h-4 w-4" />
                    <span>Calling {invocation.toolName}...</span>
                  </div>
                </CardContent>
              </Card>
            )
          case "result":

            if (invocation.result?.__cancelled === true) {
              return (
                <Card key={index} className="bg-card text-card-foreground shadow-sm text-xs my-1 p-1 gap-1 w-full overflow-hidden">
                  <CardHeader className="p-2 pb-0">
                    <div className="flex justify-between items-start">
                      <CardTitle className="text-xs">{invocation.toolName}</CardTitle>
                    </div>
                  </CardHeader>
                  <CardContent className="p-2">
                    <div className="flex items-center gap-2 text-sm text-muted-foreground">
                      <span>Cancelled {invocation.toolName}</span>
                    </div>
                  </CardContent>
                </Card>
              )
            }

            return (
              <Card key={index} className="bg-card text-card-foreground shadow-sm text-xs my-1 p-1 gap-1 w-full overflow-hidden">
                <CardHeader className="p-2 pb-0">
                  <div className="flex justify-between items-start">
                    <div className="flex flex-col space-y-1 overflow-hidden">
                      <div className="flex items-center space-x-2 flex-wrap">
                        <CardTitle className="text-xs">{invocation.toolName}</CardTitle>
                        {repoInfo && (
                          <div className="flex items-center text-muted-foreground overflow-hidden text-ellipsis">
                            <GitBranch className="w-3 h-3 mx-1 flex-shrink-0" />
                            <span className="overflow-hidden text-ellipsis whitespace-nowrap">{repoInfo.owner}/{repoInfo.repo}</span>
                            {repoInfo.branch && (
                              <span className="ml-1 overflow-hidden text-ellipsis whitespace-nowrap">({repoInfo.branch})</span>
                            )}
                          </div>
                        )}
                      </div>
                    </div>
                    <div className="flex-shrink-0">
                      <CheckCircle2 className="text-foreground w-4 h-4" />
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="p-2">
                  <div className="flex flex-wrap gap-2 overflow-x-auto">
                    <Dialog open={isParamsDialogOpen} onOpenChange={setParamsDialogOpen}>
                      <DialogTrigger asChild>
                        <Button variant="outline" size="sm" className="h-7 flex-shrink-0">
                          <Terminal className="w-3 h-3 mr-2" />
                          View Parameters
                        </Button>
                      </DialogTrigger>
                      <DialogContent className="max-w-3xl max-h-[80vh] overflow-y-auto">
                        <DialogHeader>
                          <DialogTitle>Input Parameters</DialogTitle>
                          <DialogDescription>
                            Input parameters for {invocation.toolName}
                          </DialogDescription>
                        </DialogHeader>
                        <pre className="text-xs font-mono whitespace-pre-wrap break-all bg-muted/50 p-4">
                          {JSON.stringify(getToolParameters(invocation), null, 2)}
                        </pre>
                      </DialogContent>
                    </Dialog>
                    <Dialog open={isResultDialogOpen} onOpenChange={setResultDialogOpen}>
                      <DialogTrigger asChild>
                        <Button variant="outline" size="sm" className="h-7 flex-shrink-0">
                          <Code2 className="w-3 h-3 mr-2" />
                          View Full Result
                        </Button>
                      </DialogTrigger>
                      <DialogContent className="max-w-3xl max-h-[80vh] overflow-y-auto">
                        <DialogHeader>
                          <DialogTitle>Tool Result</DialogTitle>
                          <DialogDescription>
                            View the complete result from {invocation.toolName}
                          </DialogDescription>
                        </DialogHeader>
                        <pre className="text-xs font-mono whitespace-pre-wrap break-all bg-muted/50 p-4">
                          {JSON.stringify(invocation.result, null, 2)}
                        </pre>
                      </DialogContent>
                    </Dialog>
                  </div>
                </CardContent>
              </Card>
            )
          default:
            return null
        }
      })}
    </div>
  )
}
