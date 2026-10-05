set -eu
cd /home/christopherdavid/work/verse-battle-host-agent1
ready=/home/christopherdavid/work/verse-battle-licensed-ready-agent1-aeM0u85t
cp /home/christopherdavid/work/openagents-target-verse-battle-agent1/debug/examples/battle_scale "$ready/battle_scale-catchup-admission"
chmod 500 "$ready/battle_scale-catchup-admission"
sha256sum "$ready/battle_scale-catchup-admission"
git rev-parse HEAD
git diff -- crates/verse-world/Cargo.toml crates/verse-world/src/service/net.rs crates/verse-world/src/service/net/resume_tests.rs crates/verse-world/src/service/net/session_pipeline.rs > "$ready/catchup-admission-source.patch"
scratch=$(mktemp -d /home/christopherdavid/work/verse-battle-blocked-write-agent1-XXXXXXXX)
mkdir "$scratch/home" "$scratch/tmp" "$scratch/runtime"
chmod 700 "$scratch/runtime"
export HOME="$scratch/home" TMPDIR="$scratch/tmp" XDG_RUNTIME_DIR="$scratch/runtime"
unset DISPLAY WAYLAND_DISPLAY
export LD_LIBRARY_PATH=/nix/store/baw82sps59binavc16lfp7drj1g7nw9n-vulkan-loader-1.4.341.0/lib:/run/opengl-driver/lib
export VK_DRIVER_FILES=/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json
export VERSE_BATTLE_PACK=/home/christopherdavid/work/verse-battle-licensed-assets-agent1-mTtMHjWo/runtime-pack.json
export VERSE_BATTLE_RESOLUTION=2560x1440 VERSE_QUALITY=high VERSE_GPU_TIMING=1
"$ready/battle_scale-catchup-admission" combined "$ready/combined-catchup-admission-01.json" 120 > "$ready/combined-catchup-admission-01.log" 2>&1
