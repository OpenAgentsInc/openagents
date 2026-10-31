#!/usr/bin/env bash
# The desktop app's headless checks: build it, draw each case offscreen and
# compare it with its golden in goldens/ (OpenAgents --compare: at most 0.1% of
# pixels off by more than 48 of 255). Nothing runs Claude Code, Codex or
# Psionic: every case is the demo engine, and a shot has its own pasteboard.
#   apps/openagents-desktop/check.sh            build, then check
#   apps/openagents-desktop/check.sh --update   build, then rewrite the goldens (look at them before committing)
#   NO_BUILD=1 apps/openagents-desktop/check.sh check the app already built
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
app="$here/build/OpenAgents.app/Contents/MacOS/OpenAgents"
out="$here/build/check"
update=0; [[ "${1:-}" == --update ]] && update=1
[[ -n "${NO_BUILD:-}" ]] || "$here/build.sh" >/dev/null
mkdir -p "$out" "$here/goldens"
export BROWSER=/usr/bin/true PSIONIC_HOST=none

# name | size | steps | a line the run must print (or empty)
cases=(
  "chat-1320x880|2640,1760|fixture open|"
  "chat-900x600|1800,1200|fixture open|"
  "type-and-paste|2640,1760|newchat ; type Hello from Coax ; key cmd+a ; menu Copy ; key end ; type , pasted:  ; menu Paste ; pasteboard|pasteboard: Hello from Coax"
)

fail=0
for c in "${cases[@]}"; do
  IFS='|' read -r name size steps expect <<<"$c"
  log="$("$app" --shot "$out/$name.png" --size "$size" --demo --do "$steps" 2>&1 || true)"
  if [[ -n "$expect" ]] && ! grep -qF -- "$expect" <<<"$log"; then echo "FAIL $name: no \"$expect\" in: $log"; fail=1; continue; fi
  if ((update)); then cp "$out/$name.png" "$here/goldens/$name.png"; echo "updated goldens/$name.png"; continue; fi
  if result="$("$app" --compare "$out/$name.png" "$here/goldens/$name.png")"; then echo "ok   $name: $result"
  else echo "FAIL $name: $result (shot: $out/$name.png)"; fail=1; fi
done
exit $fail
