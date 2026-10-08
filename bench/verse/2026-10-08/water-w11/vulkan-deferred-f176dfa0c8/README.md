# Deferred Vulkan query measurements

All 15 RTX 4080 fixed views return 96 valid GPU samples out of 96.
The focused water tests pass (59 tests). Low pond and storm views and
Medium storm exceed the original GPU targets; the policy reduces effects,
and each view retains visible water. High storm averages 1.846 ms of worker
CPU per displayed frame. These records precede budget calibration.
Images remain in private scratch; their hashes are in `summary.json`.
