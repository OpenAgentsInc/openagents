#!/usr/bin/env bash
# Enable the repo's local pre-push guard.
# Policy: NO GitHub Actions CI. Main pushes run cargo fmt --all -- --check
# and cargo test --workspace.
# Run once per checkout (including agent checkouts that push to main):
git config core.hooksPath .githooks && echo "pre-push guard enabled (cargo fmt + cargo test --workspace)"
