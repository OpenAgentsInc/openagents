# Guarded dictation insertion

The [Coax guard](../../../../apps/openagents-desktop/zeron/dictation.coax)
qualifies a transcript as one editable composer change. It captures the draft,
selection and send counter under a positive request ticket. A matching final
result consumes that ticket. A different or cancelled ticket cannot write, and
an older completion cannot consume a newer request.

The screen must cancel at each edit, selection change, IME transition,
navigation or reset. That event rule prevents edit followed by undo from
reviving a request. The complete snapshot comparison provides a second check.
The root host also checks the originating chat before calling the guard.

A successful final performs `composer_paste` once. It preserves literal Unicode,
newlines and code-looking text, with only deterministic spaces around adjacent
text/punctuation. The whole replacement and both join spaces must fit before
any composer/history mutation. Empty, NUL, malformed UTF-8 and over-capacity
results refuse. It never calls submit or an engine. Existing `sent` and `last`
observations remain unchanged; one undo restores the old draft and selection.

Ten new test groups pass with their dependency checks: **51/51** total.
Focused root mutation finishes **218/218 killed, zero survivors**, without
waivers. The first mutation epoch exposed missing right-side joining and exact
capacity tests. Those observations remain in `mutation1.stdout`; actual
both-sided and 7,997/7,998-byte cases then killed the surviving sites without
changing production code. Initial checks-before-code diagnostics and a tooling
cast correction remain alongside the final evidence.

The [native boundary witness](../../../../apps/openagents-desktop/tests/dictation-insertion-native.coax)
and its thin [C driver](../../../../apps/openagents-desktop/tests/dictation-insertion-native.c)
pass at O0, O2 and fatal ASan/UBSan. The native witness checks literal newlines,
undo, stale/cancelled completions, edit followed by undo, Unicode refusal and
unchanged send observations. LeakSanitizer is disabled because it is unsupported
on this Mac. No microphone, model or window runs in this component gate; the
application's actual Parakeet/capture/UI integration has separate evidence.

`record.json` pins six actual source inputs and both compiler binaries. The
actual generated C/header are compressed byte-for-byte; no executable or model
is committed. `manifest.json` pins this component's evidence. Passing unchanged
legacy model or whole-app gates were not repeated.

```shell
coax/bin/coax test apps/openagents-desktop/zeron/dictation.coax --library
coax/bin/coax mutate apps/openagents-desktop/zeron/dictation.coax --root-only --jobs 4
coax/bin/coax c apps/openagents-desktop/tests/dictation-insertion-native.coax --library --header /tmp/dictation-insertion.h > /tmp/dictation-insertion.c
cc -std=c11 -O2 -ffp-contract=off -fno-fast-math -I/tmp /tmp/dictation-insertion.c apps/openagents-desktop/tests/dictation-insertion-native.c -lm -o /tmp/dictation-insertion
/tmp/dictation-insertion
```
