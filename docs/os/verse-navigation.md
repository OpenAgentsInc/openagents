# Verse OS navigation journal

Owner request, 2026-11-03: sharp full-size WoW; independently enter and
navigate the world; share applicable Aethyr components; then let a local
Psionic decision model choose steering actions. Issues #12728–#12730.

## Current work

The initial visible trial exposed two independent display problems. The
session fixes its compositor at 1280×720 while the actual Verse window on
desk 2 is 2548×1428. WoW's saved `GxWindowedResolution` is 831×693. Enlarging
the compositor window alone does not resize that game's internal viewport.
Battle.net also maps a blank helper as an ordinary tiled window.

Milestones: qualify presentation, record bounded navigation practice, then
connect real local Elephant decisions. Record each completed milestone's
commands, evidence and limitations here before its commit.

## Aethyr book review

Reviewed the registry/status overview, chapters 4 (typed input), 5 (replay),
9 (walking), 10 (camera), and 75 (senses and following). Chapters 4–5 provide
working `Intent` and external-event gates. Chapter 75 has checked stimuli,
bounded knowledge and fixed-step followers; its general decision machinery
remains incomplete. WoW has its own simulation and collision state, so an
external controller cannot reuse `walk` to move WoW or claim deterministic
replay of its world. Reuse the input vocabulary and record observation/action
events; keep visual facts separate from unavailable world truth.

Psionic already has `lib/game/decision.coax`: a current-context ticket,
deadline, one staged enum and exactly one legal-action take. Reuse that gate
for model proposals rather than making model confidence an input capability.
Elephant is a text decision model. Its state must contain explicitly sourced
observations, not an assertion that it has seen screenshot pixels.

## Presentation milestone (#12728)

Installed the new native bundle and restarted the visible session on desk 2
with `--desk-size 2548x1428`. The launcher rewrote only the saved window
dimension settings in installed WoW variants before starting their container.
The game mapped directly to a full 2548×1428 buffer and entered Coldridge
Valley. The large black margin and stretched 831×693 image are gone.
Desktop evidence: `/tmp/wow-native-full-desktop.png`; source capture:
`/tmp/wow-native-world.png` (and its metadata sidecar).

The host also retains which physical keys it forwarded to the desk and
releases them when focus leaves. Verified by holding W through the real
outer desk API, switching away, releasing it on the outer desk, then
successfully acquiring and capturing WoW again (previously refused with
`physical_input`). A pointer/keyboard interruption still revokes owned input.

Checks: session 35 passed; updated launcher 86 passed; compositor 101 passed.
Desk-size authority: 42 mutants killed, no survivors; game-window selection:
2 killed, no survivors. Native bundle built/installed; Nix flake evaluation
passed. tools-check reports five existing unrelated Python tools without
reason headers; these changes add no foreign-language tool.

Observation lesson: an immediately requested screenshot can precede the
game's rendered reaction. Capture sequence is a desk capture identifier,
not proof that the game applied the preceding input. Navigation must wait
briefly and verify a visible postcondition before issuing the next action.
