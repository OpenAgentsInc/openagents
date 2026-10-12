#!/usr/bin/env bash
# Captures the screens the V1 launch film shows from a local openagents-web
# (default http://127.0.0.1:4311) with headless Chrome, into DIR.
# Usage: scripts/film/launch-shots.sh DIR [BASE_URL]
set -euo pipefail
dir=${1:?DIR is required}
base=${2:-http://127.0.0.1:4311}
chrome=${CHROME:-google-chrome}
mkdir -p "$dir"
shot() { # name viewport path
  "$chrome" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --window-size="$2" --screenshot="$dir/$1.png" "$base$3" >/dev/null 2>&1
  # The gallery's sample banner is the top 32 px; the film shows the app without it.
  ffmpeg -hide_banner -loglevel error -y -i "$dir/$1.png" -vf "crop=iw:ih-32:0:32" "$dir/$1.crop.png" \
    && mv "$dir/$1.crop.png" "$dir/$1.png"
  echo "$dir/$1.png"
}
shot home-desktop 1440,900 "/ui/screens/home/signed-in?theme=dark"
shot chat-desktop 1440,900 "/ui/screens/chat-thread/code-and-links?theme=dark"
shot work-desktop 1440,900 "/ui/screens/chat-work/running?theme=dark"
shot computers-desktop 1440,900 "/ui/screens/computers/several?theme=dark"
shot signin-phone 700,1400 "/ui/screens/sign-in/login?theme=dark"
shot home-phone 700,1400 "/ui/screens/home/signed-in?theme=dark"
shot device-phone 700,1400 "/ui/screens/device/approve?theme=dark"
