# Session handoff, 2026-10-22

A long coordinating session (2026-10-21 to 2026-10-22) is stopping because its
weekly capacity is nearly used up. This page lists what is live, what is left,
and where each piece of unfinished work is. Every subagent is stopped. No
agent holds a running process or a cloud machine of its own.

## Open issues and where each stands

Seven issues are open. All seven had claims from this session; release or
retake them as you pick them up (`openagents issue release N` with the chat
IDs below, or `openagents issue claim N --force`).

| Issue | State | Where the work is |
| --- | --- | --- |
| #12460 Grid suggestion box (NIP-17 feedback) | About 80% built, unverified | Branch `wip/suggest-12460` (`a018cf33a9`), worktree `~/work/oa-suggest`. It adds `crates/grid-suggestion-box`, `crates/playtest/src/suggestion*`, `crates/everglade-web/src/suggestion_box.rs`, CLI inbox changes, and relay rate changes. Next: rebuild the browser modules, rerun the capture, run `cargo test -p` on the touched crates, rebase on main, push, close. Claim chat ID: `lane-grid-suggestion`. |
| #12462 Epic: Maya harvest | Open until its children close | `docs/research/deepseek/2026-10-21-project-maya.md`, `docs/psionic/` single-box notes. |
| #12464 Layer-major chunked prefill | Done on `main`; needs closing | 541 tok/s for one 10-layer stage, 4.45× token-by-token. The 8K run and the real-model logit comparison landed in `de952f5b52`. Close it after reading those results. |
| #12465 `consumer-q` 2–3-bit expert quant | Blocked on a bug hunt | Artifacts `q2q3-00d720cdf6` (176 GB) and `q3-00d720cdf6` (221 GB) are uploaded. One-box decode is 26.4 tok/s, but answers are garbage from the first token. The kernels match the CPU model. Next step: serve `q3`. Coherent text means 2 bits is too lossy; garbage means a bug that shows only at real size, likely scale layout or expert order. The agent stopped while comparing teacher-forced `q3` logits with the exact ones. |
| #12467 Single-box `consumer-exact` profile | Mostly done | One-box serving through a Pylon front landed in `ea5269910c`. Needs a served chat answer as evidence, then close. |
| #12469 DSpark drafting | Not started | Check the drafter's license and weights first; if they aren't usable, close it with that finding. |
| #12471 Packed FP4 compressed KV | Partly done | Packed KV on the real model is recorded in `ea5269910c`. Finish and close. |

Maya cloud machines (`oa-maya-12465-*`) are deleted. The `deepseek-v41`
spend guard is in sync.

## Closed in this session

The DeepSeek chain closed as an **experimental preview, not G1-qualified**
(#12405, #12424, #12389, #12403, epic #12369). Policy 2.3 passed every output
check and matched the oracle on G2 (5 of 6), but failed the per-layer router
score budget on long prompts. The record is in
`docs/psionic/deepseek-v41-native.md`. Another session kept committing to
#12369 after it closed (`8bf7d5f469` to `0b7b46dcf5`, a further SM120
qualification run). Check with that session before reopening anything.

Also closed, among others:
- **Grid Foreman** (#12444–#12448, #12461, #12484);
- **Haunted House** (#12472–#12477, #12485–#12487);
- **Kiln** (#12441, #12449–#12457, #12488);
- **GPUI overlays** (#12459);
- **Grid playtest fixes** (#12478–#12483);
- **Flint on Metal** (#12422);
- **Metal V4.1 stages** (#12420);
- **Deploy split** (#12427) and **livehost phase 2** (#12435);
- **Build billboard** (#12442), **help desk** (#12443), **Verse trailer** (#12458).

## What's live

- **openagents.com:** commit `075c431c9e`, the sixth deploy of the session.
  - **Sign-in:** GitHub only. `WORKOS_METHODS=github` on the `coder` service; WorkOS production has the correct GitHub client secret.
  - **Game tenants:** `/t/haunt/` and `/t/icecream/` work. Production reads `OPENAGENTS_LIVE_GAMES*`, and its runtime account can read `gs://openagentsgemini-livehost`. The tenant keys are `~/work/.secrets/livehost-{haunt,icecream}.key`; publish with `scripts/deploy/games.sh deploy GAME [--live]`.
  - **Kiln:** `OPENAGENTS_KILN_RUNNER` and `OPENAGENTS_KILN_REPO` point at `oa-kiln`. `OPENAGENTS_KILN_SITE_KEY` is mounted from Secret Manager (`openagents-web-kiln-site-key`).
- **Not yet deployed:** `9f8785a65a` (Kiln defaults to Flint, names the loaded crates, full DeepSeek name, header layout) and anything after `075c431c9e`. Run `scripts/deploy/web.sh stage HEAD`, then `promote` and `shift`. The production `coder` service has its revision name pinned. A manual `gcloud run services update` creates a revision that gets no traffic until you run `update-traffic --to-revisions NEW=100`.
- **Smoke checks from this Mac** kept ending with "Connection reset by peer" partway through, on both staging and production. Single requests work. It's either this Mac's network or rate limiting; run the suite from a cloud machine to tell which.

## Long-running services (they outlive the session)

| Service | Where | How to stop |
| --- | --- | --- |
| DeepSeek preview group | `oa-v41-preview-m0` (Tokyo), watcher `scripts/pylon/v41-live.py` from `~/work/oa-v41-live` | `python3 ~/work/oa-v41-live/scripts/pylon/v41-live.py stop`, then confirm the machine is gone (the old watcher once left it running) |
| Grid Foreman | `oa-foreman-1` | `docs/deployment/grid-foreman.md` |
| Kiln runner | `oa-kiln` | `scripts/deploy/kiln-vm.sh` |
| Flint on GCP | `oa-flint-12419` with controller `oa-flint-control-12425` | `docs/psionic/flint-native.md` |
| Mac Flint Pylon | this Mac, `~/.openagents/flint-mac/` (`server.pid`, `pylon.pid`) | Kill the Pylon PID first (it sends an offline beacon), then the server PID |
| Local Kiln dev | `kiln-dev` on 127.0.0.1:8787 and a local `kiln-runner` | PIDs in this session's scratch folder |

The DeepSeek preview group:
- **Expires** at 2026-10-23T12:50Z, with a $250 cap.
- **Head bundle:** `e411ee8ce2`, which passes model reasoning through. The worker bundle is unchanged (`185eb68c…`).

## Owner steps still open

These are in `~/work/NEEDS_OWNER.md` with details.
- **Kiln GitHub bridge:** create a fine-grained GitHub token, saved as `/etc/kiln/github-token` on `oa-kiln`. Without it the bridge can't open draft PRs.
- **Kiln repository:** announce it with `kiln-git announce`. Run `kiln-git own` for your Identity field.
- **Kiki:** set `OPENAGENTS_GAME_LAB_UNLIMITED` to Kiki's account ID, and get Kiki's sign-off on the Haunted House cards.
- **Grid Foreman:** its first command needs your desktop Foreman to publish its record (`openagents foreman publish`) or an admin request. Reinstall the `openagents` CLI from main first.
- **Relay:** deploy it in a quiet window so the Foreman is exempt from the world population cap (`docs/deployment/grid-foreman.md`).
- **Mac Flint:** decide whether this Mac serves Flint permanently. Today it isn't a launchd service, so a reboot stops it.
- **LiveKit cluster:** it's still running with no users (`oa-livekit-prod`, from the voice audit). Decide whether to tear it down.
- **Trailers:** `~/work/film-out/cut/verse-trailer.mp4` and `verse-trailer-kiki.mp4` need a Verse logo and Kiki's review.
