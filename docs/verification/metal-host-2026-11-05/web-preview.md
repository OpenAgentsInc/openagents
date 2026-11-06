# One-click web preview for an explicit native session

An explicit `--world metal` session now offers **View → Check as Players See
It**. That action opens the ordinary bundled web realm at the same local
world URL and page/room processes. **View → Return to Native World** returns
to the native host. Each transition closes the previous realm and cancels
its voice/warmup context before opening the replacement; it waits for the
replacement's first real rendered frame. Setup, local world storage and
the desktop Coder service remain in the shell.

The default remains WebKit. The action is disabled for a web-launched session
and during startup or an active transition. The shared Coax
`world-preview.coax` decides the destination from observed launch/current
backend, readiness and busy state; Swift performs view, navigation and SDK
effects. Coax also supplies both labels. Its root mutation gate kills all
24 mutants without equivalents. A future qualified native default selects
the same existing action; this component does not change that default or
opt the public web face into v1.

The actual AppKit menu round trip passes all 26 independently checked fields
in [receipt.json](web-preview/receipt.json). Both real menu actions are enabled
and refuse another transition while busy. The web view observes ready and
scene-ready, connects to room seat zero, and awaits actual GPU queue completion
through the existing host observer. Returning native renders again and
reconnects at seat zero. Its hidden document is the voice-only page, with
voice present, world exports absent and zero canvases. The old native engine
and final close have zero GPU live objects/words and retained decoded-image
bytes; audio is retired/inactive, and the replacement has not received an
audio gesture. No client engine runs behind the other host.

This is a functional UI gate, not another FPS measurement. It uses a separate
muted app/data directory, the already source-joined 85c producer/page, admitted
3f8 package and qualified current SDK from the [Mac acceptance](current-85c-final.md).
The web page uses its ordinary shipped face and host, without a canonical or
presentation override. Its public v1 migration remains a separate #12869 exit.
The new small desktop session C is emitted by the original b587 compiler;
all twelve actual source-table entries, C/header/object, face and shell
images are retained with their SHA identities. This does not infer a compiler
receipt for newer protected inputs or re-emit the world/shaders.

`world-preview-conformance.coax` accepts the raw fields and exact menu labels;
all 108 root mutants are killed without equivalents. Its driver passes 84
checks. The initial mutation run exposed a missing all-three-negative-seat
boundary; the strengthened test kills it. The first real round trip was
correctly rejected because the SDK observer expected no `coaxial` namespace
in the voice-only document. That page intentionally provides `coaxial.voice`.
The corrected observer records the exact document, voice/world exports and
canvas count; the failed receipt is retained as [first-receipt.json](web-preview/first-receipt.json).
No production behavior or acceptance bound was relaxed for that correction.

Raw files, exact isolated bundle, data, generated session and frozen sources
are under `~/.openagents/aethyrforge/metal-host-12868/web-preview/`.
[artifacts.sha256](web-preview/artifacts.sha256) binds those observations.
The rerunnable Coax driver is
`coax/web/tool.sh apps/aethyrforge-desktop/check-web-preview.coax APP DATA NEW_OUTPUT`.
It authors the command plan and invokes the real menu action; `--existing`
only reduces an already retained receipt. All owned app/child processes exit
after the actual run.
