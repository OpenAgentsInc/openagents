# Local MCP Client in Coder App - Usage Guide

## Overview

The Coder app now includes a local server implementation that processes chat requests directly within the Electron application, reducing reliance on external services and providing better privacy and offline capabilities.

## API Key Setup

To use the AI chat functionality, you'll need an OpenRouter API key:

1. Sign up for an account at [OpenRouter](https://openrouter.ai/)
2. Generate an API key in your OpenRouter dashboard
3. Add the API key to the Coder app by editing the `server.ts` file or through environment variables

## Setting Your API Key

### Option 1: Edit the server.ts file

Open `/apps/coder/src/server/server.ts` and replace this line:

```typescript
const OPENROUTER_API_KEY = process.env.OPENROUTER_API_KEY || "sk-or-your-key-here";
```

with your actual API key:

```typescript
const OPENROUTER_API_KEY = "sk-or-your-actual-key";
```

### Option 2: Using Environment Variables

Set the `OPENROUTER_API_KEY` environment variable before starting the app:

```bash
# On macOS/Linux
export OPENROUTER_API_KEY=sk-or-your-actual-key
yarn start

# On Windows
set OPENROUTER_API_KEY=sk-or-your-actual-key
yarn start
```

## Using the Chat Interface

Once set up, the chat interface works just like before, but all requests are processed locally through your Electron app rather than going to the remote service at `api.openagents.com`.

## Troubleshooting

If you see an error message like "OpenRouter API Key not configured", it means your API key hasn't been set properly. Follow the steps above to set your API key.

## Technical Details

- The local server runs in the Electron main process using Hono
- Requests are made through a custom IPC communication channel
- API calls to OpenRouter happen directly from the main process
- Streaming responses are properly handled and passed back to the UI

## Future Enhancements

- Add a UI for entering and saving your API key
- Support for switching between different AI models
- Local caching of common responses
- Offline fallback modes