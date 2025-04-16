import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardContent, CardFooter } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Spinner } from "@/components/ui/spinner";
import { Terminal, Columns, BotIcon, PlugZap, Power, CheckCircle, AlertTriangle } from "lucide-react";
import { useOpenAgent } from "@openagents/core";

interface SolverConnectorProps {
  issue: any;  // The issue object from the parent component
  githubToken: string;
}

// Connection states for the Solver agent
type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'error';

export function SolverConnector({ issue, githubToken }: SolverConnectorProps) {
  const [connectionState, setConnectionState] = useState<ConnectionState>('disconnected');
  const [errorMessage, setErrorMessage] = useState<string>('');
  const [isStartingSolver, setIsStartingSolver] = useState(false);

  // Create a formatted issue object to send to the Solver agent
  const formattedIssue = {
    id: issue.id,
    number: parseInt(issue.identifier.replace(/[^\d]/g, '')),
    title: issue.title,
    description: issue.description || "",
    source: "github", // or "linear" depending on your source
    status: issue.status.type === 'done' ? 'closed' : 'open',
    labels: issue.labels?.map((label: any) => label.name) || [],
    assignee: issue.assignee?.name,
    created: new Date(issue.createdAt),
    updated: issue.updatedAt ? new Date(issue.updatedAt) : undefined
  };

  // Extract repository context from issue or use defaults
  const repoInfo = {
    owner: "openagents", // Replace with dynamic value if available
    repo: "openagents",  // Replace with dynamic value if available
    branch: "main"      // Replace with dynamic value if available
  };

  // Use the OpenAgent hook to connect to the Solver agent
  const agent = useOpenAgent(`solver-${issue.id}`, "solver");

  // Handle connection to the Solver agent
  const connectToSolver = async () => {
    if (!githubToken) {
      setErrorMessage("GitHub token is required. Please set it in your account settings.");
      setConnectionState('error');
      return;
    }

    setConnectionState('connecting');
    setIsStartingSolver(true);

    try {
      // Set GitHub token for the agent
      await agent.setGithubToken(githubToken);

      // Set repository context
      if (agent.setRepositoryContext) {
        await agent.setRepositoryContext(repoInfo.owner, repoInfo.repo, repoInfo.branch);
      }

      // Set the current issue context
      if (agent.setCurrentIssue) {
        await agent.setCurrentIssue(formattedIssue);
      }

      // Set up the issue context with the agent
      await agent.handleSubmit(`I need help with issue ${issue.identifier}: "${issue.title}". Please analyze this issue and suggest a plan to solve it.`);

      // Start inference on the agent
      await agent.infer(githubToken);

      setConnectionState('connected');
    } catch (err) {
      console.error("Error connecting to Solver agent:", err);
      setErrorMessage(err instanceof Error ? err.message : "Failed to connect to the Solver agent. Please try again later.");
      setConnectionState('error');
    } finally {
      setIsStartingSolver(false);
    }
  };

  // This can be used to determine if the button should be disabled
  const isConnectButtonDisabled = isStartingSolver || !githubToken;

  // Disconnect from the Solver agent
  const disconnectFromSolver = () => {
    // Reset messages
    agent.setMessages([]);
    setConnectionState('disconnected');
  };

  // Update connection state based on changes in agent state
  useEffect(() => {
    if (agent.messages.length > 0 && connectionState === 'disconnected') {
      setConnectionState('connected');
    }

    // In real implementation, you'd want to handle potential error states here
  }, [agent.messages, connectionState]);

  return (
    <Card className="mb-6">
      <CardHeader className="pb-2">
        <div className="flex justify-between items-center">
          <CardTitle className="text-lg flex items-center">
            <BotIcon className="h-5 w-5 mr-2" />
            Solver Agent
          </CardTitle>
          <Badge
            variant={
              connectionState === 'connected' ? "success" :
                connectionState === 'connecting' ? "warning" :
                  connectionState === 'error' ? "destructive" : "secondary"
            }
          >
            {connectionState === 'connected' ? "Connected" :
              connectionState === 'connecting' ? "Connecting..." :
                connectionState === 'error' ? "Error" : "Disconnected"}
          </Badge>
        </div>
      </CardHeader>

      <CardContent>
        {connectionState === 'disconnected' && (
          <div className="text-center py-6">
            <Terminal className="h-12 w-12 mx-auto mb-4 text-muted-foreground" />
            <h3 className="text-lg font-medium mb-2">Solver Agent Disconnected</h3>
            <p className="text-muted-foreground mb-4">
              The Solver agent can analyze this issue and help implement a solution.
              It will create a structured plan and guide you through fixing the issue.
            </p>
          </div>
        )}

        {connectionState === 'connecting' && (
          <div className="text-center py-6">
            <Spinner className="h-12 w-12 mx-auto mb-4" />
            <h3 className="text-lg font-medium mb-2">Connecting to Solver Agent</h3>
            <p className="text-muted-foreground">
              Establishing connection and analyzing issue #{issue.identifier}...
            </p>
          </div>
        )}

        {connectionState === 'error' && (
          <div className="text-center py-6">
            <AlertTriangle className="h-12 w-12 mx-auto mb-4 text-destructive" />
            <h3 className="text-lg font-medium mb-2">Connection Error</h3>
            <p className="text-muted-foreground mb-4">
              {errorMessage || "Failed to connect to the Solver agent. Please try again."}
            </p>
          </div>
        )}

        {connectionState === 'connected' && (
          <div className="py-2">
            <div className="flex items-center mb-4">
              <CheckCircle className="h-5 w-5 mr-2 text-green-500" />
              <span className="font-medium">Solver Agent Connected</span>
            </div>

            {agent.messages.length > 1 && (
              <div className="border rounded-md p-3 mb-4 bg-muted/50">
                <h4 className="font-medium mb-2">Latest Update:</h4>
                <p className="text-sm">
                  {agent.messages[agent.messages.length - 1].content.substring(0, 150)}
                  {agent.messages[agent.messages.length - 1].content.length > 150 ? '...' : ''}
                </p>
              </div>
            )}

            <div className="flex flex-col gap-2">
              <Button size="sm" variant="outline" onClick={() => window.open(`/chat?agent=solver&issue=${issue.id}`, '_blank')}>
                <Columns className="h-4 w-4 mr-2" />
                Open Full Solver Interface
              </Button>
            </div>
          </div>
        )}
      </CardContent>

      <CardFooter className="flex justify-end">
        {connectionState === 'disconnected' && (
          isConnectButtonDisabled ? (
            <Button
              variant="secondary"
              className="opacity-50 cursor-not-allowed"
            >
              <PlugZap className="h-4 w-4 mr-2" />
              Connect to Solver
            </Button>
          ) : (
            <Button
              onClick={connectToSolver}
            >
              <PlugZap className="h-4 w-4 mr-2" />
              Connect to Solver
            </Button>
          )
        )}

        {(connectionState === 'connected' || connectionState === 'error') && (
          <Button
            variant="outline"
            onClick={disconnectFromSolver}
          >
            <Power className="h-4 w-4 mr-2" />
            Disconnect
          </Button>
        )}
      </CardFooter>
    </Card>
  );
}
