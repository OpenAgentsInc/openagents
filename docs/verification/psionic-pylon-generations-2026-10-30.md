# Durable Pylon generations, 2026-10-30

A fresh process previously started its beacon at generation 1. The native
fleet retains a provider's highest generation, so a restarted process needs
an explicitly durable epoch rather than a process-local lifecycle counter.

`psionic/lib/pylon/generation.coax` now owns the public counter contract.
`<identity-state>.generation` is canonical text containing the version,
signing provider's public key and last reserved exact JSON integer. Missing
state starts at 2 to supersede the old executable's generation 1. A restart
reserves the next value, capped at 2^53-1. Invalid text, another provider,
exhaustion or a failed durable-write receipt grants no publishable epoch.

The host supplies an exclusive nonblocking lifetime lock on a separate
`.generation.lock` inode. It creates private temporary files, fsyncs the
contents, renames atomically and fsyncs the parent directory. The Coax
program admits the result before connecting to the relay or writing its
relay-list/beacon. Read errors are distinct from missing files. The doctor
remains read-only. There is no new signing-key interface or wire protocol.

The owner must retain one authoritative state directory per provider.
Deleting or restoring an older counter with the same key defeats durable
monotonicity; cloned identities and storage rollback are outside this claim.

## Checks run

The new authority root passed all 5 checks, including its existing demo-label
dependency, in both ordinary and release profiles. Mutation found one missing
malformed-digit boundary; the new `:` case kills it. Final root-only result:
**118 mutants, 118 killed, 0 survivors**, without equivalence waivers.

```sh
coax test psionic/lib/pylon/generation.coax --library
coax test psionic/lib/pylon/generation.coax --library --release
coax mutate psionic/lib/pylon/generation.coax --library --root-only --jobs 8
psionic/tests/pylon/generation_host.sh
```

The native host harness passed ASan/UBSan on macOS and on the Google Cloud
Ubuntu Linux RTX PRO VM (GCC 13.3). It checks atomic replacement and private
mode, missing versus failed reads, failed writes preserving the old record,
exclusive ownership through fork, lock release, and symlink/FIFO/public-mode
lock refusal. Personal Linux over Tailscale was unreachable during this slice;
the actual Linux check ran on Google Cloud instead.

The changed Pylon C/entropy header were emitted with the existing qualified
Mac Coax compiler. The sanitized Mac Pylon then ran against a local Coax
relay, the previously qualified toy model server and one unchanged native
fleet process:

```sh
python3 -I psionic/tests/pylon/generation_restart.py \
  PYLON RELAY SERVE FLEET OUTPUT_DIRECTORY
```

The same provider advertised generation 2, answered an encrypted remote task,
stopped, restarted with generation 3, and answered another through the **same
fleet process**, retaining its prior provider state. No fallback was configured.
Both answers were the frozen toy decision. A duplicate process with that state,
a corrupt counter and a failed durable commit all exited before any relay
connection. The sanitized Pylon/relay logs were clean. Captured result:
`psionic/fixtures/cloud-pylons/2026-10-30/generation-restart.json`.

The Linux native Pylon also built on Google Cloud using GCC 13.3 and the
existing Nix OpenSSL 3.6.3/glibc runtime closure. Its binary SHA-256 is
`dc17b0930bdc059602ddbc7bc87aed5dbfd57d37631169e5a9c7d0775a3cf48f`.
Coax does every counter decision; the C side only observes and commits files.
No unchanged compiler/model suite was rerun.
