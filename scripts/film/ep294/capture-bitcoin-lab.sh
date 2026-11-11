#!/usr/bin/env bash
# Episode 294: record the Bitcoin Lab tour headless, one continuous take down the lane.
# The character runs to each pane, stops so it reads, runs on, then along the chain.
#   ~/work/film-out/ep294/capture-bitcoin-lab.sh [URL]
# URL defaults to the live site. TOOL_CHECKOUT names a checkout with coax/bin/coax for
# coax/web/tool.sh (default ~/work/openagents; `coax/build.sh` installs the compiler).
#
# How the walk is timed: the clip steps game time one fixed frame at a time through
# globalThis.coaxialClipStep (coax/web/host.js clip mode). The --eval script wraps that step and
# presses or lets go of W at given frame numbers, so the timing is exact however slowly the Mac
# draws. (Chained --keep clips reload this page, so one clip with a timeline is used instead.)
set -euo pipefail
URL="${1:-https://openagents.com/worlds/bitcoin-lab}"
CO="${TOOL_CHECKOUT:-$HOME/work/openagents}"
OUT="${OUT:-$HOME/work/film-out/ep294/lab}"
SIZE="${SIZE:-1920x1080}"
FPS=30
LEAD=1.0          # seconds of game time before recording starts (the world settles)
# The legs after the lead, walk or hold, in seconds. Stations stand 15 apart and the run is 7.5 a
# second, so a 2 s run takes you from one pane's reading spot to the next one's.
LEGS="walk:0.55 hold:3.4"
for _ in 2 3 4 5 6 7; do LEGS="$LEGS walk:2.0 hold:3.2"; done
LEGS="$LEGS walk:5.5 hold:2.5"
plan=""; t=$LEAD; total=0
for leg in $LEGS; do
  kind="${leg%%:*}"; secs="${leg#*:}"
  f=$(python3 -c "print(round($t*$FPS))")
  [[ $kind == walk ]] && plan="$plan[$f,'keydown']," || plan="$plan[$f,'keyup'],"
  t=$(python3 -c "print($t+$secs)"); total=$(python3 -c "print($total+$secs)")
done
JS="(()=>{const o=globalThis.coaxialClipStep;let f=0;const p=[${plan}];globalThis.coaxialClipStep=async(n=1,fps=30)=>{let r=0;for(let k=0;k<n;k++){for(const [at,ev] of p)if(at===f)dispatchEvent(new KeyboardEvent(ev,{key:'w'}));f++;r=await o(1,fps);}return r;};})()"
mkdir -p "$OUT"
cd "$CO"
coax/web/tool.sh clip "$URL" "$OUT/tour.mp4" --size "$SIZE" --fps $FPS --seconds "$total" --skip $LEAD --eval "$JS"
echo "wrote $OUT/tour.mp4 ($total s)"
