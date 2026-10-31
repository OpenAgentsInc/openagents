# Nix TLS packaging for the warm Elephant fleet

The private Elephant fleet now serves **100%** from
`oa-mesh-elephant-m0nix-20261031a`, immutable digest
`sha256:d3a7f0a62e034b5b8baf48fd06106a2894d96d727829839cb03bf9554d938dcb`.
It uses Nix-built OpenSSL and a complete Nix runtime closure in a scratch image.
The previously qualified M0 emitted C, start script, host, native sources and
explicit compilation flags were retained. Normal Nix fixup strips installed
debug sections before closure scanning; the compiler and development headers
are excluded from the runtime.

The [first warm rollout](fleet-warm-rollout-2026-10-30.md) reused the existing
Debian APT container recipe. That was a packaging mistake: it did not satisfy
AGENTS' Linux TLS requirement. Its observations remain an intermediate phase;
they have not been overwritten. The replacement was qualified at zero traffic
and independently reviewed before promoting the same digest.

## Paired public observations

One owned normal demo identity made each phase's exact guard cases 0–2, with
one active request and six-second start spacing. The final calls used the same
key as the earlier phases and separate guide/browser pilot. No identity
rotation, quota exemption, model substitution or retry was used.

| Phase | Case 0 | Case 1 | Case 2 | Median |
| --- | ---: | ---: | ---: | ---: |
| Original fleet | 5901 ms | 5830 ms | 5744 ms | 5830 ms |
| Intermediate APT M0 | 939 ms | 887 ms | 937 ms | 937 ms |
| Final Nix M0 | 770 ms | 736 ms | 631 ms | 736 ms |

The final median is 5094 ms below the original sample. The three final calls
returned HTTP 200, the original Elephant checkpoint/provider and a valid guard
contract. These paired contexts measure the warm service path. The CLI timing
includes helper startup, NIP-98 signing, HTTPS, API, fleet, relay, Pylon and
process completion. The separate small samples do not establish an additional
performance effect from changing TLS packaging.

[Raw final observations](data/fleet-nix-rollout-2026-10-30/public-final.jsonl)
and the [Coax replay](data/fleet-nix-rollout-2026-10-30/public-final-summary.json)
retain all three calls. The replay explicitly reports 3 of 16 eligible editorial
contexts observed; this deployment slice is not a completed quality cohort or
ambient-gameplay promotion.

## Inputs and runtime proof

The [recipe](../../psionic/platform/fleet-container/default.nix) imports the
[fixed pin](../../psionic/platform/fleet-container/nixpkgs-pin.json): Nixpkgs
`885ba4d038f10c3d17d7a92e8c3cc185b99c9158`, immutable release archive and verified
unpacked NAR hash
`049l6x2bgjbq6j96as6l5ck0gcy0w92p7f8pyj1dqqisn84h5js5`.
The official Nixpkgs channel was resolved once into this fixed release/NAR pin;
the finished recipe has no floating package source. The official Nix builder is also pinned
by image digest. Build `a50d3a8f-4cad-40d4-b98c-3598132735ec` succeeded in about
158 seconds with an 840-second limit.

The [post-fixup receipts](data/fleet-nix-rollout-2026-10-30/NIX_TLS_PROVENANCE)
identify OpenSSL **3.5.9**, compiler **16.2.0**, and runtime output
`/nix/store/lazac5vw20j0pyahzbaxz8z8hyf7m9zf-openssl-3.5.9`.
[ELF interpreter](data/fleet-nix-rollout-2026-10-30/ELF_INTERPRETER),
[dynamic entries](data/fleet-nix-rollout-2026-10-30/ELF_DYNAMIC) and
[ldd](data/fleet-nix-rollout-2026-10-30/ELF_LDD) bind the final binary to that
libssl/libcrypto output and Nix glibc. The
[eleven store roots](data/fleet-nix-rollout-2026-10-30/RUNTIME_CLOSURE) include
absolute loader dependencies, dash, CA certificates and small runtime libraries,
including libgcc support. Docker reports an uncompressed image size of
**52,477,724 bytes**; the registry image is **18,599,930 bytes**.

Build checks exercised the rootfs through chroot, readable CA/passwd/group/NSS,
writable `/tmp` under UID 10001, shell syntax and native invalid-config exit 2.
A separate actual-image [offline probe](data/fleet-nix-rollout-2026-10-30/native-proof-log.txt)
confirmed final USER `10001:10001`, unchanged entrypoint/CA environment and
successful default-user permission/native-launch assertions in Step 2.
The same image then answered a real private TLS/DNS-to-Pylon request with HTTP
200 in **473.839 ms**, passing the
[guard decoder and exact checkpoint/provider pins](data/fleet-nix-rollout-2026-10-30/candidate-answer-1-verified.json).
Unauthenticated candidate access returned **403**.

Two unsuccessful auxiliary attempts are retained. The
[first build](data/fleet-nix-rollout-2026-10-30/failed-build-status.json) failed
before image creation: copying sealed Nix directory modes made the assembly
root unwritable. The corrected assembly writes its manifest first and extracts
the closure with delayed directory-permission restoration. The offline probe's
**overall job failed** later at optional Step 3, exit **127**, because it tried
to run `openssl` from the library-only runtime output. That separate CLI output
is not shipped. Step 2 passed; the final libssl/libcrypto linkage and real TLS
answer passed. The failed optional command was not relabeled or rerun.

## Promotion, retained state and rollback

The sanitized Coax [candidate](data/fleet-nix-rollout-2026-10-30/candidate-verified.json),
[promoted](data/fleet-nix-rollout-2026-10-30/promoted-verified.json) and
[final](data/fleet-nix-rollout-2026-10-30/final-verified.json) gates passed without
sanitizer findings. They preserve the complete requested template and resolved
revision specs except image, template annotations, the three selected service
security annotations, private IAM bindings/version, readiness and exact pins.
Runtime account, concurrency, CPU/memory, timeouts, min/max instance settings,
API authentication/routes and demo quotas remain unchanged. No GPU model or
Pylon service was restarted or reconfigured. The existing M0 and rollout-policy
qualification results were retained; unchanged mutation/model gates were not
rerun. **Mammoth remains deferred.**

Only our old `m0-warm` tag was removed after promotion, so the intermediate
revision no longer retains a tagged minimum instance. The final `m0-nix` tag
remains. Both older immutable revisions are preserved. Emergency rollback to
the intermediate warm revision is:

```sh
gcloud run services update-traffic oa-mesh-elephant --project=openagentsgemini --region=us-central1 --to-revisions=oa-mesh-elephant-m0warm-20261031a=100
```

That historical rollback uses Debian TLS packaging and would require a renewed
Nix correction. The still older `oa-mesh-elephant-00002-dgz` also remains.
[Deployment facts](data/fleet-nix-rollout-2026-10-30/deployment.json),
[context ledger](data/fleet-nix-rollout-2026-10-30/container-SOURCES.sha256) and
[private snapshot hashes](data/fleet-nix-rollout-2026-10-30/private-snapshot-SHA256SUMS)
retain provenance. Emitted C source commit is `d6f9d6fd1134e32bae3bf7688fb24ec60e7ce8bd`;
the new packaging recipe is recorded by its own content hashes. The coordinating
agent deleted the single demo key after all calls and verified its absence.
Owned proxies were closed, and no task auth-header files remain. Public records
contain synthetic responses and selected metadata, with no credentials, auth
events, headers or cookies.
