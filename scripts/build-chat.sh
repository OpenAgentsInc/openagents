#!/bin/bash

# Exit on error
set -e

echo "Building chat app..."

# Navigate to chat directory
cd chat

# Install dependencies
yarn install --frozen-lockfile

# Build for production
yarn build

echo "Chat app built successfully!"
