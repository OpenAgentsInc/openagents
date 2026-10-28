# Pinned crypto/Nostr vector sources

This is the provenance slice of `crypto-vectors-pinned` (#ef11). It is test
maintenance data, not a shipped Coax crypto implementation or conformance claim.
All upstream files are retained byte for byte. No downloaded program, package
installer or build script was executed to obtain them.

`manifest.json` is the machine-readable inventory: source URL, immutable Git
revision or numbered RFC/archive identity, retrieval date, byte count, SHA-256,
license/notice and intended coverage. The NIST archive members also pin the
complete archive digest and exact member path. Upstream text/response files keep
original newlines; do not run formatting tools over `upstream/`.

Run from the repository root:

```sh
python3 coax/tests/vectors/check_vectors.py
python3 coax/tests/vectors/test_integrity.py
python3 coax/tests/vectors/check_vectors.py --fetch   # explicit network comparison
```

The first two commands are offline and need only Python's standard library as
optional test tooling. They are not invoked by ordinary compiler/app builds.
`--fetch` re-reads pinned HTTPS resources and checks their bytes, including the
NIST archive before reading its members; it never rewrites committed files.
Changed pins require review and a separate commit. Checks detect modified,
missing, unlisted and symlinked corpus files, mutable Git URLs, missing source/
license metadata, generator drift and changed derived input identities.
The Git review establishes trust in the manifest: a hash alone does not prove
that a maliciously replaced manifest is authentic.

## Scope and reader rules

- **SHA-256:** complete NIST CAVS 11.0 byte-oriented ShortMsg and LongMsg
  response files. `Len` is in bits; `Len = 0, Msg = 00` represents an empty
  message, not one zero byte. The source archive also contains other algorithms
  and Monte Carlo data; those are not claimed here.
- **HMAC/HKDF/PBKDF2:** full numbered RFCs preserve the source context. Consumers
  select RFC 4231 SHA-256 cases 1–7 (including the truncated case), RFC 5869
  SHA-256 cases 1–3 and RFC 7914 section 11 PBKDF2-HMAC-SHA256 vectors. Pinning a
  source RFC does not run or validate its algorithms.
- **ChaCha20, hex/base64 and JCS:** retain RFC 8439, 4648 and 8785. The first
  Nostr stack uses the ChaCha20 block/stream vectors, not a claim to implement
  every AEAD/Poly1305 example. The initial JCS consumer is integer-only; the
  RFC's floating-point cases are not silently advertised as supported.
- **BIP-340:** all 19 official CSV rows, including invalid signatures and
  message lengths 0, 1, 17 and 100. The associated license is retained.
- **NIP-44:** the complete published corpus is obtained from the Unlicense
  JavaScript subtree. Its digest is exactly the checksum in the pinned NIP-44:
  `269ed0f69e4c192512cc779e78c555090cebc7c785b609e338a62afc3ce25040`.
  The separately pinned NIP-44 text supplies the extended-prefix boundary
  vectors at 65,535/65,536/65,537 bytes absent from that original corpus.
- **ECDH:** the complete Wycheproof secp256k1 source has 752 cases. It is
  **unadapted**. Its encoded public keys, flags, case IDs and expected outcomes
  remain intact. A later reviewed adapter must establish x-only/raw-x NIP-44
  semantics; never silently discard unsuitable/invalid cases or substitute
  libsecp's default hashed ECDH result.
- **Events/NIP-19/NIP-59:** exact copies of the already vendored pinned NIPs
  provide serialization, examples and wrapping rules. BIP-173 supplies valid/
  invalid base checksum examples; Segwit-specific rules are not NIP-19 rules.
  NIP-59 cross-implementation/deck fixtures and field/scalar oracle-generated
  edge cases are later consumer/harness outputs, not falsely labelled official
  vectors by this task.
- **JSON:** RFC 8259 and a deliberately selected nine-file JSONTestSuite subset
  exercise UTF-8, escaped/unescaped controls, malformed escapes, trailing comma,
  numeric spelling, UTF-16 BOM and deep nesting. `i_` cases mean implementation-
  dependent upstream; the consumer's explicit UTF-8/depth profile decides refusal.
  This is not the entire JSONTestSuite or a claim to accept every JSON value.

## Derived source index

`derived/catalog.json` is mechanically produced by `make_catalog.py`. It indexes
BIP-340 row identities/message lengths, NIP-44 section sizes and extended-prefix
published table fields, Wycheproof source counts, and NIST case lengths. It
computes no cryptographic expected outputs. Its manifest entry pins every input
hash and the generator hash; the audit reproduces its exact bytes.

```sh
python3 coax/tests/vectors/make_catalog.py --write
```

That explicit maintenance command refreshes only the derived catalog and its
manifest entry. It cannot update upstream pins. Native/Coax/Wasm/iOS vector
execution, fuzzing, secret handling and production review remain separate tasks.

## License and source notices

BIP-340 is BSD-2-Clause OR MIT OR CC0-1.0; BIP-173 declares BSD-2-Clause in its
text. The NIP-44 JavaScript subtree is Unlicense; Wycheproof is Apache-2.0;
JSONTestSuite is MIT. Their provided license files are retained. RFCs retain
complete IETF/Internet Society copyright and redistribution notices (including
the older RFC 4231 terms). NIST's U.S. Government archive supplies CAVS notices
and no standalone license file. The NIP texts already vendored by this repository
have no standalone license declaration in those files; this inventory preserves
their source notices and does not invent a new license grant. The derived source
index is OpenAgents Apache-2.0; it does not relicense upstream material.

## Inventory

All dates below are retrieval dates (2026-10-28), not claims about publication.
The complete revision/license/archive fields are in `manifest.json`.

| File | Source | SHA-256 |
| --- | --- | --- |
| [`upstream/rfc/rfc4231.txt`](upstream/rfc/rfc4231.txt) | [pinned source](https://www.rfc-editor.org/rfc/rfc4231.txt) | `72178527ce93500e730bc8eb182b857e583096d652b64ece0879c52ba1df973b` |
| [`upstream/rfc/rfc5869.txt`](upstream/rfc/rfc5869.txt) | [pinned source](https://www.rfc-editor.org/rfc/rfc5869.txt) | `7a40eb3835b35fc947eb12a2ed614db079d43b26e50dbc537c31fba16397089c` |
| [`upstream/rfc/rfc7914.txt`](upstream/rfc/rfc7914.txt) | [pinned source](https://www.rfc-editor.org/rfc/rfc7914.txt) | `df55932f8b6a5d271f36a634d91a25903724e189ba6c120bf7257cd47f10b197` |
| [`upstream/rfc/rfc8439.txt`](upstream/rfc/rfc8439.txt) | [pinned source](https://www.rfc-editor.org/rfc/rfc8439.txt) | `25bef70fbf7a07ff45c2fe4cb7c6ce954eac687413d8610603268b4e4415324c` |
| [`upstream/rfc/rfc4648.txt`](upstream/rfc/rfc4648.txt) | [pinned source](https://www.rfc-editor.org/rfc/rfc4648.txt) | `84e14418f795d503be5f34bf23ce4ebaa119e9ec7c9f667d8caeb111385b178f` |
| [`upstream/rfc/rfc8785.txt`](upstream/rfc/rfc8785.txt) | [pinned source](https://www.rfc-editor.org/rfc/rfc8785.txt) | `63d52294eb0e3f0014174288186d388b4ddbf2c67d1ce8af1d9726eb0c3ab240` |
| [`upstream/rfc/rfc8259.txt`](upstream/rfc/rfc8259.txt) | [pinned source](https://www.rfc-editor.org/rfc/rfc8259.txt) | `61a5378f4255c720beb2a4b4a63b29540147c140f36988bf086291989b4cd2d7` |
| [`upstream/bip340/test-vectors.csv`](upstream/bip340/test-vectors.csv) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0340/test-vectors.csv) | `34c9d1d9c3a88d524bc80778540dc43f8306ec249a7485293063c376db851c2d` |
| [`upstream/bip340/LICENSE`](upstream/bip340/LICENSE) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0340/LICENSE) | `e4974cd8532036eda7e540a9930fee0c020cb297d455033232606298a89adbd4` |
| [`upstream/bip173/bip-0173.mediawiki`](upstream/bip173/bip-0173.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0173.mediawiki) | `b19eab06b264bbd15ad7f7d640fa34de8f4a8809fba8a6ecc3e165545b27a543` |
| [`upstream/nip44/nip44.vectors.json`](upstream/nip44/nip44.vectors.json) | [pinned source](https://raw.githubusercontent.com/paulmillr/nip44/671a1f04bcfacaf125b0db68adc45bc9ce0e763b/javascript/test/nip44.vectors.json) | `269ed0f69e4c192512cc779e78c555090cebc7c785b609e338a62afc3ce25040` |
| [`upstream/nip44/LICENSE`](upstream/nip44/LICENSE) | [pinned source](https://raw.githubusercontent.com/paulmillr/nip44/671a1f04bcfacaf125b0db68adc45bc9ce0e763b/javascript/LICENSE) | `f0510d3b9d1223c45e8fdd2b49270ec8734fff491c7f7d1b948ee70162e1ceff` |
| [`upstream/wycheproof/ecdh_secp256k1_test.json`](upstream/wycheproof/ecdh_secp256k1_test.json) | [pinned source](https://raw.githubusercontent.com/C2SP/wycheproof/d17cc89fa5c8f4f4d378b5385ebf75da5d0926c9/testvectors_v1/ecdh_secp256k1_test.json) | `049fa2be70e9db836a043aba6132b53cd97de4c0de5277bca6d65a2770075cae` |
| [`upstream/wycheproof/LICENSE`](upstream/wycheproof/LICENSE) | [pinned source](https://raw.githubusercontent.com/C2SP/wycheproof/d17cc89fa5c8f4f4d378b5385ebf75da5d0926c9/LICENSE) | `58d1e17ffe5109a7ae296caafcadfdbe6a7d176f0bc4ab01e12a689b0499d8bd` |
| [`upstream/json/LICENSE`](upstream/json/LICENSE) | [pinned source](https://raw.githubusercontent.com/nst/JSONTestSuite/1ef36fa01286573e846ac449e8683f8833c5b26a/LICENSE) | `8bd0e0578be788c617ea01d18b2a8146e3746ae50bddadc65a5f9d3aad08ad49` |
| [`upstream/json/y_string_escaped_control_character.json`](upstream/json/y_string_escaped_control_character.json) | [pinned source](https://raw.githubusercontent.com/nst/JSONTestSuite/1ef36fa01286573e846ac449e8683f8833c5b26a/test_parsing/y_string_escaped_control_character.json) | `1a9f37085203c6d712945c5fb87ad5e08fa38515e51ee4a637c7558bb1b82725` |
| [`upstream/json/y_string_utf8.json`](upstream/json/y_string_utf8.json) | [pinned source](https://raw.githubusercontent.com/nst/JSONTestSuite/1ef36fa01286573e846ac449e8683f8833c5b26a/test_parsing/y_string_utf8.json) | `adec79032c3509dce21c9b0b96445a0fe398533d6647d12abc639eef16e621ab` |
| [`upstream/json/n_string_unescaped_ctrl_char.json`](upstream/json/n_string_unescaped_ctrl_char.json) | [pinned source](https://raw.githubusercontent.com/nst/JSONTestSuite/1ef36fa01286573e846ac449e8683f8833c5b26a/test_parsing/n_string_unescaped_ctrl_char.json) | `c85db33857913bc4c4d4f044c72cd06d50ffb5644b2e5093ef8b46ebfe340ed0` |
| [`upstream/json/n_string_incomplete_escape.json`](upstream/json/n_string_incomplete_escape.json) | [pinned source](https://raw.githubusercontent.com/nst/JSONTestSuite/1ef36fa01286573e846ac449e8683f8833c5b26a/test_parsing/n_string_incomplete_escape.json) | `df3709e1b19bc2562a9bc0301265c237ca5a5b35bd9bc5c3a7f45c57092e0537` |
| [`upstream/json/n_string_invalid-utf-8-in-escape.json`](upstream/json/n_string_invalid-utf-8-in-escape.json) | [pinned source](https://raw.githubusercontent.com/nst/JSONTestSuite/1ef36fa01286573e846ac449e8683f8833c5b26a/test_parsing/n_string_invalid-utf-8-in-escape.json) | `1c1ef70149258ff7ee082817269431c45e34f75a2268c3dff56f35d3a4859f1e` |
| [`upstream/json/n_object_trailing_comma.json`](upstream/json/n_object_trailing_comma.json) | [pinned source](https://raw.githubusercontent.com/nst/JSONTestSuite/1ef36fa01286573e846ac449e8683f8833c5b26a/test_parsing/n_object_trailing_comma.json) | `60e04bc71fcc4c57abeef95a9604a96aa60f833881a2f028d27bd4f03e176603` |
| [`upstream/json/y_number_int_with_exp.json`](upstream/json/y_number_int_with_exp.json) | [pinned source](https://raw.githubusercontent.com/nst/JSONTestSuite/1ef36fa01286573e846ac449e8683f8833c5b26a/test_parsing/y_number_int_with_exp.json) | `6d014f6467daa9753d57f0d75067fe62c9f3b0116455a3222c26e58432541b0d` |
| [`upstream/json/i_string_UTF-16LE_with_BOM.json`](upstream/json/i_string_UTF-16LE_with_BOM.json) | [pinned source](https://raw.githubusercontent.com/nst/JSONTestSuite/1ef36fa01286573e846ac449e8683f8833c5b26a/test_parsing/i_string_UTF-16LE_with_BOM.json) | `6a9c15ecc8fc3da72b0ba5e3539e07f2aad3c704b496fe3496579dc723ce49c5` |
| [`upstream/json/i_structure_500_nested_arrays.json`](upstream/json/i_structure_500_nested_arrays.json) | [pinned source](https://raw.githubusercontent.com/nst/JSONTestSuite/1ef36fa01286573e846ac449e8683f8833c5b26a/test_parsing/i_structure_500_nested_arrays.json) | `a6c6b45361ff77e7372a015a1f0289e9c09d1fe1ed59cf9773d599c55acf57cf` |
| [`upstream/nist/Readme.txt`](upstream/nist/Readme.txt) | [pinned source](https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/shs/shabytetestvectors.zip) | `24bf370543f9521f7eb44f49e8d489ac9e45a21126f883d8e32e7fffd1d10227` |
| [`upstream/nist/SHA256ShortMsg.rsp`](upstream/nist/SHA256ShortMsg.rsp) | [pinned source](https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/shs/shabytetestvectors.zip) | `75e1cb83994638481808e225b9eb0c1ebd0c232d952ac42b61abce6363be283c` |
| [`upstream/nist/SHA256LongMsg.rsp`](upstream/nist/SHA256LongMsg.rsp) | [pinned source](https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/shs/shabytetestvectors.zip) | `6fac36f37360bcf74ffcf4465c18e30d6d5a04cc90885b901fc3130c16060974` |
| [`upstream/nips/01.md`](upstream/nips/01.md) | [pinned source](https://raw.githubusercontent.com/nostr-protocol/nips/400d157c53264b8e58c7b3cdbd21ef546fb7a157/01.md) | `d5c5ce2b5639eee440b5d342dbe81af42008838cb260ec04acaffaffd29dcaf7` |
| [`upstream/nips/19.md`](upstream/nips/19.md) | [pinned source](https://raw.githubusercontent.com/nostr-protocol/nips/400d157c53264b8e58c7b3cdbd21ef546fb7a157/19.md) | `5b53c98637ae0cb8e6d99e1570e32da2c5842154ba898393439f334f1fd90aad` |
| [`upstream/nips/44.md`](upstream/nips/44.md) | [pinned source](https://raw.githubusercontent.com/nostr-protocol/nips/400d157c53264b8e58c7b3cdbd21ef546fb7a157/44.md) | `b5f89374e4e1dbdee7881e8573b4313b6a89430a9c8d518471599ae0660cf0e2` |
| [`upstream/nips/59.md`](upstream/nips/59.md) | [pinned source](https://raw.githubusercontent.com/nostr-protocol/nips/400d157c53264b8e58c7b3cdbd21ef546fb7a157/59.md) | `b46b078ff5b219fd4ab33a53b479b49d4cbd604b88be46d6b978d53d8f8b43a0` |
| [`derived/catalog.json`](derived/catalog.json) | `make_catalog.py` (derived) | `eb5be14f52c41f115153b3d9d4189b30c1c984e62ddc1278793eef6b80321d27` |
