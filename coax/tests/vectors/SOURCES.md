# Pinned crypto/Nostr vector sources

This is the provenance slice of `crypto-vectors-pinned` (#ef11). It is test
maintenance data, not a shipped Coax crypto implementation or conformance claim.
All upstream files are retained byte for byte. No downloaded program, package
installer or build script was executed to obtain them.

`manifest.json` is the machine-readable inventory: source URL, immutable Git
revision, numbered RFC/archive identity or a `static:` author-hosted file
pinned by its digest, retrieval date, byte count, SHA-256,
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
- **SHA-512:** the ShortMsg and LongMsg response files from the same NIST CAVS
  11.0 archive (same archive digest), added 2026-11-02 for Bitcoin From
  Scratch chapter 7. The same `Len = 0, Msg = 00` rule applies. SHA-384,
  SHA-512/224, SHA-512/256 and the Monte Carlo files are not pinned.
- **HMAC/HKDF/PBKDF2:** full numbered RFCs preserve the source context. Consumers
  select RFC 4231 SHA-256 cases 1–7 (including the truncated case), RFC 5869
  SHA-256 cases 1–3 and RFC 7914 section 11 PBKDF2-HMAC-SHA256 vectors. Pinning a
  source RFC does not run or validate its algorithms.
- **ChaCha20, hex/base64 and JCS:** retain RFC 8439, 4648 and 8785. The first
  Nostr stack uses the ChaCha20 block/stream vectors, not a claim to implement
  every AEAD/Poly1305 example. The initial JCS consumer is integer-only; the
  RFC's floating-point cases are not silently advertised as supported.
  Bitcoin chapter 12 reads RFC 8439's Poly1305 and AEAD examples (sections
  2.5.2, 2.6.2 and 2.8.2, appendices A.3, A.4 and A.5) from the same file.
- **ChaCha20-Poly1305:** the complete Wycheproof `chacha20_poly1305_test.json`
  (325 cases) at the same revision as the ECDH file, retrieved 2026-11-02 for
  Bitcoin chapter 12. Its 316 cases with a 96-bit nonce give their expected
  result; the 9 cases with other nonce sizes must refuse the nonce.
- **ECDSA:** the complete Wycheproof `ecdsa_secp256k1_sha256_bitcoin_test.json`
  (464 cases, 100 public keys) at the same revision as the ECDH file,
  retrieved 2026-11-02 for Bitcoin chapter 19. Its Bitcoin variant requires
  strict DER and low-S; every case gives its expected result.
- **ECDSA signing:** tiny-secp256k1's complete `tests/fixtures/ecdsa.json`
  at revision `e1fd7a77`, retrieved 2026-11-02 for Bitcoin chapter 20. Its
  2,020 `valid` rows give a key, a 32-byte hash and the compact signature
  that libsecp256k1 makes with RFC 6979 and low-S; its 4 `invalid.sign`
  rows are keys that must refuse. Chapter 20 signs every `valid` row byte
  for byte. The `extraEntropy` rows (libsecp256k1's `ndata`) and the
  `verify` rows are not claimed. RFC 6979's own appendix vectors have no
  secp256k1 case. Bitcoin chapter 22 claims each `valid` row's
  `recoveryId` (0 or 1): recovery with it gives the row's key, and
  recoverable signing gives it. Its 8 `invalid.recover` rows must refuse.
- **ECDSA public key recovery:** libsecp256k1's recovery module tests
  (`src/modules/recovery/tests_impl.h`) at release `v0.8.0` (the oracle's
  pinned commit `6e2c8bc4`), with its `COPYING`, retrieved 2026-11-02 for
  Bitcoin chapter 22. Its edge cases are claimed: a signature by the
  invalid key 0, whose true R recovers to infinity and whose IDs 2 and 3
  put r + n past p; `(r, s) = (4, 4)`, which recovers with all four IDs;
  and `(1, 1)`, `(0, 1)` and `(1, 0)`. Its DER-parsing checks are not.
- **Bitcoin signed messages:** Bitcoin Core's `src/test/util_tests.cpp`
  (the `message_sign`, `message_verify` and `message_hash` cases) and
  `test/functional/rpc_signmessagewithprivkey.py`, both at the revision of
  the other Bitcoin Core files, retrieved 2026-11-02 for Bitcoin chapter 22.
  Every signature, address and expected result in those cases is claimed,
  except that the all-zero signature of `message_verify` refuses at its
  header byte here, where Core reports `ERR_PUBKEY_NOT_RECOVERED`. The
  rest of `util_tests.cpp` is not claimed.
- **BIP-340:** all 19 official CSV rows, including invalid signatures and
  message lengths 0, 1, 17 and 100. The associated license is retained.
- **RIPEMD-160:** the authors' paper (Dobbertin, Bosselaers and Preneel,
  `AB-9601.pdf`), whose Appendix B lists the nine RIPEMD-160 test values.
  The authors host it without version history, so its revision is `static:`
  and its SHA-256 is the pin; `--fetch` detects a changed file. Bitcoin
  chapter 6 reads the values from the PDF's text stream. The RIPEMD-128
  values in the same appendix are not claimed.
- **BIP-32:** the BIP text at the same `bitcoin/bips` revision as BIP-173.
  Bitcoin chapter 6 decodes its extended keys and checks that HASH160 of each
  parent public key starts with the child's parent fingerprint. The key
  derivation itself is not claimed here.
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
- **SHA-256d and tagged hashes (Bitcoin chapter 5):** three files give
  published bytes and their published digests. btcd's `genesis_test.go` holds
  the 285 serialized bytes of the mainnet genesis block. Bitcoin Core's
  `chainparams.cpp` asserts its block hash and Merkle root. Bitcoin Core's
  `tx_valid.json` holds mainnet transactions whose comments name their txids
  (`23b397ed…`, `c99c49da…` and `f7fdd091…`). BIP-341's
  `wallet-test-vectors.json` gives the TapLeaf, TapBranch, TapTweak and
  TapSighash outputs (leaf hashes, Merkle roots, tweaks and signature hashes
  over each `sigMsg`). The chapter copies hex from these files; it does not
  claim to run the transactions' scripts or the other BIP-341 checks.
- **SipHash-2-4 (Bitcoin chapter 10):** the reference implementation's
  `vectors.h` at `veorq/SipHash` commit `32d06760`, whose `vectors_sip64`
  table gives the 64 SipHash-2-4 tags for the key `00 01 … 0f` and the
  inputs `""`, `00`, `00 01`, … up to 63 bytes. The 128-bit and HalfSipHash
  tables in the same file are not claimed. Bitcoin Core's `hash_tests.cpp`
  copies the same 64 tags as `siphash_4_2_testvec` and adds one Bitcoin
  case: SipHash with k0 = 1 and k1 = 2 of the SHA-256d of an empty
  version-1 transaction is `0x79751e980c2a0a35`. Its streaming,
  SipHash-1-3 and `siphash.json` checks are not claimed.
- **Point encodings (Bitcoin chapter 17):** Bitcoin Core's
  `script_tests.json`, at the same revision as `tx_valid.json`, holds public
  keys in script pushes: the generator G compressed (`0279be…`) and
  uncompressed (`0479be…`), a second key in both forms (`038282…` and
  `048282…`), the hybrid key `0679be…` and malformed pushes that the
  `STRICTENC` cases refuse with `PUBKEYTYPE`. Chapter 17 reads those keys and
  the comments that name each failure; it does not run the scripts. It also
  reads the SEC 1 points inside the Wycheproof ECDH public keys whose
  SubjectPublicKeyInfo header is the named-curve secp256k1 one (455 distinct
  valid uncompressed points, one compressed, and the off-curve,
  invalid-compressed, twist and empty points), and the keys in BIP-32,
  BIP-340 and BIP-341's files. The ECDH results themselves stay unadapted.
- **Strict DER (Bitcoin chapter 21):** Bitcoin Core's `interpreter.cpp`
  (`IsValidSignatureEncoding`, `IsLowDERSignature`,
  `IsDefinedHashtypeSignature`, `CheckSignatureEncoding`) and `pubkey.cpp`
  (`ecdsa_signature_parse_der_lax`, `CheckLowS`) at the same revision as
  `tx_valid.json`, which the chapter transliterates rule by rule. From
  `script_tests.json` it takes the 89 rows whose scriptSig is only pushes
  and whose scriptPubKey is pushes then `CHECKSIG` or `CHECKMULTISIG`
  (optionally `NOT`), in each group of rows sharing scripts where some row
  sets `DERSIG` or `LOW_S` or expects `SIG_DER`, `SIG_HIGH_S` or
  `SIG_HASHTYPE`; it computes their legacy signature hashes, and does not
  claim the other rows. bitcoinjs/bip66 `test/fixtures.json` at commit
  `08169640` (MIT) gives 9 valid DER encodings with their `r` and `s` and 15
  invalid ones; the chapter uses its bytes, not its error strings.
- **Base58Check (Bitcoin chapter 26):** Bitcoin Core's
  `base58_encode_decode.json`, `key_io_valid.json` and `key_io_invalid.json`
  at the same revision as `tx_valid.json`, retrieved 2026-11-02. All 21
  Base58 pairs are used both ways. Of the 70 valid entries, the 38 that are
  Base58 (P2PKH and P2SH addresses and WIF keys on mainnet, testnet4, signet
  and regtest) are used; the 32 Bech32 and Bech32m addresses are left to
  chapters 27–29. All 70 invalid strings must refuse as an address and as a
  WIF. Chapter 26 also reads the 17 extended public keys in BIP-32's text.
- **Bech32m and segwit addresses (Bitcoin chapter 28):** BIP-350's text at
  the same revision as BIP-173's, retrieved 2026-11-02, for its 7 valid and
  14 invalid Bech32m strings and its 8 valid and 15 invalid segwit
  addresses, with the reasons it gives. Chapter 28 also reads BIP-173's 6
  valid and 10 invalid segwit addresses from the already pinned BIP-173 text,
  and the 32 Bech32 and Bech32m addresses of `key_io_valid.json` that
  chapter 26 left. `segwit_addr.py` is BIP-350's reference Python
  implementation from `sipa/bech32` at commit `7a7d7ab1`. It is stored as
  `segwit_addr.py.txt`, bytes unchanged, so the repository's tools rule
  doesn't read it as one of our tools; `coax/tests/bitcoin/segwit.py`
  loads it after checking its digest and uses it as the oracle.
- **BIP-39 (Bitcoin chapter 31):** trezor/python-mnemonic's `vectors.json`,
  its English word list and its license at revision `b57a5ad7`, and the BIP-39
  text at the same `bitcoin/bips` revision as BIP-32, retrieved 2026-11-02.
  All 24 English vectors (entropy, phrase and seed with the passphrase
  `TREZOR`) are used; their `xprv` column belongs to BIP-32 (chapter 32).
  The first vector of each of the 11 other languages is used to show that the
  ASCII-only English reader refuses them and that the seed is PBKDF2 of the
  phrase's bytes. `english.txt` has the SHA-256 that `bip39.coax` checks
  its inline copy against.
- **Transactions (Bitcoin chapter 34):** Bitcoin Core's `tx_invalid.json`
  beside `tx_valid.json`, and the sources the chapter transliterates, all at
  the same revision: `primitives/transaction.h` (`UnserializeTransaction`,
  `SerializeTransaction`), `serialize.h` (`ReadCompactSize`, `MAX_SIZE`),
  `core_io.cpp` (`DecodeTx`, `CheckTxScriptsSanity`), `script/script.cpp`
  and `script/script.h` (`HasValidOps`, `GetScriptOp` and their limits) and
  `consensus/consensus.h` (`WITNESS_SCALE_FACTOR`). Every transaction of
  the two JSON files (122 and 93) is read and written back; their prevouts,
  scripts and flags are left to the script and consensus chapters.
  rust-bitcoin's `transaction.rs` at tag `bitcoin-0.32.5` (commit
  `17ce61c1`, CC0-1.0, license retained) publishes the P2WPKH transaction
  `f5864806…` with its txid, wtxid, weight and vsize, and the zero-input
  extended transaction of `tx_no_input_deserialization`. The `mainnet/`
  files are mempool.space's raw bytes and JSON for that transaction, the
  Taproot key-path spend `33e794d0…` and the script-path spend
  `37777def…`, retrieved 2026-11-02; each raw file hashes to its txid, so
  the bytes cannot change, and the chapter uses the JSON's size and weight.
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

BIP-340 is BSD-2-Clause OR MIT OR CC0-1.0; BIP-173 and BIP-350 declare BSD-2-Clause in their
text, and BIP-341 declares BSD-3-Clause in its text. Bitcoin Core is MIT,
btcd is ISC and rust-bitcoin is CC0-1.0; their license files are retained. The
mainnet transactions are public blockchain data. SipHash is CC0-1.0, MIT or
Apache-2.0 with LLVM exception; its CC0 file is retained. The NIP-44 JavaScript subtree is Unlicense; Wycheproof is Apache-2.0;
JSONTestSuite is MIT. Their provided license files are retained. `segwit_addr.py`
carries its MIT notice in its own header. RFCs retain
complete IETF/Internet Society copyright and redistribution notices (including
the older RFC 4231 terms). NIST's U.S. Government archive supplies CAVS notices
and no standalone license file. The NIP texts already vendored by this repository
have no standalone license declaration in those files; this inventory preserves
their source notices and does not invent a new license grant. The derived source
index is OpenAgents Apache-2.0; it does not relicense upstream material.

## Inventory

All dates below are retrieval dates (2026-10-28; the RIPEMD-160 and BIP-32
files, the Bitcoin chapter 5, 17, 26, 28 and 34 files and the SipHash files, 2026-11-02), not claims about
publication.
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
| [`upstream/bip32/bip-0032.mediawiki`](upstream/bip32/bip-0032.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0032.mediawiki) | `e5e00a8289db2f681052cf24a745320afc225e66b25d1e489a7c884d2fc7f11f` |
| [`upstream/ripemd160/AB-9601.pdf`](upstream/ripemd160/AB-9601.pdf) | [authors' copy](https://homes.esat.kuleuven.be/~bosselae/ripemd160/pdf/AB-9601/AB-9601.pdf) | `1e235c15d30b57a868b41d870dbebd86df4c0a54e3a709868192f777464ffb87` |
| [`upstream/nip44/nip44.vectors.json`](upstream/nip44/nip44.vectors.json) | [pinned source](https://raw.githubusercontent.com/paulmillr/nip44/671a1f04bcfacaf125b0db68adc45bc9ce0e763b/javascript/test/nip44.vectors.json) | `269ed0f69e4c192512cc779e78c555090cebc7c785b609e338a62afc3ce25040` |
| [`upstream/nip44/LICENSE`](upstream/nip44/LICENSE) | [pinned source](https://raw.githubusercontent.com/paulmillr/nip44/671a1f04bcfacaf125b0db68adc45bc9ce0e763b/javascript/LICENSE) | `f0510d3b9d1223c45e8fdd2b49270ec8734fff491c7f7d1b948ee70162e1ceff` |
| [`upstream/wycheproof/ecdh_secp256k1_test.json`](upstream/wycheproof/ecdh_secp256k1_test.json) | [pinned source](https://raw.githubusercontent.com/C2SP/wycheproof/d17cc89fa5c8f4f4d378b5385ebf75da5d0926c9/testvectors_v1/ecdh_secp256k1_test.json) | `049fa2be70e9db836a043aba6132b53cd97de4c0de5277bca6d65a2770075cae` |
| [`upstream/wycheproof/chacha20_poly1305_test.json`](upstream/wycheproof/chacha20_poly1305_test.json) | [pinned source](https://raw.githubusercontent.com/C2SP/wycheproof/d17cc89fa5c8f4f4d378b5385ebf75da5d0926c9/testvectors_v1/chacha20_poly1305_test.json) | `fe61d25f90e1bde4461d00eafe61049e5f29bd999f36b766df9cda90906ad53d` |
| [`upstream/wycheproof/ecdsa_secp256k1_sha256_bitcoin_test.json`](upstream/wycheproof/ecdsa_secp256k1_sha256_bitcoin_test.json) | [pinned source](https://raw.githubusercontent.com/C2SP/wycheproof/d17cc89fa5c8f4f4d378b5385ebf75da5d0926c9/testvectors_v1/ecdsa_secp256k1_sha256_bitcoin_test.json) | `d3729ce83ff9740c45e74921009a4061eff6c09185b8dd1225ccd06ac57048a4` |
| [`upstream/tiny-secp256k1/ecdsa.json`](upstream/tiny-secp256k1/ecdsa.json) | [pinned source](https://raw.githubusercontent.com/bitcoinjs/tiny-secp256k1/e1fd7a77bb0ca87fb4726fb1fd4f424f30a04db9/tests/fixtures/ecdsa.json) | `b830aa30d0ab3b6c51440caffac9206b24ae5450a387a2db73789f5ce795595d` |
| [`upstream/tiny-secp256k1/LICENSE`](upstream/tiny-secp256k1/LICENSE) | [pinned source](https://raw.githubusercontent.com/bitcoinjs/tiny-secp256k1/e1fd7a77bb0ca87fb4726fb1fd4f424f30a04db9/LICENSE) | `ae6f2b755062444d218c8de466a0c73b686f89285855353a2eae0ade5933d661` |
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
| [`upstream/bip341/wallet-test-vectors.json`](upstream/bip341/wallet-test-vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0341/wallet-test-vectors.json) | `403e19fb81dd1f31e745699216308f61fb403774b2aafa87b631b8f7c042d37f` |
| [`upstream/bitcoin-core/tx_valid.json`](upstream/bitcoin-core/tx_valid.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/data/tx_valid.json) | `0abeb1a6e4b942e4e43ef8d49b7356c1f6204bcb14163f24153f4d0ec5418011` |
| [`upstream/bitcoin-core/chainparams.cpp`](upstream/bitcoin-core/chainparams.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/kernel/chainparams.cpp) | `cfadf1478377c32625280520655b598674e6d487f08d7f7a80ac69252104b5e1` |
| [`upstream/bitcoin-core/script_tests.json`](upstream/bitcoin-core/script_tests.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/data/script_tests.json) | `ccfadc13f9b06888faf57a4a2f86dd8feff35c80aa710cbf3eb421f326b8d8b7` |
| [`upstream/bitcoin-core/base58_encode_decode.json`](upstream/bitcoin-core/base58_encode_decode.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/data/base58_encode_decode.json) | `20d51011f49339714c28b9244cc5238f4c78bb9206dc8fc61500aed6fc2682ca` |
| [`upstream/bitcoin-core/key_io_valid.json`](upstream/bitcoin-core/key_io_valid.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/data/key_io_valid.json) | `90bd1d35d12763e0d00c5400b2c9fe551e532a821e1e466c20cad3aced70a7fe` |
| [`upstream/bitcoin-core/key_io_invalid.json`](upstream/bitcoin-core/key_io_invalid.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/data/key_io_invalid.json) | `c3ca74ddad7c01faaca7c26063537feb73ff2be03eb7fc83a42a655d2979261d` |
| [`upstream/bip350/bip-0350.mediawiki`](upstream/bip350/bip-0350.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0350.mediawiki) | `63634b06aa8bae88b31929674736e74964c4598684269ac2b0b140c43a7a0dec` |
| [`upstream/sipa-bech32/segwit_addr.py.txt`](upstream/sipa-bech32/segwit_addr.py.txt) | [pinned source](https://raw.githubusercontent.com/sipa/bech32/7a7d7ab158db7078a333384e0e918c90dbc42917/ref/python/segwit_addr.py) | `2884ce04a36c8374c4249177cd90e91e545ac73b5c9260307bbeb74ae2e9de0f` |
| [`upstream/bip39/vectors.json`](upstream/bip39/vectors.json) | [pinned source](https://raw.githubusercontent.com/trezor/python-mnemonic/b57a5ad77a981e743f4167ab2f7927a55c1e82a8/vectors.json) | `fa3b937b7cff9c9b8ecd3aa011faeb8d6dd67993174b72326e83f4de8fdb30f8` |
| [`upstream/bip39/english.txt`](upstream/bip39/english.txt) | [pinned source](https://raw.githubusercontent.com/trezor/python-mnemonic/b57a5ad77a981e743f4167ab2f7927a55c1e82a8/src/mnemonic/wordlist/english.txt) | `2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda` |
| [`upstream/bip39/LICENSE`](upstream/bip39/LICENSE) | [pinned source](https://raw.githubusercontent.com/trezor/python-mnemonic/b57a5ad77a981e743f4167ab2f7927a55c1e82a8/LICENSE) | `d5e3c7c62a84e80073201e2f6e5130e9e6804fa05f8ac4f8b26a13c7d3969697` |
| [`upstream/bip39/bip-0039.mediawiki`](upstream/bip39/bip-0039.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0039.mediawiki) | `afcbcbed36fe9eb734bd607398a8c124683ded2a75c3830e1b16c47b043a9134` |
| [`upstream/bitcoin-core/COPYING`](upstream/bitcoin-core/COPYING) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/COPYING) | `b028769f3852a9368ab10bd754ff01ebb741f84a2fa658c9aff82a631bc6ecfc` |
| [`upstream/btcd/genesis_test.go`](upstream/btcd/genesis_test.go) | [pinned source](https://raw.githubusercontent.com/btcsuite/btcd/dd169fe0c32a56087d28b92fff3b8cf4fb43477a/chaincfg/genesis_test.go) | `e188c4fb27cccec22a0853be80f5c9b308e88045baccbf81a2058b01c7155452` |
| [`upstream/btcd/LICENSE`](upstream/btcd/LICENSE) | [pinned source](https://raw.githubusercontent.com/btcsuite/btcd/dd169fe0c32a56087d28b92fff3b8cf4fb43477a/LICENSE) | `10db068dbe11e2b3d652a3d4ae3cdd7998b84b12cdefe3d8083a2b63c295c13f` |
| [`upstream/siphash/vectors.h`](upstream/siphash/vectors.h) | [pinned source](https://raw.githubusercontent.com/veorq/SipHash/32d067603b93b47828700880649198e0bfbbcffa/vectors.h) | `212c44114a63c6d84710b8627f3bc5ce155698accf2ea7bbff1c4b69c9f48d31` |
| [`upstream/siphash/LICENSE_CC0`](upstream/siphash/LICENSE_CC0) | [pinned source](https://raw.githubusercontent.com/veorq/SipHash/32d067603b93b47828700880649198e0bfbbcffa/LICENSE_CC0) | `5537d4d10b76b81b6e8dfd8b644480a4b1efa332fbb0cdb61126c5be781ef7b4` |
| [`upstream/bitcoin-core/hash_tests.cpp`](upstream/bitcoin-core/hash_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/hash_tests.cpp) | `c567bcc5c403b7098d1cf4caaa0a9817e3dc2438ba77c8f1fbef78100b3cab3f` |
| [`upstream/bitcoin-core/interpreter.cpp`](upstream/bitcoin-core/interpreter.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/script/interpreter.cpp) | `57a6f328c7e6202f499e48366a86eb26e09c8ebb78e92eef7e839207230be333` |
| [`upstream/bitcoin-core/pubkey.cpp`](upstream/bitcoin-core/pubkey.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/pubkey.cpp) | `13e59d462a35ba37f818538454228822718c3328d219a6a022e345d69c4b1d99` |
| [`upstream/bip66/fixtures.json`](upstream/bip66/fixtures.json) | [pinned source](https://raw.githubusercontent.com/bitcoinjs/bip66/08169640fc2d9a5f4007f7a4493a2f4627c07cfc/test/fixtures.json) | `69b1504212b2596fdc62669e6193aebbb42c6e82ceec9f99ced205d557b7e83d` |
| [`upstream/bip66/LICENSE`](upstream/bip66/LICENSE) | [pinned source](https://raw.githubusercontent.com/bitcoinjs/bip66/08169640fc2d9a5f4007f7a4493a2f4627c07cfc/LICENSE) | `51ad9defd71911dbe413daf277bcb2e1077743f5ee5ae48d0a3b90a5b7486f55` |
| [`upstream/bitcoin-core/tx_invalid.json`](upstream/bitcoin-core/tx_invalid.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/data/tx_invalid.json) | `0c02ce44ff3a880458f9569a25589315a07f924fcaadac828613d4615776ca52` |
| [`upstream/bitcoin-core/transaction.h`](upstream/bitcoin-core/transaction.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/primitives/transaction.h) | `329c0f009edcda9dc2d3ff00e16066b1ff0f47d349a74c5cfb933aec4492c3fa` |
| [`upstream/bitcoin-core/serialize.h`](upstream/bitcoin-core/serialize.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/serialize.h) | `9acb4bab2fe63ef351429b19d6ed75a9e719705060999c4e5dba78ce7c08db6f` |
| [`upstream/bitcoin-core/core_io.cpp`](upstream/bitcoin-core/core_io.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/core_io.cpp) | `00ba5afda75e3505209f276b49b7934760ad19aed7cfdb55457533a2f74c6034` |
| [`upstream/bitcoin-core/script.cpp`](upstream/bitcoin-core/script.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/script/script.cpp) | `210b1720acce295de54c6a08c76b97fa4cfe835f64e0ff7e0730c57ebebb8c55` |
| [`upstream/bitcoin-core/script.h`](upstream/bitcoin-core/script.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/script/script.h) | `43e4eb10ee85ac31996e257d0c5dc45cbe3b29b2d61d1a72d101453ba64ded01` |
| [`upstream/bitcoin-core/consensus.h`](upstream/bitcoin-core/consensus.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/consensus/consensus.h) | `aa55254b379a2cc5f1835b506e248f80b4f020120782d2780ce52ff7c8cb827d` |
| [`upstream/rust-bitcoin/transaction.rs`](upstream/rust-bitcoin/transaction.rs) | [pinned source](https://raw.githubusercontent.com/rust-bitcoin/rust-bitcoin/17ce61c171905ec0a23155a1a31b0c67776ee436/bitcoin/src/blockdata/transaction.rs) | `2e5cd2614aa667a2cf9da4b87cfc7fd17fbf26fc610a1ba79c0dce6f2e70be8d` |
| [`upstream/rust-bitcoin/LICENSE`](upstream/rust-bitcoin/LICENSE) | [pinned source](https://raw.githubusercontent.com/rust-bitcoin/rust-bitcoin/17ce61c171905ec0a23155a1a31b0c67776ee436/LICENSE) | `7179683e8000e6bdc9bbc60d85edf0a4ac8e76f951857f54fcb775d5886f1309` |
| [`upstream/mainnet/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206.hex`](upstream/mainnet/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206.hex) | [mempool.space API](https://mempool.space/api/tx/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206/hex) | `4d9ba5d97cdb82847551d6a487e3479958b97b1c5cc9549c932c9bc261a2bf8e` |
| [`upstream/mainnet/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206.json`](upstream/mainnet/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206.json) | [mempool.space API](https://mempool.space/api/tx/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206) | `66d8bb646ababf838e207b78a0c6878c0d136709ea768b71d5ef78dc2a642f51` |
| [`upstream/mainnet/33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036.hex`](upstream/mainnet/33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036.hex) | [mempool.space API](https://mempool.space/api/tx/33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036/hex) | `4864b8eda5c76a9ae3235130b6a0370f59533a4c20962c89ec1d4ce67d1fcecc` |
| [`upstream/mainnet/33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036.json`](upstream/mainnet/33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036.json) | [mempool.space API](https://mempool.space/api/tx/33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036) | `49f1314013f8bffd9c13f8a45d54983a5f9b317c96d4b8dba1330e11a353a63b` |
| [`upstream/mainnet/37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8.hex`](upstream/mainnet/37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8.hex) | [mempool.space API](https://mempool.space/api/tx/37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8/hex) | `2098bb60eb4f47c230213ba7bd72a0b00dfcd6d295b59469b64ef1de8d72640d` |
| [`upstream/mainnet/37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8.json`](upstream/mainnet/37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8.json) | [mempool.space API](https://mempool.space/api/tx/37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8) | `8a9d44ab04ac3e4cf22472446690fcc5fad2b69ef9fb2d8711809338c51270b1` |
| [`upstream/bitcoin-core/util_tests.cpp`](upstream/bitcoin-core/util_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/util_tests.cpp) | `ff05ad11d27d2444a7ced5a31c933fa2793ac5f8d4d3cc52b5f17fdefbc6cca7` |
| [`upstream/bitcoin-core/rpc_signmessagewithprivkey.py`](upstream/bitcoin-core/rpc_signmessagewithprivkey.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/rpc_signmessagewithprivkey.py) | `56e6f567ff363e884b27c03baee123deb9600ee2f62825317257a3e133fdd30b` |
| [`upstream/secp256k1/recovery/tests_impl.h`](upstream/secp256k1/recovery/tests_impl.h) | [pinned source](https://raw.githubusercontent.com/bitcoin-core/secp256k1/6e2c8bc4ecdc6e71dbe7a368f360d8d453ce435d/src/modules/recovery/tests_impl.h) | `fc23430ea6e87f8f37bbf641236e71ead587e5438cf3d34f93de7ef39ebf3978` |
| [`upstream/secp256k1/COPYING`](upstream/secp256k1/COPYING) | [pinned source](https://raw.githubusercontent.com/bitcoin-core/secp256k1/6e2c8bc4ecdc6e71dbe7a368f360d8d453ce435d/COPYING) | `a735999c7e5649df6fcda6fb06ab97435851c392b1b93494ae8725f37441632f` |
| [`derived/catalog.json`](derived/catalog.json) | `make_catalog.py` (derived) | `eb5be14f52c41f115153b3d9d4189b30c1c984e62ddc1278793eef6b80321d27` |
