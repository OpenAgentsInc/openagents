#!/bin/bash
# Test our deployed worker streaming endpoint directly

echo "=== Testing deployed worker /api/ai/chat/stream ==="
echo ""

# First need to get a session cookie by logging in, or we can test unauthenticated behavior
curl -v -N -X POST "https://openagents-web.openagents.workers.dev/api/ai/chat/stream" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "anthropic/claude-sonnet-4.5",
    "messages": [{"role": "user", "content": "List files in the src folder"}],
    "max_tokens": 500,
    "tool_choice": "required",
    "system": "You are an agent. Use tools.",
    "tools": [
      {
        "type": "function",
        "function": {
          "name": "view_folder",
          "description": "View contents of a folder",
          "parameters": {
            "type": "object",
            "properties": {
              "path": {"type": "string", "description": "Path to folder"}
            },
            "required": ["path"]
          }
        }
      }
    ]
  }' 2>&1 | head -50

echo ""
