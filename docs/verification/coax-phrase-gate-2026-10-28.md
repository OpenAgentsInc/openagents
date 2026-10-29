# Coax experimental phrase gate — 2026-10-28

Task `unlock-gate-coax` (#51e6) consumes the internally reviewed [versioned format](../protocol/2026-10-28-phrase-gate-v2.md) shipped in `fa594f441d`. It adds a separate public-fixture/staging reader and writer. Production `gate.ts`, `inject.ts`, real entries, sign-in, mobile and TestFlight are unchanged. This is internal implementation verification, not independent production cryptographic approval.

## Implemented boundary

Coax owns phrase normalization, fixed framing and work bounds, trusted context derivation, original cryptogram projection, PBKDF2 → HKDF → ChaCha20/HMAC, MAC-before-decrypt, canonical locator admission, progress and cancellation, and cleanup-gated result publication. One dedicated worker/Wasm closure performs the complete v2 operation. No master key, PRK or key halves are returned to the UI. Only an admitted locator or sealed entry, progress or refusal is transported.

The payload cipher adapter requires the output view to have exactly the input width; a frame cannot offer its tag or adjacent bytes as generic cipher output capacity. The C face has fixed disjoint input/private/output storage. A single borrowed `PgScratch` partition defines the 12 registered regions of the 22,128-byte private workspace; checks verify each region's exact count and first/last byte against independent offsets. The worker erases copied phrase/context/entry/entropy inputs after initialization and identified private work before Coax admits a terminal result. The UI receives a copied terminal result only after successful disposal. A trap, failed erasure, failed cleanup observation or unverified disposal permanently retires the worker. Further initialization and requests refuse; the client settles pending requests and terminates a fatal worker. Initialization and pre-run disposal failures follow the same rule.

JS handles worker scheduling, scalar/buffer transport and OS entropy only. An absent, throwing, partially writing or wrong-return entropy provider refuses without plaintext/entry output. No PRNG, clock fallback or deterministic product entropy exists. Test entropy is injected only in fixture adapters. Worker inactivity is bounded to 30 seconds by default (trusted test configuration permits 100–30,000 ms); termination refuses and does not acknowledge successful erasure.

The explicit client snapshots mutable trusted context/relay/recipient and raw-entry inputs, requires an exact successful Coax limits reply before any legacy attempt, serializes supersession, and preserves raw v2 JSON entry bytes. The legacy adapter is explicitly supplied: the fixture build derives a side-effect-free exact prefix of the frozen legacy source and pins its full-source hash against the reviewed fixture. Importing the client with a `coaxial-unlocks` element present does not arm the production terminal or load a presentation module.

Legacy remains first, with its original exact-terminal normalization, AES-GCM bytes and cheap truncated check. V2 has a separate ASCII-only profile. Combined candidates are capped at eight. Existing legacy copies continue to permit their cheap offline oracle; a v2 wrapper cannot remove that weakness while the old copy remains available.

## Retained evidence

The scoped compiler is `coax0 0.16.0`, SHA-256 `46d332386114ff213a2083440469332742915ea99b2506a9619730420fda2719`. Source hashes and detailed observations are retained under [fixture evidence](../../tools/coax-nostr/phrase-gate/evidence/).

- Coax loaded closure: 146 passing check groups, including invalid scalar ABI spans, context/entry shape, duplicate keys, exact capacities, scratch separation, corrupted metadata, original salt/nonce stability, cancellation and cleanup refusal. The public completed-PBKDF2 state model is confined to Coax unit checks; no worker shortcut exists.
- Final whole loaded-graph authority mutation: **4,315/4,315 killed, zero survivors and zero exemptions**. The graph was frozen throughout the run; source/compiler fingerprints and the retained final log identify it.
- Native Clang ASan/UBSan: 82 full-round/admission records in each checked/release profile, plus cancellation, explicit abort, invalid chunk budgets, uncertain cleanup and identified-buffer observations. Leak checking was not enabled; no whole-process erasure assertion is made.
- Actual emitted Wasm: the same 82 records in each checked/release profile and lifecycle checks. Successful attempts execute all 210,000 rounds. No unexpected host imports are stubbed into success.
- Actual Chrome: 49 cases in each checked/release profile. These include real OS-randomized seal/open, wrong phrase and malformed entries, cooperative cancellation, supersession, mutable-context snapshots, missing modules, dead/stalled workers, malformed counts replies, init/pre-run/terminal cleanup failure and traps followed by attempted reuse, all four entropy-provider failure forms, and byte-exact artifact rollback.
- Independent Node/OpenSSL composition: complete 4,096/4,097-byte packets, magic/header truncations, all 32 MAC-byte tamper positions and semantic/canonical failures augment the pinned original corpus. Newly OS-randomized browser packets are opened by that independent oracle. The complete 4,096-byte wire is 4,202 bytes and its base64 is 5,604 characters; it passes framing/MAC and refuses its invalid locator after decrypt. The complete 4,097-byte packet refuses before stretching/decryption. Every MAC position is exercised natively and in Wasm; Coax composition checks sample both ends while the shared full-byte comparison retains its own checks.
- Production `gate.ts` and `inject.ts` are byte-for-byte compared with `fa594f441d`; the original public legacy entry is never re-encrypted or rewritten.

## Staging and rollback

The isolated HTTPS service is [coax-phrase-gate-staging](https://coax-phrase-gate-staging-ezxz4mgdsq-uc.a.run.app). It contains public fixtures only and has no event fetch, project key, relay connection, presentation loader or production injection. Only this new fixture service has a public-invoker binding; existing IAM policies are unchanged.

Its root selects an explicit baseline or experimental artifact. The baseline contains the unchanged legacy reader, original entry and hashed manifest and does not require Wasm. The experimental artifact contains a separate dual reader and normal worker. Normal deployable modules exclude all fault/observation exports and fixture phrase fallbacks; controlled hooks exist only in excluded `build/` artifacts.

The local browser matrix activates a complete baseline artifact, activates the experimental artifact, then restores the exact baseline reader/entry/manifest and verifies every asset hash and the old phrase. The separate HTTPS matrix does the same through actual Cloud Run revisions, using the visible button and retained manifests. The declared 30-day minimum rollback window remains a prerequisite for any future real migration. This test does not begin or automatically complete a production cutover window.

## Limits

The locator must exactly match caller-supplied relay and recipient policy and exact canonical three-string JSON bytes. Its narrower URI profile accepts `wss` with DNS authorities through the shared bounded URI parser. This is not DNS/TLS/SSRF, Nostr signature, schema or remote authorization proof. Neither the entry's own context nor JS URL normalization grants authority.

Identified-buffer observations do not establish comprehensive erasure of compiler stack temporaries, registers, immutable JS strings, JIT/GC, OS or debugger copies. The composition is the reviewed experimental format, not RFC 8439 AEAD or NIP-44. A production migration requires separate review and owner acceptance of those remaining boundaries. No compiler-wide, iOS or unrelated engine suites were rerun.
