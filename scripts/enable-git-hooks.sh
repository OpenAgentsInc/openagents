#!/usr/bin/env bash
# Enable the repo's local pre-push gate (check:deploy must pass before pushing to main;
# Tier 1 QA smoke runs warning-only after it).
# Policy: NO GitHub Actions CI — this is how every push to main is gated.
# Run once per checkout (including agent checkouts that push to main):
git config core.hooksPath .githooks && echo "pre-push gate enabled (check:deploy blocking; QA smoke warning-only)"
