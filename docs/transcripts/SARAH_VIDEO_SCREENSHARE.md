# Sarah video screenshare control

Date: 2026-07-24.
Status: production runbook.
Audience: harnesses that record or edit Sarah episodes with live product
picture.

Use this document when a Sarah episode needs a live product screen in the
picture, not only a still image. Prefer a controlled screenshare. Keep the
spoken script short. Move the picture with a shot script.

## Rule

1. The spoken transcript is the authority for words.
2. Local clip manifests and long-take manifests may be stale. Do not let them
   lengthen the spoken text.
3. Prefer a live Omega window over a PNG or JPG of the same frame.
4. Burn an honest product-state label into the edit.
5. Do not present a planned feature as a shipped feature.

## When to cut

Use a screenshare when Sarah points at a product surface that exists now.
Examples:

- Omega Welcome setup (`Welcome to Omega`)
- A public repository page
- A Desktop or mobile attention surface that is real and redacted

Use a still only when the live surface is unavailable, private, or unstable.
Label the still honestly.

## Interleave model

Record two tracks, then cut:

1. **Sarah picture** — talking-head generation from the short script.
2. **Product screenshare** — a live Omega (or other) window driven by a shot.

In the edit:

| Spoken beat | Picture |
| --- | --- |
| Opening strategy lines | Sarah |
| Fork / Zed decision | Optional brief Cursor or Zed reference, or stay on Sarah |
| "Out of the box…" through naming Omega | Live Omega Welcome screenshare |
| Closing name / hold | Hold Welcome, or return to Sarah |

Do not invent extra spoken lines only to fill a long screenshare. Hold the
screen under the existing short lines, or add silence under the screenshare
with a lower-third label.

## Control Omega on macOS

Tool:

```sh
node scripts/omega-screen-control/omega-screen-control.mjs shot welcome-setup
```

What it does:

- Launches Omega with a fresh `--user-data-dir` so the window is clean
- Sets `ZED_STATELESS=1` so a second instance does not hand off to an
  already-open Omega window
- Sets `ZED_EXPERIMENTAL_A11Y=1` for AccessKit when labels exist
- Opens Welcome through `Help → Editor Onboarding` when first-run does not
  auto-open
- Clicks theme and keymap with window-relative coordinates when AX names are
  sparse

If machine Keychain custody is still Ready, Welcome will not auto-open for a
fresh data dir. Reset the channel identity with the Omega CLI (Ready UI only
shows Protect, not Reset):

```sh
cargo run -p omega_identity --bin omega-identity -- --channel rc status
cargo run -p omega_identity --bin omega-identity -- --channel rc wipe --yes
```

Useful commands:

```sh
# launch, open Editor Onboarding, hold
node scripts/omega-screen-control/omega-screen-control.mjs shot welcome-hold

# open Welcome on an already-running controlled instance
node scripts/omega-screen-control/omega-screen-control.mjs menu \
  --bar Help --item "Editor Onboarding"

# window-relative click (0..1 fractions of the front Omega window)
node scripts/omega-screen-control/omega-screen-control.mjs click-at --x 0.22 --y 0.42

# Finish Setup when identity is ready (cmd-enter)
node scripts/omega-screen-control/omega-screen-control.mjs key --combo cmd+enter

# stop the controlled instance
node scripts/omega-screen-control/omega-screen-control.mjs quit
```

Set the binary:

```sh
export OMEGA_BIN="/path/to/Omega.app/Contents/MacOS/omega"
# or
export OMEGA_APP="/path/to/Omega.app"
```

Owner gates:

- Grant Accessibility permission to the terminal or agent host.
- Grant Screen Recording if the host will capture the Omega window.
  After a new grant, restart Cursor (or the terminal app) before capture APIs
  work.
- `Create identity` may open a Keychain or secure-input prompt once.
- `Finish Setup` stays disabled until the local Nostr identity is ready.
- First-run Welcome auto-opens only when custody is not Ready. Otherwise use
  `Help → Editor Onboarding` or `Help → Show Welcome`.

## Labels

| Surface | Label |
| --- | --- |
| Fresh Welcome from a current Omega build | `OMEGA WELCOME - CURRENT` |
| Public fork / README capture | `OMEGA SOURCE - CURRENT` |
| Inherited Zed-looking build | `OMEGA FORK BUILD - UPSTREAM ZED IDENTITY` |
| Stock Zed reference | `STOCK ZED - CURRENT REFERENCE` |
| Planned UI not running | `ACCEPTED ARCHITECTURE - NOT RUNNING` |

## Episode 262

Keep [`262.md`](262.md) short.
Drive the Welcome screenshare with `welcome-setup` or `welcome-hold`.
Do not restore the longer stale clip-manifest wording into the spoken script.
See [`262-production-requests.md`](262-production-requests.md).

## Acting as Sarah

When a harness drafts Sarah's spoken words, follow
[`../sarah/ACTING_AS_SARAH_RUNBOOK.md`](../sarah/ACTING_AS_SARAH_RUNBOOK.md).
When a harness drives the product picture for a Sarah episode, follow this
document.
