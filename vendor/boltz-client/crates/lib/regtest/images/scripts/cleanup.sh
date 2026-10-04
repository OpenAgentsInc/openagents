#!/usr/bin/env bash
set -xe

echo "Running cleanup"

TEMP_DIR="/root/tmp"
mkdir -p "$TEMP_DIR"

# Persist
FILES=(
    ".elements/elements.conf"
    ".elements/elements.cookie"
    ".boltz-backend/boltz.conf"
    ".boltz-backend/seed.dat"
    ".boltz-client/boltz.toml"
)

for file in "${FILES[@]}"; do
    cp "/root/$file" "$TEMP_DIR"
done

# Wipe data dirs
WIPE_DIRS=(
    ".bitcoin"
    ".elements"
    ".lightning-1"
    ".lightning-2"
    ".lnd-1"
    ".lnd-2"
    ".lnd-3"
    ".boltz-backend"
    ".boltz-client"
    ".arkd"
    ".fulmine"
)

for dir in "${WIPE_DIRS[@]}"; do
    rm -rf "/root/$dir"/*
done

# Restore persisted files
for file in "${FILES[@]}"; do
    cp "$TEMP_DIR/$(basename "$file")" "/root/$file"
done

echo "Cleanup complete"
