#!/bin/bash

# List your Claude sessions
echo "📋 Your Claude sessions..."
echo ""

cd "$(dirname "$0")/.."
node scripts/migrate-user-data.js sessions