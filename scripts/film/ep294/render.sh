#!/usr/bin/env bash
# Episode 294, cut 1: re-render everything from the sources in one command.
#   ~/work/film-out/ep294/render.sh            # capture the stills and the tour, then cut
#   SKIP_CAPTURE=1 ~/work/film-out/ep294/render.sh   # cut from what's already captured
# Needs a checkout with coax/bin/coax for the headless tools (TOOL_CHECKOUT, default
# ~/work/openagents; run `coax/build.sh` there once). One headless browser and one encode run
# at a time.
set -euo pipefail
SRC="$(cd "$(dirname "$0")" && pwd)"   # these scripts
HERE="${EP_DIR:-$HOME/work/film-out/ep294}"   # footage, stills, work, output
export EP_DIR="$HERE"
CO="${TOOL_CHECKOUT:-$HOME/work/openagents}"
LIB="$CO/coax/lib"
cd "$HERE"
mkdir -p stills lab
if [[ -z "${SKIP_CAPTURE:-}" ]]; then
  # 1. The Bitcoin Lab tour (headless, live site).
  TOOL_CHECKOUT="$CO" OUT="$HERE/lab" "$SRC/capture-bitcoin-lab.sh"
  # 2. The Coax book and the Psionic page (headless).
  (cd "$CO" && coax/web/tool.sh shot "https://openagents.gitbook.io/the-coax-programming-language" "$HERE/stills/coax-book.png" 1920x1200 8000)
  (cd "$CO" && coax/web/tool.sh shot "https://openagents.com/projects/psionic" "$HERE/stills/psionic.png" 1920x1200 8000)
  # 3. Real source, as editor pages to scroll.
  python3 "$SRC/render_code.py" "$LIB/nostr/event_sign.coax" stills/code-nostr.png "coax/lib/nostr/event_sign.coax" 1280 1 90
  python3 "$SRC/render_code.py" "$LIB/crypto/sha256.coax" stills/code-sha256.png "coax/lib/crypto/sha256.coax" 1280 1 120
  python3 "$SRC/render_code.py" "$LIB/bitcoin/header.coax" stills/code-header.png "coax/lib/bitcoin/header.coax" 1280 1 140
  ffmpeg -v error -y -sseof -0.6 -i "$HOME/Desktop/294/book.mov" -update 1 stills/book-last.png
fi
# 4. The cut (each segment, then the final pass), and its copy in Downloads.
rm -f work/seg*.mp4
python3 "$SRC/assemble.py" "$SRC/cut1.json" episode-294-cut1.mp4
cp episode-294-cut1.mp4 "$HOME/Downloads/episode-294-cut1.mp4"
echo "done: $HERE/episode-294-cut1.mp4"
