#!/bin/bash

echo "🗑️  Manual Deletion Instructions:"
echo ""
echo "Since Convex has read limits, please manually delete all data:"
echo ""
echo "1. Go to: https://dashboard.convex.dev"
echo "2. Select your project (proficient-panther-764)"
echo "3. Go to the 'Data' tab"
echo "4. Click on 'messages' table"
echo "5. Select all messages and delete them"
echo "6. Click on 'sessions' table"
echo "7. Select all sessions and delete them"
echo ""
echo "Press Enter when you've completed the deletion..."
read

echo ""
echo "📥 Now importing ONE session..."
cd ../overlord && bun run src/index.ts import --user-id="claude-code-user" --api-key="test" --limit=1

echo ""
echo "✅ Done! Check your imported session at http://localhost:3003"