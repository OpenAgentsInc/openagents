# Aethyr 77: bounded dialogue and replay

Recorded November 6, 2026, Chicago time, for [#12610](https://github.com/OpenAgentsInc/openagents/issues/12610).

The native CPU profile implements authored character admission, bounded JSON
requests and replies, one pending request, stale-response refusal, fixed-step
fallback, and an admitted conversation log that replays without send intentions.
The [chapter](../../../aethyr/book/ch077-people-who-talk-back.md) remains partly
implemented. No realm adopts this module yet.

## Results

- The complete wire suite passed **117 of 117 checks**, including its loaded
  character, session and JSON dependencies. See [wire-tests.log](wire-tests.log).
- Root mutation scopes retained all dependency checks: character **55 of 55**,
  session **366 of 366**, and wire **97 of 97** mutants killed. There were **zero
  survivors**, with no equivalent-mutant waivers. See
  [character-mutate.log](character-mutate.log), [ask-mutate.log](ask-mutate.log)
  and [wire-mutate.log](wire-mutate.log).
- The worked example matched its committed stdout fixture in debug and release,
  exited 0, and wrote no stderr. The deliberately broken empty-question example
  matched its diagnostic fixture in both modes, exited 2, and wrote no stderr.
  [qualification.json](qualification.json) records compiler and source hashes,
  exit codes and the qualified target. The adjacent `.out` and `.err` files are
  the captured process output.
- The [book structure gate](book-structure.log) and
  [tools check](tools-check.log) passed. Listings match their source markers,
  examples are inventoried, and chapter status agrees with the registry.

The example exercises four deterministic transport responses: ordinary words,
instruction-like words that leave game state unchanged, an HTTP failure, and a
deadline. It then replays the accepted record with zero send intentions. These
are transport fixtures, not measurements against a hosted model or live server.

## Failing checks written before the fixes

The retained red receipts contain assertion failures for character admission,
blank required text, opening a session, deadline polling, clock rewind and JSON
request construction: [character-red.log](character-red.log),
[blank-red.log](blank-red.log), [ask-red.log](ask-red.log),
[timeout-red.log](timeout-red.log), [clock-red.log](clock-red.log) and
[wire-red.log](wire-red.log). They are regression evidence, not passing results.

## Book evidence review

The initial structural book check failed on eight existing capability evidence
fingerprints, before checking the new chapter's links. See
[book-baseline-red.log](book-baseline-red.log). Each previous fingerprint matches
the file immediately before commit `f14974e547`. Removing only the newly added
` #effects(all)` annotations produces byte-identical old and new evidence files
for all eight paths. The generated fixtures, assertions, and bounded capability
claims are unchanged. Only those eight evidence fingerprints were refreshed;
no claim or feature qualification was expanded.

[capability-evidence-review.json](capability-evidence-review.json) names each
claim, reviewed file and old/new SHA-256. This review does not claim to rerun the
compiler's release qualification.

## Scope still open

Realm UI and ownership integration, authentication and daily caps, hosted-model
evaluation, cross-visit memory, durable realm event storage, permissions and
voice need their own implementation and checks. This record qualifies no
browser, Wasm, GPU, phone or production deployment. No room or page code changed,
so room/page suites, drift and Wasm budgets do not apply to this chapter change.
