# Shell MCP Server Setup Instructions

## Prerequisites

1. Install the Shell MCP Server using pip:

```bash
pip install mcp-shell-server
```

## Configuration

When starting the server, provide the allowed commands through the ALLOW_COMMANDS environment variable:

```bash
ALLOW_COMMANDS="ls,cat,pwd,grep,wc,touch,find" npm run dev
```

You can customize the list of allowed commands based on your security requirements.

## Available Commands

After setup, the AI will have access to execute any of the allowed shell commands.
