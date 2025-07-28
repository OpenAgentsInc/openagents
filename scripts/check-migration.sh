#!/bin/bash

# Quick migration status check
echo "🔍 Checking Claude sessions migration status..."
echo ""

cd "$(dirname "$0")/.."
node scripts/migrate-user-data.js status