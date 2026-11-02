# Mesh qualification tooling

The own-code receipt and source-repair controls run in Coax:

```sh
coax/web/tool.sh verse/icecream/check-mesh-band-tooling.coax /tmp/mesh-band-tooling
```

This replaces `test_mesh_band_tooling.py`. It exercises eleven full-check
receipt cases and seven independent C++ reference-patch cases. All fixtures,
command receipts and outputs remain in the supplied directory. It generates
no Coax mesh and runs no geometry comparison.

`check_mesh_bands.py` runs the independently repaired C++ geometry reference;
its reason3 header identifies that boundary. The Coax tooling tests call its
existing functions through `--check-reused RECEIPT EXPECTED.json` and
`--repair-source INPUT.cpp OUTPUT.cpp`. These modes only expose the functions
under test. Normal `test-mesh.sh` qualification retains its existing flags.

The thirteen named producer-loss controls also run in Coax:

```sh
coax/web/tool.sh verse/icecream/check-mesh-band-controls.coax \
  --qualification-dir /tmp/mesh-band-qualification --work-dir /tmp/mesh-band-controls
```

The nominal qualification receipt, complete source closure, producer compiler
and independent oracle must match. `--coax FILE` selects that producer compiler;
`--source-root DIR` selects an explicitly frozen source checkout when needed.
`COAX` selects the compiler used to build the observer tool. These identities
remain separate.

The harness emits each deliberate loss normally before its intended verifier
may reject it. A matching prior result reuses only exact artifacts and ordered
verifier commands. Changed cached bytes or selectors refuse without silently
running producers again. Use a new owned output directory for a new profile.
The gate retains transcripts and original prior receipts, with180second child
and810second whole deadlines. It is scoped producer-loss evidence, not
whole-library mutation or complete renderer/current-toolchain qualification.
