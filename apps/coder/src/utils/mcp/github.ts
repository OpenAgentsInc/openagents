import { Client } from "@modelcontextprotocol/sdk/client/index.js";

export const createGithubMcpClient = async () => {
  // TODO: Replace with proper WebSocket/HTTP transport
  // For now, just create the client without connecting
  const client = new Client(
    {
      name: "coder-github",
      version: "1.0.0"
    },
    {
      capabilities: {
        resources: {},
        tools: {}
      }
    }
  );

  // Mock successful connection for UI demo
  return client;
};

export const listRepositoryIssues = async (client: Client, owner: string, repo: string) => {
  // Mock response for demo
  return [
    {
      number: 1,
      title: "Demo Issue 1",
      state: "open",
      created_at: new Date().toISOString()
    },
    {
      number: 2,
      title: "Demo Issue 2",
      state: "closed",
      created_at: new Date().toISOString()
    }
  ];
};

export const listPullRequests = async (client: Client, owner: string, repo: string) => {
  // Mock response for demo
  return [
    {
      number: 100,
      title: "Demo PR 1",
      state: "open",
      created_at: new Date().toISOString()
    },
    {
      number: 101,
      title: "Demo PR 2",
      state: "merged",
      created_at: new Date().toISOString()
    }
  ];
};

export const viewFileContents = async (client: Client, owner: string, repo: string, path: string) => {
  // Mock response for demo
  return {
    content: "# Demo File\nThis is a mock file content for demonstration.",
    sha: "abc123",
    size: 123
  };
};