#!/usr/bin/env bash
# Enable the repo's local fast pre-push guard.
# Policy: NO GitHub Actions CI. Main pushes block on cheap local policy checks by
# default; set OPENAGENTS_PRE_PUSH_FULL_GATE=1 when you intentionally want the
# hook to run the full check:deploy suite before pushing.
# Run once per checkout (including agent checkouts that push to main):
git config core.hooksPath .githooks && echo "pre-push guard enabled (fast policy checks; full gate opt-in with OPENAGENTS_PRE_PUSH_FULL_GATE=1)"
