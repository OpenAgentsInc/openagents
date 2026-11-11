#!/usr/bin/env bash
# The desktop app's headless checks: build it, run each case's coax drive
# operations offscreen on the agent clock (apps/shared/host/drive.c), compare
# its element table row by row with goldens/NAME.tree (ids, roles, labels and
# boxes: the text first), then its picture with goldens/NAME.png (OpenAgents
# --compare: at most 0.1% of pixels off by more than 48 of 255). Nothing reaches Claude Code, Codex or
# Psionic: a case is the demo engine, or the app in an empty home with a PATH
# that has no claude, or only the signed-out stand-in in zeron/fixtures; a
# shot has its own pasteboard.
#   apps/openagents-desktop/check.sh            build, then check
#   apps/openagents-desktop/check.sh --update   build, then rewrite the goldens, trees and pictures (look at them before committing)
#   NO_BUILD=1 apps/openagents-desktop/check.sh check the app already built
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
app="$here/build/OpenAgents.app/Contents/MacOS/OpenAgents"
out="$here/build/check"
update=0; [[ "${1:-}" == --update ]] && update=1
[[ -n "${NO_BUILD:-}" ]] || "$here/build.sh" >/dev/null
mkdir -p "$out" "$here/goldens"
export BROWSER=/usr/bin/true PSIONIC_HOST=none

home="$out/home"; rm -rf "$home" "$out/settings.json" "$out/sessions" "$out/store";
mkdir -p "$out/keys"; echo '{"shortcuts":{"toggle_sidebar":"cmd+b","new_session":"alt+cmd+t"}}' > "$out/keys/settings.json"
mkdir -p "$home/work/openagents" "$home/work/zeron"
so="$here/zeron/fixtures/signed-out:/usr/bin:/bin"
asks="$here/zeron/fixtures/asks:/usr/bin:/bin"
room="$here/zeron/fixtures/room:/usr/bin:/bin"
stream="$here/zeron/fixtures/stream:/usr/bin:/bin"
mkdir -p "$home/.codex"; echo '{}' > "$home/.codex/auth.json"   # Codex's login on disk, for the stand-in codex
three="clock settle ; send Fix the sidebar width ; clock settle ; folder $home/work/openagents ; clock settle ; send Review the tab bar ; clock settle ; folder $home/work/zeron ; clock settle"
# focus-button and focus-composer: the keyboard's focus ring (Tab, apps/shared/host/focus-ring.h, #13182);
# choose-folder: New session's folder chooser answered from the driver, no panel (choose PATH, then cancel)
# name | size | steps | a line the run must print (or empty) | engine: demo, or the PATH the app starts with in an empty home | more arguments
cases=(
  "chat-1320x880|2640,1760|fixture open||demo"
  "chat-900x600|1800,1200|fixture open||demo"
  "type-and-paste|2640,1760|newchat ; type - Hello from Coax ; key cmd+a ; menu Copy ; key end ; type - , pasted: ; menu Paste ; state pasteboard|pasteboard: Hello from Coax|demo"
  "chat-sonnet|2640,1760|clock settle ; model Sonnet ; send What is two plus two? ; clock +1200 ; state last ; clock settle within 10000 ; state last|last: Answered by claude-sonnet-5: two plus two is four, and the sum stays four however you group it.|$stream"
  "claude-missing|2640,1760|clock settle||/usr/bin:/bin"
  "claude-signed-out|2640,1760|clock settle ; send hello ; clock settle||$so"
  "approval-waiting|2640,1760|access supervised ; send list the files ; clock settle ; state status|status: Waiting for your approval|$asks|--still-clock"
  "approval-allow|2640,1760|access supervised ; send list the files ; clock settle ; answer allow ; clock settle within 10000 ; state status ; state last|last: Allowed: ls found a.txt and b.txt.|$asks|--still-clock"
  "approval-deny|2640,1760|access supervised ; send list the files ; clock settle ; answer deny ; clock settle within 10000 ; state last|last: Refused: The user denied this.|$asks|--still-clock"
  "room-ask|2640,1760|clock settle ; folder $home/work/zeron ; mode codex ; clock settle ; key cmd+1 ; send Ask Codex to review the diff ; clock settle within 15000 ; clock settle ; state last|last: Codex reviewed it: Review: the diff renames a to b; it reads well. (80 chars read)|$room"
  "room-tab|2640,1760|clock settle ; folder $home/work/zeron ; mode codex ; clock settle ; key cmd+1 ; send @Codex 1 the tests pass on main ; clock +50 ; send Ask Codex to review the diff ; clock settle within 15000 ; clock settle ; room ; clock settle||$room"
  "room-target|2640,1760|clock settle ; folder $home/work/zeron ; mode codex ; clock settle ; key cmd+1 ; send @Codex 1 the tests pass on main ; clock +50 ; send Ask Codex to review the diff ; clock settle within 15000 ; clock settle ; key cmd+2 ; clock settle||$room"
  "room-codex-asks|2640,1760|clock settle ; folder $home/work/zeron ; mode codex ; clock settle ; send Ask Claude whether the build is green ; clock settle within 20000 ; clock settle ; state last|last: Claude says: The build is green. (tools: agents_list, ask_agent, tell_agent, room_read)|$room"
  "settings-quit|2640,1760|$three ; menu Toggle Sidebar ; key cmd+1 ; menu Close Tab ; state desk|desk: active=1 tabs=1,2 sessions=3 collapsed=1|$so|--settings $out/settings.json"
  "settings-reopen|2640,1760|clock settle ; state desk ; state window|desk: active=1 tabs=1,2 sessions=3 collapsed=1|$so|--settings $out/settings.json"
  "settings-shortcuts|2640,1760|clock settle ; key cmd+s ; key cmd+b ; shortcut New Session ; clock settle|shortcut: New Session alt+cmd+t|/usr/bin:/bin|--settings $out/keys/settings.json"
  "store-quit|2640,1760|clock settle ; send What is two plus two? ; clock settle within 10000 ; folder $home/work/zeron ; clock settle ; send Review the tab bar ; clock settle within 10000 ; state desk|desk: active=1 tabs=0,1 sessions=2 collapsed=0|$stream|--settings $out/store/settings.json"
  "store-reopen|2640,1760|clock settle ; key cmd+1 ; clock settle ; send One more? ; clock settle within 10000 ; state last|last: Continuing session 5e55a0e1-0000-4000-8000-000000000001. Answered by default: two plus two is four, and the sum stays four however you group it.|$stream|--settings $out/store/settings.json"
  "sidebar-open|2640,1760|$three||$so"
  "sidebar-900x600|1800,1200|$three||$so"
  "focus-button|2640,1760|fixture open ; ax-key tab|focus sidebar.new|demo"
  "focus-composer|2640,1760|fixture open ; ax-key tab ; ax-key tab ; ax-key tab ; ax-key tab ; type composer Hello|focus composer|demo"
  "choose-folder|2640,1760|clock settle ; tap sidebar.new ; clock +50 ; choose ; choose $home/work/zeron ; clock settle ; tap sidebar.new ; clock +50 ; cancel ; state desk|desk: active=1 tabs=0,1 sessions=2 collapsed=0|$so"
  "sidebar-shortcuts|2640,1760|$three ; key cmd+1 ; key shift+down ; key shift+up ; menu Close Tab ; menu Toggle Sidebar ; menu New Session ; clock settle ; state desk|desk: active=3 tabs=0,2,3 sessions=4 collapsed=1|$so"
)

fail=0
for c in "${cases[@]}"; do
  IFS='|' read -r name size steps expect engine extra <<<"$c"
  read -r -a more <<<"${extra:-}"
  if [[ "$engine" == demo ]]; then log="$("$app" --shot "$out/$name.png" --size "$size" --demo --cwd "$home/work/openagents" --do "$steps" --tree "$out/$name.tree" 2>&1 || true)"
  else log="$(env -i HOME="$home" USER="$USER" PATH="$engine" BROWSER=/usr/bin/true PSIONIC_HOST=none "$app" --shot "$out/$name.png" --size "$size" --cwd "$home/work/openagents" ${more[@]+"${more[@]}"} --do "$steps" --tree "$out/$name.tree" 2>&1 || true)"; fi
  [[ -f "$out/$name.tree" ]] && sed -i '' "s|$home|~|g" "$out/$name.tree"
  if [[ -n "$expect" ]] && ! grep -qF -- "$expect" <<<"$log"; then echo "FAIL $name: no \"$expect\" in: $log"; fail=1; continue; fi
  if ((update)); then cp "$out/$name.png" "$here/goldens/$name.png"; cp "$out/$name.tree" "$here/goldens/$name.tree"; echo "updated goldens/$name.png and .tree"; continue; fi
  # the text first, row by row (the element table: every row's id, role, label and box), then the picture
  if ! d="$(diff "$here/goldens/$name.tree" "$out/$name.tree" 2>&1)"; then echo "FAIL $name: its rows differ from goldens/$name.tree:"; head -12 <<<"$d"; fail=1; continue; fi
  if result="$("$app" --compare "$out/$name.png" "$here/goldens/$name.png")"; then echo "ok   $name: $result"
  else echo "FAIL $name: $result (shot: $out/$name.png)"; fail=1; fi
done
exit $fail
