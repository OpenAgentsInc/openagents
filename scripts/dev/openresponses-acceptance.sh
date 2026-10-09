#!/bin/sh
# The Open Responses acceptance suite (https://www.openresponses.org/compliance)
# against a gateway (#11068). The suite is the upstream repository's own
# runner (Apache-2.0), cloned at a pinned commit into a scratch directory
# and run with bun; nothing of it is vendored here.
#
#   OPENAGENTS_API_KEY=oak_... scripts/dev/openresponses-acceptance.sh \
#       http://127.0.0.1:8790/v1 google/gemini-3.8-flash [--json results.json]
#
# The key is read from the environment and never printed. Other arguments
# after the model pass through to the runner (--filter, --verbose, --json).
set -eu

base=${1:?usage: openresponses-acceptance.sh BASE_URL MODEL [runner options]}
model=${2:?usage: openresponses-acceptance.sh BASE_URL MODEL [runner options]}
shift 2
: "${OPENAGENTS_API_KEY:?set OPENAGENTS_API_KEY}"
commit=${OPENRESPONSES_COMMIT:-1e33c10fb3c19480751658eae966dad8e9c91514}
work=${OPENRESPONSES_DIR:-${TMPDIR:-/tmp}/openresponses-$commit}

if [ ! -d "$work/.git" ]; then
    git clone -q https://github.com/openresponses/openresponses "$work"
fi
git -C "$work" fetch -q origin "$commit" 2>/dev/null || true
git -C "$work" checkout -q "$commit"
(cd "$work" && bun install --silent > /dev/null && bun run --silent generate:zod > /dev/null)

cd "$work"
OPENRESPONSES_API_KEY=$OPENAGENTS_API_KEY exec bun run bin/compliance-test.ts \
    --base-url "$base" --model "$model" "$@"
