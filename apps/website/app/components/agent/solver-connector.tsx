import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardContent, CardFooter } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Spinner } from "@/components/ui/spinner";
import { Terminal, Columns, BotIcon, PlugZap, Power, CheckCircle, AlertTriangle } from "lucide-react";
import { useChat } from "@ai-sdk/react";

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

  // Note: This chat instance is specifically for the Solver agent
  // It's separate from the main chat in the issues page
  const {
    messages,
    input,
    handleInputChange,
    handleSubmit,
    isLoading,
    data,
    error,
    append
  } = useChat({
    api: `https://agents.openagents.com/agent/solver`, // Use the Solver agent endpoint
    headers: {
      'X-GitHub-Token': githubToken
    },
    body: {
      repoOwner: "openagents", // Replace with your actual repo owner
      repoName: "openagents", // Replace with your actual repo name
      issue: formattedIssue
    },
    id: `solver-${issue.id}`, // Unique ID for this chat session
  });

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
      // Send an initial message to the Solver agent with context
      await append({
        role: 'user',
        content: `I need help with issue ${issue.identifier}: "${issue.title}". Please analyze this issue and suggest a plan to solve it.`
      });

      setConnectionState('connected');
    } catch (err) {
      console.error("Error connecting to Solver agent:", err);
      setErrorMessage("Failed to connect to the Solver agent. Please try again later.");
      setConnectionState('error');
    } finally {
      setIsStartingSolver(false);
    }
  };

  // Disconnect from the Solver agent
  const disconnectFromSolver = () => {
    setConnectionState('disconnected');
    // Note: In a real implementation, you might want to send a message to the agent
    // to properly close the connection or reset the agent state
  };

  // Update connection state based on errors
  useEffect(() => {
    if (error) {
      console.error("Solver agent error:", error);
      setErrorMessage(error.message || "An error occurred with the Solver agent");
      setConnectionState('error');
    }
  }, [error]);

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

            {messages.length > 1 && (
              <div className="border rounded-md p-3 mb-4 bg-muted/50">
                <h4 className="font-medium mb-2">Latest Update:</h4>
                <p className="text-sm">
                  {messages[messages.length - 1].content.substring(0, 150)}
                  {messages[messages.length - 1].content.length > 150 ? '...' : ''}
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
          <Button
            onClick={connectToSolver}
            disabled={isStartingSolver || !githubToken ? true : undefined}
          >
            <PlugZap className="h-4 w-4 mr-2" />
            Connect to Solver
          </Button>
        )}

        {(connectionState === 'connected' || connectionState === 'error') && (
          <Button
            variant="outline"
            onClick={disconnectFromSolver}
            disabled={false ? true : undefined}
          >
            <Power className="h-4 w-4 mr-2" />
            Disconnect
          </Button>
        )}
      </CardFooter>
    </Card>
  );
}
