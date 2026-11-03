# Nostr consumers, second pass: the project page verifies, the runner's Coax signer, transports (2026-11-03)

Issue #12630, after `7f7754750f`
([first record](2026-11-03-nostr-consumers.md)). Test keys and public events
only, on coderos-4080 (Linux x86_64). No production signer, key, payment or
deployment changed. Compiler: the checked release build `727901f6a2be…`
(the build of this tree's own fingerprint was still running).

## The project page verifies what it renders

`apps/openagents-web/project-verify.coax` (new, `#authority`) is the page's
separately compiled unit. It carries the capacity kernel
(`project-capacity.coax`) and the NIP-19 names (`project-names.coax`), and
adds `project_frame_verified`. That function reads one relay frame with
`nostr_message_read`, recomputes the event id with `nostr_event_read`, and
verifies BIP-340 with `nostr_fast_verify_vartime`. `project-live.coax` calls
it through `#foreign` on every EVENT frame, whether from a watch, a catch-up
query or the cross-project asks, before `read_frame` stores anything. A frame
that fails is dropped and counted in `Live.forged`; during sync or a query it
also makes the view incomplete. `trust_relay` is a test-only switch that the
native catch-up suite sets, because its simulated relay sends made-up ids;
the page never sets it.

| Check | Result |
| --- | --- |
| `coax test apps/openagents-web/project-verify.coax` | 256 of 256: a real captured comment verifies; one changed content, id or signature byte is refused; a frame with anything after the event is refused; non-EVENT frames are refused |
| `coax mutate … --root-only` | 15 of 15 killed |
| `project-live-checks.coax` (native, linked with the unit) | 5 of 5. A real comment held back from the transcript and sent with one content byte changed is refused (`forged` 3, the page not dirty). Sent as captured, it is kept. Two unsigned events are refused. The transcript's 237 signed events are all kept (`forged` 0). |
| `project-frames.coax` (native) | 10 of 10 |
| `project-checks.coax --library --steps 2000000000` | 88 of 88 |
| `project-catchup.coax` (native) | 9 of 10. "table output exhaustion and a catch-up timeout keep the previous complete view" fails the same way at `7f7754750f` without this change (checked on a copy of the app with the three files at HEAD), so the failure predates this work. |
| `coax/web/wasm.sh project.coax … project-verify.coax` | builds `project.wasm`, 2,170,778 bytes (dev). `project-names-face.h` now includes the carrier's header `Project-verify.h`. |

There is no browser record yet. The page was served locally (`coax serve`)
and opened in headless, muted Chrome, but WebGPU lost the device under both
the GPU adapter and SwiftShader on this Linux machine, so nothing drew and
nothing observable confirms the in-browser check. The registry leaves the
project page's verify axis without a record.

## Transport: what carries the API's and the pages' events

These are host services: TLS is terminated by Google's front end, in front
of `coax/web/api/lb.sh`'s load balancer for the API; the pages open their
relay socket with the browser's `new WebSocket(` in `coax/web/host.js`.
Read-only observations from this machine:

- `GET https://openagents.com/api/playtest/health`: TLS 1.3
  (`TLS_AES_256_GCM_SHA384`, `X25519MLKEM768`), HTTP/2, a certificate for
  `openagents.com` from Google Trust Services WR3 that verified, `server:
  Google Frontend`, `via: 1.1 google`, 200.
- A WebSocket handshake to `https://relay.openagents.com/` with `Origin:
  https://openagents.com`: TLS 1.3, a certificate for `relay.openagents.com`
  (Google Trust Services WR3) that verified, `101 Switching Protocols`, the
  correct `Sec-WebSocket-Accept` for the RFC 6455 sample key.

These records cover what carries the bytes. They are not a browser run of the
pages.

## The hosted runner signs in Coax, behind a switch

- `coax/web/agent/runner-sign.coax` has `runner_coax_pubkey` and
  `runner_coax_sign` (`event_sign.coax`). Its checks: 400 of 400, including a
  test key's NIP-98 event that verifies and carries its signer's key.
- `runner.c` reads `OA_RUNNER_SIGNER`. With `coax` it signs NIP-98 with Coax.
  Unset, or anything else, keeps libsecp256k1 exactly as before; unsetting it
  is the rollback.
- The identity rule: under `coax` the runner derives the public key with both
  signers and refuses to start if they differ. The secret stays in process
  memory as libsecp256k1's keypair does, and is never logged.
- `deploy.sh` compiles `runner-sign.coax` into the image's `src/`, and the
  Dockerfile links it. The production default is unchanged; switching
  production is an owner action.

Evidence, with a test key (SHA-256 of a fixed label), all local:
`coax/web/agent/runner_sign_check.c` (`runner.c` itself, `load_key` and
`nip98`) signs headers for two URLs. `coax/web/api/nip98_stdin.c` (the API's
`nostr.c` over `nip98.generated.c`, a separate program, as the runner and
oa-api are) checks them:

| Signer | Public key | API verdict | One base64 byte changed | Header for another URL |
| --- | --- | --- | --- | --- |
| libsecp256k1 | `1a9a672c…7712` | 2 of 2 ok | refused (bad id) | refused (u tag) |
| coax | `1a9a672c…7712` | 2 of 2 ok | refused (bad id) | refused (u tag) |

libsecp256k1, curl and OpenSSL came from Nix for this local build.

## Not done in this pass

- **The unlock opener** (`coax/web/unlock/lib.ts`, nostr-effect). Moving its
  signing, verification and NIP-44 to Coax needs a Coax Wasm module the
  TypeScript opener can call, with its own switch and a test against
  `unlock.test.ts`. Not started.
- **Browser runs** of the homepage, the project page and web sign-in in
  headless Chrome: WebGPU doesn't draw headless here (see above). A run needs
  a probe of the page's state that doesn't depend on drawing, or a Mac.
- **iOS**: needs the Mac (NEEDS_OWNER.md, Nostr book follow-ups, item 8).
- **Encrypted key storage** for the CLI, the Pylon and the coder host: a
  design for the owner (item 7), not built.
