# Presentation phrase-gate format review

Task `unlock-gate-format` (#2cde), author `codex-nostr`. This is a reviewed
experimental format proposal and public fixture corpus, **not an enabled app
reader/writer or a production security approval**. The current AES-GCM adapter,
entry bytes, UI and deployment behavior remain unchanged. No production phrase,
key, locator or identity was read to make these fixtures.

## Reviewed artifacts

- Draft: `docs/protocol/2026-10-28-phrase-gate-v2.md`, SHA-256
  `788ee8b398ae1f854c4d871200d0636e94bff9db30c8d3da2c8c6619277cc1e7`.
- Frozen public fixtures:
  `coax/tests/nostr/data/phrase-gate-draft-2026-10-28.json`, SHA-256
  `de751670e9a33cf7c6e454849bd165c854e54e9f25448a2dbd1a0db1b284723c`.
- Namespace reservation: `proto/openagents/access/phrase/v2/README.md`.
  It advertises no compiled protobuf API. Any later projection carries the
  original packed bytes, never reconstituted MAC input.

Non-author internal reviewer: `/root/nostr_codecs`, worker
`codex-nostr-codecs`. The reviewer read the draft, generator, fixtures and
oracle paths separately from their author. This is an internal format review,
not the independent security/target/owner review required before production.

The review found three ambiguities, all resolved in the pinned draft:

1. Context was too loosely described. It now binds the canonical trusted
   domain/environment/module/presentation record; scope changes require
   different records and missing configuration cannot default to zero.
2. The entry budget could be read as eight per format. The cap is explicitly
   eight combined, with format-specific normalization and legacy-first dispatch
   that preserves separately configured legacy access.
3. Canonical plaintext acceptance was ambiguous. An authenticated pointer must
   equal the complete canonical encoding exactly; duplicates, unknown fields,
   alternate order/escaping and trailing whitespace refuse.

The reviewer then checked the actual header offsets, lengths, labels/NULs,
PBKDF2/HKDF arguments, role split, ChaCha counter mapping and full original
header/ciphertext MAC. No construction mismatch remained in that scoped pass.
Recommended magic/header-truncation and complete size/base64 boundary tests
were added before freezing the test evidence.

## Explicit test tooling

```sh
node --test coax/tests/nostr/phrase_gate_format.test.ts
python3 coax/tests/nostr/phrase_gate_primitives.py
```

Five Node tests pass. The unchanged real `gate.ts` reader opens the pinned
AES-GCM fixture at its actual210000 rounds. Public v2 fixtures pin every key
schedule/header/ciphertext/tag byte. Wrong phrase, version, flags, KDF/cipher,
rounds, count, context, salt, nonce, ciphertext and MAC refuse. Tests observe
zero decryption calls on preflight/MAC refusal, and authenticated bad shape,
duplicate/unknown/noncanonical JSON refuses after private decryption.

Tests also cover missing context, outer version/shape, invalid magic, truncated
headers, exact16/40 normalized and128 raw input boundaries, non-ASCII/control
input, strict base64/pad bits, a complete authenticated4096-byte plaintext
(4202-byte wire/5604-byte base64, admitted through crypto then refused for
noncanonical pointer shape), and a complete4097-byte plaintext (4203-byte wire,
refused before decryption). A declared-length error cannot mask those complete
size boundaries. The test-only reference helper can manufacture invalid
cryptograms; it is not a product writer.

Python independently pins the label bytes, context canonical/hash calculation,
header layout and manual HKDF expansion. The existing **actual Coax** PBKDF2,
HKDF, ChaCha20(counter1) and HMAC primitives match every master/PRK/key/ciphertext/
tag byte in checked and release native C. Each PBKDF2 run completes210000 rounds
in206 bounded1024-round steps. This is one new composed fixture, not a repeat
of their unrelated RFC/vector suites. Primitive/compiler/object evidence is
retained in the companion JSON report. Normal compiler/app builds remain
Python-free; these scripts are explicit test tooling.

The reference crypto uses installed Node/OpenSSL. Its16-byte ChaCha IV is the
little-endian32-bit counter1 followed by the12-byte IETF nonce, with at most64
blocks. That bounded state mapping is checked against the separately implemented
Coax cipher; it is not a claim that every generic OpenSSL nonce/counter API is
interchangeable. Primitive references are
[PBKDF2](https://www.rfc-editor.org/rfc/rfc8018.html#section-5.2),
[HKDF](https://www.rfc-editor.org/rfc/rfc5869.html#section-2) and
[IETF ChaCha20](https://www.rfc-editor.org/rfc/rfc8439.html#section-2.4).
The proposed composition is neither ChaCha20-Poly1305 nor NIP-44.

## Implementation and rollout boundary

This review permits the **test-key experimental Coax implementation task** to
start against these exact bytes. It permits no production format flip,
AES-GCM removal, new secret approval, claim of equivalent security, or phone
sign-in change. Later protocol changes require a new reviewed draft/version;
published production bytes cannot be revised in place.

The implementation must pass native/actual-browser interoperability,
wrong-input/cancellation/cleanup/capacity checks, legacy/v2 combined dispatch,
and a real staging rollback using paired reader/entry artifacts. Production
still waits for the epic's independent security review, target/guard/codegen
and erasure evidence, explicit rollout approval and the declared compatibility
window. A copied legacy cheap phrase oracle and already decrypted data cannot
be revoked by changing the page bundle.
