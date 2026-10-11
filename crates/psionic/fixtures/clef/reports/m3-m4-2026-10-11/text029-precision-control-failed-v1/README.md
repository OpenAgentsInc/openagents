# Full precision control: first attempt failed before trunk execution

This preserves the first approved attempt exactly. It failed after 3.681 seconds because the audit helper tried to view a scalar f32 head parameter directly as bytes. PyTorch rejects that zero-dimensional dtype view. No decoder layer ran, A's head replay had not run, and no new model answer was produced. This is a diagnostic harness failure, not evidence about numerical parity.

Peak accounted memory was 762,724,352 bytes under the unchanged 12 GiB cap. The guard recorded no memory rejection or cleanup error, stopped its owned child group, and released both leases. Bounds were start24/floor12 GiB, two CPU threads, 300 seconds, disk2 GiB/floor25 GiB. No GPU, build, quiet lease, desktop app, or system service was used.

The complete failed receipt, traceback, guard, and executed source snapshot are included. Each snapshot was copied before repair and its hash matched the original frozen source check. All originals remain in scratch. Absolute account path prefixes are replaced with named tokens; original/published hashes and exact reversible replacement are recorded. No arrays, weights, binaries, or raw memory samples are included.

The proposed later repair flattens only the scalar parameter's byte-view input while preserving its original shape metadata. This bundle contains no repaired execution and does not change the frozen 103/104 runtime acceptance failure.
