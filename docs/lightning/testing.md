# Testing the Spark wallet

Each layer gets the test that can actually catch its bugs. Cheap, complete
checks come first; live money comes last, in small amounts.

| Layer | Test | Runs | Proves |
| --- | --- | --- | --- |
| Coax authority | `#test`, `#forall`, `#authority` | Every build | The policy is right on every case in its stated domains |
| Coax authority | `coax mutate` | Every change to `wallet.coax` | The checks pin the code down: every mutant fails one |
| Coax boundary | Fuzzing the exported procedures | Nightly and before release | Hostile host input never traps, overflows or reaches `send` |
| Records | Imported `payment.v1` wire checks and round trips | Every build | Records encode and decode exactly; malformed bytes are refused |
| Keys | Published BIP-39, BIP-32 and NIP-06 vectors, plus a Spark identity vector | Every build | The words give the same keys as everyone else's code |
| Swift host | XCTest with a fake SDK | Every build | Every SDK result and error maps to the right state |
| Swift host | Simulator UI tests | Before each TestFlight build | The flows work: quote, confirm, send, receive, restore |
| SDK integration | Breez's local regtest environment | Before upgrading the SDK, and before release | Real Spark and Lightning flows against real operators and an SSP |
| Live | Mainnet with a throwaway seed and a few sats | Before release, and after each SDK upgrade | Lightning, Lightning addresses and deposits on the real network |

## 1. Coax checks

`metareal/wallet.coax` is `#authority`. So:

- every procedure except `main` and `#foreign` ones must be run by a check
  (E0380);
- the checks run on every compile, so a failing check stops the app build;
- since 0.5.1, empty domains, checks that only reach dead calls, and
  malformed budgets can't report success.

What to check:

- **Units.** `sat_to_msat` and back:
  - exhaustively over a small range;
  - by `#test` at 0, 1, `MAX_SAT` and `MAX_SAT + 1`;
  - negative input refused.
- **Send decision.** `may_send` over every combination of state, small
  amounts, fees, caps, limits and expiry. The check restates the rule in the
  plainest form, as `amount + fee > left`; the code uses the overflow-safe
  `amount > left - fee`. One `#test` checks the rule near `MAX_SAT`.
- **Outcomes.** Every (state, report) pair, so `unknown` can't turn into a
  failure and a finished payment can't move.
- **Sequences.** A bounded model of the state machine. A `#forall` over
  sequences of up to four events from {quote, confirm, cancel, expire, sent,
  failed, timed out, resolved} checks these invariants:
  - at most one `sendPayment` per quote;
  - a retry reuses its key;
  - `succeeded` is final.
  Eight events over four steps is 4,096 cases, well inside the budget. Use
  enum bindings, one per step.
- **Limits.** Daily and per-payment limits over small amounts, including
  exactly at the limit.
- **Warnings.** Exit-data staleness and backup reminders over small time
  ranges, including the boundaries.

## 2. Mutation

`coax mutate metareal/wallet.coax` must report 0 survivors. It changes:

- operators, including boundary swaps such as `<` to `<=`;
- int literals;
- bool literals;
- conditions;
- assignments, calls, and `+=`/`-=`.

A surviving mutant means a missing check. Write the check; don't mark the
line.

A `// mutant: equivalent` mark is a reviewed claim that no input can tell
the change apart. Since 0.5.1 it counts only as a real line comment on the
mutant's line, not inside a string or block comment. Each mark is named in
the commit message.

The sketch in [coax-integration.md](coax-integration.md) kills 33 of 33
mutants today.

## 3. Fuzzing

`coax fuzz` is planned (verification report §7.6, stage V3), not built. It
takes a `[] u8` target, adds coverage counters in the emitter, and turns
crashes into fixtures. Until it lands:

- **Harness.** A small C file in `metareal/platform/` reads fuzz bytes as a
  stream of host calls:
  - quote fields;
  - confirm and cancel;
  - SDK reports;
  - timer ticks;
  - the clock running backwards;
  - values at and past `MAX_SAT`;
  - negative fees.

  It drives the generated C library through its `#export_c` procedures.
- **Build.** Apple's clang has no libFuzzer (`-fsanitize=fuzzer` is an
  unknown option). Use one of two builds, neither adding a new toolchain:
  - On the Mac: the harness carries its own small driver. It replays the
    corpus, then mutates inputs from a seeded PRNG, all under
    `-fsanitize=address,undefined`.
  - On a Linux pool host: build the same harness with upstream clang's
    `-fsanitize=fuzzer,address,undefined` for coverage-guided runs.
- **What the harness asserts:**
  - nothing traps;
  - `send` is never returned twice for one quote;
  - the reported balance never goes negative;
  - every state reached is in the table.
- **Corpus.** Keep it in `metareal/fuzz/wallet/`. Every crash becomes a
  `coax/tests/fail/` fixture or a `#test` before the fix lands.
- **Second target.** The `payment.v1` decoder once imported, the same way
  `tests/proto/runtime.coax` covers the home profile:
  - every two-byte input;
  - truncations;
  - overflow and zigzag edges;
  - unknown fields.
- **Move it** to `coax fuzz` when that lands.

We don't fuzz the SDK itself. It is Breez's code, and their CI covers it.
We fuzz what we feed into it and what we do with its answers.

## 4. Keys

- BIP-39 vectors (the published English list with an empty passphrase) and
  BIP-32 vectors, checked in the language that derives the keys.
- NIP-06's published vector, giving the same npub from the same words.
- A recorded Spark identity public key for a fixed test mnemonic on mainnet
  (account 1) and on regtest (account 0). Write it once from the SDK and
  keep it, so an SDK upgrade that changes derivation fails at once.
- No real words in any fixture.

## 5. Swift tests

- **The fake.** `SparkHost` talks to a protocol with the SDK calls it uses.
  The test double returns every result and error the SDK can:
  - success with and without a preimage;
  - fee changed since the quote;
  - insufficient funds;
  - network down;
  - timeout after send;
  - duplicate idempotency key;
  - and the rest.
- **What each test asserts:**
  - the Coax state that results;
  - what the screen shows;
  - that no second `sendPayment` call is made.
- **Keychain:**
  - an existing `seed-v1` item is read and never overwritten;
  - a missing one is created from the identity's words, never from
    `Seed::generate`;
  - nothing logs the words or the entropy.
- **Lifecycle.** Background during `sending`, then relaunch: the state is
  `unknown`, then resolved by `getPayment` under the same key.

## 6. Local regtest

Spark's hosted regtest has no Lightning. Breez's repository has a full
local network in `regtest/local/`:

- bitcoind;
- three Spark operators;
- an SSP with its own Lightning node;
- an LNURL server;
- a data-sync service;
- a mempool explorer.

A wallet connects with the SDK's regtest config plus five changes from its
README: the Spark config file, the chain service at
`http://127.0.0.1:8090/api`, the deposit claim fee, `lnurlDomain`, and the
sync server URL.

- **Run it on a cloud host, not the owner's Mac.** It is third-party
  `docker compose` and Nix code: untrusted, and heavy. Use the GCE pool
  (`openagents cloud`) with the clone pinned to the SDK tag we ship.
- **Run it in full** before each SDK upgrade and before release:
  - receive and send BOLT11;
  - Spark address transfers;
  - a deposit, its claim and its refund;
  - a Lightning address;
  - an interrupted send resumed after restart;
  - exporting and importing exit data;
  - one unilateral exit or `recoverFunds`.
- Record the time and fees of each step. The May 2026 failures were slow
  syncs and stuck leaves, so also record sync and history latency under a
  burst of 50 small payments.

## 7. Mainnet

- A throwaway seed on a test device, funded with a few thousand sats.
  Never the owner's wallet.
- Steps:
  - receive a BOLT11 from an outside wallet;
  - pay a BOLT11 to an outside wallet;
  - a Lightning address both ways;
  - a small on-chain deposit and its claim;
  - the app closed while a payment arrives;
  - restore on a second device from the words.
- Check the trust note and the backup warning read correctly.
- Write the results, with times and fees, in the release notes for that
  build.

## What these tests don't prove

- That the operators delete their key shares. Nothing can; it's the trust
  model.
- That an exit succeeds in a real outage. Local regtest exercises the code
  path, not Lightspark's behaviour.
- That the SDK is correct. We pin it, read its release notes, and run the
  local regtest suite before each upgrade.
- Coverage and mutation scores show the checks are sharp, not that the
  policy is the right policy. The policy is reviewed by people.
