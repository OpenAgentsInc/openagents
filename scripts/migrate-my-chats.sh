#!/bin/bash

# Migrate Claude sessions to your user account
echo "🚀 Migrating your Claude sessions..."
echo ""

cd "$(dirname "$0")/.."

echo "Step 1: Checking current status..."
node scripts/migrate-user-data.js status
echo ""

echo "Step 2: Previewing migration..."
node scripts/migrate-user-data.js preview
echo ""

read -p "Do you want to proceed with the migration? (y/N): " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo ""
    echo "Step 3: Performing migration..."
    node scripts/migrate-user-data.js migrate
    echo ""
    echo "Step 4: Verifying results..."
    node scripts/migrate-user-data.js sessions
else
    echo ""
    echo "Migration cancelled. Run this script again when you're ready."
fi