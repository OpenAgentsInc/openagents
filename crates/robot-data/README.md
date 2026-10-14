# Recorded robot motion

`robot-data` reads local LeRobot **v2.1** and **v3.0** datasets for
[#11605](https://github.com/OpenAgentsInc/openagents/issues/11605).
Arrow and Parquet versions are pinned to 60.0.0. Files, metadata, page sizes,
decoded table memory, frame counts, joint vectors, timestamps, and cameras
have explicit bounds. File paths stay under the supplied dataset root.

`Dataset::open_pinned` verifies an immutable source revision, file hashes,
and license notices. Joint replay also works without camera files. The
optional `video` feature decodes one local frame with system `ffmpeg` under
`supervise`, with a deadline and memory limit. Missing video returns a blank
frame. V3 replay includes the episode's offset within a shared video file.

`Mapping` requires named joints, units or calibration ranges, direction,
offsets, and robot limits. The reader never infers degrees, radians, or
calibration from a feature name. `Playback` has one recorded clock for
scrubbing, speed, pause, and hidden-view suspension; it has no execution API.

The small Apache-2.0 SO-101 fixtures contain the first four frames of two
episodes at `avishekjana/so101-dataset-v2` revision
`73c13ed578e293bd08ec0b368216ae66caa272e6`. Joint values, actions, and timestamps
match the independent PyArrow reference exactly. The v2.1 fixture converts
the same rows into the earlier layout. Each fixture includes its source card,
license, notice, and checked file manifest. Camera files are omitted.

Regenerate with `python fixtures/prepare.py SCRATCH` in an isolated environment
with `pyarrow==21.0.0`. `SCRATCH` must be an explicit persistent scratch folder.
Run `cargo test -p robot-data --features video` for table, mapping, clock,
path, corruption, and local camera tests; the camera test requires `ffmpeg`.

The example `so101-view-mapping.json` maps normalized positions onto the pinned
URDF's ranges for visualization. The recording does not include the physical
arm's motor calibration; use your own calibration table to reproduce its
physical angles. The example is never benchmark or hardware-control evidence.
