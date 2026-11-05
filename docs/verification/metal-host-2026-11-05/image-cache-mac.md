# Metal canonical image-cache adoption

The v1 Metal adapter now uses the portable `RhgiImages` owner from
`verse/native/render-images.c`. Coax admits each immutable URL, encoded
extent and decoded allocation before retention. Foundation observes physical
file size and performs an exact bounded stream read; it supplies no decoder.
The existing `/worlds/` to `/realms/` bundle mount remains explicit, with
resolved physical path containment. Shared URL-to-bundle normalization is a
separate follow-up; these seven authored paths have no query or escapes.
Pre-contract faces retain their existing platform image path.

The CPU cache owns decoded pixels through GPU recovery. NSData borrows those
bytes without another retained copy. Actual texture ledger admission and
upload determine readiness; a decoded image alone does not. Failed cache-full
views live only in the current bounded frame observations and cannot append
an unbounded negative dictionary. GPU teardown waits for submitted work,
clears media views, then releases the CPU owner.

Actual normal and AddressSanitizer runs both upload/read back all 64,512
logical bytes of the seven original 48×48 JPEGs with zero differences, before
and after recovery. Observed CPU ownership is `[7,64512]` in both generations
and zero bytes after close. The old GPU generation and final ledger both
report zero resources/words. Missing files and malformed metadata pass the
actual six-frame notice/recovery workload normally; malformed-during-decode
passes under ASan. Each first image is failed/not-ready, emits the exact 607
`URL 0 0` row, and leaves later images rendering. Both phases retain exactly
seven identities and 55,296 decoded bytes, then release all bytes on close.

The existing portable cache's 111/111 admission and 74/74 receipt mutation
gates at `04d2d01521` are reused unchanged. The changed bank driver and its
dependencies pass 128 checks. The tools scanner passes on the changed tree.
The native build switches RenderTexture's entry to `native-images.coax` and
tracks both portable C/header inputs when deciding to rebuild the SDK.

The SDK uses the exact already-qualified HTTP MetalRender emission (86-source
join) and the portable RenderTexture emission (27-source join), replacing the
old texture object rather than linking two codec owners. The new MetalRealm,
portable effects source, generated texture object and Swift effects driver
are instrumented in the ASan run. The unchanged frozen 17a producer library
is not instrumented in this adapter run; its earlier producer instrumentation
has a separate receipt. Mac framework leak detection is disabled, so this is
not an LSan claim.

Small receipts and object/image SHA identities are in `image-cache-mac/`.
Full source snapshots, compiler profiles, previous source joins, raw GPU
bytes, plans, build diagnostics and output receipts remain under
`~/.openagents/aethyrforge/metal-host-12868/image-cache/`, in `mac-*` paths.
The compiler is the unchanged b587 0.50 image whose original receipt verifies
in the genuine 73b root, as recorded by the preceding component. This does
not relabel current compiler inputs, the frozen 17a realm or the complete
desktop packaging as current-main qualification. WebKit remains default.
