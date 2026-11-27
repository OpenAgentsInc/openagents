# Book delivery routes — 2026-11-26

Fresh qualification of the executable CPU book routes and the full Coax book
example suite against the current `coax0 0.52.0` compiler. These are bounded
native teaching and workflow exits, not whole-book, GPU, live-network, or
production qualification.

## Environment & Identity

| Field | Observed value |
| --- | --- |
| Observed Git HEAD | `da6ab05135e414fb2736a1e9f9744463548288b8` |
| Compiler | `coax0 0.52.0` (`sha256: 8a8bcc05b49468bf40c17400ab7eb677e41bfe24549617d60f3ca9a54fbfd7aa`) |
| Native C compiler | `Apple clang version 21.0.0 (clang-2100.1.1.101)` (`arm64-apple-darwin25.4.0`, `sha256: 12bed4523661307059b879b9b54e77a73176e9d27d27a0e40363271d8f0668ba`) |
| Swift compiler (`ch20_interop`) | `Apple Swift version 6.3.3 (swiftlang-6.3.3.1.3 clang-2100.1.1.101)` |
| Python | `3.15.1` |
| Host target | `Darwin 25.4.0` (`arm64`, Apple Silicon / macOS) |
| Combined index | [index.json](index.json) |

## Route Qualification Summary

All **8 non-Coder** catalog routes in [routes.json](../../books/routes.json)
passed fresh execution (`53/53` step/mode runs, `0` failures, `0` transcript
drift) and then passed selective retention via `--previous` (`53/53` retained,
`0` example re-executions). The 9th route (`coder-harness-memory`) was
intentionally excluded from this batch and is verified separately by the
parallel Coder book sync commit.

| Route | Book | Status | Fresh step/mode runs | Fresh wall (s) | Retained runs (0 re-executed) | Retention wall (s) |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| [coax-current-ffi](coax-current-ffi.json) | `coax` | PASS | 7 / 7 | 4.341 | 7 / 7 | 0.898 |
| [aethyr-core-replay](aethyr-core-replay.json) | `metaverse-engine` | PASS | 6 / 6 | 4.510 | 6 / 6 | 0.761 |
| [psionic-public-models](psionic-public-models.json) | `psionic` | PASS | 6 / 6 | 23.631 | 6 / 6 | 0.909 |
| [nostr-event-client](nostr-event-client.json) | `nostr` | PASS | 4 / 4 | 14.296 | 4 / 4 | 0.741 |
| [executor-network-flywheel](executor-network-flywheel.json) | `psionic` | PASS | 10 / 10 | 5.568 | 10 / 10 | 1.242 |
| [bitcoin-crypto-settlement](bitcoin-crypto-settlement.json) | `bitcoin` | PASS | 8 / 8 | 8.417 | 8 / 8 | 1.022 |
| [everglade-world-srd-dragon](everglade-world-srd-dragon.json) | `everglade` | PASS | 6 / 6 | 5.791 | 6 / 6 | 1.106 |
| [openagents-app-economy](openagents-app-economy.json) | `openagents` | PASS | 6 / 6 | 11.496 | 6 / 6 | 0.921 |
| `coder-harness-memory` | `coder` | Deferred (Coder sync) | — | — | — | — |
| **Total (executed routes)** | **8 / 9** | **PASS (8 / 8)** | **53 / 53** | **78.050** | **53 / 53** | **7.600** |

Wall times include dependency discovery and tool-identity probes on this host;
they are observations, not performance guarantees. Each JSON record binds the
exact step `recipe_sha256`, full source-closure `inputs` SHA-256 map and
`closure_sha256`, expected fixture SHA-256, observed stdout/stderr SHA-256 and
byte counts, tool and target identities, and the raw `scripts/book_routes.py`
receipt SHA-256 and `integrity_sha256` for both fresh and retained runs.

Per `#13840` (`docs/verification/.gitignore` and `coax/web/prepush.coax`
`pp_verif_bloat`), raw `.assets/*.stdout` / `.stderr` files are not tracked in
git; committed JSON records carry no `/tmp/...` paths, local `$HOME`/`$PATH`
dumps, or secrets.

## Coax Book Examples (`coax/book/check-examples.sh`)

| Check | Command | Result | Scope | Wall (s) |
| --- | --- | --- | --- | ---: |
| Coax book examples | `timeout 300 bash coax/book/check-examples.sh` | **PASS** (`exit 0`, **119 / 119** units) | All `.coax` examples (debug + release + `#test`/`#forall`), `ch17_glow` checks & Metal emission, `ch27_frontend`, `ch20_interop` (C + Swift), `ch15_requests.mutation`, `ch24_workflow` native host, and `check-structure.py` | 78.201 |

## Runner & Catalog Checks

- `python3 -m unittest scripts/tests/test_book_routes.py`: **PASS** (`27 / 27` tests).
- `python3 scripts/book_routes.py --check`: **PASS** (`9 profiles, 30 steps; metadata valid`).
- `bash scripts/check-books.sh --check`: **PASS** (all 11 sub-checks / all 9 books: `coax` registry & structure, `psionic`, `metaverse-engine`, `nostr`, `coder`, `bitcoin`, `openagents`, `everglade`, `claude-code`, and `book_routes.py --check`; verified after rebasing onto `7ec3ac4d64` following the `openagents/book/chapters.json` and Coder book `2d323aed9c` sync commits).

## Scope & Honesty Boundaries

- The `coax-current-ffi` workflow step executes C debug and optimized builds
  against a temporary local file and committed protobuf descriptor.
- `psionic-public-models` trains and resumes its tiny 76-parameter synthetic
  module; frozen MNIST experiments are not repeated.
- `nostr-event-client`, `bitcoin-crypto-settlement`, `executor-network-flywheel`,
  and `openagents-app-economy` use committed public test vectors and pure-Coax
  admission/replay logic with no live sockets, relays, wallets, or mainnet funds.
- `aethyr-core-replay` and `everglade-world-srd-dragon` exercise offline CPU
  simulation, replay, terrain seams, and SRD/dragon rules with no browser or
  WebGPU window.
- [Delivery instructions](../../books/DELIVERY.md) document clean-checkout use,
  selective retention, and publication receipts. This verification performed no
  GitBook publication or authenticated live check, and makes no claim of live
  body parity.
- Observed Git revision is HEAD at execution, with per-file SHA-256 hashes
  identifying the actual checked-out inputs. Receipts are trusted maintenance
  records, not cryptographic signatures or execution attestation. No original
  model weights, user keys, wallet, production relay, or visible browser were
  accessed.
