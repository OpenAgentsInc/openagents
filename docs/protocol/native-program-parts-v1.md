# Native program parts v1

`coax.native.program-parts.v1` is an installed-runtime artifact profile. Each
compiler-generated module has `build/NAME.js` and `build/NAME.tail.js`. Concatenate
their raw bytes in that order, decode UTF-8 once, then evaluate one script.
`tree-commands.js` and `runtime.js` keep their existing independent execution order.

The first part contains one byte through 8 MiB; the tail contains 0–8 MiB and is required even
when empty. A nonempty tail requires an exactly 8 MiB first part. The complete
program is at most 16 MiB. The shared Coax codec owns these bounds. An absent,
oversized or noncanonical pair refuses before evaluation. A split inside a
multi-byte UTF-8 sequence is valid because decoding follows concatenation.

Both parts are ordinary independently hashed resources in the unchanged signed
`coax.native.package.v1` envelope. Its 8 MiB per-file, 32 MiB total and 24-file
bounds remain unchanged. Native release packages omit the four non-executed Wasm
reference artifacts to leave room for all program pairs and world inventories.
Archives retain those Wasm files for the independent runtime oracle.

Only runtimes whose installed loader supports this profile receive these pairs;
the changed loader and generated policy change the runtime fingerprint. Existing
runtime feeds remain intact. The initial feed is built from the archive's exact
resources, so installing a fresh build does not prompt for its own bytes again.
