#!/bin/bash
# Test script for Stripe LLM API format validation
# Usage: ./test-llm-api.sh

set -e

# Check for STRIPE_SECRET_KEY
if [ -z "$STRIPE_SECRET_KEY" ]; then
    echo "Error: STRIPE_SECRET_KEY environment variable not set"
    echo "Usage: STRIPE_SECRET_KEY=sk_test_xxx ./test-llm-api.sh"
    exit 1
fi

# Check for STRIPE_CUSTOMER_ID (optional, will use test value)
CUSTOMER_ID="${STRIPE_CUSTOMER_ID:-cus_test}"

echo "Testing Stripe LLM API..."
echo "Customer ID: $CUSTOMER_ID"
echo ""

# Test 1: Basic chat (no tools)
echo "=== Test 1: Basic chat (no tools) ==="
curl -s -X POST https://llm.stripe.com/chat/completions \
  -H "Authorization: Bearer $STRIPE_SECRET_KEY" \
  -H "Content-Type: application/json" \
  -H "X-Stripe-Customer-ID: $CUSTOMER_ID" \
  -d '{
    "model": "anthropic/claude-sonnet-4.5",
    "messages": [{"role": "user", "content": "Say hello in 5 words or less"}],
    "max_tokens": 50
  }' | jq .

echo ""

# Test 2: Chat with tools (OpenAI format)
echo "=== Test 2: Chat with tools (OpenAI format) ==="
curl -s -X POST https://llm.stripe.com/chat/completions \
  -H "Authorization: Bearer $STRIPE_SECRET_KEY" \
  -H "Content-Type: application/json" \
  -H "X-Stripe-Customer-ID: $CUSTOMER_ID" \
  -d '{
    "model": "anthropic/claude-sonnet-4.5",
    "messages": [{"role": "user", "content": "What files are in the src folder?"}],
    "max_tokens": 200,
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
  }' | jq .

echo ""

# Test 3: Tool result format
echo "=== Test 3: Tool call + result flow ==="
curl -s -X POST https://llm.stripe.com/chat/completions \
  -H "Authorization: Bearer $STRIPE_SECRET_KEY" \
  -H "Content-Type: application/json" \
  -H "X-Stripe-Customer-ID: $CUSTOMER_ID" \
  -d '{
    "model": "anthropic/claude-sonnet-4.5",
    "messages": [
      {"role": "user", "content": "List the src folder"},
      {
        "role": "assistant",
        "content": null,
        "tool_calls": [{
          "id": "call_123",
          "type": "function",
          "function": {"name": "view_folder", "arguments": "{\"path\":\"src\"}"}
        }]
      },
      {
        "role": "tool",
        "tool_call_id": "call_123",
        "content": "Directory: src\nmain.rs\nlib.rs\nutils/"
      }
    ],
    "max_tokens": 200,
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
  }' | jq .

echo ""
echo "All tests completed!"
