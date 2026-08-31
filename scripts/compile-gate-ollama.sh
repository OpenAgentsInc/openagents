#!/usr/bin/env bash
# Compile gate: run the codegen prompt per model, extract the answer, feed it
# to rustc. Companion to the bench plan doc §3 step 4.
set -uo pipefail
OLLAMA=${OLLAMA:-http://127.0.0.1:11434}
WORK=$(mktemp -d)
KEEP=${KEEP_WORK:-0}
trap '[ "$KEEP" = 1 ] || rm -rf "$WORK"; [ "$KEEP" = 1 ] && echo "workdir kept: $WORK" >&2; true' EXIT

PROMPT='Write a Rust function: pub fn parse_kv(line: &str) -> Result<Vec<(String, String)>, String> that parses "key=value" pairs separated by ";" into a vector, trimming whitespace, returning Err on any entry without "=" (the error message should name the offending entry). Return ONLY the function, no prose, no tests, no example.'

for model in "$@"; do
  echo "--- $model" >&2
  out="$WORK/$(echo "$model" | tr ':/.' '___').rs"
  errdump="$WORK/last.err"
  OLLAMA_HOST="$OLLAMA" MODEL="$model" PROMPT="$PROMPT" OUTFILE="$out" python3 - <<'PYEOF'
import json, os, urllib.request
model, prompt, outfile = os.environ["MODEL"], os.environ["PROMPT"], os.environ["OUTFILE"]
body = json.dumps({"model": model, "prompt": prompt, "stream": False, "think": False,
                   "options": {"num_predict": 600, "temperature": 0}}).encode()
req = urllib.request.Request(os.environ["OLLAMA_HOST"] + "/api/generate",
                             data=body, headers={"Content-Type": "application/json"})
with urllib.request.urlopen(req, timeout=900) as r:
    text = json.load(r).get("response", "")
open(outfile, "w").write(text)
PYEOF
  # extract fenced block when present
  if grep -q '```' "$out"; then
    awk '/^```(rust)?$/{f=!f; next} f' "$out" > "$out.clean"
  else
    cp "$out" "$out.clean"
  fi
  # rustc needs a writable temp dir, so compile inside $WORK (TMPDIR=/dev
  # breaks it: "couldn't create a temp dir ... /dev/rmeta...").
  # --crate-name: rustc derives the crate name from the filename and rejects
  # the extra dot in `<name>.rs.clean`, which failed every model spuriously.
  rustc --edition 2021 --crate-type lib --crate-name gate --emit=metadata \
        --out-dir "$WORK" "$out.clean" 2>"$out.err"
  rc=$?
  if [ $rc -eq 0 ]; then
    verdict="COMPILES"
  elif [ -s "$out.clean" ]; then
    verdict="FAILS ($(grep -c '^error' "$out.err") errors)"
    echo "kept: $out.clean / $out.err" >&2
  else
    verdict="NO-CODE-EXTRACTED"
  fi
  printf "%-36s %s\n" "$model" "$verdict"
done
