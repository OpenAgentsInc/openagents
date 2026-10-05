# The phone joins the chamber (#10586)

Recorded on 2026-10-05 against `main` at `becedf0d40`, on a macOS
(Apple silicon) contributor machine.

## What runs

- The phone and the desktop window play the chamber through one session,
  `verse::imported::chamber_session::Session`. It owns the client worker
  thread, the replica, movement prediction and frame intervals, tracked
  input, and the frame the engine renderer draws. The desktop window
  (`imported::remote_window`) keeps only the keyboard and mouse, its panels,
  and the recorder, which reads the session's notes.
- On the phone (`coder-mobile`, mounted by the OpenAgents app's bare Grid),
  a `ritual.json` in the zone cache directory opens the RITUAL arch. Walking
  through it opens `chamber::Play`, which connects on its own thread, signs
  with the phone's world identity, and draws on the Grid's `GridEngine`
  surface.
- `verse::imported::chamber_loopback::Loopback` is an in-process chamber
  host for the RITUAL scene over memory streams. It runs the same client,
  worker, and session code a TLS host does.

## Test mechanism

Run the session and window tests:

```sh
cargo test -p verse --features remote-chamber,imported-desktop --lib -- \
  chamber_session ritual grid_engine remote_window
```

Result: 14 passed, 0 failed. The lifecycle tests against the loopback host:

| Test | What it shows |
|---|---|
| `a_session_plays_the_loopback_chamber_and_stops_cleanly` | The session joins, holds the forward stick until the predicted player has moved half a metre, builds an engine frame with instances, and stops with `Stopped::Closed`. |
| `a_host_that_goes_away_fails_the_session` | Severing the host's connections ends the worker; `step` reports `Chamber update stream stopped`, and `stop` reports `Stopped::Failed`. |
| `a_new_session_after_a_stop_takes_the_same_character_back` | A second connection after a stop, as a resumed phone makes, holds the same life. |
| `a_dead_player_respawns_once_into_a_new_life` | A dead player's repeated respawn sends one request, and the authority grants a new, controlled life. |
| `native_prediction_binds_local_input_renders_it_and_retires_acknowledgments` | The desktop's prediction test, now on the shared session: tracked moves, teleport resets, batched movement intervals, and refused bindings. |

Run the phone tests:

```sh
cargo test -p coder-mobile --lib
```

Result: 128 passed, 1 failed, 5 ignored. The failure is
`zone_transition_clears_input_and_keeps_plaza_identity_out_of_ruins`, which
fails the same way on `main` without this work (#10633). The phone's chamber
tests against the loopback host:

| Test | What it shows |
|---|---|
| `suspend_stops_the_worker_and_resume_rejoins_the_same_character` | Suspend stops the worker and draws no chamber frame; resume connects again and holds the same life; the frame timing counts frames and at least 14 actors. |
| `a_host_that_goes_away_fails_the_play` | A severed host turns the visit into `Failed`, which returns the player to the Grid. |
| `a_dead_player_respawns_from_the_phone` | The phone's Respawn grants a new life. |
| `a_ritual_file_beside_the_zone_packs_opens_the_arch` | The bare Grid's configuration names `ritual.json` only when it exists. |
| `the_ritual_arch_opens_the_chamber_and_a_refused_connection_returns_to_the_grid` | Walking through the arch opens the chamber; a refused connection puts the player back in front of the arch with the reason shown. |

## Frame times on a phone: not measured here

The 20-actor frame-time run on the Android emulator did not complete. The
emulator (`coder_mobile_api35`, arm64, SwiftShader) booted, but the
machine's disk filled while the OpenAgents Android release build and the
`openagents` CLI build ran (134 MiB free; both builds failed with
`No space left on device`). No frame times were recorded. The device run is
in `NEEDS_OWNER.md` under "The phone in the chamber (#10586)"; the app
writes `chamber-frames.json` beside `ritual.json` during that run, which is
the receipt to retain here.
