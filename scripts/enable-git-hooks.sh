#!/usr/bin/env bash
# Enable the repo's local pre-push gate (check:deploy must pass before pushing to main;
# Tier 1 QA smoke + Tier 2 async GCE QA trigger run warning-only after it).
# Policy: NO GitHub Actions CI — this is how every push to main is gated.
# Run once per checkout (including agent checkouts that push to main):
git config core.hooksPath .githooks && echo "pre-push gate enabled (check:deploy blocking; QA smoke + async GCE trigger warning-only)"
