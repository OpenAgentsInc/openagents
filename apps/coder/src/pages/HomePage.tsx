import { Button } from "@openagents/ui";
import React, { useEffect, useState } from "react";
import { Ionicons } from "@expo/vector-icons";
import loadIconFonts from "../shims/load-icon-fonts";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { createGithubMcpClient, listRepositoryIssues, listPullRequests, viewFileContents } from "../utils/mcp/github";

export default function HomePage() {
  const [mcpClient, setMcpClient] = useState<Client | null>(null);
  const [connectionStatus, setConnectionStatus] = useState<"disconnected" | "connecting" | "connected" | "error">("disconnected");
  const [result, setResult] = useState<string>("");

  // Load icon fonts on component mount
  useEffect(() => {
    loadIconFonts();
    connectToMcp();
  }, []);

  const connectToMcp = async () => {
    setConnectionStatus("connecting");
    try {
      const client = await createGithubMcpClient();
      setMcpClient(client);
      setConnectionStatus("connected");
    } catch (error) {
      console.error("Failed to connect to MCP server:", error);
      setConnectionStatus("error");
    }
  };

  // Function to render Ionicons
  const renderIcon = (iconName: string) => {
    return <Ionicons name={iconName as any} size={20} color="#ffffff" />;
  };

  const handleListIssues = async () => {
    if (!mcpClient) return;
    try {
      const issues = await listRepositoryIssues(mcpClient, "OpenAgentsInc", "openagents");
      setResult(JSON.stringify(issues, null, 2));
    } catch (error) {
      setResult(`Error listing issues: ${error}`);
    }
  };

  const handleListPRs = async () => {
    if (!mcpClient) return;
    try {
      const prs = await listPullRequests(mcpClient, "OpenAgentsInc", "openagents");
      setResult(JSON.stringify(prs, null, 2));
    } catch (error) {
      setResult(`Error listing PRs: ${error}`);
    }
  };

  const handleViewFile = async () => {
    if (!mcpClient) return;
    try {
      const content = await viewFileContents(mcpClient, "OpenAgentsInc", "openagents", "README.md");
      setResult(JSON.stringify(content, null, 2));
    } catch (error) {
      setResult(`Error viewing file: ${error}`);
    }
  };

  return (
    <div className="flex h-full flex-col items-center justify-start gap-4 p-4">
      <div className="flex flex-col items-center gap-2 mb-4">
        <h2 className="text-xl font-bold">GitHub MCP Demo</h2>
        <div className={`text-sm ${
          connectionStatus === "connected" ? "text-green-500" : 
          connectionStatus === "connecting" ? "text-yellow-500" :
          connectionStatus === "error" ? "text-red-500" :
          "text-gray-500"
        }`}>
          Status: {connectionStatus}
        </div>
      </div>

      <div className="flex flex-row gap-4">
        <Button
          label="List Issues"
          variant="primary"
          leftIcon="list-outline"
          renderIcon={renderIcon}
          onClick={handleListIssues}
          disabled={connectionStatus !== "connected"}
        />

        <Button
          label="List PRs"
          variant="secondary"
          leftIcon="git-pull-request-outline"
          renderIcon={renderIcon}
          onClick={handleListPRs}
          disabled={connectionStatus !== "connected"}
        />

        <Button
          label="View README"
          variant="primary"
          leftIcon="document-text-outline"
          renderIcon={renderIcon}
          onClick={handleViewFile}
          disabled={connectionStatus !== "connected"}
        />
      </div>

      {result && (
        <div className="mt-4 w-full max-w-3xl">
          <pre className="bg-gray-800 p-4 rounded-lg overflow-auto max-h-96">
            {result}
          </pre>
        </div>
      )}
    </div>
  );
}