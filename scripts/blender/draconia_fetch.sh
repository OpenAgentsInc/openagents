#!/bin/sh
# Fetches the CC0 MakeHuman assets Draconia's build reads (base mesh,
# targets, rig and weights) at the pinned commit, outside the repository.
# The assets are CC0 1.0 (makehuman/LICENSE.md, section C); MakeHuman's
# AGPL program code is not used.
set -eu
COMMIT=a8bc2d54ff0ac92e78ff71431b1023eda42bf482
DEST=${1:-$HOME/work/film-out/draconia/src/mh}
if [ ! -d "$DEST/.git" ]; then
  git clone --filter=blob:none --sparse --no-checkout https://github.com/makehumancommunity/makehuman.git "$DEST"
fi
cd "$DEST"
git sparse-checkout set makehuman/data/3dobjs makehuman/data/targets makehuman/data/rigs makehuman/data/eyes
git fetch --depth 1 origin "$COMMIT"
git checkout --detach "$COMMIT"
echo "MakeHuman CC0 data at $DEST/makehuman/data"
