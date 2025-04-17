#!/bin/bash

# Script to clean node_modules and reinstall dependencies
# This helps resolve React version conflicts in the monorepo

echo "Cleaning node_modules directories..."
rm -rf node_modules
rm -rf packages/*/node_modules
rm -rf apps/*/node_modules

echo "Removing yarn.lock file..."
rm -f yarn.lock

echo "Reinstalling dependencies..."
yarn install

echo "Dependency cleanup completed. Try running the app now to see if the React hooks issue is fixed."