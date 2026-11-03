# Nostr consumers, third pass: the pages in the browser, the unlock opener on Coax, project-catchup (2026-11-03)

Issue #12630, after `be6f47f8c6`
([first record](2026-11-03-nostr-consumers.md),
[second](2026-11-03-nostr-consumers-2.md)). Test keys and public events only,
on coderos-4080 (Linux x86_64, RTX 4080). Nothing was deployed, and no
production signer, key or payment changed. Every browser run was headless
and muted, on a temporary profile. Every page was served from this checkout
on 127.0.0.1.

Compiler: the checked release build `727901f6a2be…`, the one the second pass
used. A bootstrap of this tree's own fingerprint (`50a0fcdf…`) was stopped at
the 15-minute limit. `coax test --native-link` is newer than that build, so
the native catch-up suite was linked by hand, as `coax/web/suites.coax` links
it: `coax lib` for the suite and the unit, then `native-check.c`. On this
machine `cc` is not a C compiler, so `gcc` linked it.

## project-catchup: the check that was failing

**Cause.** `80eb4249bd` (2026-11-02, "keep the newest activity rows that fit
instead of failing the snapshot") changed `write_log` in
`apps/openagents-web/project-fold.coax`: a row that doesn't fit the 16 KB log
is left out instead of marking the tables incomplete. The check "table output
exhaustion and a catch-up timeout keep the previous complete view" had sent
a 20 KB note on issue 4, a task nobody works on. That note only ever reached
the log, so after `80eb4249bd` nothing was exhausted: the page went live
with the note left out of the log (`incomplete` 0, state live, 462 events
kept). The second pass saw the same failure at `7f7754750f`, before its own
change.

**Fix (the check, not the page).** The note now goes on issue 2, the task in
progress (claimed by `old-worker`). A working task's latest note goes into
WORK_PLAN_INFO, which holds 16 KB (`INFO_CAP`) and still refuses whole, so
the candidate isn't complete, the view stays as it was, and `incomplete` is 5.
The page's behaviour is unchanged. The comment at the check says why.

| Check | Result |
| --- | --- |
| `project-catchup.coax` (native, with `project-verify.coax`), before | 9 of 10, the failure at line 384 |
| the same, after | 10 of 10 |

## The project page verifies in the browser

`coax/web/nostr-consumers-browser.coax` (new, a Coax tool on `cdp.coax`)
drives `/projects/coax/` in headless Chrome. Before the page loads, it wraps
the page's `WebSocket` so that the socket for `wss://relay.openagents.com`
opens on a local Coax relay instead. In the tampering runs, the wrapper
changes one content byte of one event's frame on its way in. This stands in
for a relay that tampers: the Coax relay verifies everything it stores and
can't serve a forged event. The page's own code, `host.js` and
`project.wasm` (with `project-verify.coax`, `project_frame_verified`) are as
served. The page's facts come from a new read-only export,
`coaxial_field_probe` in `apps/openagents-web/project-face.c`, read through
`coaxial.fieldProbe` in `coax/web/host.js`: the line's state, `forged`, the
events kept, frames heard, why a sync is incomplete, and the catch-up's
place.

- **The relay's events.** `apps/openagents-web/project-live.coax` asks for
  five filters (the project, its issues, and the key's comments, statuses
  and repository). Those filters were paged back through
  relay.openagents.com, read-only with `openagents nostr req` under a
  profile with no key: 3,069 events (capture SHA-256 `7a73c36d63b94635…`).
  `openagents project capture` alone got 384, the relay's first answer per
  subscription. Against that, the page correctly never completed: it keeps
  the ids of the tasks it baked, and 61 of the order's 109 items were
  missing (`incomplete` 4). The tool's `export` wrote every event but one
  held comment. The relay (`coax/relay/build.sh`, `--store-capacity 8192`)
  imported and re-verified all 3,068: digest `e39af19f…` at
  `--now 1794200000`.
- **The page.** `project.wasm` 2,347,961 bytes (dev). The adapter was
  `hardware nvidia lovelace` in every run (`--require-hardware`).

| Run | What the page did | Result |
| --- | --- | --- |
| `control`: every frame as sent | live; 3,068 kept, 0 refused (`forged` 0), 3,512 frames, 32 catch-up pages, `incomplete` 0 | pass |
| `sync`: comment `9f324a27…` with one content byte changed during the catch-up | refused each time it came (`forged` 3 of 3 changed); each refusal ended the sync (`incomplete` 1) and the retry met it again; never live; 129 of 3,068 kept when the window closed | pass |
| `live`: held comment `b3943b71…` published after the page was live (the relay takes writes only from an authenticated key; test key 7 answered its NIP-42 AUTH), changed by one byte on its way in | still live; `forged` 0 → 1; kept 3,068 → 3,068; `incomplete` 0 | pass |

In the native checks, the same comment sent unchanged is kept
(`project-live-checks.coax`, second record). The registry now names this
record for the project page's verify axis.

## Web sign-in in the browser: signing and transport

The same tool opens `/login`
(`apps/openagents-web/private.coax`, `private.wasm` 474,588 bytes, with
`coax/lib/nostr/weblogin.coax`). It holds every `/api/` request at the
network layer (CDP `Fetch`) and answers it itself, so nothing leaves the
machine. `/api/auth/viewer` gets 200 with nothing (no one is signed in), the
login POST gets 401, and anything else gets 404. In a fresh profile the page
makes its own throwaway words account. That account is a test key, and it
goes when the profile is deleted. Neither its words nor its key were printed.

- The page's Wasm signed a NIP-98 login (`weblogin_sign`). `host.js`
  (`fetch(`) carried it: `POST http://127.0.0.1:18800/api/auth/key`, with a
  654-byte `Authorization: Nostr …` and the body `{"kind":"login"}`, after
  two `/api/auth/viewer` asks.
- The API's own check, `nip98_check_coax` (`coax/web/api/nip98.coax`, which
  oa-api runs as `nip98.generated.c`), against the browser's clock:
  **ok**, signer `9d5cc1d3…747c`, the throwaway account's public key.
- The same header with one base64 byte changed was refused ("bad tag
  field"). For another URL it was refused too ("the u tag is not this
  URL").
- Key storage, observed but not changed: the account's 16 bytes of entropy
  are in this profile's IndexedDB (`coaxial`/`kept`) as 32 hex digits, in
  the clear. That is the owner's decision (NEEDS_OWNER.md, Nostr book
  follow-ups, item 7), so web sign-in stays incomplete on key storage only.

Transport here is the browser's `fetch` to this page's own origin. The TLS
leg to openagents.com is in the second record.

## The unlock opener signs and verifies in Coax, behind a switch

- `coax/web/unlock/unlock-nostr.coax` (new, `#authority`) provides
  `unlock_pubkey`, `unlock_sign` (`event_sign.coax`), `unlock_verify` (the id
  is recomputed from the fields, then BIP-340 with `schnorr_fast.coax`) and
  `unlock_id_matches` (for the unsigned NIP-59 rumor).
  `unlock-nostr-face.c` exposes them as Wasm with fixed buffers, and the
  secret is wiped after each call. The release module is 151,305 bytes.
- `coax/web/unlock/lib.ts` routes the opener's public key, AUTH signing,
  wrap and seal verification and rumor id check through `nostr`, which is
  nostr-effect unless `useNostr(coaxNostr(wasm))` installs the Coax module.
  `page.ts` installs it when the gate's config names a module. Without one,
  it is nostr-effect as before. A module that won't load stops the opening,
  with no silent fallback. `inject.ts` ships the module, content-named, only
  with `COAXIAL_UNLOCK_SIGNER=coax`, which `deploy.sh` builds through
  `wasm.sh`. Unset is the rollback. NIP-44 and the sealer (`seal.ts`) stay
  on nostr-effect.

| Check | Result |
| --- | --- |
| `coax test coax/web/unlock/unlock-nostr.coax` | 403 of 403; its own 5: a test key's public key (scalar 1 gives the generator); an AUTH signed with a test key verifies and carries that key, and a changed challenge byte or signature digit is refused; a seal with one changed content byte is refused; another key's signature over the same fields is refused; bad keys, aux, tags, output room and non-events are refused |
| `coax mutate … --root-only` | 33 of 33 killed |
| `bun test` (`coax/web/unlock`) | 17 of 17, 7 of them new: the switch and its rollback; public keys equal nostr-effect's for test keys; Coax-signed AUTH verifies with nostr-effect and the reverse; a tampered content, id, signature, another key's signature or pubkey is refused; a sealed deck opens through the Coax opener with every verdict Coax's (verify 1059, verify 13, id 3188); a tampered wrap, a rewrapped seal with a changed signature, and a resealed artifact whose content changed under its id are each refused |

The registry keeps the opener's sign and verify as foreign: production still
runs nostr-effect. Turning the switch on is an owner action (NEEDS_OWNER.md,
Nostr book follow-ups, item 9).

## Registry and report

`coax/lib/nostr/consumers.coax` names this record for the project page's
verify axis and for web sign-in's sign and transport axes. Web sign-in's
records are now test-key records. Checks: 9 of 9; `coax mutate`: 92 of 92
killed. Report:
[consumers-2026-11-03-3](../../nostr/book/reports/consumers-2026-11-03-3.md).
**3 of 10** profiles are complete (the API's NIP-98 check, the homepage, the
project page).

## Not done

- **Owner:** encrypted key storage (item 7: the CLI, the Pylon, the coder
  host and now web sign-in); the iOS run (item 8); the unlock opener's
  production switch (item 9); the hosted runner's production signer
  (`OA_RUNNER_SIGNER=coax`, second record).
- **Engineering:** the hosted runner's key storage (an env var) and its
  transport record; the unlock opener's NIP-44 in Coax, and a browser run of
  the opener with the Coax module; a browser record of the homepage beyond
  its existing one. The full `project-catchup` and `unlock-nostr` suites ran
  on the release compiler above, not on a bootstrap of this tree.
