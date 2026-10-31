# Owned-plant checkpoint/replay precursor

The [scope and API](../../psionic/docs/physics/oscillator-replay.md) and
[qualification](../../psionic/fixtures/physics/oscillator-replay-qualification.json)
retain 13 checks,463/463 root mutants killed and 20,733 native assertions in each
debug/release/ASan-UBSan build. 24 checkpoints restore across 980 original 109
transitions. Independent canonical bytes/parse/trajectory checks cover all 24,
including 720 ticks, near 2^31 ticks, empty queues and signed zero. All 1,312 single-
byte corruptions and 61 independently encoded semantic faults refuse atomically.
Seven first-divergence cases retain tick, field and source-operation evidence.

All six cross-build restores refuse, while state/control traces match exactly.
Snapshot bytes differ where actual executable identities and checksums should.
Runtime caller-provided identities are not self-attestation; the native harness
binds actual executable/source/profile/target observations in development.
Chapter 109/111 source identities and their passing gates were reused unchanged.

Portable default physics units include the new Coax core/codec. Native replay
qualification is an explicit physics/replay-native selection because its
executed profile is Darwin-arm64, not a silent cross-target claim. Dispatcher
syntax/unit listing and the added exact book listing were checked separately;
completed numerical and mutation gates were not repeated during publication.
