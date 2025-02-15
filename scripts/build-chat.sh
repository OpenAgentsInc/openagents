#!/bin/bash

# Exit on error
set -e

echo "Building chat app..."

# Navigate to chat directory
cd chat

# Install dependencies
npm install

# Build for web
npm run build:web:prod

echo "Chat app built successfully!"
