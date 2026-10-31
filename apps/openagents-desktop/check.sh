#!/usr/bin/env bash
# The desktop app's headless checks: build it, draw each case offscreen and
# compare it with its golden in goldens/ (OpenAgents --compare: at most 0.1% of
# pixels off by more than 48 of 255). Nothing reaches Claude Code, Codex or
# Psionic: a case is the demo engine, or the app in an empty home with a PATH
# that has no claude, or only the signed-out stand-in in zeron/fixtures; a
# shot has its own pasteboard.
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

home="$out/home"; rm -rf "$home"; mkdir -p "$home/work/openagents" "$home/work/zeron"
so="$here/zeron/fixtures/signed-out:/usr/bin:/bin"
three="wait 1 ; send Fix the sidebar width ; folder $home/work/openagents ; wait 1 ; send Review the tab bar ; folder $home/work/zeron ; wait 2"
# name | size | steps | a line the run must print (or empty) | engine: demo, or the PATH the app starts with in an empty home
cases=(
  "chat-1320x880|2640,1760|fixture open||demo"
  "chat-900x600|1800,1200|fixture open||demo"
  "type-and-paste|2640,1760|newchat ; type Hello from Coax ; key cmd+a ; menu Copy ; key end ; type , pasted:  ; menu Paste ; pasteboard|pasteboard: Hello from Coax|demo"
  "claude-missing|2640,1760|wait 2||/usr/bin:/bin"
  "claude-signed-out|2640,1760|wait 2 ; send hello ; wait 2||$so"
  "sidebar-open|2640,1760|$three||$so"
  "sidebar-900x600|1800,1200|$three||$so"
  "sidebar-shortcuts|2640,1760|$three ; keyevent cmd+1 ; keyevent shift+down ; keyevent shift+up ; menu Close Tab ; menu Toggle Sidebar ; menu New Session ; wait 1 ; desk|desk: active=3 tabs=0,2,3 sessions=4 collapsed=1|$so"
)

fail=0
for c in "${cases[@]}"; do
  IFS='|' read -r name size steps expect engine <<<"$c"
  if [[ "$engine" == demo ]]; then log="$("$app" --shot "$out/$name.png" --size "$size" --demo --cwd "$home/work/openagents" --do "$steps" 2>&1 || true)"
  else log="$(env -i HOME="$home" USER="$USER" PATH="$engine" BROWSER=/usr/bin/true PSIONIC_HOST=none "$app" --shot "$out/$name.png" --size "$size" --cwd "$home/work/openagents" --do "$steps" 2>&1 || true)"; fi
  if [[ -n "$expect" ]] && ! grep -qF -- "$expect" <<<"$log"; then echo "FAIL $name: no \"$expect\" in: $log"; fail=1; continue; fi
  if ((update)); then cp "$out/$name.png" "$here/goldens/$name.png"; echo "updated goldens/$name.png"; continue; fi
  if result="$("$app" --compare "$out/$name.png" "$here/goldens/$name.png")"; then echo "ok   $name: $result"
  else echo "FAIL $name: $result (shot: $out/$name.png)"; fail=1; fi
done
exit $fail
