#!/bin/bash

# Replace with your Anthropic API key
API_KEY="your_api_key_here"

# Test with string value for tool_choice
curl https://api.anthropic.com/v1/messages \
  -H "x-api-key: $API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-3-5-sonnet-latest",
    "max_tokens": 1024,
    "messages": [
      {
        "role": "user",
        "content": "What tool would you use to get information about a GitHub repository?"
      }
    ],
    "tools": [
      {
        "name": "get_repository_info",
        "description": "Get information about a GitHub repository",
        "input_schema": {
          "type": "object",
          "properties": {
            "owner": {
              "type": "string",
              "description": "The repository owner"
            },
            "repo": {
              "type": "string",
              "description": "The repository name"
            }
          },
          "required": ["owner", "repo"]
        }
      }
    ],
    "tool_choice": "auto"
  }'

# Test with object value for tool_choice
curl https://api.anthropic.com/v1/messages \
  -H "x-api-key: $API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-3-5-sonnet-latest",
    "max_tokens": 1024,
    "messages": [
      {
        "role": "user",
        "content": "What tool would you use to get information about a GitHub repository?"
      }
    ],
    "tools": [
      {
        "name": "get_repository_info",
        "description": "Get information about a GitHub repository",
        "input_schema": {
          "type": "object",
          "properties": {
            "owner": {
              "type": "string",
              "description": "The repository owner"
            },
            "repo": {
              "type": "string",
              "description": "The repository name"
            }
          },
          "required": ["owner", "repo"]
        }
      }
    ],
    "tool_choice": {"type": "auto"}
  }'