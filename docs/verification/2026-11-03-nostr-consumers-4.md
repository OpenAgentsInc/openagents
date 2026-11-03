# Nostr consumers, fourth pass: the runner's transport and key, the opener's NIP-44 in Coax, the opener in the browser (2026-11-03)

Issue #12630, after `8ac68215ed`
([first record](2026-11-03-nostr-consumers.md),
[second](2026-11-03-nostr-consumers-2.md),
[third](2026-11-03-nostr-consumers-3.md)). Test keys only, on coderos-4080
(Linux x86_64, RTX 4080), everything on 127.0.0.1. Nothing was deployed, and
no production signer, key, secret or payment was touched. Every browser run
was headless and muted, on a temporary profile.

## The compiler: built from this tree

`COAX_MUTATION=0 coax/build.sh` bootstrapped this tree's own fingerprint,
`50a0fcdf53c24da1…` (the third pass stopped the same bootstrap at the
15-minute limit and used the release build `727901f6`). It finished in about
7 minutes, under the limit, and its checked build is in the release cache.
Every check and mutation run below used it, except where a line says
`727901f6`. The consumer checks on it:

| Check | Result |
| --- | --- |
| `coax test coax/lib/nostr/consumers.coax` (before this pass's change) | 9 of 9 |
| `coax test coax/web/unlock/unlock-nostr.coax` (with this pass's NIP-44) | 500 of 500 |
| `coax test coax/web/agent/runner-sign.coax` | 409 of 409 |
| `coax test apps/openagents-web/project-verify.coax` | 278 of 278 |
| `coax test coax/lib/nostr/weblogin.coax` | 436 of 436 |
| `coax test coax/web/api/nip98.coax` | 199 of 199 |

## The hosted runner: what carries its requests

The runner (`coax/web/agent/runner.c`) sends every request to oa-api with
libcurl (`curl_easy_init`, `api()`), NIP-98 signed, to
`OA_API_ORIGIN` (`https://openagents.com` in production, `deploy.sh`).

**End to end, with the Coax signer.** A new Coax tool,
`coax/web/agent/runner-transport.coax`, is a stand-in oa-api on 127.0.0.1. It
starts the real `oa-agent`, wakes it with `POST /wake`, hands out one turn
from `next` and then none, and checks every request's `Authorization` with
the API's own `nip98_check_coax` (`coax/web/api/nip98.coax`, which oa-api
runs as `nip98.generated.c`) for its exact URL, method and body. The signer
must be the test key: SHA-256 of a fixed public label, as in
`runner_sign_check.c`. The engine was the runner checks' stand-in,
`coax/web/agent/test-engine.py`, so no model ran. Its own checks: 296 of 296.

`oa-agent` was built as `deploy.sh` builds it: `runner-sign.coax` to C with
`--run-steps 250000000`, then `gcc` with `runner.c` and libsecp256k1 0.7.1,
curl 8.21.0 and OpenSSL 3.6.3 from Nix.

| Signer | Requests | API verdict | Signer key | Turn | One base64 byte changed | Same header, another URL |
| --- | --- | --- | --- | --- | --- | --- |
| `OA_RUNNER_SIGNER=coax` | 6 in 36 ms: `next` ×2, `state`, `status`, `event` (2 events), `end` | 6 of 6 ok | `1a9a672c…7712` | done | refused | refused (u tag) |
| unset (libsecp256k1, the default) | the same 6 in 28 ms | 6 of 6 ok | `1a9a672c…7712` | done | refused | refused (u tag) |

The engine reported `key:yes runner:no`. It had the model's key in its
environment and not the runner's.

**TLS.** A throwaway EC CA and a certificate for 127.0.0.1 were made with
`openssl`, and `openssl s_server -brief` listened on loopback. The runner ran
with `OA_RUNNER_SIGNER=coax` and `OA_API_ORIGIN=https://127.0.0.1:PORT`.

- **The CA not trusted.** The handshake reached TLS 1.3
  (TLS_AES_256_GCM_SHA384), and libcurl ended the connection when the
  certificate didn't verify. No request arrived, on the first try or the retry.
- **The CA trusted** (`SSL_CERT_FILE`, which this libcurl honours). TLS 1.3,
  TLS_AES_256_GCM_SHA384, and one request arrived:
  `POST /api/private/transport-check/next`. The tool checked the decrypted
  capture (`--capture FILE --origin https://127.0.0.1:PORT`): the API's check
  passed it, signed by the test key. Checked for another origin, it was
  refused: "the u tag is not this URL".

So libcurl over OpenSSL carries the runner's requests, with TLS 1.3 and a
verified certificate. In the image these are Debian bookworm's `libcurl4` and
`libssl3`, with the system CA store (`Dockerfile`). The leg from the API to
openagents.com (TLS 1.3 and h2 through Google Frontend) is in the second
record. The registry now names this record for the runner's transport axis.

## The hosted runner: its key in production (from code and docs)

No secret was read: the key below is the test key.

- **Where it comes from.** `deploy.sh` sets
  `--set-secrets OA_RUNNER_KEY=oa-agent-runner-key:latest`. Cloud Run reads
  the key from Secret Manager when an instance starts and puts it in the
  environment of `oa-agent`, the container's only process (`Dockerfile` `CMD`,
  `USER node`).
- **What the runner does with it.** `load_key` in `runner.c` parses the hex
  into a local array, keeps it as libsecp256k1's keypair (or, under `coax`,
  in `coax_secret`), wipes the local copy and calls
  `unsetenv("OA_RUNNER_KEY")`.
- **Child processes.** None gets the key in its environment:
  - The engine gets a fixed list of variables (`runner.c`).
  - Git gets `git_envp`.
  - The chat engine gets its own list (`chat-runner.c`).
  - `oa-seat`, and the Chromium it starts, inherits the environment after the
    `unsetenv`; its seat key arrives on stdin.
- **Still readable after `unsetenv`.** With the test key,
  `/proc/<pid>/environ` of the running `oa-agent` still held
  `OA_RUNNER_KEY=` and the whole key, under both signers. `unsetenv` removes
  the pointer, but the bytes stay in the process's initial environment block,
  which any process running as the same user can read. Forks of the runner
  (one turn loop per project) carry the same block until they exec.
  `ANTHROPIC_API_KEY` and `OA_CHAT_SEAL_KEY` reach the runner the same way and
  are only `unsetenv`'d; this run checked only the runner's key.

The fix is a design choice between the delivery options, so it's added to
NEEDS_OWNER.md (Nostr book follow-ups, item 7) and not built. The registry
keeps the runner's key storage as plaintext.

## The unlock opener's NIP-44 in Coax

`coax/web/unlock/unlock-nostr.coax` (`#authority`) now also has:

- `unlock_conversation_key`: `nip44-key.coax` (ECDH, then HKDF-extract). It
  wipes its scratch on every path.
- `unlock_encrypt` and `unlock_decrypt`: `nip44.coax` at its 2-byte-prefix
  profile, `NIP44_COMPAT` (1 to 65,535 bytes), which is what nostr-effect and
  NIP-07 signers read. A payload opens only when its MAC holds, and a refusal
  returns the negative NIP-44 code.

`unlock-nostr-face.c` exposes them with fixed buffers: `un_conversation_key`,
`un_encrypt`, `un_decrypt`, and `un_ck` and `un_nonce`. `un_wipe` now also
clears the conversation key, the nonce, the NIP-44 work and the output after
each call.

`coax/web/unlock/lib.ts` adds `conversationKey`, `encrypt` and `decrypt` to
its `Nostr` interface: nostr-effect's in `effectNostr`, the module's in
`coaxNostr`.

- The opener's `localOpener(...).decrypt` goes through `nostr`, so with
  `COAXIAL_UNLOCK_SIGNER=coax` the wrap, the seal and the artifact are opened
  in Coax.
- `sealDeck` makes the artifact's, the seal's and the wrap's payloads and
  signatures through `nostr` too.
- Unchanged: the rumor's id hash and the throwaway key (nostr-effect), a
  NIP-07 signer's own NIP-44, and `publish`'s AUTH.
- Unset, everything is nostr-effect as before: the rollback. The release
  module is 174,705 bytes (it was 151,305).

| Check | Result |
| --- | --- |
| `coax test coax/web/unlock/unlock-nostr.coax` | 500 of 500. Its 3 new checks: test keys 1 and 2 give the vector's conversation key both ways, and an off-curve peer, a zero secret and short views are refused; the vector's payload for "a" with nonce 1 is made byte for byte and opened, and one changed ciphertext character or another key is refused on the MAC with nothing written, as is a `#` version; 65,536 bytes is refused as capacity, as is an 87,473-character payload, and short buffers are refused |
| `coax mutate … --root-only` | 38 of 38 killed (33 before; 0 survivors) |
| `bun test` (`coax/web/unlock`), with the module built by `coax/web/wasm.sh` | 23 of 23; the 6 new ones are below |

The 6 new `bun test` checks run against the upstream vectors
(`coax/tests/vectors/upstream/nip44/nip44.vectors.json`, paulmillr/nip44) and
nostr-effect, with test keys:

- **Conversation keys.** All 35 valid `get_conversation_key` rows match.
  All 8 invalid rows are refused, as nostr-effect refuses them.
- **Payloads.** The 10 `encrypt_decrypt` rows are made byte for byte with
  their nonces and opened; nostr-effect opens Coax's payloads. The 3 long rows
  match their plaintext and payload SHA-256s.
- **Refusals.** The 12 invalid payloads are refused by both, with Coax's
  reasons in order: version, version, base64, MAC, MAC, padding ×3,
  payload ×4. The 4 invalid lengths (0, 65,536, 100,000 and 10,000,000) are
  refused by both, and 65,535 bytes make an 87,472-character payload.
- **Round trips.** Payloads of 1, 32, 33 and 65,535 bytes, and Unicode,
  make the round trip both ways between Coax and nostr-effect, with a fresh
  nonce each time. One changed character, or another key's conversation, is
  refused on the MAC.
- **Decks.** A deck sealed through Coax opens with nostr-effect, and one
  sealed by nostr-effect opens through Coax. All three openings (the wrap,
  the seal and the artifact) and their conversation keys are Coax's.
- **Re-signed tampering.** Each of these is refused on the MAC, though every
  signature on it is valid, and the untouched layers open:
  - a wrap whose ciphertext was changed by one character and signed again by
    its throwaway key;
  - a seal changed the same way and signed again by the author, inside a
    valid wrap;
  - a seal whose rumor was encrypted to another key.

## The opener in the browser, with the Coax module

`coax/web/nostr-consumers-browser.coax` has a new verb, `unlock URL MODE`.
For the run:

- A local Coax relay (`coax/relay/build.sh`, in memory) held one gift wrap.
- `bun seal.ts`, the production sealer (nostr-effect), sealed the wrap from
  test key 24 to test key 25 and published it to the relay, answering the
  relay's NIP-42 AUTH.
- `bun inject.ts` staged a page with `COAXIAL_UNLOCK_SIGNER=coax` and the
  module (`unlock-nostr.12ad68773501.wasm`). Its one entry was the phrase
  `coax opener test`, pointing at the relay, the wrap's id and test key 25.
- `coax/web/serve.sh --static` served the staged page on 127.0.0.1.

The page's index.html was a blank stand-in. The page's Coax UI was stood in
for by a recorder of what the gate and the opener send it (`coaxial.send`);
the codes and the key it would send were raised as its `coaxial` events. The
gate, the opener, `lib.ts` and the Coax module ran as staged. The run counted
each call into the module's `un_*` exports, and kept each socket's and
resource's URL.

Each run raised a wrong phrase, then the phrase, then a wrong key (test key
26), then the key (test key 25).

| Run | What the page was sent | Calls into the Coax module | Result |
| --- | --- | --- | --- |
| `control` | armed, refused, granted, key needed, key wrong, loading, then the deck ("Coax opener") | pubkey 2, sign 1 (the AUTH), verify 2 (wrap, seal), id 1 (artifact), conversation key 3, decrypt 3 (wrap, seal, artifact) | pass |
| `tamper`: one content character of the wrap's EVENT frame changed on its way in | the same through loading, then "not a valid gift wrap"; no deck | pubkey 2, sign 1, verify 1 (refused), id 0, conversation key 0, decrypt 0 | pass |

In both runs, every socket and resource URL was on 127.0.0.1, and the
adapter was `hardware nvidia lovelace` (`--require-hardware`).

The pasted key is kept in this profile's IndexedDB (`coaxial-unlock`,
`keys`) as 64 hex digits in the clear (`coax/web/unlock/page.ts`, `keep`).
The registry had the opener's key storage as unused. It is now plaintext,
with `page.ts` as its source, and it waits on the owner's storage decision
with the others (item 7). The opener's transport is the browser's
WebSocket (`fetchEvent` in `coax/web/unlock/lib.ts`), recorded here, and the
registry names this record for it.

## Registry and report

`coax/lib/nostr/consumers.coax`:

- The hosted runner's and the opener's transport axes name this record.
- The opener's key storage is now plaintext (`page.ts`, `coaxial-unlock`).
- Both records are test-key records.
- Each one's compiler line now names its Coax part.

Checks: 9 of 9 (two new assertions). `coax mutate`: 92 of 92 killed.
Report:
[consumers-2026-11-03-4](../../nostr/book/reports/consumers-2026-11-03-4.md).
**3 of 10** profiles are complete, as before. What still blocks each:

- **The hosted runner:** its production signer (libsecp256k1 by default) and
  its key storage. It was also blocked on transport before this pass.
- **The opener:** its production switch (nostr-effect by default) and its key
  storage. Its transport is now recorded.

## Not done

- **Owner:**
  - Item 7, encrypted key storage, now including the hosted runner's
    delivery and the opener's IndexedDB key.
  - Item 8, the iOS run.
  - Item 9, the opener's switch, which now covers NIP-44 too.
  - The hosted runner's production signer.
- **Engineering found here, not fixed.** `turn.coax` limits the model's file
  tools to the checkout by comparing path strings (`en_inside`) and doesn't
  resolve symlinks. A symlink committed to a project's repository could lead
  the Read tool to `/proc/1/environ`, the runner's environment block that
  still holds the key, or to the mounted deploy keys. This needs its own
  issue.
