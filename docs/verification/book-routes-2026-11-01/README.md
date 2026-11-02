# Book delivery routes — 2026-11-01

Issue [#12624](https://github.com/OpenAgentsInc/openagents/issues/12624).
Four bounded profiles passed with the explicitly selected Coax image, then
retained all their matching results without running another example. These
are native teaching/workflow exits, not whole-book or production qualification.

| Profile | Fresh step/mode executions | Retained step/mode executions | Example executions during retention |
| --- | ---: | ---: | ---: |
| [Coax / local C boundary](coax-current-ffi.json) | 7 | [7](coax-current-ffi-retained.json) | 0 |
| [Aethyr CPU replay](aethyr-core-replay.json) | 6 | [6](aethyr-core-replay-retained.json) | 0 |
| [Psionic public modules](psionic-public-models.json) | 6 | [6](psionic-public-models-retained.json) | 0 |
| [Nostr event and scripted client](nostr-event-client.json) | 4 | [4](nostr-event-client-retained.json) | 0 |

The Coax workflow step itself executes C debug and optimized builds. The
Psionic module example actually trains/resumes its tiny synthetic model;
MNIST training is not repeated. The Nostr client uses scripted transport, and
Aethyr starts no browser. All receipts bind actual source/descriptor and
fixture hashes, observed compiler/C tool/Python identities, options, controlled
environment and host target, with raw stdout/stderr in the adjacent `.assets`
directories. Matching output, status and final input checks establish each pass.

The fresh route wall times, including discovery and tool observations, were
8.797, 77.950, 403.206 and 285.958 seconds respectively on this shared host.
They are not performance benchmarks or guarantees. Retention took less than
1.4 seconds per route and invokes tool-identity probes, not the examples.

[Tooling evidence](tooling/final-qualification.json) retains the 27-test runner
suite, interpreter/source identities and actual failure logs. It covers
selective invalidation, closure/receipt/log integrity, target/tool/environment
changes, path/alias safety, deadlines, interruption and final rechecks. The
initial descriptor-type failure had no saved raw log and remains attributed
as such. The later Bash-input failure and final run have actual copied logs.
Original receipt paths name the private observation location; their copied
files keep the same basenames under [tooling/](tooling/).

The publisher's 15-test observation is [attributed separately](tooling/publisher-tests-observation.json),
without inventing a missing raw transcript. Two existing publication gate
regressions also [pass](tooling/publisher-legacy-tests.log). The metadata
workflow [passes](tooling/metadata.log), including the four registries, exact
excerpts, local links and the route catalog.

[Delivery instructions](../../books/DELIVERY.md) explain clean-checkout use,
selective retention and publication receipts. This work implements source-bound
publication tracking, but performs no real GitBook publication or authenticated
live check. Live body parity is not inferred from page titles.

The observed Git revision is HEAD at execution, with source hashes identifying
the actual working-tree inputs. Receipts are trusted maintenance records, not
signatures, a clean-tree assertion or execution attestation. No model original
weights, user keys, wallet, production relay or visible browser were accessed.
