# CoderOS to Coax: handoff, 2026-11-01

Paused at about 1:15 AM CDT on 2026-11-01. The plan and every row's history
are in `2026-10-30-coderos-and-coax-audit.md`. This note says what is
installed now, what changed about the plan, and what to do next.

## The host's real source

`coderos-4080` is built from `OpenAgentsInc/coder`, `os/`, not from
`AtlantisPleb/coderos-hosts`. At `6fa6244f3` that tree rebuilt the running
system byte for byte (`/nix/store/rfh1wx…`). `coderos-hosts` is stale (last
pushed 2026-10-26) and builds a different system. Step 1 in the audit, which
was to copy the modules into `coderos-hosts`, is therefore not needed: the
modules were never frozen.

The edit loop is:

    cd ~/coder/os && nixos-rebuild build --flake .#coderos-4080
    nix store diff-closures /run/current-system ./result
    sudo nixos-rebuild switch --flake .#coderos-4080

A switch exits 4 while the Blue microphone is unplugged, because
`coderos-microphone-gain` fails. Everything else in the switch applies.

## Installed on coderos-4080

Each one is a commit in `OpenAgentsInc/coder` `main`, and a previous boot
entry rolls each one back.

| What | Coder commit | Check on the host |
| --- | --- | --- |
| CPU limits unit runs Coax `cpu-limits` | 522c690b4 | Wrote 125 W and 88 percent, and printed the shell's lines |
| `coder-desk` is Coax | c64e7d14c | `list`, `screens`, `focused` and `reading` were byte-identical against the live desk |
| `screen-record`, the recording HUD, presentation mode and dictation are Coax | 476010916 | A real 4-second 2560×1440 screen recording |
| `coderos-camera` daemon is Coax, with the hand tracker | 783e89e9d | Took `/dev/video0`, fed `/dev/video10`, recorded 1280×720, and ran the hand model |
| `coder-update` turned off | e61bd893e | Its timer is gone; it only rebuilt an unchanged `coder` |
| tty2 runs the Coax compositor in place of Hyprland | 4144df9c4 | Took the screen without root, opened Coder, started Xwayland, and handed the screen to tty1 and back |

The programs build from the C that Coax emits, in `os/pkgs/coax`
(`README.md` there says how to refresh it). Version
`0-openagents-e3fe48cf88`.

Changes in openagents:
- e3fe48cf88: the compositor owns its VT. Ctrl+Alt+F*n* switches VT; on a
  switch away it drops DRM master and the input grab, and on return it takes
  both back.
- 5d7a35ee40: the cursor plane another session leaves on the CRTC is cleared.

The compositor also drew on the real screen for the first time, from tty6.

## Live state

- **tty1:** still the Rust `coder-compositor` session. It was not touched.
- **tty2:** waits for its VT, then runs the Coax compositor. The log is
  `/run/user/1000/coax-compositor.log`.
- **Camera daemon:** the Coax one runs as the user unit
  `coderos-camera-serve`, started by hand. The session's start list starts
  it again on the next login.

## Next

1. **Owner: Ctrl+Alt+F2 at the desk.**
   - Try Super+Return, the tiling keys, the mouse, and an X11 program.
   - Ctrl+Alt+F1 should return to the session.
   - Watch for keys that stick after a switch, and for the pointer.
2. **The Coax compositor has no desk socket.** `coder-desk` inside tty2
   cannot reach it. Serve the desk protocol (`os/desk.coax`) from it before
   it can replace tty1.
3. **Replacing tty1** needs:
   - the desk socket;
   - the camera overlay and HUD placement checked against the Rust session;
   - the hands input path, which the Rust compositor reads from
     `hands.sock`;
   - an owner yes, because it restarts the session.
4. **The update agent (`os/coax-update.coax`)** builds into its own `bin/`,
   but the host installs through Nix. Its next form refreshes
   `os/pkgs/coax` from openagents `main` and runs a rebuild. That is not
   written yet, so no timer is installed.
5. **Not done:** driving more than one real monitor, the Blue microphone,
   the owner's play-through, and retiring the kiosk.
