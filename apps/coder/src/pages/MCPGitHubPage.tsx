import React, { useState } from "react";
import { useMCP } from "@openagents/core";
import { Button } from "@openagents/ui";

export default function MCPGitHubPage() {
  const { status, result, error, serverUrl, callTool } = useMCP();
  const [owner, setOwner] = useState<string>("OpenAgentsInc");
  const [repo, setRepo] = useState<string>("openagents");
  const [branch, setBranch] = useState<string>("main");
  const [path, setPath] = useState<string>("README.md");

  const handleGetContents = () => {
    callTool("get_file_contents", {
      owner,
      repo,
      path,
      branch,
    });
  };

  return (
    <div className="font-mono flex h-full">
      {/* Left Pane - Form */}
      <div className="w-1/2 p-8 flex flex-col items-center justify-center gap-4 text-white border-r border-white/20">
        <div className="mb-4 text-center">
          <p>MCP Status: {status}</p>
          <p className="text-sm text-gray-400 mt-1">{serverUrl}</p>
          {error && <p className="text-red-500 mt-2">Error: {error.message}</p>}
        </div>

        <div className="flex flex-col gap-4 items-center w-full max-w-md">
          <input
            type="text"
            value={owner}
            onChange={(e) => setOwner(e.target.value)}
            className="bg-black border border-white rounded px-3 py-2 w-full text-white"
            placeholder="Repository owner"
          />
          <input
            type="text"
            value={repo}
            onChange={(e) => setRepo(e.target.value)}
            className="bg-black border border-white rounded px-3 py-2 w-full text-white"
            placeholder="Repository name"
          />
          <input
            type="text"
            value={branch}
            onChange={(e) => setBranch(e.target.value)}
            className="bg-black border border-white rounded px-3 py-2 w-full text-white"
            placeholder="Branch name"
          />
          <input
            type="text"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            className="bg-black border border-white rounded px-3 py-2 w-full text-white"
            placeholder="File path"
          />
          <Button
            label="Get File Contents"
            variant="primary"
            onPress={handleGetContents}
          />
        </div>
      </div>

      {/* Right Pane - Result */}
      <div className="w-1/2 p-8 bg-black border-l border-white/20">
        <div className="h-full flex flex-col">
          <h2 className="text-white mb-4 text-xl">Result</h2>
          {result && (
            <pre className="flex-1 text-white bg-black border border-white rounded p-4 overflow-y-auto font-mono text-sm whitespace-pre-wrap break-all">
              {typeof result === 'string'
                ? JSON.stringify(JSON.parse(result), null, 2)
                : JSON.stringify(result, null, 2)
              }
            </pre>
          )}
        </div>
      </div>
    </div>
  );
}
