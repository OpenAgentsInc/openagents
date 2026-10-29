# Scrypt cleanup lifecycle policy, initial evidence

This experimental, standalone Coax policy describes a future adapter's
cleanup sequencing. A result cannot become visible until scratch and internal
state have both been acknowledged as erased and an explicit publish transition
has occurred. On failure or cancellation, a third acknowledgment is required
for the separately staged derived key. A failure or cancellation before publication prevents success;
the first failure/cancellation is sticky. Publication is the linearization
point: cancellation after publication cannot retract an already visible result.
The adapter must serialize cancellation, publication and result access.

The policy does not run scrypt, wipe any bytes, inspect pointers, enforce writer
quiescence, or authenticate acknowledgments. A trusted host must own and protect
the policy, keep it separate from wiped state, prevent aliases and further writes,
join all writers before wiping, and acknowledge only actual successful erasures.
The host must bind SC_KEY to the exact staged output and actually erase it on
failure or cancellation. Host integration, untrusted pointer admission, NFKC
and physical device execution
are unfinished. Registers, stack temporaries, compiler-generated copies and OS
snapshots remain outside this model. No production-secret approval is claimed.

Three Coax checks pass and all 118 mutants were killed (163 selective checks,
2 skipped by the existing scheduler). An independent finite reference model
exhaustively compared bounded structurally valid and invalid states, end reasons,
acknowledgments and publication against generated checked/release code on native
ASan/UBSan and actual Wasm. The generated C compiled for arm64 iOS Simulator;
this is not an iOS execution. The run used coax0 0.22.0
(`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`).

Checked C SHA-256 `9f14fb3f1040093f44cc699cdea1656cf6b2def3c01f1b929468fdc47f0b3522`,
Wasm `21f10e05b9f5276ce1975231c1dadc82fd87e0c2fbd0a88107d33812bdeb8b40`;
release C `9ae3ceb4c6553f65b957541b63f015734ec12c9113151e189be9ad64e5f2419b`,
Wasm `20e401a03e307af55422728b32164619a453b0dec6d2f75fbf7862f5244c9941`.

```sh
coax/bin/coax test coax/lib/crypto/scrypt_cleanup.coax
coax/bin/coax mutate coax/lib/crypto/scrypt_cleanup.coax --mutation-stats
python3 coax/tests/nostr/scrypt_cleanup.py
```

Independent review of policy `50e7371ec6bf3267b85aeb600ac0c75e95c3eb5b6b638d2ad9e34fa4c8581d71` found no concrete transition defect within these trusted-host assumptions. It is not an erasure or production approval.
