#!/bin/sh
# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
# Copy every file in src/ to dest/ as NAME.bak
mkdir -p dest
for f in $(ls src); do
  cp src/$f dest/$f.bak
done
