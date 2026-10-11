# Ollama zero-cache build and scalar tests

The public source staging, paired offline builds, and all 16 selected scalar
lifecycle tests passed. The two Go 1.26.3 builds use the same pinned Ollama source
and installed MLX libraries. Their only product source difference calls the
existing cache close operation before each scoring request.

[Summary](summary.json) records resource peaks and binary hashes. Each phase has
its original guard receipt, memory and disk samples, and complete command logs.
The [test result](work/tests.json) binds both exact test binaries and event logs.
[Build details](work/build.json) and [staging details](work/stage.json) bind the
source trees, compiler, dependencies, and native libraries.

These tests use scalar fake caches whose State methods return no arrays. They
cover first and repeated scores, divergent suffixes, long chunked inputs,
cancellation recovery, repeated close, unchanged generation reuse, and the
loaded MLX library identity. They do not prove real tensor cleanup, model answer
parity, observed token ID equality, or latency improvement. No server or model
ran, and no installed app or service changed.

The [pre-execution plan](PLAN.md) preserves its original prose. Its two proposal
links point to the included [source proposal](source-proposal/README.md). The original
source freeze is cbc7e6d902405a941aec083b325c1dad7f87580ed3877253ec2f59dcfe10bded.
Publication replaces local paths with $SCRATCH, $WORKTREE, and $HOME; original
and published file hashes are recorded separately. Weights, native libraries,
compiled binaries, module caches, and full downloaded source trees are excluded.
The staged source manifests and module checksum records remain included.
