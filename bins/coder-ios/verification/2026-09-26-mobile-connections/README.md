# Mobile connection and control verification

Build **0.5.0 (46)** is available in internal TestFlight. See the
[assessment](../../../../docs/coder/verification/2026-09-26-mobile-connections.md)
and [combined acceptance and release evidence](../2026-09-26-world-interactions/README.md).

- [Public relay subscription receipt](public-relay-subscriptions.json): fresh-key AUTH and both world subscriptions accepted, with no world/chat publication or retained player data.
- [Probe source](public-relay-probe.rs): temporary read-only diagnostic, retained for review.
- [Initial mobile suite](initial-mobile-tests.log): two assertions still expected the old verbose UI copy. The behavioral world and pairing fixtures passed. Updated presentation assertions are checked in the final run.

The combined record retains the native failures, corrected retests, normal
optimized launch, exact archived source, and TestFlight confirmation. No
physical-device acceptance or model/benchmark run is claimed.
