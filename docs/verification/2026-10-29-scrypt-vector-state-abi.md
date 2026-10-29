# Generated-ABI native scrypt vector helpers

The optional large native scrypt and ROMix vector runners used Python ctypes
mirrors of state layouts. Those layouts became stale when ROMix gained owned
workspace fields. Their bounded test-only C helpers now compile against the
generated Coax headers, allocate 64 MiB for the public test input, run at most
2,048 steps of 64 iterations, require completion and compare the derived output
to `hashlib.scrypt`. This corrects the test harness; it is not a production
API and its `free` is not a wipe.

On the current checked and release native builds, each helper passed its full
N=65536, r=8, p=1 64 MiB vector. Their existing small-case native, actual Wasm
and sanitizer tests also passed. Independent read-only review verified the
generated-header use, bounds and assertions; it did not rerun the tests. The
separate scrypt-host fixtures, not these helpers, provide workspace-wipe
coverage. These same runners also passed against the pending owned-PBKDF job
integration.
