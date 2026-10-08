#!/bin/sh
set -eu
export CARGO_TARGET_DIR=/Users/christopherdavid/work/openagents-target-agent2
cd /Users/christopherdavid/.codex/worktrees/10907-budgeted-relight/openagents
repair_scratch=/Users/christopherdavid/.openagents/scratch/codex-01a119ae-8a0b-7850-9b7f-ca9fe5a3203b
openagents lease build --keep-target-dir --receipt "$repair_scratch/10907-continuation-gles-build-lease.json" -- cargo test -p verse --lib gles_tests:: > "$repair_scratch/10907-continuation-gles-build.log" 2>&1
openagents lease build --keep-target-dir --receipt "$repair_scratch/10907-continuation-capture-tests-build-lease.json" -- cargo test -p verse --example baked_light_capture --features capture > "$repair_scratch/10907-continuation-capture-tests-build.log" 2>&1
openagents lease build --keep-target-dir --receipt "$repair_scratch/10907-continuation-capture-build-lease.json" -- cargo build -p verse --example baked_light_capture --features capture > "$repair_scratch/10907-continuation-capture-build.log" 2>&1
