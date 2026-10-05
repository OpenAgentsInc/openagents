# Actor-scoped caster checks

Evidence for audit V16 and [issue #10635](https://github.com/OpenAgentsInc/openagents/issues/10635).
The controlled chamber profile contains ten original abilities and nine physics
catalog spells. Each paired fixture casts from two admitted actor lives and
compares 26 subsequent 50-millisecond updates against checkpoint restoration.
Other fixtures check scoped DCs, commands, concentration, moving Gust ownership,
class/cooldown transfer, legacy checkpoint layout, and consumed refusal fences.
These are correctness checks on one platform, with two casters per fixture;
they establish no raid-size timing or cross-platform replay guarantee.

Commands use the pinned toolchain and the agent's retained Cargo target directory:

```sh
cargo test -p verse-world --features service-reach,studio --lib
cargo test -p verse-world --features service-reach,studio --lib play::caster::tests
cargo test -p verse --no-default-features --lib pack::tests
cargo check -p openagents-desktop -p coder-mobile -p openagents-cli
cargo check -p verse --no-default-features --features web --target wasm32-unknown-unknown
cargo fmt -p verse-world -p verse --check
```

`world-service.log` records 510 passes and two intentional subprocess helpers
ignored. `casters.log` records six final caster tests; the extra moving-Gust test
was added after the full world run. The first Verse run passed 505 tests and
found two stale generated packs. The regeneration logs record their rebuilds;
`content-packs.log` records all 22 pack tests passing afterward, and `verse.log`
records the final full Verse run: 507 passes and 13 intentional GPU/fixture checks
ignored. Native and browser
compiler logs and a source/checksum manifest accompany this evidence. The source
manifest identifies the final code; logs identify their commands and results.
No owner host, display session, device, release gate, or Clippy was used.
