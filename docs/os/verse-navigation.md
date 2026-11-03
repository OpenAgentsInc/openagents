# Verse OS navigation journal

Owner request, 2026-11-03: sharp full-size WoW; independently enter and
navigate the world; share applicable Aethyr components; then let a local
Psionic decision model choose steering actions. Issues #12728–#12730.

## Initial findings

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
The game's Graphics page initially reported a 2487×1394 client viewport
inside that window. Selected Windowed (Fullscreen), applied and confirmed:
the page then reported 2548×1428 at 100% render scale. This final UI change
removed the remaining edge margin. The host's existing runner forces ordinary
windowed settings before starting umu. The launcher now waits for that process
boundary and atomically restores the desk dimensions and `gxMaximize=1` before
Play. A fresh launch retained 2548×1428 and borderless mode without repeating
the UI change. This remains windowed fullscreen, not exclusive fullscreen.
Final desktop capture: `/tmp/wow-native-borderless.png`.
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
passed. At that milestone tools-check reported five existing unrelated
Python tools without reason headers; the presentation changes added no
foreign-language tool.

Observation lesson: an immediately requested screenshot can precede the
game's rendered reaction. Capture sequence is a desk capture identifier,
not proof that the game applied the preceding input. Navigation must wait
briefly and verify a visible postcondition before issuing the next action.

The long visible trial exposed a separate lifetime bug: `vk-realm` exited
when its timing array reached 100,000 frames, despite `--seconds 31536000`.
It now retains the recent half of a bounded statistics window and counts
lifetime presentations separately. A private Vulkan window with
`--stats-frames 64 --seconds 4` presented 226 frames, retained 34 timing
samples, exited on its requested time and reported zero live GPU objects.
The visible session was restarted with that fix. Panel policy: eight checks
passed; the native bundle was rebuilt and installed.

## Navigation and local decision milestone (#12729, #12730)

Installed `verse-navigate` and `verse-sense` with the native bundle. The
Classic Era addon in `os/wow/VerseNavigation` samples public position,
facing, life, combat and menu state into a visible colour strip. Coax reads
that strip from the same captured game image. The addon has no movement
effects; all actions go through desk-control-v2's targeted input lease.

The loop records observe → propose → recheck → bounded action → release →
settle → observe. It reuses Aethyr `Intent`, replay event envelopes and
angle arithmetic, and Psionic's current-context decision ticket. The new
shared pilot and distribution decoder have checked, mutation-qualified
contracts. See the [protocol](../protocol/external-navigation-v1.md) and
the [Aethyr chapter](../../aethyr/book/ch075-creatures-that-follow-and-watch.md).

Entered Coldridge Valley and calibrated actual movement/facing. An initial
rule waypoint hit a wall and stopped on no progress; a revised waypoint
arrived. Original Elephant is now running through the existing local
Psionic CUDA service, with its model identity pinned and no fallback.
A long numeric prompt oscillated; a compact left/right/ahead observation
worked. Elephant reached three subsequent waypoints: ten actions near the
graveyard, six toward its gate, and nine up the snowy clearing. The last
trial moved from (19637,45891) to (19670,46110), inside the declared radius
of (19670,46120). Each arrival was measured from a new observation.

Elephant chooses a short steering action from semantic sensor facts. It
does not see screenshot pixels. I chose the waypoints after inspecting
the images. General terrain planning, obstacle recovery, combat and quest
selection remain future work; this is a qualified local waypoint loop.

Checks: controller closure 148 passed; direct CLI 87 passed. Entry-source
mutation runs killed all 39 pilot, 87 sensor, 91 navigation-policy and 120
distribution-decoder mutants, with zero survivors. Live checks exercised
menu refusal, a stop file during a verified held key, and SIGKILL during a
verified held key. Both cancellation cases released input; the same live
fixture passed with an ASan/UBSan controller and leak checks enabled.
The installed tools also captured and decoded the live game successfully.
Protobuf compilation, Aethyr book structure/source/link checks, native
bundle installation and Nix flake evaluation passed. Final tools-check
reports nine unrelated tools without reason headers after rebasing onto
the concurrent main-branch changes (six before that rebase).
The new Lua sampler declares its SDK-only exception in its source.

Usage is in [verse-gaming.md](verse-gaming.md). Successful and failed
trial journals, the native screenshot, sanitizer results and limitations
are retained in the [verification record](../verification/verse-navigation-2026-11-03/README.md).
