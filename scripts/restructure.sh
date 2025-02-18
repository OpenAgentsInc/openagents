#!/bin/bash
set -e

echo "Starting reorganization..."

# 1. Rename 'chat' to 'frontend'
if [ -d "chat" ]; then
  if [ -d "frontend" ]; then
    echo "Error: Both 'chat' and 'frontend' directories exist. Aborting."
    exit 1
  else
    echo "Renaming 'chat' to 'frontend'..."
    mv chat frontend
  fi
else
  echo "'chat' directory not found. Skipping renaming."
fi

# 2. Create 'backend' directory if it doesn't exist
if [ ! -d "backend" ]; then
  echo "Creating 'backend' directory..."
  mkdir backend
else
  echo "'backend' directory already exists."
fi

# 3. Move Rust project files into 'backend'
if [ -d "src" ]; then
  echo "Moving 'src' to 'backend/src'..."
  mv src backend/
else
  echo "'src' directory not found. Skipping."
fi

if [ -f "build.rs" ]; then
  echo "Moving 'build.rs' to 'backend/build.rs'..."
  mv build.rs backend/
else
  echo "'build.rs' not found. Skipping."
fi

if [ -f "Cargo.toml" ]; then
  echo "Moving 'Cargo.toml' to 'backend/Cargo.toml'..."
  mv Cargo.toml backend/
else
  echo "'Cargo.toml' not found. Skipping."
fi

if [ -d "templates" ]; then
  echo "Moving 'templates' to 'backend/templates'..."
  mv templates backend/
else
  echo "'templates' directory not found. Skipping."
fi

# 4. (Optional) Move Tailwind config to the frontend folder if it belongs there.
if [ -f "tailwind.config.cjs" ]; then
  echo "Moving 'tailwind.config.cjs' to 'frontend/tailwind.config.cjs'..."
  mv tailwind.config.cjs frontend/
else
  echo "'tailwind.config.cjs' not found. Skipping."
fi

echo "Reorganization complete!"
