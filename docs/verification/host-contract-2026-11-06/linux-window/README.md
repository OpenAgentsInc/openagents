# Linux window integration

This record extends #12869 with the current Coax 0.52 sources. The full
window/room/audio and default-selection results remain separate from the
completed text dispatcher observations below.

## Whole text delivery

`rh_text` delivers a complete text commit or schedules platform error 604.
It uses the producer's actual inbox capacity and the shared measured-span
authority for v1. Empty commits still reach the producer. Ordinary `rh_send`
messages retain their existing byte clipping behavior. Pre-contract faces
do not need the v1 span export.

The physical recording producer has a 128-byte inbox. The Coax driver
offers `Aé🙂`, an empty commit, 129 bytes, a network poll and another Unicode
commit. It records the return value, event count, kind, byte count and hash
after each operation. Refused text publishes no prefix; only the later poll
delivers 604. The final valid commit succeeds.

The retained [red](text-red.json) catches the unfinished adapter dropping
the empty commit. The [normal](text-green.json) and
[ASan/UBSan/LSan](text-asan-rows.json) runs match all 25 authored fields.
Both complete SDK builds also preserve all 110 hostile action-packet fields.
Their [normal](text-normal.json) and [instrumented](text-asan.json) receipts
fence the actual compiler, sources, C/header/profile products and executables.
The instrumented run enables leak detection and has no sanitizer diagnostic.
The text observation authority kills [37/37 root mutants](text-mutation.log),
without equivalent exclusions. The changed driver passes 105 checks.

Replay from an isolated Linux worktree:

```sh
COAX_JOBS=6 coax/web/tool.sh aethyr/host/build-linux-action-packet.coax \
  "$PWD" "$PWD/coax/bin/coax" /tmp/text-normal-new
ASAN_OPTIONS=detect_leaks=1 COAX_JOBS=6 \
  coax/web/tool.sh aethyr/host/build-linux-action-packet.coax \
  "$PWD" "$PWD/coax/bin/coax" /tmp/text-asan-new \
  --sanitize --prepared /tmp/text-normal-new
```

Use a new output directory for each run. The prepared instrumented build
reuses only the source-fenced successful Coax emission; it compiles and runs
the actual SDK and generated producer/observer under the sanitizers.

Physical window acceptance remains required before switching the desktop
default. The dispatcher observations alone do not qualify that integration.
