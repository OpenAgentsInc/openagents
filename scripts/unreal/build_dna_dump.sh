#!/bin/sh
# Builds Epic's OpenRigLogic (MIT) outside the repository and links
# scripts/unreal/dna_dump.cpp against it.
#   build_dna_dump.sh [ORL_DIR]   (default ~/work/film-out/draconia/src/openriglogic)
set -eu
ORL=${1:-$HOME/work/film-out/draconia/src/openriglogic}
[ -d "$ORL" ] || git clone --depth 1 https://github.com/EpicGames/openriglogic.git "$ORL"
cmake -S "$ORL" -B "$ORL/build" -DCMAKE_BUILD_TYPE=Release >/dev/null
cmake --build "$ORL/build" -j 12 --target riglogic413_5_0 >/dev/null 2>&1 || cmake --build "$ORL/build" -j 12 >/dev/null
lib=$(ls "$ORL"/build/libriglogic*.a | head -1)
c++ -std=c++17 -O2 "$(dirname "$0")/dna_dump.cpp" -I"$ORL/include" "$lib" -o "$ORL/build/dna_dump"
echo "$ORL/build/dna_dump"
