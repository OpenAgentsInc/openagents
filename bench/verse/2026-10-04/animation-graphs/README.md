# Native animation graph acceptance

The first 100-second native run uses commit `6e9d35f9ec`, at 3456 × 2104 with four samples. All 7,377 recorded frames evaluate 14 actor instances through compiled semantic graphs. The event proof retains 428 markers, including 68 from respawned lives, with no duplicate identities or generation regressions. The inspected screenshot shows the adventurer, living cultists, grounded corpses, and Claude in the original chamber.

The device callback queues 428 footsteps, 21 fire launches, 98 impacts, seven shields, and one ritual loop. It produces nonzero stereo PCM at 48 kHz with no device errors, refused commands, or dropped capture blocks. `window/audio.wav.gz` is the mixer output supplied to the device callback, not acoustic loopback. Decompress it to play the WAV. The audio uses original procedural synthesis.

Performance acceptance fails: work p95 17.841625 ms / maximum 25.249042 ms; delivered p95 18.494 ms / maximum 81.638292 ms; zero dropped simulation time. These are CPU frame-work and window-interval measurements, not GPU timestamps. The full profile, budget, marker trace, source digests, PCM, screenshots, and unchanged-world reload receipt are retained under `window`. No passing-budget claim is made.

A subsequent implementation shares immutable local-pose buffers across transactional playback candidates and reuses exact blend endpoints. Its 82 engine/asset tests pass, including shared-buffer identity and refusal preservation. This optimization has not yet been measured in a native run. #10494 remains open for native animation and performance acceptance.
