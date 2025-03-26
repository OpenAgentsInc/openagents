import { Button } from "@openagents/ui";
import React, { useEffect, useState } from "react";
import { Ionicons } from "@expo/vector-icons";
import loadIconFonts from "../shims/load-icon-fonts";

declare global {
  interface Window {
    api: {
      listIssues: (owner: string, repo: string) => Promise<any>;
      listPullRequests: (owner: string, repo: string) => Promise<any>;
      viewFileContents: (owner: string, repo: string, path: string) => Promise<any>;
    };
  }
}

export default function HomePage() {
  const [result, setResult] = useState<string>("");

  // Load icon fonts on component mount
  useEffect(() => {
    loadIconFonts();
  }, []);

  // Function to render Ionicons
  const renderIcon = (iconName: string) => {
    return <Ionicons name={iconName as any} size={20} color="#ffffff" />;
  };

  const handleListIssues = async () => {
    try {
      const issues = await window.api.listIssues("OpenAgentsInc", "openagents");
      setResult(JSON.stringify(issues, null, 2));
    } catch (error) {
      setResult(`Error listing issues: ${error}`);
    }
  };

  const handleListPRs = async () => {
    try {
      const prs = await window.api.listPullRequests("OpenAgentsInc", "openagents");
      setResult(JSON.stringify(prs, null, 2));
    } catch (error) {
      setResult(`Error listing PRs: ${error}`);
    }
  };

  const handleViewFile = async () => {
    try {
      const content = await window.api.viewFileContents("OpenAgentsInc", "openagents", "README.md");
      setResult(JSON.stringify(content, null, 2));
    } catch (error) {
      setResult(`Error viewing file: ${error}`);
    }
  };

  return (
    <div className="flex h-full flex-col items-center justify-start gap-4 p-4">
      <div className="flex flex-col items-center gap-2 mb-4">
        <h2 className="text-xl font-bold">GitHub MCP Demo</h2>
      </div>

      <div className="flex flex-row gap-4">
        <Button
          label="List Issues"
          variant="primary"
          leftIcon="list"
          renderIcon={renderIcon}
          onPress={handleListIssues}
        />

        <Button
          label="List PRs"
          variant="secondary"
          leftIcon="git-branch"
          renderIcon={renderIcon}
          onPress={handleListPRs}
        />

        <Button
          label="View README"
          variant="primary"
          leftIcon="document"
          renderIcon={renderIcon}
          onPress={handleViewFile}
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