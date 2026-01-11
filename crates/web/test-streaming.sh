#!/bin/bash
# Test streaming endpoint with tools

# Get Stripe key from wrangler secrets or env
if [ -z "$STRIPE_SECRET_KEY" ]; then
    echo "Need STRIPE_SECRET_KEY"
    exit 1
fi

CUSTOMER_ID="${STRIPE_CUSTOMER_ID:-cus_test}"

echo "=== Testing STREAMING with tool_choice: required ==="
echo ""

curl -N -X POST https://llm.stripe.com/chat/completions \
  -H "Authorization: Bearer $STRIPE_SECRET_KEY" \
  -H "Content-Type: application/json" \
  -H "X-Stripe-Customer-ID: $CUSTOMER_ID" \
  -d '{
    "model": "openai/codex-sonnet-4.5",
    "messages": [{"role": "user", "content": "List files in the src folder"}],
    "max_tokens": 500,
    "stream": true,
    "tool_choice": "required",
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
  }' 2>/dev/null

echo ""
echo ""
echo "=== Done ==="
