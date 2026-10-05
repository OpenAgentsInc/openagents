# Dedicated Verse host

`verse-host` serves authenticated TLS worlds through `verse-world`. Its normal
build depends on portable content admission and simulation, Tokio, and Rustls.
It does not link a renderer, window library, font library, retained reader, or
agent. REACH hosting remains available through `openagents chamber host`.

Compile an original world with the Rust content tool:

```sh
cargo run -p verse-content --features compiler -- ritual /tmp/verse-ritual
cargo run -p verse-content --features compiler -- observatory /tmp/verse-observatory
```

Each command requires a new directory and writes `pack.json`, verified runtime
textures, `scene.json`, and `profile.json`. The ritual uses the combat profile;
the observatory uses its authored floor and social seat/switch profile. Both use
the same asset, scene, collision, identity, authority, and checkpoint contracts.
The compiler does not create enrollment keys or TLS credentials.

Configure a host using the existing [TLS host configuration](../../docs/verse/networking.md).
Set `scene` and `pack` to the generated files. For the observatory, copy the
object in `profile.json` into `social_profile`. Supply the instance, listen
address, enrollment public keys, DER certificate, and owner-only private key.

```sh
cargo run -p verse-host -- /tmp/host.json --check 300
cargo run -p verse-host -- /tmp/host.json
```

`--check` admits content, collision, ownership, and saved state, advances the
shared fixed schedule, and validates a checkpoint. It opens no listener and
writes no new checkpoint. Serving uses the same admission path and schedule;
`state_dir` enables durable storage. SIGINT or SIGTERM requests clean shutdown.
Check mode validates the configured TLS paths but does not open credentials.
