import hashlib, json, re, subprocess, time
from pathlib import Path
scratch = Path(__file__).parent
root = Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents')
source = 'f193982ec03bc55cd00345e7146c5b566a4c62b2'
log = (scratch / '10936-main-integration-pbr-build.log').read_text()
binary = Path(re.search(r'Running unittests.*\(([^\n]*?/verse_pbr-[a-f0-9]+)\)', log).group(1))
tests = [
 'pbr::gpu::reactive_lit_tests::photo_sprite_mrt_matches_color_and_rejects_hidden_or_covered_emission',
 'pbr::gpu::reactive_lit_tests::photo_encode_tracks_a_reactive_head_over_a_stationary_receiver_with_camera_motion',
 'pbr::gpu::reactive_lit_tests::photo_encode_preserves_reactive_coverage_and_history_beside_an_hdr_ribbon',
 'pbr::temporal::reactive_tests::a_reactive_head_cannot_seed_neighbor_history_through_color_conditioning',
 'pbr::gpu::reactive_lit_tests::production_lit_projection_marks_every_visible_faceted_sample',
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
 record = dict(source=source, command=command, test=test, binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), start_unix=time.time())
 print('Running ' + test, flush=True)
 with (scratch / f'10936-main-integration-native-{index}.log').open('w') as output:
  result = subprocess.run(command, cwd=root, stdout=output, stderr=subprocess.STDOUT)
 record.update(exit=result.returncode, end_unix=time.time()); records.append(record)
 (scratch / '10936-main-integration-native-manifest.json').write_text(json.dumps(records, indent=2) + '\n')
 if result.returncode: raise SystemExit(result.returncode)
