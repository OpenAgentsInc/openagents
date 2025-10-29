#!/usr/bin/env bash
set -euo pipefail

# claude-capture.sh
# Wrapper to run the local `claude` CLI with a prompt, force JSON output,
# and persist the full JSON response under docs/logs/claude/ with a
# timestamped filename. Also prints a short summary to stdout.
#
# Usage examples:
#   scripts/claude-capture.sh -p "Search for doc about opencode and summarize in 3 sentences."
#   scripts/claude-capture.sh -p "Generate ACP types" -- --model claude-3.7-sonnet
#   scripts/claude-capture.sh -f prompt.txt
#
# Environment:
#   CLAUDE_BIN  Path to the `claude` binary (default: "claude")

CLAUDE_BIN=${CLAUDE_BIN:-claude}

if ! command -v "$CLAUDE_BIN" >/dev/null 2>&1; then
  echo "error: claude CLI not found. Set CLAUDE_BIN or install the CLI." >&2
  exit 1
fi

PROMPT=""
PROMPT_FILE=""
PASSTHRU=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    -p|--prompt)
      PROMPT=${2:-}
      shift 2
      ;;
    -f|--file)
      PROMPT_FILE=${2:-}
      shift 2
      ;;
    --)
      shift
      PASSTHRU=("${@}")
      break
      ;;
    *)
      PASSTHRU+=("$1")
      shift
      ;;
  esac
done

if [[ -z "${PROMPT}" && -n "${PROMPT_FILE}" ]]; then
  if [[ ! -f "${PROMPT_FILE}" ]]; then
    echo "error: prompt file not found: ${PROMPT_FILE}" >&2
    exit 2
  fi
  PROMPT=$(cat "${PROMPT_FILE}")
fi

if [[ -z "${PROMPT}" ]]; then
  echo "usage: $0 -p \"<prompt>\" [-- <extra claude args>]" >&2
  echo "       $0 -f prompt.txt [-- <extra claude args>]" >&2
  exit 2
fi

# Ensure JSON output unless caller explicitly set one
HAS_FMT=0
for a in "${PASSTHRU[@]:-}"; do
  if [[ "$a" == "--output-format" ]]; then HAS_FMT=1; break; fi
done
if [[ $HAS_FMT -eq 0 ]]; then
  PASSTHRU+=("--output-format" "json")
fi

TS=$(date +%Y%m%dT%H%M%S)
OUTDIR="docs/logs/claude"
mkdir -p "$OUTDIR"

# Create a safe slug from the prompt (first 6 words)
SLUG=$(echo "$PROMPT" | tr -s ' ' | cut -c1-60 | tr ' /:\\' '____' | tr -cd '[:alnum:]_\-')
if [[ -z "$SLUG" ]]; then SLUG="prompt"; fi
OUTFILE_JSON="$OUTDIR/${TS}-${SLUG}.json"
OUTFILE_META="$OUTDIR/${TS}-${SLUG}.meta.json"
OUTFILE_TXT="$OUTDIR/${TS}-${SLUG}.txt"

TMP=$(mktemp)
set +e
"$CLAUDE_BIN" -p "$PROMPT" "${PASSTHRU[@]}" >"$TMP" 2>&1
CODE=$?
set -e

mv "$TMP" "$OUTFILE_JSON"

# Try to extract a short textual result and write a sibling .txt for quick reading
if command -v jq >/dev/null 2>&1; then
  jq -r 'try .result // empty' "$OUTFILE_JSON" > "$OUTFILE_TXT" || true
  # Write a small metadata file capturing prompt and command used
  jq -n --arg prompt "$PROMPT" \
        --arg cmd "$CLAUDE_BIN -p <prompt> ${PASSTHRU[*]}" \
        --arg ts "$TS" \
        --arg file "$OUTFILE_JSON" \
        '{prompt:$prompt, command:$cmd, timestamp:$ts, output:$file}' > "$OUTFILE_META" || true
else
  # Fallback: just copy the raw JSON to .txt
  cp "$OUTFILE_JSON" "$OUTFILE_TXT" || true
  printf '{"prompt":%q,"timestamp":"%s","output":%q}\n' "$PROMPT" "$TS" "$OUTFILE_JSON" > "$OUTFILE_META" || true
fi

echo "Saved: $OUTFILE_JSON"
if [[ -s "$OUTFILE_TXT" ]]; then
  echo "Summary:"; head -n 12 "$OUTFILE_TXT" || true
fi

exit "$CODE"

