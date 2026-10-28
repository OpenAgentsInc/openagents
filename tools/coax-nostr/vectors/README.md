# Native Coax vector runner

`crypto-vector-runner` (#0e80) provides optional test tooling. It is not part
of ordinary compiler/app generation and introduces no Python product dependency.

```sh
tools/coax-nostr/vectors.sh --list
tools/coax-nostr/vectors.sh --report /tmp/fingerprint-vectors.json
python3 tools/coax-nostr/vectors/test_runner.py
```

The initial runnable fixture feeds all 129 pinned NIST SHA-256 ShortMsg/LongMsg
rows through the existing Coax compiler-fingerprint implementation. **This is
runner evidence, not production SHA-256 qualification, a streaming API or Nostr
signing/admission.** Other crypto suites remain explicitly unimplemented and a
request for them, including `--suite all`, fails rather than silently skipping.

A consumer can supply an explicit local Coax adapter:

```sh
tools/coax-nostr/vectors.sh --suite sha256 --adapter /path/to/adapter.coax
```

The adapter exports exactly this caller-buffer C boundary:

```coax
vector_digest :: (input: [] u8, out: [] u8) -> bool #export_c
```

It must write a 32-byte digest and return true on success. Inputs are bounded by
65,536 bytes; the host supplies distinct valid buffers. This selects a vector
format, not a supported/approved capability. Module owners supply actual streaming,
KDF, keyed/cipher or protocol adapters and additional suite-specific drivers as
those algorithms land. Their checks, mutation, cross-target vectors, fuzzing and
security review remain separate gates. No absent module is treated as tested.

## Evidence boundary

Before a run, the source inventory checker verifies exact pinned upstream bytes,
provenance and derived-data drift without network access. The reader validates
NIST byte-oriented records; `Len=0, Msg=00` means zero bytes, not one zero byte.
Incomplete/duplicate/malformed fields, bad lengths and empty inputs refuse.

The Coax frontend compiles the explicit adapter to C11. The native `driver.c`
performs only bounded input/output and comparison against the independent pinned
expected digests; it contains no hash implementation or expected-value generator.
A real bad Coax candidate and a changed expected row are both caught by tests.

The compiler executable is copied into temporary storage for one coherent run.
Reports identify that binary/version, every transitive Coax/descriptor input,
generated C, driver source, native artifact, compiler/flags/mode, corpus hashes,
record identities and encoded-record digest. Input changes during generation
refuse. Debug paths may change artifact hashes across worktrees; this report does
not claim bit-reproducible binaries or validate the compiler itself.

Default builds use clang ASan/UBSan with no sanitizer recovery. `--mode release`
selects Coax wrapping semantics; `--optimized` explicitly uses unsanitized -O2.
No pointer-validity claim is inferred from compile-time checks. Native children
have a 60-second timeout, disabled core dumps and 1 MiB output-file limits; empty,
truncated or mismatched results fail. This is a trusted local test harness, not a
sandbox for arbitrary downloaded programs. It never downloads/executes source.

The fixture itself is checked Coax: its preflight refuses invalid counts/capacity
before output; all bytes of the empty digest and error no-write behavior are
checked. It shares the unchanged fingerprint algorithm and is deliberately not
an independent algorithm oracle. The NIST expected digest bytes are independent.
