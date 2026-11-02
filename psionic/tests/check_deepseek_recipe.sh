#!/usr/bin/env bash
# Chapter 101: Maya's recipe for V4.1 Flash as a plan. The recipe's checks,
# debug/release transcripts and optional mutation gate are owned by Coax.
# Firm reason: 5 (thin working-directory and exec wrapper for the Coax recipe check).
cd "$(dirname "$0")/../.." || exit
exec coax/web/tool.sh psionic/tests/check-deepseek-recipe.coax "$@"
