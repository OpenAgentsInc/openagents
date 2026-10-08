import hashlib
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

scratch = Path(__file__).parent
root = Path('/Users/christopherdavid/.codex/worktrees/10907-budgeted-relight/openagents')
target = Path('/Users/christopherdavid/work/openagents-target-agent2')
source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
mode = sys.argv[1]
if mode == 'native':
    text = (scratch / '10907-continuation-products-build.log').read_text()
    binary = Path(re.search(r'Running unittests.*\(([^\n]*?/verse_pbr-[a-f0-9]+)\)', text).group(1))
    tests = [
        'pbr::temporal::tests::hidden_motion_cannot_overwrite_the_visible_surface',
        'pbr::temporal::tests::empty_motion_keeps_camera_reprojection_with_stale_object_data',
        'pbr::temporal::reactive_tests::moving_reactive_geometry_cannot_leave_history_in_a_bright_trail',
        'pbr::temporal::reactive_tests::reactive_history_rejection_matches_the_actual_bilinear_footprint',
        'pbr::gpu::baked_tests::static_light_patches_cross_rows_and_layers_and_survive_late_bakes_until_restore',
        'pbr::gpu::baked_tests::rigid_light_ranges_wrap_max_grow_rebind_and_restore_direct_ambient',
    ]
    records = []
    for index, test in enumerate(tests):
        command = [str(binary), test, '--ignored', '--exact', '--test-threads=1']
        record = dict(source=source, test=test, command=command,
                      binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), start_unix=time.time())
        print('Running ' + test, flush=True)
        with (scratch / f'10907-continuation-native-{index}.log').open('w') as log:
            result = subprocess.run(command, cwd=root, stdout=log, stderr=subprocess.STDOUT)
        record.update(exit=result.returncode, end_unix=time.time())
        records.append(record)
        (scratch / '10907-continuation-native-manifest.json').write_text(json.dumps(records, indent=2) + '\n')
        if result.returncode:
            raise SystemExit(result.returncode)
    raise SystemExit(0)

binary = target / 'debug/examples/baked_light_capture'
env = os.environ.copy()
env.update(VERSE_QUALITY='high', VERSE_HOME=str(scratch / '10907-capture-cache'),
           VERSE_KIT_PACK='/Users/christopherdavid/.openagents/verse/zones-cache/c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e.vtp',
           VERSE_KIT_BAKE=str(scratch / '10907-matched-bake/090f106f9c459f62f9cd67902a18531b7e00f9d15e292e0b60a0dd7b231b0b6b.vlay'),
           VERSE_KIT_UNPINNED='1', OPENAGENTS_CAPTURE_SOURCE_COMMIT=source)
names = {'blend': '10907-continuation-blend', 'full': '10907-continuation-full', 'preflight': '10907-continuation-preflight'}
args = {'blend': ['128', '2', '--blend-only'],
        'full': ['128', '10441', '--skip-blend', '--repair-hold-seconds', '600'],
        'preflight': ['128', '2', '--preflight-only']}
name = names[mode]
command = [str(binary), str(scratch / name)] + args[mode]
record = dict(source=source, profile='dev with pinned workspace optimized dependencies',
              binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), command=command,
              environment={key: env[key] for key in ('VERSE_QUALITY', 'VERSE_HOME', 'VERSE_KIT_PACK', 'VERSE_KIT_BAKE', 'VERSE_KIT_UNPINNED', 'OPENAGENTS_CAPTURE_SOURCE_COMMIT')},
              start_unix=time.time())
manifest = scratch / (name + '-manifest.json')
manifest.write_text(json.dumps(record, indent=2) + '\n')
print('Starting ' + name, flush=True)
with (scratch / (name + '.log')).open('w') as log:
    result = subprocess.run(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
record.update(exit=result.returncode, end_unix=time.time())
manifest.write_text(json.dumps(record, indent=2) + '\n')
print(name + ' exit ' + str(result.returncode), flush=True)
raise SystemExit(result.returncode)
