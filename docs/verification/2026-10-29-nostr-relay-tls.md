# wss:// for the native relay client: a TLS socket effect

This is a prerequisite of Psionic chapter 24 and closes item 3 ("wss://") of
[the relay client record](2026-10-29-nostr-relay-client.md). The owner
approved it on 2026-10-29 (AGENTS.md, Rules: the C host may link one TLS
library for the TLS handshake, certificate verification and record
encryption only). No board task covers TLS, so a note is on #7de9
`nostr-relay-state-machine`. Recorded 2026-10-29 by claude-tls.

## What landed

| File | What |
| --- | --- |
| `coax/lib/net/relay_uri.coax` (`#authority`) | The relay URL decision, in Coax. It decides `ws://` or `wss://` (`tls`), the host (for both resolution and certificate verification), the port (explicit, or 80 for ws and 443 for wss) and the request target. Authority syntax is `uri.coax`'s `uri_parse`, which is unchanged because `fi-policy` and pinned evidence use it. `ws://` is parsed as its `wss://` spelling with every range shifted back by one. Other schemes are refused. 4 checks, plus the 8 in uri.coax that it loads |
| `coax/platform/tls-native.{h,c}` | The TLS effect over a `socket-native` handle (the same handle number). It is OpenSSL 3 on Linux. The policy is fixed and has no switch: TLS 1.2 minimum (OpenSSL negotiates 1.3 first), SNI set to the host, `SSL_VERIFY_PEER` against OpenSSL's default verify paths (the system store), `SSL_set1_host` with no partial wildcards, and a second check of the verify result and peer certificate after the handshake. There is no compression and no renegotiation. Reads, writes and the handshake are nonblocking and bounded by timeouts. Errors map to eight codes: EOF, TIMEOUT, ERROR, FULL, UNSUPPORTED, HANDSHAKE, UNTRUSTED (-9), NAME (-10) and VERSION (-11). A TCP close without close_notify is ERROR, not EOF. The Apple branch is a stub that returns `CXTLS_UNSUPPORTED` (see "Missing") |
| `coax/platform/socket-native.{h,c}` | Adds `cxsock_fd(h)` for tls-native.c only (it is not a `#foreign` import). Nothing else changes |
| `coax/platform/tls-env.sh` | Sourced by builds. It sets `CXTLS_CFLAGS` and `CXTLS_LIBS` (see "How OpenSSL is found") |
| `coax/tests/net/tls.sh` | The test: certificates, fixtures, the driver under ASan/UBSan, and the cases below |
| `coax/tests/net/tls_ws.coax`, `tls_test.c` | The test driver. `relay_uri` picks the plain or TLS effect, and `ws.coax` runs the handshake, frames and message assembly (`ws_take`). It is built with `coax c --library` and called from C. Mask keys and the nonce are fixed test bytes |
| `coax/tests/net/tls_fixture.py` | A stdlib TLS endpoint, either a WebSocket echo server or a bare TLS server, optionally capped at TLS 1.1 or 1.2 |
| `coax/tests/net/tls_relay.py` | Runs the relay e2e's `relay_fixture.py` (claude-relay-e2e's file, imported unchanged) behind `ssl.SSLContext.wrap_socket` |

The test-only trust hook is the `CXTLS_TEST_TRUST` compile macro. When it is
defined, the context trusts only the PEM file named by `$CXTLS_TEST_CA`,
instead of the system store. Without the macro the code doesn't exist, and
the variable is ignored. Only `tls.sh` defines it.

## How OpenSSL is found

On Linux, `tls-env.sh` runs `nix build --no-link --print-out-paths
nixpkgs#openssl.dev nixpkgs#openssl.out`. This is the same mechanism
`coax/web/toolchain.sh` uses for the Wasm toolchain (the CUDA host instead
`dlopen`s NVRTC from `CUDA_HOME` or `PSIONIC_NVRTC`, which doesn't apply to a
linked library). It returns the store paths already on this machine
(`/nix/store/27dj4bd2…-openssl-3.6.3-dev` and
`/nix/store/p26kxyf1…-openssl-3.6.3`). These are the same build as the
system `openssl` binary. The binary gets an rpath to that store path. To use
another build, set `OPENSSL_DEV` and `OPENSSL_LIB`, or `CXTLS_CFLAGS` and
`CXTLS_LIBS`. Nothing is installed system-wide.

The repository has no flake or `shell.nix`. A permanent entry would be a
flake `devShells.default = pkgs.mkShell { packages = [ pkgs.openssl.dev
pkgs.pkg-config ]; }` with nixpkgs pinned by `flake.lock`. `tls-env.sh` would
then read `pkg-config --cflags --libs openssl` inside it. That needs an
owner decision on adding a flake, so it is not done here.

## What ran

On 2026-10-29 on Linux 6.18.49 (NixOS), with gcc 15.2.0, OpenSSL 3.6.3
(9 Jun 2026) and Python 3.13 `ssl`. The C was built with
`-fsanitize=address,undefined -fno-sanitize-recover=undefined`,
`ASAN_OPTIONS=detect_leaks=1`. The compiler was coax0 0.23.0, from the main
checkout's `coax/bin/coax`, because the worktree build was still running
under host load ~160. All of the following came from one
`coax/tests/net/tls.sh --public` run:

| Case | Result |
| --- | --- |
| `relay_uri.coax` checks | 12/12 passed (4 its own, 8 uri.coax's) |
| `coax mutate relay_uri.coax --jobs 3 --shard I/3`, merged | 264 mutants, 264 killed, 0 survived, no exemptions |
| Handshake with the test CA | TLS 1.3 (0x0304) |
| WebSocket upgrade and echo, `wss://localhost:P/echo?x=1` | echo matches, TLS 1.3 |
| Server capped at TLS 1.2: upgrade and echo | echo matches, TLS 1.2 (0x0303) |
| Leaf for `other.test`, reached as `localhost` | -10 `CXTLS_NAME` |
| Leaf for `localhost` from a CA that isn't trusted | -9 `CXTLS_UNTRUSTED` |
| Server capped at TLS 1.1 | -11 `CXTLS_VERSION`. Control: `openssl s_client -tls1_1` completes TLS 1.1 against the same server, so the refusal is the client's minimum |
| `ws://` at a TLS port | fails (-3) |
| `https://` URL | -20, refused by `relay_uri` |
| Good server with `CXTLS_TEST_CA` empty, so the system store applies | -9: the system store does not trust the test CA |
| `relay_fixture.py` behind TLS: REQ, then its 3 stored events and EOSE, then our close frame | 3 events. Fixture log: `handshake mask req mask close`, no errors |
| ASan/UBSan | no reports, no leaks |
| Public, read-only `["REQ","coax-tls",{"kinds":[0],"limit":2}]` until EOSE, system trust store | `wss://nos.lol`: 2 events before EOSE. `wss://relay.primal.net`: 2 events (a second run, with a URL argument) |
| Public `wss://relay.damus.io` | -8 `CXTLS_HANDSHAKE`. The network policy on this host blocks it: `curl https://relay.damus.io/` fails with the same "wrong version number" (a non-TLS reply), while `curl https://nos.lol/` works. It was not retried |

When the worktree's own compiler (coax0 0.24.0) finished building, the
checks ran with it once more: `tls.sh` passed every local case, and
mutation was again 264/264 killed over 3 shards.

One earlier run failed the relay case. The fixture logged a
`ConnectionResetError` because the driver closed the TCP socket right after
sending its close frame. The driver now completes the RFC 6455 closing
handshake (`tw_bye` reads until the server's close frame) before it closes
the transport. After the fix, the full `--public` run above and three more
local runs all passed.

Every key and certificate is generated by `openssl` into a `mktemp -d`
directory for each run, valid for one day, and deleted on exit. None is
committed.

`gcc -std=c11 -Wall -Wextra -Werror` is clean for tls-native.c with and
without `-DCXTLS_TEST_TRUST`, and for the Apple stub when compiled with
`-D__APPLE__`. That only proves the stub compiles; it was not built on a
Mac.

## Missing (not claimed)

1. **The Apple branch.** Network.framework (`nw_connection` with TLS
   options) owns its own TCP connection, so it can't wrap a socket-native
   descriptor. It needs its own connect and a bridge from a dispatch queue to
   these blocking calls. Until then, Apple native builds get
   `CXTLS_UNSUPPORTED`, and the driver must fall back to ws:// or fail.
2. **The relay client's real driver** (`relay_e2e.coax`, claude-relay's)
   still calls `cxsock_*` directly. Switching it, or the production Pylon
   driver, to choose the effect from `relay_uri(url).tls` is the wiring step.
   `tls_ws.coax` shows the pattern (`tw_open`, `tw_write`, `tw_fill`).
3. **A permanent Nix entry** (a flake devShell), as described above.
4. **Certificate revocation** (OCSP/CRL) is not checked. OpenSSL doesn't do
   it by default, and NIP relays rarely staple OCSP. Expiry is checked.
5. **`ws_request` sends `Host: name:443`** for wss on the default port, because
   it omits the port only for 80. That is valid HTTP, and nos.lol and primal
   accept it. Omitting the port when it is the scheme's default is a change
   to `ws.coax` (claude-relay's file) and was not made here.
