# Model Context Protocol (MCP) Architecture

## Introduction

The Model Context Protocol (MCP) is an open protocol that standardizes how applications provide context to Large Language Models (LLMs). MCP serves as a standardized interface between AI models and external data sources or tools, similar to how USB-C provides a standardized way to connect devices to various peripherals.

This document outlines our approach to implementing MCP in OpenAgents applications, specifically leveraging Cloudflare's tools and infrastructure.

## Core MCP Architecture

![MCP Architecture](https://cf-assets.www.cloudflare.com/zkvhlag99gkb/1EyiTXzB4FvBs2zEfzuNTp/5ce4b55457348e9ab83e6d9cf35d8c3c/image7.png)

### Key Components

1. **MCP Hosts**: AI assistants or applications that need to access external capabilities (like Claude, Cursor, or our OpenAgents applications)
2. **MCP Clients**: Protocol clients embedded within MCP hosts that connect to MCP servers and invoke tools
3. **MCP Servers**: Applications that expose tools, prompts, and resources that MCP clients can use

### Local vs. Remote MCP

MCP supports two primary modes of operation:

1. **Local MCP Connections**:
   - Communication over standard input/output (stdio)
   - Runs on the same machine as the client
   - No authentication required
   - Limited to desktop applications

2. **Remote MCP Connections**:
   - Communication over HTTP with Server-Sent Events (SSE)
   - Accessible over the internet
   - Requires authentication and authorization
   - Works with web and mobile applications
   - Enables broader user adoption

## Cloudflare MCP Implementation

Cloudflare provides a comprehensive stack for building and deploying remote MCP servers:

### 1. Authorization with workers-oauth-provider

The [workers-oauth-provider](https://github.com/cloudflare/workers-oauth-provider) library implements the OAuth 2.1 protocol for MCP servers, handling:

- Dynamic client registration
- Authorization server metadata
- Token issuance and validation

The OAuth flow for MCP follows this pattern:

![MCP OAuth Flow](https://cf-assets.www.cloudflare.com/zkvhlag99gkb/VTPBfZ4hRPdq2TWE5VOjS/00abc97e4beedf59a4101957612fd503/image5.png)

Authorization options include:
- Self-handled authorization and authentication
- Integration with third-party OAuth providers (GitHub, Google)
- Integration with your own OAuth provider

### 2. Transport with McpAgent

The [McpAgent](https://github.com/cloudflare/agents/blob/2f82f51784f4e27292249747b5fbeeef94305552/packages/agents/src/mcp.ts) class handles the remote transport layer:

- Uses Durable Objects to maintain persistent connections
- Supports Server-Sent Events (SSE) for real-time communication
- Will support future transport mechanisms like Streamable HTTP
- Handles serialization and message exchange automatically

A minimal MCP server implementation looks like:

```javascript
import { McpAgent } from "agents/mcp";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";

export class MyMCP extends McpAgent {
  server = new McpServer({
    name: "Demo",
    version: "1.0.0",
  });

  async init() {
    this.server.tool(
      "add",
      { a: z.number(), b: z.number() },
      async ({ a, b }) => ({
        content: [{ type: "text", text: String(a + b) }],
      })
    );
  }
}
```

### 3. Tools Implementation

MCP tools are functions that MCP servers provide and MCP clients can call. They are defined using:

- A name
- A schema (using Zod for validation)
- An implementation function

Tools can:
- Access external APIs
- Perform computations
- Return structured data
- Include images and other media

### 4. Stateful Servers with Durable Objects

MCP servers on Cloudflare can maintain state across sessions:

- Each client session is backed by a Durable Object
- State can be persisted in a SQL database
- Enables complex, stateful applications beyond simple API proxies
- Supports games, shopping carts, knowledge graphs, and more

Example of a stateful MCP server:

```javascript
import { McpAgent } from "agents/mcp";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";

type State = { counter: number }

export class MyMCP extends McpAgent<Env, State, {}> {
  server = new McpServer({
    name: "Demo",
    version: "1.0.0",
  });

  initialState: State = {
    counter: 1,
  }

  async init() {
    this.server.resource(`counter`, `mcp://resource/counter`, (uri) => {
      return {
        contents: [{ uri: uri.href, text: String(this.state.counter) }],
      }
    })

    this.server.tool('add', 'Add to the counter', { a: z.number() }, async ({ a }) => {
      this.setState({ ...this.state, counter: this.state.counter + a })
      return {
        content: [{ type: 'text', text: String(`Added ${a}, total is now ${this.state.counter}`) }],
      }
    })
  }
}
```

## Integration with OpenAgents

### Implementation Strategy

For OpenAgents, we will implement MCP in the following ways:

1. **MCP Client Integration**:
   - Integrate MCP client capabilities into our web, mobile, and desktop applications
   - Support both local and remote MCP connections
   - Implement proper authentication flows

2. **MCP Server Development**:
   - Build MCP servers for our core agent capabilities
   - Deploy as Cloudflare Workers with Durable Objects
   - Implement OAuth for secure access

3. **Cross-Platform Compatibility**:
   - Use `mcp-remote` adapter for clients that only support local connections
   - Ensure consistent experience across platforms

### Architecture Diagram

```
┌──────────────────────────────────────┐      ┌────────────────────────────────┐
│         OpenAgents Clients           │      │      OpenAgents MCP Servers    │
│                                      │      │                                │
│  ┌─────────┐  ┌─────────┐  ┌──────┐  │      │  ┌─────────┐  ┌─────────────┐  │
│  │  Web    │  │ Desktop │  │Mobile│  │      │  │ Coding  │  │ Knowledge   │  │
│  │  App    │  │  App    │  │ App  │  │      │  │ Server  │  │   Server    │  │
│  └────┬────┘  └────┬────┘  └──┬───┘  │      │  └────┬────┘  └──────┬──────┘  │
│       │            │          │      │      │       │              │         │
│  ┌────┴────────────┴──────────┴───┐  │      │  ┌────┴──────────────┴───────┐ │
│  │        MCP Client Library      │  │      │  │    McpAgent + OAuth       │ │
│  └───────────────────┬────────────┘  │      │  └───────────────┬───────────┘ │
└──────────────────────┬───────────────┘      └──────────────────┬─────────────┘
                       │                                         │
                       │         HTTP + OAuth + SSE              │
                       └─────────────────────────────────────────┘
```

## Benefits for OpenAgents

Implementing MCP provides several advantages:

1. **Interoperability**: Connect with the growing ecosystem of MCP-compatible tools and services
2. **Standardization**: Use a well-defined protocol for agent-tool interactions
3. **Flexibility**: Switch between different LLM providers while maintaining the same tools
4. **Security**: Leverage OAuth for secure access to user resources
5. **Scalability**: Build on Cloudflare's global infrastructure
6. **Stateful Agents**: Create agents that maintain context across interactions

## Next Steps

1. Set up the Cloudflare development environment
2. Implement basic MCP servers for core functionality
3. Integrate MCP client capabilities into our applications
4. Develop authentication flows for secure access
5. Test with various LLM providers

## References

- [Model Context Protocol Introduction](https://modelcontextprotocol.io/introduction)
- [Cloudflare Remote MCP Servers Blog Post](https://blog.cloudflare.com/remote-model-context-protocol-servers-mcp/)
- [Cloudflare MCP Documentation](https://developers.cloudflare.com/agents/model-context-protocol/)
- [MCP Authorization Documentation](https://developers.cloudflare.com/agents/model-context-protocol/authorization/)
- [MCP Tools Documentation](https://developers.cloudflare.com/agents/model-context-protocol/tools/)
- [MCP Transport Documentation](https://developers.cloudflare.com/agents/model-context-protocol/transport/)
