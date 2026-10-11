# Image import repair

The first image hardware run completed all6 cases/14 answers and10 refusals
within the52 GiB memory limit, but the enclosing qualification failed with
`KeyError: sources_rechecked`. `proof` loads `acceptance_helpers`, which prepends
the repository tools directory to `sys.path`. A later ordinary import therefore
selected the shared image runner, not the new frozen local copy. That shared
runner lacks the intended extra source/disk checks and freshly hashes the GGUF.
Its source matches the earlier passed image run, but offline checks cannot
restore the missing checks during the first execution.

Keep every original source, descriptor, result, and failed exit receipt unchanged.
The new wrapper loads the ORIGINAL frozen local `native_image_parity.py` by an
explicit `importlib` file specification under a unique name. Before calling it,
verify its SHA-256, module `__file__`, and `inspect.getsourcefile(module.run)`.
The model-free test reproduces ordinary-import shadowing, proves the explicit
path wins despite it, and rejects a shared path, wrong digest, and missing path.
It never calls a runner, loads weights, or uses GPU.

Bind the new wrapper to original harness
`b0448dfd6314cfa0e1b53570d865f66d0a060923bdebf48e47637651d2c1600b`,
the successful candidate, GPU fixture and text receipts, the first failed image
receipt, and the archived shared-runner hash. Use only a new `images-2` output
and new lease receipts. Original local helper source is unchanged. There is no
rebuild, no repeated text suite, and no new numerical variant.

After parent and peer review plus an explicit workload handoff, allow one
corrected image execution with GPU/memory52/disk2 leases. Bounds remain52 GiB
max(RSS,physical footprint),64 GiB startup availability,12 GiB available floor,
600-second startup,300-second requests,1800-second server lifetime,27 GiB free
disk at start,25 GiB disk floor, and2 GiB total qualification outputs. Keep
chunk512,resident-f16,positional upload,fused-tensorops,weight-shapes,fused-fields,
sequential Delta,prefix0,batch1,and two CPU threads. The6/14+10 gate is unchanged.
No quiet lease, desktop/system action, automatic retries, or raised bounds.
This repairs an observed wrapper bug; it does not erase the failed first attempt
or change any model-accuracy or latency acceptance.
