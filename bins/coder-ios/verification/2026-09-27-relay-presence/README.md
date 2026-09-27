# Public relay presence verification

Issue [#9728](https://github.com/OpenAgentsInc/openagents/issues/9728).
September 27, 2026. This record covers the shared Verse `Session` transport
against `wss://relay.openagents.com`. It is network evidence from one Mac,
not physical-phone or native-renderer acceptance.

## Two shared sessions

[`two-peers.json`](two-peers.json) records a passing run with the current mobile
cadence: pose frames every 3 seconds while moving or every 5 seconds while idle,
and changed durable state at most every 30 seconds. The two sessions became
online after 409 and 468 milliseconds. Each received the other's live avatar,
then its changed position. A third, independently authenticated witness
received three signed `23300` frames per publisher, all four online `33301`
entity states, and all four offline cleanup states. The run took 8.929 seconds.
The witness validates the signatures and NIP-MV shape before recording events.

[`two-peers-idle10.json`](two-peers-idle10.json) retains the earlier passing
8.753-second run before idle cadence changed from 10 to 5 seconds. That shorter
run did not exercise the idle timeout edge. The change avoids scheduling or
network jitter putting a 10-second keepalive behind the crowd's 10-second stale
threshold; neither short run establishes a long-duration reliability rate.

The peers use fresh in-memory keys and the same `Session` implementation the
mobile app uses. They publish no chat, profile, greeting, model request, or
benchmark run. Their four offline addressable state records remain on the
relay; their keys are discarded and their sockets close when the process exits.
The witness subscribes only to the two probe public keys. Session subscriptions
also read the existing plaza, but this evidence retains no other users' events.

## Reproduce

From the repository, use a separate worktree target directory:

```sh
CARGO_TARGET_DIR=/absolute/path/to/target \
  cargo +1.97.1 run -p verse --no-default-features \
  --example presence_probe -- wss://relay.openagents.com
```

The probe requires an explicit relay. Its observation deadline is 45 seconds,
with at most 3 additional seconds for an unsuccessful run's cleanup attempt.
It exits nonzero if either session fails to see the other's live and changed
avatar, the witness fails authentication or subscription, or the witness misses
required frames and online/offline states. It always prints the bounded evidence
record after observation, including failed results. Private keys never enter output.

The example passed focused Clippy with warnings denied:

```sh
cargo +1.97.1 clippy -p verse --no-default-features \
  --example presence_probe -- -D warnings
```

## Native observation mode

For a separate simulator or phone check, obtain only its public world key from
the passive native observation metadata, then run:

```sh
cargo +1.97.1 run -p verse --no-default-features \
  --example presence_probe -- wss://relay.openagents.com \
  --observe-phone HEX_PUBLIC_KEY
```

This mode runs for 120 seconds plus up to 3 seconds for cleanup. Its synthetic
peer stays near the phone's latest verified public pose. A separate witness
retains only that peer's and the explicitly supplied phone's signed presence
records. It cannot authenticate or publish as the phone. Native screenshots,
visible remote avatars, online entity counts, and lifecycle evidence must be
recorded separately; this mode alone cannot prove what the phone rendered.

## Relay compatibility and limits

[`public-relay-nip11.json`](public-relay-nip11.json) is the live NIP-11 response.
It advertises NIP-42, `auth_required: false`, `restricted_writes: true`, and a
maximum query limit of 127. The shared session's world subscriptions completed
successfully despite requesting up to 500 retained states. This check does not
establish that every retained state was returned.

The repository's default rate limits are 60 events per public key per minute
and 120 per IP per minute; NIP-11 does not declare the live deployment's numeric
limits. At continuous movement, the mobile cadence plans about 20 frame events
and four durable entity-state events per minute, before gestures or other
activity. Two peers fit the defaults for this traffic alone.

The original desktop cadence plans about 600 frame events and 40 durable
entity-state events per minute while moving. Its fourfold backoff still leaves
about 150 frame events per minute, above the default public-key allowance.
This record does not claim that desktop cadence works against an unmodified
public relay. Clients need a compatible cadence or a separately configured
relay policy, and all clients sharing one IP share its event allowance.
