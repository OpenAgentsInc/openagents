# Shared studio acceptance, 2026-10-06

The [scripted receipt](simulated.json) passed against source commit
`edcacd9581d751dc90ce11c051a98ee60333d31e`. It records the capture binary
hash, host key, request IDs, exact reviewed base/HEAD/tree, run admission,
artifact digest, trace digest, and local merge result.

The shared sheet submitted one goal, answered its displayed question, opened
its task review, and merged that review locally. Desk and seat openings named
the same resources. Reopening and reconnecting sent no new command; repeated
confirmation and exact transport redelivery did not duplicate the operation.
The scratch origin stayed unchanged. All five created tasks were archived.

This is a headless semantic capture with the existing scripted engine, no
model call, and no inference cost. It took 798 ms and four scripted passes.
Limits were 32 passes, 10 seconds per socket request, and a 90-second outer
process deadline. The run used a temporary HOME, root, task store, repository,
keys, and socket; its environment contained only PATH, HOME, and TMPDIR.

The retained [manifest](artifacts/manifest.json), artifact bytes, and ATIF trace
preserve the original recorded hashes. Scripted run records do not establish
independent check success or real-engine performance. The earlier live studio
audit remains separate evidence. Native physical-key/render acceptance and a
new bounded real-engine receipt are unverified owner checks in
[NEEDS_OWNER.md](../../../../NEEDS_OWNER.md).

## Reproduce

Use the pinned toolchain and an existing external Cargo target directory.
Run from the repository root. The output directory must be new.

```sh
: "${CARGO_TARGET_DIR:?Set the existing external target directory}"
cargo build -p terminal-studio --example studio-acceptance --features acceptance
python3 - <<'PYTHON'
import os, pathlib, subprocess, tempfile
binary = pathlib.Path(os.environ["CARGO_TARGET_DIR"]).resolve() / "debug/examples/studio-acceptance"
source = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
output = pathlib.Path("/tmp/studio-workbench-new-receipt").resolve()
with tempfile.TemporaryDirectory(prefix="studio-acceptance-") as scratch:
    root = pathlib.Path(scratch)
    (root / "home").mkdir()
    (root / "tmp").mkdir()
    subprocess.run([str(binary), source, str(output)], check=True, timeout=90,
                   env={"PATH": os.environ["PATH"], "HOME": str(root / "home"),
                        "TMPDIR": str(root / "tmp")})
PYTHON
```

Validation: nine terminal-studio tests, the fresh-workspace regression test,
the Verse consumer check, and the retained acceptance run passed. The first
preflight exposed an empty studio's missing repository labels; the host now
advertises admitted labels without exposing paths or creating task state.
