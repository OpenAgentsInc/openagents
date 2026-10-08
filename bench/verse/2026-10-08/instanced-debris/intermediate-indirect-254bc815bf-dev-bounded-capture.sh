#!/bin/sh
set -e
capture_scratch=/Users/christopherdavid/.openagents/scratch/codex-01a119ae-8a0b-7850-9b7f-ca9fe5a3203b
capture_binary=/Users/christopherdavid/work/openagents-target-agent0/debug/examples/meteor_showcase_capture
python3 - <<'PY_CHECK'
import re, subprocess
from pathlib import Path
root=Path('/Users/christopherdavid/.openagents/scratch/codex-01a119ae-8a0b-7850-9b7f-ca9fe5a3203b')
s=(root/'10936-10938-indirect-checks.log').read_text()
assert s.count('test result: ok.')>=7
assert 'Finished `dev`' in s.splitlines()[-1]
pbr=re.search(r'Running unittests.*\(([^\n]*?/verse_pbr-[a-f0-9]+)\)',s).group(1)
verse=re.search(r'Running unittests.*\(([^\n]*?/verse-[a-f0-9]+)\)',s).group(1)
for binary,test,name in [(pbr,'pbr::temporal::tests::hidden_motion_cannot_overwrite_the_visible_surface','visibility'),(pbr,'pbr::temporal::tests::empty_motion_keeps_camera_reprojection_with_stale_object_data','empty-motion'),(verse,'render::pipelined_frames_keep_exact_selected_pixels_and_complete_every_index','pipeline')]:
    with (root/('10936-10938-indirect-native-'+name+'.log')).open('w') as out:
        subprocess.run([binary,test,'--ignored','--exact'],stdout=out,stderr=subprocess.STDOUT,check=True)
    assert 'test result: ok. 1 passed' in (root/('10936-10938-indirect-native-'+name+'.log')).read_text()
PY_CHECK
export VERSE_QUALITY=high
export VERSE_KIT_UNPINNED=1
export VERSE_KIT_PACK=/Users/christopherdavid/.openagents/verse/zones-cache/dae1612d4c22438a933c27b406c1e18fe134b13eab8eb5240ddcf5506ffb0b93.vtp
shasum -a 256 "$capture_binary" > "$capture_scratch/10936-10938-indirect-binary.sha256"
"$capture_binary" "$capture_scratch/10937-high-indirect-bounded" --live --settle-light --no-video --seconds 16 --impact-frame 469 --smoke-frame 600 > "$capture_scratch/10937-high-indirect-bounded.log" 2>&1
