# Project page tools in Coax

2026-11-01. The TypeScript under `coax/web/project/` is gone. Every tool it held
now runs as an `openagents project` verb, written in Coax, and the site build,
`coax/web/release.sh` and `coax/web/deploy.sh` call those verbs.

## How parity was checked

For each tool, you run the old TypeScript once, read-only, against the same
input as the Coax verb, then compare the outputs with `cmp` or `diff`. Where the
outputs differ, the table below gives the difference and why it is acceptable.
Generator comments and timestamps differ in every pair and are not listed.

## Port table

| TypeScript removed | Coax replacement | Commit | Parity evidence |
| --- | --- | --- | --- |
| `snapshot.ts`, `tables.ts`, `model.ts` | `openagents project snapshot` (`apps/openagents-cli/project-page.coax`) | ce382ee763 | Each project's `work.coax` matches the TypeScript output. Sorting uses a byte compare instead of `localeCompare`, and numbers keep their raw text. Today's data gives the same order both ways. |
| `list.ts` | `openagents project index` | ce382ee763 | The project list and the Markdown pages match. |
| `baked.ts` | `openagents project bake [--list]` | ce382ee763 | `baked.coax` and each `commits.coax` match. |
| `landing.ts`, `landing-reader.ts` | `openagents project landing` (`project-landing.coax`) | f66fc0b2d7 | `newland-data.coax`, the targets and the seed match. A failed relay read writes nothing (checked by hand). |
| `pylons.ts` | the pylon tables in `project landing` | f66fc0b2d7 | The rows match. The age column differs: each tool computes it from its own clock. |
| `review.ts` (`report`, `keygen`) | `openagents project review report` and `review keygen` (`project-review.coax`, `review-rules.coax`) | 45dbf1f02f | The report lists the same commits and reviews. The Coax report caps its inputs and refuses anything over the caps; the TypeScript had no caps. `keygen` writes a mode 600 key and prints only the public key. |
| `publish-data.sh` | `openagents project publish-data [SLUG...] [--dry-run]` (`project-data.coax`) | c1a65a6a6a | The `llms-full.txt` splice gives the same bytes as the old bun snippet on both buckets' files (746,919 bytes, 3 sections changed). |
| `adopt.ts` | `openagents project adopt SLUG` | c1a65a6a6a | The request path, content type, body and NIP-98 event (kind, pubkey, content, `u`, `method`, `payload` tags) match. Coax adds a `nonce` tag. An unreachable host exits 1, and a bad key exits 2. |
| `capture.ts` | `openagents project capture [--tables]` | c1a65a6a6a | Back-to-back captures give the same 390 frames (567,708 bytes) and the same tables. Only the relay's AUTH challenge differs. |
| `details.ts`, `links.ts`, `relay.ts`, `history-fixture.ts`, `details-fixture.ts`, `links-fixture.ts` | the Coax fold in `apps/openagents-web/project.coax`, checked against frozen fixtures | 7453cdaf40 | `coax/tests/nostr/fixtures/project_details.json` and `project_history.json` hold the TypeScript output, frozen. `project_details.py` and `project_history.py` pass on native sanitizers and Wasm. |
| `nip19.test.ts` | `coax/web/project/names-check.coax` | 7453cdaf40 | 828 nostr-tools names (1 key, 105 event ids, 3 coordinates) are frozen in `coax/lib/nostr/fixtures/coax_relay_names.coax`. All 20 checks pass. |
| `nostr-plan.test.ts` | `openagents project plan-rules` (`project-plan-rules.coax`) | 7453cdaf40 | 785 checks over 321 items and 150 coax-nostr tasks pass. A mutated rule makes it fail. |
| `live-catchup.test.ts` | `apps/openagents-web/project-catchup.coax`, a native suite linked with `project-capacity.coax` | 3ad0f7e9b8 | All 10 checks pass in about 10 seconds. A mutation run made 3 of them fail. |
| `review.test.ts` | `apps/openagents-cli/review-rules.coax` | 45dbf1f02f | 416 checks pass. |
| `coax/tests/nostr/nip34_parity.ts` | `coax/tests/nostr/fixtures/nip34_dump.jsonl`, compared by the `cmp` command in the `nip34_dump.coax` header | 7453cdaf40 | The TypeScript reported 237 matches and 0 differences before it was frozen. |
| `build-reader.sh`, `snapshot.ts` entry points, `capture.ts` and `adopt.ts` scripts | the CLI verbs above | 86f453f412 | Every caller now calls the CLI. |

## Tests not ported

These tests checked behavior that a Coax test cannot reach, or code that no
longer exists:

- **The `nip19.test.ts` browser layout check.** `project-frames` checks pf_5 and
  pf_6 cover the drawn layout.
- **The WebGPU browser render, the `COAX_LIVE_REAL` public read and the
  frozen-path environment tests in `live-catchup.test.ts`.** They needed headless
  Chrome or the live relay. The scenarios that need neither run in
  `project-catchup.coax`. See `coax/web/project/LIVE-CATCHUP.md` for the dated
  public capture.
- **The `LandingReader` Wasm capacity test.** It tested the TypeScript wrapper.
  `project-capacity.coax` and the landing Coax tests (`landing-nostr`,
  `landing-browser`, `landing-fold`) cover the capacity rules.
- **`coax/web/tests/newland_reader.test.ts`.** It tested the TypeScript
  `landing-reader` and `pylons` wrappers, which are deleted.
- **`details.test.ts`, `links.test.ts`, `relay.test.ts`.** They tested deleted
  TypeScript. The frozen fixtures above keep their expected output.

## What stays

`coax/web/project/package.json` and `bun.lock` stay. The `node_modules` they
install hold nostr-tools, which these checks use as an independent oracle:
`nostr_bech32.py`, the NIP-44 and NIP-59 fixtures, `relay_session.py`,
`pylon_e2e.py` and `golden-check.sh`. No TypeScript source remains in
`coax/web/project/`.
