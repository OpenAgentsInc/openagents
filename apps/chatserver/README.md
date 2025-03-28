# OpenAgents Chat Server

A Cloudflare Workers-based chat server for OpenAgents that integrates with LLMs and MCP tools.

## Features

- **LLM Integration**: Uses Cloudflare Workers AI with Llama 3.3
- **Tool Support**: Integrates with MCP servers for tool execution
- **GitHub Tools**: Built-in support for GitHub operations via MCP
- **Streaming Responses**: Real-time streaming of responses and tool results
- **Authentication Pass-through**: Secure token handling

## Getting Started

### Installation

```bash
npm install
```

### Development

```bash
npm run dev
```

### Deployment

```bash
npm run deploy
```

## API Usage

### Chat Endpoint

**POST /** - Main chat endpoint

Request:
```json
{
  "messages": [
    { "role": "user", "content": "Create a GitHub issue in OpenAgentsInc/openagents" }
  ]
}
```

Headers:
```
Authorization: Bearer github_pat_xxxxx
Content-Type: application/json
```

Response:
- Streaming response in Vercel AI SDK format
- Includes tool calls and results

## MCP Integration

This server integrates with Model Context Protocol (MCP) servers to provide tool execution capabilities:

- **GitHub MCP Server**: Provides GitHub API operations
- Additional MCP servers can be added by updating the configuration

For detailed documentation on the MCP integration, see [MCP Chat Integration](../docs/mcp-chat-integration.md).

## Architecture

The chat server follows a layered architecture:

1. **Chat API**: Handles client requests and streaming responses
2. **LLM Integration**: Connects to Cloudflare Workers AI
3. **MCP Client**: Routes tool calls to MCP servers
4. **Tool Processing**: Handles tool execution and result formatting

## Development

### Running Tests

```bash
npm test
```

### Adding New MCP Servers

To add a new MCP server:

1. Update the `initMcp` function in `src/index.ts`
2. Add the server URL and name
3. Update tool definitions in `src/mcp/tools.ts` if needed

## License

[MIT](../LICENSE)