# Verse tab presence verification

September 28, 2026. This record covers avatar presence in the OpenAgents app's
Verse tab (the bare world, `verse-bare`). The live exchange ran in the iOS
simulator (iPhone 17 Pro, iOS 26.5) against `wss://relay.openagents.com`. It
does not cover a physical device or two phones.

## Automated tests (loopback relay)

A NIP-01 relay fixture on a loopback socket
(`crates/verse/tests/support/loopback_relay.rs`) verifies signatures,
forwards ephemeral events, and keeps addressable ones. It has no
authentication or rate limits.

- `cargo test -p verse --no-default-features --test presence`: two
  presence-only sessions come online and see each other's live avatar, one sees
  the other move, and a plaza session on the same relay sees neither. Each
  publishes only kind `23300` and `33301` events tagged `verse-bare`, with the
  avatar alone and no display name. After a session is dropped, it publishes
  nothing more.
- `cargo test -p coder-mobile --lib bare`: the real bare scene joins at the
  mobile cadence. A peer's avatar appears as one live entity and is drawn in
  gray with no light, glow, or neon. When the peer walks for three seconds at
  one pose per second, the scene draws continuous motion, with no drawn step
  over 0.5 meters, and ends where the peer stopped. The peer sees the scene's
  stick-driven walk. Pausing reports `paused`, closes the session, and stops
  publishing; resuming starts a new session.
- `cargo test -p verse --no-default-features --lib crowd`: a render delay just
  past a three-second frame interval interpolates between poses; the delay is
  bounded.

## Production relay exchange

The app ran with `--tab verse`. Its script log gave the world public key
`c25458d5…722e3c`, which is not the device key. A synthetic peer then joined
the same world:

```sh
cargo run -p verse --no-default-features --example presence_probe -- \
  wss://relay.openagents.com --observe-phone HEX_PUBLIC_KEY --bare
```

In `--bare` mode, the peer uses `Session::start_presence` in `verse-bare` at
the mobile cadence. It walks back and forth 4 meters in front of the phone's
latest verified pose. An independently authenticated witness retains only the
two keys' signed presence records.

[`phone-peer.json`](phone-peer.json) records a passing 120.2-second run:

- The peer was online after 397 ms.
- It received the phone's live avatar after 952 ms.
- The witness recorded 19 signed phone frames and 40 peer frames.
- The witness recorded the peer's offline state and no errors.
- The phone's published avatar moved from z 15.59 to 28.39 to 40.87 while a
  relaunch with `--verse-script walk,walk,walk` walked it forward.

The phone's log reported `world connected players 1` while the peer was
present.

- [`peer-walking-1.png`](peer-walking-1.png) and
  [`peer-walking-2.png`](peer-walking-2.png), taken three seconds apart, show
  the peer's white-and-gray avatar mid-stride at two positions ahead of the
  local player.
- [`after-phone-walk.png`](after-phone-walk.png) shows the peer still in front
  after the phone walked about 25 meters. The peer followed the phone's
  published poses, so the peer received the phone's movement.

## Limits

This is simulator evidence with one app instance and a headless peer on the
same Mac. It is not a two-phone check. The public relay's numeric rate limits
are not published. At the mobile cadence, each player sends about 20 frames
and at most a few states per minute. That is within the repository default of
60 events per key per minute.
