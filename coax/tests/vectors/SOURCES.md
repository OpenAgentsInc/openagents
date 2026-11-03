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
python3 coax/tests/vectors/check_pins.py           # the Core target, inventory, citations, chapter index
python3 coax/tests/vectors/test_pins.py
python3 coax/tests/vectors/check_vectors.py --fetch   # explicit network comparison
```

These four commands are offline and need only Python's standard library as
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
  parent public key starts with the child's parent fingerprint. Bitcoin
  chapter 32 uses all of test vectors 1 to 5: every chain of vectors 1 to 4
  as an `xprv` and an `xpub` from its seed, and each of vector 5's 16 invalid
  keys with the reason BIP-32 gives. `coax/tests/bitcoin/make_bip32_vectors.py`
  reads them from this file after checking its SHA-256.
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
- **Addresses (Bitcoin chapter 29):** BIP-86, BIP-84 and BIP-49 at the same
  revision as BIP-350's, retrieved 2026-11-02. Chapter 29 copies BIP-86's
  three internal keys with their output keys, scriptPubKeys and addresses,
  BIP-84's three public keys with their P2WPKH addresses, and BIP-49's
  public key, key hash, redeem script, script hash and P2SH-P2WPKH testnet
  address. The extended keys and mnemonics in the same texts are not used.
  It also reads the one `scriptPubKey` case of BIP-341's
  `wallet-test-vectors.json` that has no script tree (its tweak, output key,
  scriptPubKey and address), all 54 addresses of `key_io_valid.json` with
  their scriptPubKeys, and all 70 strings of `key_io_invalid.json`.
- **Derivation paths (Bitcoin chapter 33):** BIP-44 at the same revision,
  retrieved 2026-11-03. Chapter 33 copies its six example paths; BIP-44
  declares no license, so the text is kept for test provenance only. The
  chapter also reads BIP-86's account keys and BIP-84's and BIP-49's account
  and address keys from the files pinned for chapter 29, and NIP-06's first
  vector from `nostr/nips/official/06.md`.
- **Output descriptors (Bitcoin chapter 30):** BIP-380 to BIP-386 and BIP-389
  at the bips revision above, and Bitcoin Core's `src/test/descriptor_tests.cpp`
  at the Core revision above, retrieved 2026-11-03. The BIPs are BSD-2-Clause;
  Core's file is MIT. `coax/tests/bitcoin/descriptor.py` reads the BIPs' test
  vectors and Core's `Check`, `CheckUnparsable` and `CheckMultipath` calls,
  runs BIP-380's `descsum_create` and `descsum_check` from the pinned text, and
  generates `coax/tests/bitcoin/descriptor_vectors.coax` from the BIPs.
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
- **Merkle trees (Bitcoin chapter 46):** Bitcoin Core's
  `consensus/merkle.cpp` and `merkle.h` (`ComputeMerkleRoot` with its
  `mutated` flag, `BlockWitnessMerkleRoot`, `TransactionMerklePath`),
  `test/merkle_tests.cpp` (`merkle_test` and the older tree builder it checks
  against), `validation.cpp` (`CheckMerkleRoot`, `IsBlockMutated`, and how a
  mutated block is refused without marking its header invalid) and
  `policy/policy.h` (`MIN_STANDARD_TX_NONWITNESS_SIZE`), all at the same
  revision. `block413567.raw` is Core's benchmark block (1,557
  transactions) from `src/bench/data/`. BIP-54 (Consensus Cleanup, bips
  commit `927b6de9`, CC0-1.0) states the proposed 64-byte transaction rule;
  it has no deployment parameters at that commit. The `mainnet/` block files
  are mempool.space's raw bytes and JSON for block 100,000 (`…1afd33e506`,
  four transactions) and block 514,999 (`…0ea61097`, 19 transactions, five
  of them SegWit), retrieved 2026-11-02; each header hashes to its block
  hash and its Merkle root commits to every transaction.
- **Block headers and proof of work (Bitcoin chapter 47):** Bitcoin Core's
  `pow.cpp` (`DeriveTarget`, `CheckProofOfWorkImpl`), `arith_uint256.h` and
  `arith_uint256.cpp` (`base_uint<256>`, `SetCompact`, `GetCompact`),
  `chain.cpp` (`GetBitsProof`), `primitives/block.h` (the header layout),
  `test/pow_tests.cpp` (the `CheckProofOfWork` tests and `GetBitsProof_test`)
  and `test/arith_uint256_tests.cpp` (`bignum_SetCompact`), all at revision
  `ddea9196`. The `.header` files are mempool.space's 80-byte headers, as
  hex, for the genesis block, blocks 32,250 to 32,260 (the first change of
  target, at 32,256) and block 900,000, retrieved 2026-11-02; each hashes to
  the block hash in its file name, so the bytes cannot change.
- **Retargeting and chainwork (Bitcoin chapter 48):** Bitcoin Core's
  `pow.cpp` (`GetNextWorkRequired`, `CalculateNextWorkRequired`,
  `PermittedDifficultyTransition`), `test/pow_tests.cpp` (the retarget
  cases), `kernel/chainparams.cpp` (`nMinimumChainWork`,
  `defaultAssumeValid`) and `consensus/consensus.h` (`MAX_TIMEWARP`), all
  pinned above at `ddea9196`. `mainnet/retarget-boundaries.txt` holds 968
  mainnet headers, one per line as height, block hash and the 80 bytes in
  hex: the genesis block, heights 2016k - 1 and 2016k for every k from 1 to
  483, and block 966,143. They were assembled on 2026-11-02 from
  mempool.space's `/api/v1/blocks/{height}` records, fetched at each
  multiple of 2,016 and at 966,143; each line's 80 bytes hash to its block
  hash, so the bytes cannot change. The file is not one download, so
  `check_vectors.py --fetch` cannot re-read it from its URL;
  `make_retarget_vectors.py` checks every header's hash and proof of work
  instead.
- **P2P messages (Bitcoin chapter 65):** Bitcoin Core's `protocol.h` and
  `protocol.cpp` (the message header, the message types, `CInv` and the
  service flags), `net.h` and `net.cpp` (`V1Transport`'s header and checksum
  checks, `MAX_PROTOCOL_MESSAGE_LENGTH`), `netaddress.h` and `netaddress.cpp`
  (BIP-155's network IDs and address sizes), `net_processing.cpp` (the
  `version`, `inv`, `getheaders`, `headers`, `addr` and `feefilter` limits)
  and `test/functional/test_framework/messages.py` (Core's own Python
  serializer, the cross-check's oracle), all at revision `ddea9196`, with
  `chain.cpp` (`LocatorEntries`) and `kernel/chainparams.cpp` (each network's
  message start) pinned above. BIP-155's text at the bips revision `927b6de9`
  gives the `addrv2` format. The developer reference's
  `reference/p2p_networking.rst` at developer.bitcoin.org revision `813ba3fb`
  gives the annotated hexdumps of a `verack` header and of the `getblocks`,
  `headers`, `inv`, `addr`, `addrv2`, `feefilter`, `ping` and `version`
  payloads; `make_p2p_vectors.py` reads them from the pinned bytes.
- **Taproot signature hashes (Bitcoin chapter 43):** BIP-341's
  `wallet-test-vectors.json` gives the seven key-path `sigMsg` and `sigHash`
  values; it has no script-path or annex case. For those, Bitcoin Core's
  `test/functional/test_framework/script.py` at revision `ddea9196` is the
  oracle: `taproot_reference.py` runs its `TaprootSignatureMsg` and
  `BIP341_sha_*` functions as written, checks that they give the seven
  published messages, and `make_taproot_sighash_vectors.py` uses them to
  write the script-path and annex messages the Coax tests compare. Those
  are generated values, not published ones.
- **ElligatorSwift (Bitcoin chapter 66):** BIP-324's text, its
  `reference.py` and its three test-vector files, all at the bips revision
  `927b6de9`. `ellswift_decode_test_vectors.csv` gives 76 encodings and
  their x, `xswiftec_inv_test_vectors.csv` 32 pairs of u and x with the t
  of each of the eight cases, and `packet_encoding_test_vectors.csv` seven
  key exchanges, whose decoded x, shared x and shared secret chapter 66
  claims. `make_ellswift_vectors.py` reads all three, and `ellswift.py`
  runs `reference.py` as the cross-check's oracle.
- **The v2 transport (Bitcoin chapter 67):** BIP-324's
  `packet_encoding_test_vectors.csv` and `reference.py`, pinned above for
  chapter 66, give seven packets with every derived key, terminator and
  session ID, and the whole ciphertext or its last 128 bytes. Bitcoin
  Core's `bip324.cpp` (`BIP324Cipher`) at revision `ddea9196` is pinned
  for the cipher's structure; `net.cpp` (`V2Transport`, `V2_MESSAGE_IDS`,
  `GetMessageType`, `ProcessReceivedGarbageBytes`), `net.h`, `protocol.h`
  and `serialize.h`, pinned above, give the message-ID table and the
  limits. `make_v2transport_vectors.py` reads the vectors, and
  `v2transport.py` runs `reference.py` and reads the Core files as the
  cross-check's oracle.
- **Transaction checks (Bitcoin chapter 52):** Bitcoin Core's
  `consensus/tx_check.cpp` (`CheckTransaction`) at revision `ddea9196`,
  with the already pinned `consensus/tx_verify.cpp` (`CheckTxInputs`,
  `IsFinalTx`), `transaction_tests.cpp`, `tx_valid.json` and
  `tx_invalid.json`.
- **Sigops, weight and block limits (Bitcoin chapter 53):** Bitcoin
  Core's `test/sigopcount_tests.cpp` and `test/miner_tests.cpp` at
  revision `ddea9196`, with the already pinned `script.cpp`, `script.h`,
  `interpreter.cpp`, `tx_verify.cpp`, `validation.cpp` and `consensus.h`.
  `mainnet/<block 514,999>.spent.json` is mempool.space's record of the
  outputs block 514,999 spends, with each transaction's fee and sigop
  cost; `make_sigops_vectors.py` checks every spent script against the
  block's own scriptSigs and witnesses, and the fees against the coinbase.
- **FROST (Bitcoin chapter 102):** the FROST draft BIP, BIP-445
  (`bip-0445.md`, version 0.12.1), its reference implementation
  (`python/frost_ref/`, `trusted_dealer.py`, `tests.py`), the vendored
  secp256k1lab it runs on and its seven vector files at
  siv2r/bip-frost-signing revision `ba299bad`. `make_frost_vectors.py`
  copies every case into Coax data; `frost.py` checks every pin, runs
  the reference's tests over the pinned files and writes the example's
  transcript with its functions.
- **MuSig2 (Bitcoin chapter 101):** BIP-327 (`bip-0327.mediawiki`), its
  reference implementation `reference.py` and its eight vector files
  (`key_sort`, `key_agg`, `nonce_gen`, `nonce_agg`, `sign_verify`,
  `tweak`, `det_sign` and `sig_agg`) at bitcoin/bips revision
  `ada34196`. `make_musig_vectors.py` copies every case into Coax data;
  `musig.py` runs the pinned `reference.py` over the pinned files and
  writes the example's transcript with its functions.
- **Adaptor signatures (Bitcoin chapter 103):** no BIP publishes adaptor
  signature vectors, so `coax/tests/bitcoin/adaptor.py` generates them.
  It implements pre-signing, pre-verification, adapting and extraction on
  the point and hash functions of BIP-340's pinned `reference.py`
  (`upstream/bip340/reference.py`, checked by hash), checks every final
  signature with that file's `schnorr_verify`, and writes
  `coax/tests/bitcoin/adaptor_vectors.coax`. Its keys, aux bytes, secrets
  and messages are SHA-256 of fixed labels. Test data only.
- **The subsidy and the coinbase (Bitcoin chapter 54):** Bitcoin Core's
  `test/validation_tests.cpp` at revision `ddea9196`, with the already
  pinned `validation.cpp`, `script.h`, `amount.h` and `chainparams.cpp`.
  `mainnet/coinbase-proofs.json` holds the header, coinbase, transaction
  count, fees and reward of 14 mainnet blocks as mempool.space serves
  them, with the coinbase's Merkle branch computed from the block's txid
  list; `make_subsidy_vectors.py` proves each coinbase in its header, links
  each pair of neighbouring blocks by the previous-block hash, and checks
  each claim against the reward and each BIP-34 height.
- **PSBT (Bitcoin chapter 94):** BIP-174, BIP-370 and BIP-371 at bips
  revision `5aadddb5`, whose test-vector sections hold every published
  valid and invalid PSBT and BIP-174's worked example; Bitcoin Core's
  `psbt.h`, `psbt.cpp`, `pubkey.h`, `wallet/test/psbt_wallet_tests.cpp`
  and `test/functional/data/rpc_psbt.json` at revision `ddea9196`, with
  the already pinned tiny-secp256k1 `ecdsa.json` for the `extraEntropy`
  rows. `make_psbt_vectors.py` decides each of the 155 distinct PSBTs with
  `psbt_reference.py` and checks the verdict against the published one,
  replays Core's signer, combiner, finalizer and extractor cases, and
  recomputes each `extraEntropy` signature.
- **Coin selection (Bitcoin chapter 95):** Bitcoin Core's
  `wallet/coinselection.cpp`, `wallet/coinselection.h`, `wallet/spend.cpp`,
  `policy/feerate.cpp`, `wallet/test/coinselector_tests.cpp` and
  `wallet/test/coinselection_tests.cpp` at revision `ddea9196`.
  `make_coinselect_vectors.py` ports each Branch and Bound, CoinGrinder,
  Single Random Draw, Knapsack and waste case, runs it through
  `coinselect_reference.py`, a transliteration of Core's selectors with
  its ChaCha20 random draws, and checks the result against the one Core's
  test states.
- **Generic signed messages (Bitcoin chapter 96):** BIP-322 and its
  `basic-test-vectors.json` and `generated-test-vectors.json` at bips
  revision `5aadddb5`. `make_bip322_vectors.py` checks every hash,
  signature and error against `bip322_reference.py`, a Python signer and
  verifier, and adds constructed cases for the upgradeable rules and
  Taproot script paths.
- **Silent payments (Bitcoin chapter 97):** BIP-352 version 1.1.2,
  its `send_and_receive_test_vectors.json`, its `reference.py` with the
  helpers and the secp256k1lab it imports, at bips revision `5aadddb5`.
  `silent.py` runs `reference.py` on the vectors, and
  `make_silent_vectors.py` checks every case against
  `silent_reference.py`, a Python implementation, and adds constructed
  addresses for the version rules.
- **Multisig (Bitcoin chapter 100):** BIP-67 and BIP-387 at bips
  revision `5aadddb5`, whose test-vector sections hold the sorted-key
  2-of-n vectors and the `multi_a` and `sortedmulti_a` descriptors with
  their scriptPubKeys; Bitcoin Core's `script/solver.cpp` at revision
  `ddea9196`, the `MatchMultisig` and `MatchMultiA` that
  `make_multisig_vectors.py` transliterates. It recomputes each BIP-67
  script and address and each BIP-387 scriptPubKey, resolving WIF, hex
  and BIP-32 keys in Python.
- **Header rules (Bitcoin chapter 51):** Bitcoin Core's `validation.cpp`
  (`ContextualCheckBlockHeader`), `chain.h`, `consensus/consensus.h`,
  `versionbits.h`, `versionbits_impl.h` and `versionbits.cpp` at revision
  `ddea9196`, and `chainparams.cpp` at v22.0 (`a0988140`) for Taproot's
  Speedy Trial deployment. The `.headers` files are mempool.space's
  80-byte headers of mainnet blocks 840,102 to 840,113 and of testnet4's
  genesis and blocks 2,005 to 2,016, in a row, retrieved 2026-11-03.
- **The witness commitment (Bitcoin chapter 49):** BIP-141 (bips commit
  `927b6de9`, public domain) states the commitment, the reserved value and
  the rule that the last matching output counts. Bitcoin Core's
  `consensus/validation.h` (`GetWitnessCommitmentIndex`,
  `MINIMUM_WITNESS_COMMITMENT`), pinned as `consensus_validation.h`, and
  `validation.cpp` (`CheckWitnessMalleation`, `IsBlockMutated`,
  `GenerateCoinbaseCommitment`), pinned above, give the rules.
  `test_framework/blocktools.py` (`add_witness_commitment`,
  `get_witness_script`) and `messages.py` build the cases from mainnet
  blocks 514,999 and 100,000, and `make_witness_vectors.py` transliterates
  the rules as the cross-check's oracle.
- **The handshake (Bitcoin chapter 68):** Bitcoin Core's
  `node/protocol_version.h` at revision `ddea9196` gives the protocol
  versions. `net_processing.cpp` (`ProcessMessage`, `PushNodeVersion`,
  `GetDesirableServiceFlags`, the final alert), `net.cpp`
  (`CheckIncomingNonce`, `InactivityCheck`, `V2Transport`), `net.h`,
  `protocol.h` and `messages.py`, pinned above, give the rules, limits and
  payloads. `make_handshake_vectors.py` writes Core's payload bytes, and
  `handshake.py` transliterates the rules and runs BIP-324's `reference.py`
  as the cross-check's oracle.
- **Compact blocks (Bitcoin chapter 73):** BIP-152 (bips commit
  `927b6de9`, public domain) states the messages, the short IDs and the two
  modes. Bitcoin Core's `blockencodings.h` and `blockencodings.cpp`
  (`CBlockHeaderAndShortTxIDs`, `PartiallyDownloadedBlock`) give the
  rebuilding rules, `test/blockencodings_tests.cpp` the cases the library
  ports, and `net_processing.cpp`, pinned above, the high-bandwidth choice
  and the depth limits, all at revision `ddea9196`.
  `test_framework/crypto/siphash.py` gives `messages.py` its
  `siphash256`; `make_cmpct_vectors.py` builds the messages with them from
  `block413567.raw` and Core's test blocks, and `cmpct.py` transliterates
  the rules over mainnet block 514,999 as the cross-check's oracle.
- **Addresses and the address manager (Bitcoin chapter 69):** Bitcoin
  Core's `addrman.h`, `addrman_impl.h` and `addrman.cpp` (`AddrManImpl`:
  buckets, `Add`, `Good`, `Select`, `GetAddr`, test-before-evict and the
  `peers.dat` format), `netgroup.h` and `netgroup.cpp` (`GetGroup`),
  `random.h` (`FastRandomContext`) and `test/addrman_tests.cpp`, the cases
  the library ports, all at revision `ddea9196`. `net_processing.cpp`
  (`ProcessAddrs`, `RelayAddress`, the `getaddr` handler), `net.cpp` (the
  `getaddr` cache, `ThreadDNSAddressSeed`), `netaddress.h`,
  `netaddress.cpp`, `protocol.h` and `chacha20.py`, pinned above, give the
  rest. `addrman.py` transliterates them as the cross-check's oracle.
- **Connections (Bitcoin chapter 70):** Bitcoin Core's
  `node/eviction.cpp` and `node/eviction.h` (`SelectNodeToEvict`,
  `ProtectEvictionCandidatesByRatio`), `netbase.cpp` (`IsBadPort`),
  `random.cpp` (`MakeExponentiallyDistributed`), `test/util/net.cpp`
  (`GetRandomNodeEvictionCandidates`) and
  `test/net_peer_eviction_tests.cpp`, the cases the library ports, all at
  revision `ddea9196`. `net.cpp` (`ThreadOpenConnections`,
  `CreateNodeFromAcceptedSocket`), `net.h`, `net_processing.cpp`
  (`EvictExtraOutboundPeers`), `random.h`, `addrman.cpp` and
  `netgroup.cpp`, pinned above, give the rest. `connman.py` transliterates
  them as the cross-check's oracle.
- **Denial of service (Bitcoin chapter 71):** Bitcoin Core's
  `banman.cpp` and `banman.h` (`BanMan`), `common/bloom.cpp` and
  `common/bloom.h` (`CRollingBloomFilter`), `hash.cpp` (`MurmurHash3`),
  `util/fastrange.h`, `headerssync.cpp` and `headerssync.h`,
  `net_processing.h`, `node/txdownloadman.h`,
  `node/txdownloadman_impl.cpp`, `net_permissions.h`, and
  `test/denialofservice_tests.cpp` and `test/bloom_tests.cpp`, the cases
  the library ports, all at revision `ddea9196`. `net.cpp`, `net.h`,
  `net_processing.cpp`, `chainparams.cpp`, `chain.h`, `serialize.h`,
  `script.h` and `test/hash_tests.cpp`, pinned above, give the rest.
  `dos.py` transliterates them as the cross-check's oracle.
- **Standardness (Bitcoin chapter 81):** Bitcoin Core's
  `policy/policy.cpp` and `policy/policy.h` (`IsStandardTx`,
  `GetDustThreshold`, `ValidateInputsStandardness`, `IsWitnessStandard`
  and the limits), `script/solver.cpp` (`Solver`), `validation.cpp`
  (the order of `PreChecks`), `script/script.cpp` and `script/script.h`
  (`IsPayToAnchor`, `IsUnspendable`), and `test/transaction_tests.cpp`
  (`test_IsStandard` and `max_standard_legacy_sigops`, the cases the
  library ports), all pinned above at revision `ddea9196`. `policy.py`
  transliterates them as the cross-check's oracle.
- **The block index (Bitcoin chapter 59):** Bitcoin Core's
  `node/blockstorage.cpp` (`AddToBlockIndex`, `CBlockIndexWorkComparator`),
  `validation.h` (`nBlockSequenceId`), `rpc/blockchain.cpp`
  (`GetDifficulty`), and `test/skiplist_tests.cpp` and
  `test/blockchain_tests.cpp`, the cases the library ports, all at revision
  `ddea9196`. `chain.h`, `chain.cpp`, `validation.cpp`, `pow.cpp` and
  `kernel/chainparams.cpp`, pinned above, give the rest. `blockindex.py`
  transliterates them as the cross-check's oracle.
  `mainnet/headers-0-2016.txt` holds mainnet's first 2,017 headers, genesis
  to block 2,016, one per line as height, block hash and the 80 bytes in
  hex, assembled on 2026-11-03 from mempool.space's
  `/api/v1/blocks/{height}` records; each line's 80 bytes hash to its block
  hash and name the line before, so the bytes cannot change.
  `make_blockindex_vectors.py` checks every hash and link.
- **Chain parameters (Bitcoin chapter 50):** Bitcoin Core's
  `kernel/chainparams.cpp` (every network's class), `kernel/signet.h`
  (`SIGNET_DEFAULT_CHALLENGE`, `GetSignetMessageStart`), `signet.cpp`
  (`SignetTxs::Create`, `CheckSignetBlockSolution`), `consensus/params.h`
  (the buried deployments) and `validation.cpp` (`IsBIP30Repeat`,
  `IsBIP30Unspendable`), all at revision `ddea9196`. The `signet/` files are
  mempool.space's raw bytes of default-signet blocks 1 (`…533b53`, the
  coinbase alone) and 19,990 (`…539466`, four transactions), retrieved
  2026-11-03; each header hashes to its block hash.
- **The UTXO set and MuHash3072 (Bitcoin chapter 56):** Bitcoin Core's
  `crypto/muhash.cpp` and `muhash.h`, `test/crypto_tests.cpp` (the
  `muhash_tests` vectors), `compressor.cpp`, `compressor.h` and
  `test/compress_tests.cpp`, `coins.h` (the coin record), `serialize.h`
  (VARINT), `kernel/coinstats.cpp` (`TxOutSer`, `hash_serialized_3` and
  `muhash`), `txdb.cpp` (the `'C'` key), `node/utxo_snapshot.h` (the
  `dumptxoutset` header) and `node/miner.cpp` (the coinbase layout), all at
  the same revision, retrieved 2026-11-03. The functional tests
  `feature_utxo_set_hash.py` and `rpc_dumptxoutset.py` publish the hashes the
  chapter matches; `coax/tests/bitcoin/utxo.py` reads those values from the
  pinned text and rebuilds both regtest chains with the pinned
  `test_framework/wallet.py`, `address.py` and `test_node.py` (the mock
  time). It runs Core's own `test_framework/crypto/muhash.py` and
  `chacha20.py` from the pins as the independent MuHash. All are MIT.
- **The chainstate store (Bitcoin chapter 62):** Bitcoin Core's
  `util/obfuscation.h` (the XOR key and its rotations), `dbwrapper.cpp` and
  `dbwrapper.h` (the `"\0obfuscate_key"` record, synced batch writes) and
  `txdb.cpp` (the `'B'` and `'H'` markers around a flush in partial
  batches), at the same revision, retrieved 2026-11-03, all MIT.
  `coax/tests/bitcoin/kvstore.py` checks that the two lines of
  `obfuscation.h` its transliteration rests on are in the pinned file. The
  store's format is the book's own, so no vector file is pinned for it.
- **Compact block filters (Bitcoin chapter 74):** BIP-158's
  `testnet-19.json` (ten testnet blocks with their spent scripts, basic
  filters and filter headers) and the BIP-157 and BIP-158 texts, at bips
  revision `5aadddb5`, CC0-1.0; and Bitcoin Core's `blockfilter.cpp`,
  `util/golombrice.h`, `test/blockfilter_tests.cpp`,
  `test/data/blockfilters.json` (the same file as `testnet-19.json`),
  `test_framework/blockfilter.py` and `test_framework/crypto/siphash.py`,
  at the same Core revision as above, all MIT, retrieved 2026-11-03.
  `coax/tests/bitcoin/blockfilter.py` runs Core's two Python files as the
  independent element hash and SipHash, and generates
  `coax/tests/bitcoin/blockfilter_vectors.coax` from `testnet-19.json`.
- **Lightning messages and TLVs (Bitcoin chapter 104):** BOLT 1
  (`01-messaging.md`) and BOLT 9 (`09-features.md`) at lightning/bolts
  revision `19146282`, CC-BY-4.0 as each text declares, retrieved
  2026-11-03. BOLT 1's Appendix A gives 18 BigSize decodings and 8
  encodings, Appendix B 57 TLV streams in its test namespaces `n1` and
  `n2`, and Appendix C 5 `init` messages; BOLT 9's table names the
  feature bits. `coax/tests/lightning/make_bolt1_vectors.py` reads them
  into `coax/tests/lightning/bolt1_vectors.coax`, and
  `coax/tests/lightning/bolt1.py` is the independent reader the
  cross-check runs.
- **The Noise handshake (Bitcoin chapter 105):** BOLT 8
  (`08-transport.md`) at the same lightning/bolts revision, CC-BY-4.0,
  retrieved 2026-11-03. Its Appendix A gives 5 initiator and 10 responder
  handshake tests, with every act, the session keys or the expected error,
  and the message test's six outputs across two key rotations.
  `coax/tests/lightning/make_noise_vectors.py` reads them into
  `coax/tests/lightning/noise_vectors.coax`, and
  `coax/tests/lightning/noise.py` is the independent handshake the
  cross-check runs, on `coax/tests/lightning/ln_crypto.py`.
- **Shachain (Bitcoin chapter 107):** BOLT 3 (`03-transactions.md`) at
  the same lightning/bolts revision, CC-BY-4.0, retrieved 2026-11-03. Its
  Appendix D gives 5 per-commitment secret generation tests and 9 storage
  tests, 56 inserts with the one BOLT 3 marks as an error.
  `coax/tests/lightning/make_shachain_vectors.py` reads them into
  `coax/tests/lightning/shachain_vectors.coax`, and
  `coax/tests/lightning/shachain.py` is the independent generator and
  store the cross-check runs.
- **Commitment keys and revocation (Bitcoin chapter 106):** the same
  pinned BOLT 3. Its Appendix E gives a base secret and a per-commitment
  secret, their points, and the four derived keys: `localpubkey`,
  `localprivkey`, `revocationpubkey` and `revocationprivkey`.
  `coax/tests/lightning/make_keys_vectors.py` reads them into
  `coax/tests/lightning/keys_vectors.coax`, and
  `coax/tests/lightning/keys.py` is the independent derivation the
  cross-check runs.
- **Commitment and HTLC transactions (Bitcoin chapter 108):** the same
  pinned BOLT 3. Its Appendix B gives the funding transaction, Appendix C
  16 commitments under `option_static_remotekey` and Appendix F 9 under
  `option_anchors`, with 48 HTLC-timeout and HTLC-success transactions
  between them, all signed by both peers.
  `coax/tests/lightning/make_commitment_vectors.py` reads them into
  `coax/tests/lightning/commitment_vectors.coax` after
  `coax/tests/lightning/bolt3_tx.py`, an independent Python builder and
  RFC 6979 signer, rebuilds every one, and
  `coax/tests/lightning/commitment.py` is the cross-check.
- **Open, update and close a channel (Bitcoin chapter 109):** BOLT 2
  (`02-peer-protocol.md`) at the same lightning/bolts revision,
  CC-BY-4.0, retrieved 2026-11-03: the message fields and rules of
  channel establishment v1, normal operation, message retransmission and
  the `closing_signed` close. BOLT 2 has no test vectors, so
  `coax/tests/lightning/channel.py`, an independent Python model of the
  protocol built on `bolt3_tx.py`, computes every line of the example;
  the closing transaction follows the pinned BOLT 3.
- **Onion routing and blinded paths (Bitcoin chapter 111):** BOLT 4
  (`04-onion-routing.md`) and its five vector files in `bolt04/` at the
  same lightning/bolts revision, CC-BY-4.0, retrieved 2026-11-03:
  `onion-test.json` (a five-hop onion and each hop's key),
  `onion-error-test.json` (each hop's shared secret and ammag key, and an
  error packet), `route-blinding-test.json` (a blinded path made of two
  joined paths, and each hop unblinding it),
  `blinded-payment-onion-test.json` (a payment onion into a blinded path)
  and `blinded-onion-message-onion-test.json` (an onion message along a
  blinded path). BOLT 4's "Returning Errors" trace adds a long failure's
  packet at every node. `coax/tests/lightning/make_onion_vectors.py` reads
  them into `coax/tests/lightning/onion_vectors.coax`, and
  `coax/tests/lightning/onion.py` runs them and the example on
  `coax/tests/lightning/sphinx.py`, the independent Sphinx.
- **Gossip (Bitcoin chapter 112):** BOLT 7 (`07-routing-gossip.md`) and
  `bolt07/extended-queries.json` at the same lightning/bolts revision,
  CC-BY-4.0, retrieved 2026-11-03. BOLT 7's "Test vectors" give six
  `node_announcement`s signed with the key 0x11...11, each with a
  forbidden alias; the JSON file gives ten query and reply messages, five
  of them in zlib, which BOLT 7 now forbids. BOLT 7 has no
  `channel_announcement` or `channel_update` vectors; the chapter's
  example signs its own with generated keys.
  `coax/tests/lightning/make_gossip_vectors.py` reads the vectors into
  `coax/tests/lightning/gossip_vectors.coax`, and
  `coax/tests/lightning/gossip.py` runs them and the example on
  `coax/tests/lightning/bolt7.py`, the independent reader and writer.
- **Invoices (Bitcoin chapter 114):** BOLT 11 (`11-payment-encoding.md`)
  and `bolt11/invoice-test.json` at the same lightning/bolts revision,
  CC-BY-4.0, retrieved 2026-11-03. The JSON file gives 31 invoices, 16
  valid and 15 invalid; BOLT 11's Examples section gives the key that
  signs them, the payment secret, and for 11 of them the signature, the
  recovery flag, the signed data and its SHA-256.
  `coax/tests/lightning/make_invoice_vectors.py` reads them into
  `coax/tests/lightning/invoice_vectors.coax`, and
  `coax/tests/lightning/invoice.py` runs them and the example on
  `coax/tests/lightning/bolt11.py`, the independent reader and writer.
- **Offers (Bitcoin chapter 115):** BOLT 12 (`12-offer-encoding.md`)
  and `bolt12/format-string-test.json`, `offers-test.json`,
  `payer-proof-test.json` and `signature-test.json` at the same
  lightning/bolts revision, CC-BY-4.0, retrieved 2026-11-03. They give
  12 format strings, 53 offers (20 valid), the n1 Merkle roots and a
  signed invoice request, and 5 valid and 23 invalid payer proofs with
  their keys. Seven invalid offers are malformed earlier than their
  description says; `coax/tests/lightning/make_offer_vectors.py` records
  the earlier reason, reads the vectors into
  `coax/tests/lightning/offer_vectors.coax`, and
  `coax/tests/lightning/offers.py` runs them and the example on
  `coax/tests/lightning/bolt12.py`, the independent reader and writer.
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

## The Bitcoin Core target

The Bitcoin chapters pin Bitcoin Core at one revision, recorded in
`pins.json`: `ddea9196493d3b8fd841a36a6c53efd85dac7a05`, the merge of
bitcoin#36554 on master on 2026-11-02. It is not a release tag. On
2026-11-03 it was 741 commits past `d25470d9`, where the v32 release
branch left master, and the latest release was v32.1 (`f0de87f0`,
2026-10-26). `check_pins.py` reports every Bitcoin Core file pinned at
another revision and fails unless `pins.json` lists it with a reason. One
file is: `chainparams_v22.cpp`, at v22.0 (`a0988140`), for Taproot's
deployment parameters (Bitcoin chapter 51).

Six files are assembled from many records rather than copied from one URL:
block headers and spent outputs read from mempool.space one height or one
block at a time. Their manifest entries say `"assembled": true`, and
`check_vectors.py --fetch` skips them, since no single URL returns their
bytes. Their SHA-256 is the pin, and the chapters that read them check
that each header hashes to its block hash and names the one before.

## Pins by chapter

`check_pins.py --write` builds this list from the chapters' links; the
check fails when a chapter cites a pinned file that is missing here.

<!-- pins-by-chapter:begin -->
- Bitcoin chapter 4: `upstream/nist`
- Bitcoin chapter 6: `upstream/bip32/bip-0032.mediawiki`, `upstream/ripemd160/AB-9601.pdf`
- Bitcoin chapter 7: `upstream/nist`
- Bitcoin chapter 9: `upstream/nip44/nip44.vectors.json`
- Bitcoin chapter 10: `upstream/bitcoin-core/hash_tests.cpp`, `upstream/siphash/vectors.h`
- Bitcoin chapter 11: `upstream/rfc/rfc8439.txt`
- Bitcoin chapter 12: `upstream/rfc/rfc8439.txt`, `upstream/wycheproof/chacha20_poly1305_test.json`
- Bitcoin chapter 16: `upstream/bip340/test-vectors.csv`
- Bitcoin chapter 18: `upstream/bip340/test-vectors.csv`
- Bitcoin chapter 19: `upstream/wycheproof`, `upstream/wycheproof/ecdsa_secp256k1_sha256_bitcoin_test.json`
- Bitcoin chapter 20: `upstream/tiny-secp256k1`, `upstream/tiny-secp256k1/ecdsa.json`
- Bitcoin chapter 21: `upstream/bip66`, `upstream/bip66/fixtures.json`, `upstream/bitcoin-core`
- Bitcoin chapter 22: `upstream/bitcoin-core`, `upstream/bitcoin-core/util_tests.cpp`, `upstream/secp256k1`, `upstream/secp256k1/recovery/tests_impl.h`, `upstream/tiny-secp256k1`, `upstream/tiny-secp256k1/ecdsa.json`
- Bitcoin chapter 23: `upstream/bip340/test-vectors.csv`
- Bitcoin chapter 24: `upstream/bip340/test-vectors.csv`
- Bitcoin chapter 26: `upstream/bip32`, `upstream/bip32/bip-0032.mediawiki`, `upstream/bitcoin-core`, `upstream/bitcoin-core/base58_encode_decode.json`, `upstream/bitcoin-core/chainparams.cpp`
- Bitcoin chapter 27: `upstream/bip173`, `upstream/bip173/bip-0173.mediawiki`, `upstream/nips/19.md`
- Bitcoin chapter 28: `upstream/bip173`, `upstream/bip173/bip-0173.mediawiki`, `upstream/bip350`, `upstream/bip350/bip-0350.mediawiki`, `upstream/bitcoin-core`, `upstream/bitcoin-core/key_io_valid.json`, `upstream/sipa-bech32`, `upstream/sipa-bech32/segwit_addr.py.txt`
- Bitcoin chapter 29: `upstream/bip341`, `upstream/bip341/wallet-test-vectors.json`, `upstream/bip49`, `upstream/bip49/bip-0049.mediawiki`, `upstream/bip84`, `upstream/bip84/bip-0084.mediawiki`, `upstream/bip86`, `upstream/bip86/bip-0086.mediawiki`, `upstream/bitcoin-core`, `upstream/bitcoin-core/key_io_valid.json`
- Bitcoin chapter 30: `upstream/bip380`, `upstream/bip380/bip-0380.mediawiki`, `upstream/bitcoin-core/descriptor_tests.cpp`
- Bitcoin chapter 31: `upstream/bip39`, `upstream/bip39/bip-0039.mediawiki`, `upstream/bip39/english.txt`, `upstream/bip39/vectors.json`
- Bitcoin chapter 32: `upstream/bip32`, `upstream/bip32/bip-0032.mediawiki`, `upstream/bip39`, `upstream/bip39/vectors.json`
- Bitcoin chapter 33: `upstream/bip44`, `upstream/bip44/bip-0044.mediawiki`
- Bitcoin chapter 34: `upstream/bitcoin-core`, `upstream/bitcoin-core/transaction.h`, `upstream/mainnet`, `upstream/mainnet/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206.hex`, `upstream/rust-bitcoin`, `upstream/rust-bitcoin/transaction.rs`
- Bitcoin chapter 35: `upstream/bitcoin-core`, `upstream/bitcoin-core/tx_valid.json`, `upstream/mainnet`, `upstream/mainnet/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206.json`
- Bitcoin chapter 36: `upstream/bitcoin-core`, `upstream/bitcoin-core/interpreter.h`
- Bitcoin chapter 37: `upstream/bip54/bip-0054.md`, `upstream/bitcoin-core`, `upstream/bitcoin-core/interpreter.cpp`
- Bitcoin chapter 38: `upstream/bip143/bip-0143.mediawiki`, `upstream/bitcoin-core`, `upstream/bitcoin-core/interpreter.cpp`
- Bitcoin chapter 39: `upstream/bitcoin-core`, `upstream/bitcoin-core/tx_verify.cpp`
- Bitcoin chapter 40: `upstream/bitcoin-core`, `upstream/bitcoin-core/policy.cpp`, `upstream/mainnet`, `upstream/mainnet/33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036.json`
- Bitcoin chapter 41: `upstream/bip340/reference.py`, `upstream/bip341/bip-0341.mediawiki`, `upstream/bip341/wallet-test-vectors.json`, `upstream/bip86/bip-0086.mediawiki`
- Bitcoin chapter 42: `upstream/bip341/bip-0341.mediawiki`, `upstream/bip341/wallet-test-vectors.json`, `upstream/bitcoin-core/interpreter.cpp`
- Bitcoin chapter 43: `upstream/bip341/bip-0341.mediawiki`, `upstream/bip341/wallet-test-vectors.json`, `upstream/bitcoin-core/interpreter.cpp`, `upstream/bitcoin-core/script.py`
- Bitcoin chapter 46: `upstream/bip54`, `upstream/bip54/bip-0054.md`, `upstream/bitcoin-core`, `upstream/bitcoin-core/merkle.cpp`, `upstream/mainnet`, `upstream/mainnet/000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506.json`
- Bitcoin chapter 47: `upstream/bitcoin-core`, `upstream/bitcoin-core/pow.cpp`, `upstream/mainnet`, `upstream/mainnet/000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f.header`
- Bitcoin chapter 48: `upstream/bitcoin-core`, `upstream/bitcoin-core/pow.cpp`, `upstream/mainnet/retarget-boundaries.txt`
- Bitcoin chapter 49: `upstream/bip141`, `upstream/bip141/bip-0141.mediawiki`, `upstream/bitcoin-core`, `upstream/bitcoin-core/consensus_validation.h`, `upstream/mainnet`
- Bitcoin chapter 50: `upstream/bitcoin-core`, `upstream/bitcoin-core/chainparams.cpp`, `upstream/signet`, `upstream/signet/00000086d6b2636cb2a392d45edc4ec544a10024d30141c9adf4bfd9de533b53.raw`
- Bitcoin chapter 51: `upstream/bitcoin-core`, `upstream/bitcoin-core/versionbits.cpp`, `upstream/mainnet`, `upstream/mainnet/mainnet-840102-840113.headers`, `upstream/testnet4/testnet4-0-2005-2016.headers`
- Bitcoin chapter 52: `upstream/bitcoin-core`, `upstream/bitcoin-core/tx_check.cpp`
- Bitcoin chapter 53: `upstream/bitcoin-core`, `upstream/bitcoin-core/sigopcount_tests.cpp`, `upstream/mainnet`, `upstream/mainnet/00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097.spent.json`
- Bitcoin chapter 54: `upstream/bitcoin-core`, `upstream/bitcoin-core/validation_tests.cpp`, `upstream/mainnet`, `upstream/mainnet/coinbase-proofs.json`
- Bitcoin chapter 56: `upstream/bitcoin-core`, `upstream/bitcoin-core/muhash.cpp`
- Bitcoin chapter 59: `upstream/bitcoin-core`, `upstream/bitcoin-core/skiplist_tests.cpp`, `upstream/mainnet`, `upstream/mainnet/headers-0-2016.txt`
- Bitcoin chapter 62: `upstream/bitcoin-core`, `upstream/bitcoin-core/obfuscation.h`
- Bitcoin chapter 65: `upstream/bip155`, `upstream/bip155/bip-0155.mediawiki`, `upstream/bitcoin-core`, `upstream/bitcoin-core/net.cpp`, `upstream/developer-bitcoin-org`, `upstream/developer-bitcoin-org/p2p_networking.rst`
- Bitcoin chapter 66: `upstream/bip324`, `upstream/bip324/bip-0324.mediawiki`
- Bitcoin chapter 67: `upstream/bip324`, `upstream/bip324/bip-0324.mediawiki`, `upstream/bitcoin-core`, `upstream/bitcoin-core/bip324.cpp`
- Bitcoin chapter 68: `upstream/bip324`, `upstream/bip324/reference.py`, `upstream/bitcoin-core`, `upstream/bitcoin-core/protocol_version.h`
- Bitcoin chapter 69: `upstream/bitcoin-core`, `upstream/bitcoin-core/addrman.cpp`
- Bitcoin chapter 70: `upstream/bitcoin-core`, `upstream/bitcoin-core/eviction.cpp`
- Bitcoin chapter 71: `upstream/bitcoin-core`, `upstream/bitcoin-core/banman.cpp`
- Bitcoin chapter 73: `upstream/bip152`, `upstream/bip152/bip-0152.mediawiki`, `upstream/bitcoin-core`, `upstream/bitcoin-core/blockencodings.cpp`, `upstream/mainnet`
- Bitcoin chapter 74: `upstream/bip158`, `upstream/bip158/testnet-19.json`, `upstream/bitcoin-core`, `upstream/bitcoin-core/blockfilter.cpp`
- Bitcoin chapter 81: `upstream/bitcoin-core`, `upstream/bitcoin-core/policy.cpp`
- Bitcoin chapter 94: `upstream/bip174/bip-0174.mediawiki`, `upstream/bitcoin-core`, `upstream/bitcoin-core/psbt.h`, `upstream/tiny-secp256k1`, `upstream/tiny-secp256k1/ecdsa.json`
- Bitcoin chapter 95: `upstream/bitcoin-core`, `upstream/bitcoin-core/coinselection.cpp`
- Bitcoin chapter 96: `upstream/bip322`, `upstream/bip322/bip-0322.mediawiki`
- Bitcoin chapter 97: `upstream/bip352`, `upstream/bip352/bip-0352.mediawiki`
- Bitcoin chapter 100: `upstream/bip67/bip-0067.mediawiki`, `upstream/bitcoin-core`, `upstream/bitcoin-core/solver.cpp`
- Bitcoin chapter 101: `upstream/bip327`, `upstream/bip327/bip-0327.mediawiki`
- Bitcoin chapter 102: `upstream/bip445`, `upstream/bip445/bip-0445.md`
- Bitcoin chapter 103: `upstream/bip340`, `upstream/bip340/reference.py`
- Bitcoin chapter 104: `upstream/bolts`, `upstream/bolts/01-messaging.md`
- Bitcoin chapter 105: `upstream/bolts`, `upstream/bolts/08-transport.md`
- Bitcoin chapter 106: `upstream/bolts`, `upstream/bolts/03-transactions.md`
- Bitcoin chapter 107: `upstream/bolts`, `upstream/bolts/03-transactions.md`
- Bitcoin chapter 108: `upstream/bolts`, `upstream/bolts/03-transactions.md`
- Bitcoin chapter 109: `upstream/bolts`, `upstream/bolts/02-peer-protocol.md`
- Bitcoin chapter 111: `upstream/bolts`, `upstream/bolts/04-onion-routing.md`, `upstream/bolts/bolt04/onion-test.json`
- Bitcoin chapter 112: `upstream/bolts`, `upstream/bolts/07-routing-gossip.md`
- Bitcoin chapter 113: `upstream/bolts`, `upstream/bolts/04-onion-routing.md`
- Bitcoin chapter 114: `upstream/bolts`, `upstream/bolts/11-payment-encoding.md`
- Bitcoin chapter 115: `upstream/bolts`, `upstream/bolts/12-offer-encoding.md`
- Bitcoin chapter 116: `upstream/bip340/reference.py`, `upstream/bip340/test-vectors.csv`, `upstream/bitcoin-core`, `upstream/bitcoin-core/chainparams_v22.cpp`
- Bitcoin chapter 119: `upstream/bip174`, `upstream/bip174/bip-0174.mediawiki`, `upstream/bitcoin-core`, `upstream/bitcoin-core/tx_valid.json`, `upstream/bolts`, `upstream/bolts/11-payment-encoding.md`
- Bitcoin chapter 121: `upstream/bip39`, `upstream/bip39/bip-0039.mediawiki`, `upstream/bip84`, `upstream/bip84/bip-0084.mediawiki`, `upstream/bip86`, `upstream/bip86/bip-0086.mediawiki`
- Bitcoin chapter 122: `upstream/bip327`, `upstream/bip327/reference.py`
<!-- pins-by-chapter:end -->

## License and source notices

BIP-340 is BSD-2-Clause OR MIT OR CC0-1.0; BIP-173, BIP-350 and BIP-86 declare BSD-2-Clause in their
text, BIP-84 declares CC0-1.0 and BIP-49 declares itself public domain, and BIP-341 declares BSD-3-Clause in its text. Bitcoin Core is MIT,
btcd is ISC, rust-bitcoin is CC0-1.0 and BIP-54 is CC0-1.0; their license files are retained. The
mainnet transactions and blocks are public blockchain data. SipHash is CC0-1.0, MIT or
Apache-2.0 with LLVM exception; its CC0 file is retained. The NIP-44 JavaScript subtree is Unlicense; Wycheproof is Apache-2.0;
JSONTestSuite is MIT. The Bitcoin developer reference
(developer.bitcoin.org) is MIT, BIP-155 declares BSD-2-Clause in its text and BIP-324,
with its test vectors and `reference.py`, declares BSD-3-Clause. The BOLTs declare CC-BY-4.0 in their text. Their provided license files are retained. `segwit_addr.py`
carries its MIT notice in its own header. RFCs retain
complete IETF/Internet Society copyright and redistribution notices (including
the older RFC 4231 terms). NIST's U.S. Government archive supplies CAVS notices
and no standalone license file. The NIP texts already vendored by this repository
have no standalone license declaration in those files; this inventory preserves
their source notices and does not invent a new license grant. The derived source
index is OpenAgents Apache-2.0; it does not relicense upstream material.

## Inventory

All dates below are retrieval dates (2026-10-28; the RIPEMD-160 and BIP-32
files, the Bitcoin chapter 5, 17, 26, 28, 29 and 34 files and the SipHash files, 2026-11-02), not claims about
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
| [`upstream/nist/SHA512ShortMsg.rsp`](upstream/nist/SHA512ShortMsg.rsp) | [pinned source](https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/shs/shabytetestvectors.zip) | `e53a36c03609e5a3e3cc4b6e117a499db7864c23ec825c6cec99503a45f40764` |
| [`upstream/nist/SHA512LongMsg.rsp`](upstream/nist/SHA512LongMsg.rsp) | [pinned source](https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/shs/shabytetestvectors.zip) | `b1f3f05d5c209777954d49521d7ea1349447c36a0c52849e044bc397a27dd410` |
| [`upstream/nips/01.md`](upstream/nips/01.md) | [pinned source](https://raw.githubusercontent.com/nostr-protocol/nips/400d157c53264b8e58c7b3cdbd21ef546fb7a157/01.md) | `d5c5ce2b5639eee440b5d342dbe81af42008838cb260ec04acaffaffd29dcaf7` |
| [`upstream/nips/19.md`](upstream/nips/19.md) | [pinned source](https://raw.githubusercontent.com/nostr-protocol/nips/400d157c53264b8e58c7b3cdbd21ef546fb7a157/19.md) | `5b53c98637ae0cb8e6d99e1570e32da2c5842154ba898393439f334f1fd90aad` |
| [`upstream/nips/44.md`](upstream/nips/44.md) | [pinned source](https://raw.githubusercontent.com/nostr-protocol/nips/400d157c53264b8e58c7b3cdbd21ef546fb7a157/44.md) | `b5f89374e4e1dbdee7881e8573b4313b6a89430a9c8d518471599ae0660cf0e2` |
| [`upstream/nips/59.md`](upstream/nips/59.md) | [pinned source](https://raw.githubusercontent.com/nostr-protocol/nips/400d157c53264b8e58c7b3cdbd21ef546fb7a157/59.md) | `b46b078ff5b219fd4ab33a53b479b49d4cbd604b88be46d6b978d53d8f8b43a0` |
| [`upstream/bip341/wallet-test-vectors.json`](upstream/bip341/wallet-test-vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0341/wallet-test-vectors.json) | `403e19fb81dd1f31e745699216308f61fb403774b2aafa87b631b8f7c042d37f` |
| [`upstream/bip341/bip-0341.mediawiki`](upstream/bip341/bip-0341.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0341.mediawiki) | `463690d4c409587759c0ff6e8b8fc3a2bf6cb2cdda0c06ff193d317e0ed898ec` |
| [`upstream/bip340/reference.py`](upstream/bip340/reference.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0340/reference.py) | `4b1d4ad9e60820df4a6d239733d52b014bc03e4c47afd757a2fe4d3b586d892c` |
| [`upstream/bitcoin-core/tx_valid.json`](upstream/bitcoin-core/tx_valid.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/data/tx_valid.json) | `0abeb1a6e4b942e4e43ef8d49b7356c1f6204bcb14163f24153f4d0ec5418011` |
| [`upstream/bitcoin-core/chainparams.cpp`](upstream/bitcoin-core/chainparams.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/kernel/chainparams.cpp) | `cfadf1478377c32625280520655b598674e6d487f08d7f7a80ac69252104b5e1` |
| [`upstream/bitcoin-core/tx_check.cpp`](upstream/bitcoin-core/tx_check.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/consensus/tx_check.cpp) | `66ff205c2bb482133eb864968815158e3474554870ad64e6e795fe313d803ed5` |
| [`upstream/bitcoin-core/sigopcount_tests.cpp`](upstream/bitcoin-core/sigopcount_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/sigopcount_tests.cpp) | `71fb92a1fce40f6c31339dd931cdb3f07fdc4587442e3862d1de107d3c93a225` |
| [`upstream/bitcoin-core/miner_tests.cpp`](upstream/bitcoin-core/miner_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/miner_tests.cpp) | `2e8ec65462c8b6cae821bd22b95407c6c94a4710edea2f4b4f7d785a27f2c783` |
| [`upstream/mainnet/00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097.spent.json`](upstream/mainnet/00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097.spent.json) | [mempool.space API](https://mempool.space/api/block/00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097/txs/0) | `f65a8fa8f6284871947a4a45838a93c20b7254816204f7c670c1db3034ae2e09` |
| [`upstream/bitcoin-core/validation_tests.cpp`](upstream/bitcoin-core/validation_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/validation_tests.cpp) | `174ebba0af246a7b5e3d7769be4a504f00f6d5b47a80935b57d4a8a58b648dbf` |
| [`upstream/bip174/bip-0174.mediawiki`](upstream/bip174/bip-0174.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0174.mediawiki) | `f2a8e1a9c9e31cc7f607b3c7e2419c63eeccc4bd5b725968cde54ab8cfa1d410` |
| [`upstream/bip370/bip-0370.mediawiki`](upstream/bip370/bip-0370.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0370.mediawiki) | `b2b6e9099592fa04179383f9a1fd8b201fbeef4470f4c3f1c050d25589979c57` |
| [`upstream/bip371/bip-0371.mediawiki`](upstream/bip371/bip-0371.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0371.mediawiki) | `c2e8143a3c9aee0ee38d863cf40947ddf8ee67b4f40a748fb6925ccc5ad4b791` |
| [`upstream/bitcoin-core/psbt.h`](upstream/bitcoin-core/psbt.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/psbt.h) | `75e292495ed2e8796e38f832ad0367ecb8fd2e044c588288381df5620ae65987` |
| [`upstream/bitcoin-core/psbt.cpp`](upstream/bitcoin-core/psbt.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/psbt.cpp) | `34d2216140d63ff021e4f5d41028e390c8cc9b5a5e5149484e2b8ff1c80b34f4` |
| [`upstream/bitcoin-core/rpc_psbt.json`](upstream/bitcoin-core/rpc_psbt.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/data/rpc_psbt.json) | `6dc130e0ead807b7521e0f55dc60b6b8a6c10ffaf6c623eb524d814deda0c9ce` |
| [`upstream/bitcoin-core/psbt_wallet_tests.cpp`](upstream/bitcoin-core/psbt_wallet_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/wallet/test/psbt_wallet_tests.cpp) | `e15530b0262b7114f2cb0a6552a516f7d5f9ce3a8ec714461ddfaf6ca4d7cfd1` |
| [`upstream/bitcoin-core/pubkey.h`](upstream/bitcoin-core/pubkey.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/pubkey.h) | `b586b35e4749579e9dad6c6c56c1d7ceecce20ddbd7e066b5c86cadf5d16bb71` |
| [`upstream/bitcoin-core/coinselection.cpp`](upstream/bitcoin-core/coinselection.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/wallet/coinselection.cpp) | `3b85e3d2d911bcb56428d18b2aea4bb4c19a2f95edf53efc48015e37c4adcb80` |
| [`upstream/bitcoin-core/coinselection.h`](upstream/bitcoin-core/coinselection.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/wallet/coinselection.h) | `edaf87098a5dfaa5ae1ff1b1b053f22bb2a34af5d7180a3e8f5b1f6cd6e13085` |
| [`upstream/bitcoin-core/coinselector_tests.cpp`](upstream/bitcoin-core/coinselector_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/wallet/test/coinselector_tests.cpp) | `745d3319b676dea37fe38b7ce9ebc9a679bf57a47e143b18c3a8e2188f2464fa` |
| [`upstream/bitcoin-core/coinselection_tests.cpp`](upstream/bitcoin-core/coinselection_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/wallet/test/coinselection_tests.cpp) | `7e7d4a26998d8091a5cb51963374b5896da8cb3d118dcdf5a3dfc5fa855c41cb` |
| [`upstream/bitcoin-core/spend.cpp`](upstream/bitcoin-core/spend.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/wallet/spend.cpp) | `4b79ae1bda4f0c30c1e87d0fc4dce6a6106fbdbede7ffb8d09b4e171482198cf` |
| [`upstream/bitcoin-core/feerate.cpp`](upstream/bitcoin-core/feerate.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/policy/feerate.cpp) | `3b9db74479742398ae15d8448f41ea67381611fa7bdced26f92116b0f22f0225` |
| [`upstream/bip322/bip-0322.mediawiki`](upstream/bip322/bip-0322.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0322.mediawiki) | `bdf1e930211356666e11d4961acff7a1675a280fba7b57377b48a1fb2f1b45b9` |
| [`upstream/bip322/basic-test-vectors.json`](upstream/bip322/basic-test-vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0322/basic-test-vectors.json) | `a9184da5687b9cb7a1863807f4a8f4d743ec07d4f683f707cb85ce72f5b33f47` |
| [`upstream/bip322/generated-test-vectors.json`](upstream/bip322/generated-test-vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0322/generated-test-vectors.json) | `187205f77b4a639f8d7e9a8e9d9070204a4c6658cf925569001b4d15f02a6539` |
| [`upstream/bip352/bip-0352.mediawiki`](upstream/bip352/bip-0352.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0352.mediawiki) | `e2ffb2999fe4c0131dd9fe57099b610f0d882bcf4f3cfe24c01748eeb4235c7b` |
| [`upstream/bip352/send_and_receive_test_vectors.json`](upstream/bip352/send_and_receive_test_vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0352/send_and_receive_test_vectors.json) | `2faa899c480fea1fb13b06fb669e9cfdd11738a5140ec516f46a46e68e2c2ad8` |
| [`upstream/bip352/reference.py`](upstream/bip352/reference.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0352/reference.py) | `ca44bd5826c238aa6385c65ba31dc5a09a37790eaf4ee75e12959a1d1353124a` |
| [`upstream/bip352/bitcoin_utils.py`](upstream/bip352/bitcoin_utils.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0352/bitcoin_utils.py) | `80dd73fd9aafea15a37879124bafe4a59aa78f76de2d7d778ed0a62f2c49e7b5` |
| [`upstream/bip352/bech32m.py`](upstream/bip352/bech32m.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0352/bech32m.py) | `b38e16a9a2f3960945773ccad020d15cd27dd1871c5a9dd26147a8fa9b14df5b` |
| [`upstream/bip352/ripemd160.py`](upstream/bip352/ripemd160.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0352/ripemd160.py) | `99da6dafa9804d3ca5490a736354027b10ec3d517206caad6ea28c0e097f6b9a` |
| [`upstream/bip352/secp256k1lab/COPYING`](upstream/bip352/secp256k1lab/COPYING) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0352/secp256k1lab/COPYING) | `8f13f6dc042fbc546d7217ed87f1639a9f95ba8ba0248364899564c5d4f32576` |
| [`upstream/bip352/secp256k1lab/src/secp256k1lab/__init__.py`](upstream/bip352/secp256k1lab/src/secp256k1lab/__init__.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0352/secp256k1lab/src/secp256k1lab/__init__.py) | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| [`upstream/bip352/secp256k1lab/src/secp256k1lab/secp256k1.py`](upstream/bip352/secp256k1lab/src/secp256k1lab/secp256k1.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0352/secp256k1lab/src/secp256k1lab/secp256k1.py) | `3a3316ea6717f8dd6d89eb65e80a09265551955d5c9eb872fae3e12504cc63b0` |
| [`upstream/bip352/secp256k1lab/src/secp256k1lab/bip340.py`](upstream/bip352/secp256k1lab/src/secp256k1lab/bip340.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0352/secp256k1lab/src/secp256k1lab/bip340.py) | `428ee8110175b32b67d22a88efe07a44cede2419d0f547949164f2a8f9d76b96` |
| [`upstream/bip352/secp256k1lab/src/secp256k1lab/util.py`](upstream/bip352/secp256k1lab/src/secp256k1lab/util.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0352/secp256k1lab/src/secp256k1lab/util.py) | `3aeccf814f0bd67cdfb0791a6bf054b367c580942acd44942fdf505e5a1af707` |
| [`upstream/bip67/bip-0067.mediawiki`](upstream/bip67/bip-0067.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0067.mediawiki) | `4cc48c5c159c05585962a8eb264b05ccb4ad710b1a16c870232e0f0eb1428991` |
| [`upstream/bip387/bip-0387.mediawiki`](upstream/bip387/bip-0387.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0387.mediawiki) | `92a1a95be774be27f3f373e15dcd8bf291f542cd62535dd1ccbd95bd1022464e` |
| [`upstream/bitcoin-core/solver.cpp`](upstream/bitcoin-core/solver.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/script/solver.cpp) | `d795354fafbbcef7c6d28ff8b9912e896575276b4ca85aeed2fc57f18c41174a` |
| [`upstream/mainnet/coinbase-proofs.json`](upstream/mainnet/coinbase-proofs.json) | [mempool.space API](https://mempool.space/api/block-height/840000) | `3a10ff94f45a2fbef360731be48c468ec7c2ecc3da760fb2861b97721aeb5efb` |
| [`upstream/mainnet/retarget-boundaries.txt`](upstream/mainnet/retarget-boundaries.txt) | [mempool.space API](https://mempool.space/api/v1/blocks/32256) | `06c7e555e0812abb2a12246ea1326ec79f6c0c379cc88a83bb31a5c6fb297a2e` |
| [`upstream/mainnet/headers-0-2016.txt`](upstream/mainnet/headers-0-2016.txt) | [mempool.space API](https://mempool.space/api/v1/blocks/2016) | `a938d6efa4a87e3cb4e14f4868f46a59e7c8119db9a3ee50a16be2c09e0218b8` |
| [`upstream/bip327/bip-0327.mediawiki`](upstream/bip327/bip-0327.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/ada341963ab3916e74c98e6de4ba48babc024a75/bip-0327.mediawiki) | `918555f9d67bf4c1124b004f43e7cfa2436570ce46466f06f83f836f58e8ff7c` |
| [`upstream/bip327/reference.py`](upstream/bip327/reference.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/ada341963ab3916e74c98e6de4ba48babc024a75/bip-0327/reference.py) | `1167993fe159c39c004f74d9046e931ae700a2ce9203c4cf58a51a000c1294e8` |
| [`upstream/bip327/vectors/key_sort_vectors.json`](upstream/bip327/vectors/key_sort_vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/ada341963ab3916e74c98e6de4ba48babc024a75/bip-0327/vectors/key_sort_vectors.json) | `2389fa0c146cfd7455c643ca240ec32835dcfc916f430f50dd94d0b49c9ea16c` |
| [`upstream/bip327/vectors/key_agg_vectors.json`](upstream/bip327/vectors/key_agg_vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/ada341963ab3916e74c98e6de4ba48babc024a75/bip-0327/vectors/key_agg_vectors.json) | `03c02a97e4ef3f2edfbc8e6013c127496dfcfd5889cfca60ddf009a4e9091cab` |
| [`upstream/bip327/vectors/nonce_gen_vectors.json`](upstream/bip327/vectors/nonce_gen_vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/ada341963ab3916e74c98e6de4ba48babc024a75/bip-0327/vectors/nonce_gen_vectors.json) | `2e823580fc072427f0db0f000212cc9124ad2b9dca2b58357eb65088aee4358d` |
| [`upstream/bip327/vectors/nonce_agg_vectors.json`](upstream/bip327/vectors/nonce_agg_vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/ada341963ab3916e74c98e6de4ba48babc024a75/bip-0327/vectors/nonce_agg_vectors.json) | `8409e87b81ea769759598ad3ce53b277a78afffb3a490a86ce02c4d69984524b` |
| [`upstream/bip327/vectors/sign_verify_vectors.json`](upstream/bip327/vectors/sign_verify_vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/ada341963ab3916e74c98e6de4ba48babc024a75/bip-0327/vectors/sign_verify_vectors.json) | `692eecc101f3e515c29137f05031935e1210d2a01bab91e674eb0234f095c15c` |
| [`upstream/bip327/vectors/tweak_vectors.json`](upstream/bip327/vectors/tweak_vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/ada341963ab3916e74c98e6de4ba48babc024a75/bip-0327/vectors/tweak_vectors.json) | `80ce6385ce062644ad1f4edcb9d4797f70ddb0b74769e4099f51b3c9e6ab4aff` |
| [`upstream/bip327/vectors/det_sign_vectors.json`](upstream/bip327/vectors/det_sign_vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/ada341963ab3916e74c98e6de4ba48babc024a75/bip-0327/vectors/det_sign_vectors.json) | `3d4fdb64b24e31762f20830036dc0c59d39fa896649131b54b87906ffdc6e9e8` |
| [`upstream/bip327/vectors/sig_agg_vectors.json`](upstream/bip327/vectors/sig_agg_vectors.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/ada341963ab3916e74c98e6de4ba48babc024a75/bip-0327/vectors/sig_agg_vectors.json) | `15f14c034fb2a5739d7ce638be94c5b37ea675a2e01159092dd93b59d69c3439` |
| [`upstream/bip445/bip-0445.md`](upstream/bip445/bip-0445.md) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/README.md) | `f345267e83070c31588a2812ed0ca014338b0a9c4127283e664d66f27f740458` |
| [`upstream/bip445/COPYING`](upstream/bip445/COPYING) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/COPYING) | `0365c83e8f98eeb409d344b1662fdec720325db7f7946a7a4b92e0e39b6671f1` |
| [`upstream/bip445/python/frost_ref/__init__.py`](upstream/bip445/python/frost_ref/__init__.py) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/frost_ref/__init__.py) | `dc909d8252ffa08dda4d0e39b296df4de69199492d3c805bede5ebf3f004c768` |
| [`upstream/bip445/python/frost_ref/signing.py`](upstream/bip445/python/frost_ref/signing.py) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/frost_ref/signing.py) | `60d40549b66acb130c65e36fb528e4297b8543482bdf3cee0d64d3dfc0b62a1c` |
| [`upstream/bip445/python/secp256k1lab/COPYING`](upstream/bip445/python/secp256k1lab/COPYING) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/secp256k1lab/COPYING) | `8f13f6dc042fbc546d7217ed87f1639a9f95ba8ba0248364899564c5d4f32576` |
| [`upstream/bip445/python/secp256k1lab/src/secp256k1lab/__init__.py`](upstream/bip445/python/secp256k1lab/src/secp256k1lab/__init__.py) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/secp256k1lab/src/secp256k1lab/__init__.py) | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| [`upstream/bip445/python/secp256k1lab/src/secp256k1lab/bip340.py`](upstream/bip445/python/secp256k1lab/src/secp256k1lab/bip340.py) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/secp256k1lab/src/secp256k1lab/bip340.py) | `428ee8110175b32b67d22a88efe07a44cede2419d0f547949164f2a8f9d76b96` |
| [`upstream/bip445/python/secp256k1lab/src/secp256k1lab/ecdh.py`](upstream/bip445/python/secp256k1lab/src/secp256k1lab/ecdh.py) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/secp256k1lab/src/secp256k1lab/ecdh.py) | `8ee93a3442bce4c438bd6373565071a09fd7c1075e998b7e90b51ae17a8df0e1` |
| [`upstream/bip445/python/secp256k1lab/src/secp256k1lab/keys.py`](upstream/bip445/python/secp256k1lab/src/secp256k1lab/keys.py) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/secp256k1lab/src/secp256k1lab/keys.py) | `25a7c9d4d70dec7acea57245a529ef5f8454616d075a31fe27964d3e237c2860` |
| [`upstream/bip445/python/secp256k1lab/src/secp256k1lab/secp256k1.py`](upstream/bip445/python/secp256k1lab/src/secp256k1lab/secp256k1.py) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/secp256k1lab/src/secp256k1lab/secp256k1.py) | `e68560295918b5aee07f38a2dbe5d8fce355ed46b50cfe3240c7d91508dc0b04` |
| [`upstream/bip445/python/secp256k1lab/src/secp256k1lab/util.py`](upstream/bip445/python/secp256k1lab/src/secp256k1lab/util.py) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/secp256k1lab/src/secp256k1lab/util.py) | `3aeccf814f0bd67cdfb0791a6bf054b367c580942acd44942fdf505e5a1af707` |
| [`upstream/bip445/python/tests.py`](upstream/bip445/python/tests.py) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/tests.py) | `233b155e61dd3a0edd0a25de2dc8c60e798f2867374587b01fe6ac2a4837603a` |
| [`upstream/bip445/python/trusted_dealer.py`](upstream/bip445/python/trusted_dealer.py) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/trusted_dealer.py) | `f5958392b2ede59d2ad4652d015d66b625482da1e8fbba19b5ad18b477bf8358` |
| [`upstream/bip445/python/vectors/det_sign_vectors.json`](upstream/bip445/python/vectors/det_sign_vectors.json) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/vectors/det_sign_vectors.json) | `8b324d6bce17e4c12d65f720832a486954baf2b821febf05906ea2ca81d09843` |
| [`upstream/bip445/python/vectors/nonce_agg_vectors.json`](upstream/bip445/python/vectors/nonce_agg_vectors.json) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/vectors/nonce_agg_vectors.json) | `bf4ff95a4c07e4c803ce1b599600de55be26e68f804396998a05cee6b1b6172d` |
| [`upstream/bip445/python/vectors/nonce_gen_vectors.json`](upstream/bip445/python/vectors/nonce_gen_vectors.json) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/vectors/nonce_gen_vectors.json) | `b86c932a1edd955c0fba8eba0be34b3266205ab41dc13b01eae58f1be07907d2` |
| [`upstream/bip445/python/vectors/sig_agg_vectors.json`](upstream/bip445/python/vectors/sig_agg_vectors.json) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/vectors/sig_agg_vectors.json) | `5b35cf5679cf76268c05ef9ca92188cc796a79fefed43a35a21c892b5a5f28b9` |
| [`upstream/bip445/python/vectors/sign_verify_vectors.json`](upstream/bip445/python/vectors/sign_verify_vectors.json) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/vectors/sign_verify_vectors.json) | `cebf115cd8fd0cf3b642d45c0686d5bd2d32a54e6635078eff2d9b008861b246` |
| [`upstream/bip445/python/vectors/threshold_info_vectors.json`](upstream/bip445/python/vectors/threshold_info_vectors.json) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/vectors/threshold_info_vectors.json) | `18a33d5b9da1f0d71b46c9896da0524cb32762f92e1fbdcf40f503e925e03519` |
| [`upstream/bip445/python/vectors/tweak_vectors.json`](upstream/bip445/python/vectors/tweak_vectors.json) | [pinned source](https://raw.githubusercontent.com/siv2r/bip-frost-signing/ba299badfbe647e670686de75578dd501a627028/python/vectors/tweak_vectors.json) | `94448fedf061ee1fe8286f4174de04eca59db9d1776216f5a1161fa9cb5b1fe8` |
| [`upstream/bitcoin-core/versionbits.cpp`](upstream/bitcoin-core/versionbits.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/versionbits.cpp) | `4394f6fdf7656e0e6e4eb90a11c44875dab506345bde9a54d84ed9337d524ec8` |
| [`upstream/bitcoin-core/versionbits.h`](upstream/bitcoin-core/versionbits.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/versionbits.h) | `babd3b786f7d787f88b47da1effd47f631b3e37cba95c1d501f1a553c960f100` |
| [`upstream/bitcoin-core/versionbits_impl.h`](upstream/bitcoin-core/versionbits_impl.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/versionbits_impl.h) | `6c7b5afcaff8d99b2e9e1e62d11bc76adb6164dcce8add41aabde51bcbe7a9ce` |
| [`upstream/bitcoin-core/chainparams_v22.cpp`](upstream/bitcoin-core/chainparams_v22.cpp) | [pinned source (v22.0)](https://raw.githubusercontent.com/bitcoin/bitcoin/a0988140b71485ad12c3c3a4a9573f7c21b1eff8/src/chainparams.cpp) | `55406041319840c962c4ca5a792111bcac6a30c88aca3dde812b324a15f9220c` |
| [`upstream/mainnet/mainnet-840102-840113.headers`](upstream/mainnet/mainnet-840102-840113.headers) | [mempool.space API](https://mempool.space/api/block-height/840102) | `5fce735a8802701ea89ca98442496c48c70ffd81b654355d2b03a3c9f483adf1` |
| [`upstream/testnet4/testnet4-0-2005-2016.headers`](upstream/testnet4/testnet4-0-2005-2016.headers) | [mempool.space API](https://mempool.space/testnet4/api/block-height/2005) | `a1fef921c3eea5d33e21dc6bcc92ca4405c450e3661d72a1cc1f3e56bce6b2c4` |
| [`upstream/bitcoin-core/kernel_signet.h`](upstream/bitcoin-core/kernel_signet.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/kernel/signet.h) | `8a0b9207b16dfad04214f42ac1ccc70f252025ed2a6f7ad2a5163b2a1c2fb118` |
| [`upstream/bitcoin-core/signet.cpp`](upstream/bitcoin-core/signet.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/signet.cpp) | `58f92d2bbcae1f609016e05ddf4be0b5eec1b1e126e3910ed2c5c49e7e914908` |
| [`upstream/bitcoin-core/params.h`](upstream/bitcoin-core/params.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/consensus/params.h) | `21bc0a1421ca71b057325fac96ef1a7297b737a47c3231a6c7c7ff8e0598e2f0` |
| [`upstream/signet/00000086d6b2636cb2a392d45edc4ec544a10024d30141c9adf4bfd9de533b53.raw`](upstream/signet/00000086d6b2636cb2a392d45edc4ec544a10024d30141c9adf4bfd9de533b53.raw) | [mempool.space API](https://mempool.space/signet/api/block/00000086d6b2636cb2a392d45edc4ec544a10024d30141c9adf4bfd9de533b53/raw) | `85b2a673710c23cb88dd37c6b742900627bb0a7a4df65a2f44d0c91a2b45f50d` |
| [`upstream/signet/0000008448acc419d5f54a777cf1fe519cb6a2e0998bedc9322978a276539466.raw`](upstream/signet/0000008448acc419d5f54a777cf1fe519cb6a2e0998bedc9322978a276539466.raw) | [mempool.space API](https://mempool.space/signet/api/block/0000008448acc419d5f54a777cf1fe519cb6a2e0998bedc9322978a276539466/raw) | `7f1098495673b86ddc983431f9e00c29a949b7cd733da0c514118852b7dfbb1f` |
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
| [`upstream/bip86/bip-0086.mediawiki`](upstream/bip86/bip-0086.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0086.mediawiki) | `d8d01dee331da07c2562615bc1f064c1868ec3fce61184973da76d1196c7f5b0` |
| [`upstream/bip84/bip-0084.mediawiki`](upstream/bip84/bip-0084.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0084.mediawiki) | `1900feec6cafca65b8c09906ca0658d2d742b4c9b44cb15678996985b6bfe627` |
| [`upstream/bip44/bip-0044.mediawiki`](upstream/bip44/bip-0044.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0044.mediawiki) | `6a541c79f94077ac529941a8cbd373ce077a5d6e97b6dd8209b45ac10f737eaf` |
| [`upstream/bip380/bip-0380.mediawiki`](upstream/bip380/bip-0380.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0380.mediawiki) | `34e6510bb2eba9445a68eacf3d24d5a4e5f24481477488d5552f674ac81dd5fe` |
| [`upstream/bip381/bip-0381.mediawiki`](upstream/bip381/bip-0381.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0381.mediawiki) | `f0f6060332f82454f61015815c824eea4b7b6684ce4892ef42d23e4e782fa817` |
| [`upstream/bip382/bip-0382.mediawiki`](upstream/bip382/bip-0382.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0382.mediawiki) | `6cd2574998ca0d2141d1c487e70da2dfab47abd6a2dfcc3680af2c241f01fe9d` |
| [`upstream/bip383/bip-0383.mediawiki`](upstream/bip383/bip-0383.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0383.mediawiki) | `c46f75e5869386c1852dd187725293822b9e3b761342a966fbcf99adaa112aa2` |
| [`upstream/bip384/bip-0384.mediawiki`](upstream/bip384/bip-0384.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0384.mediawiki) | `9d6b32a619e021f50f4b1a07ed8f9d08cfea599c8f484e6b1360ada9aae8541c` |
| [`upstream/bip385/bip-0385.mediawiki`](upstream/bip385/bip-0385.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0385.mediawiki) | `8c4963492f1d5b492808e5ece8be75f7f66def37f35d091074fdbfa98a6ddd08` |
| [`upstream/bip386/bip-0386.mediawiki`](upstream/bip386/bip-0386.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0386.mediawiki) | `770121b4b33a50bfadfebcd94931dd0ad04159b9fbc1d3b3c3ae17fc0cc16de8` |
| [`upstream/bip389/bip-0389.mediawiki`](upstream/bip389/bip-0389.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0389.mediawiki) | `dfe2d4673bb008565bbb5bcd01438de1c568ba0f46b1dd3bf09ee5df3bd29284` |
| [`upstream/bitcoin-core/descriptor_tests.cpp`](upstream/bitcoin-core/descriptor_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/descriptor_tests.cpp) | `823160973e3b91c52a75b265b22bd815cf826d76e29943b3509c166244b3254c` |
| [`upstream/bip49/bip-0049.mediawiki`](upstream/bip49/bip-0049.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0049.mediawiki) | `307b62faf7b9927233b5112e93d591272ba0c48303b5dbb6f0bbfbaf2347e695` |
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
| [`upstream/bitcoin-core/merkle.cpp`](upstream/bitcoin-core/merkle.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/consensus/merkle.cpp) | `abf9241785afd4b08735fde46ce9cabde9140331e061ed7e4ea74eb7f14b0af4` |
| [`upstream/bitcoin-core/merkle.h`](upstream/bitcoin-core/merkle.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/consensus/merkle.h) | `c00bd222d4d262cba361b7849020ea9af279ecd637c0137cd345757fe94ebb61` |
| [`upstream/bitcoin-core/merkle_tests.cpp`](upstream/bitcoin-core/merkle_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/merkle_tests.cpp) | `0902477c1f45d1536a1aafab6f931c478064e87099c7f8034025c09b3677fa5a` |
| [`upstream/bitcoin-core/validation.cpp`](upstream/bitcoin-core/validation.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/validation.cpp) | `65e88786ccaaf4dd0ebd14e7a0b82d7fbe6b86a8f0b2f9de4cc2db7b983283d2` |
| [`upstream/bitcoin-core/policy.h`](upstream/bitcoin-core/policy.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/policy/policy.h) | `7ffa54e839f78877c90ea59961995c38caf7015ad7280413cc49c37ab8c9a5cd` |
| [`upstream/bitcoin-core/policy.cpp`](upstream/bitcoin-core/policy.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/policy/policy.cpp) | `09dbc636d841d27a1d876bc179be3061c74d82fdd42a440d26738211637f40c2` |
| [`upstream/bitcoin-core/amount.h`](upstream/bitcoin-core/amount.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/consensus/amount.h) | `fbaf7f3186b066f911e74720117299e9b88132cf612a1b27dfe34ab55752a393` |
| [`upstream/bitcoin-core/block413567.raw`](upstream/bitcoin-core/block413567.raw) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/bench/data/block413567.raw) | `71964cee18c58675784846d498944b35daa41e36b6f65a7e8feb291def924cce` |
| [`upstream/bip54/bip-0054.md`](upstream/bip54/bip-0054.md) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/927b6de9915c9262615a6399de51b200f81e5aa4/bip-0054.md) | `6cb659a11692dc0e0f2837df9c43746121684b8aa5f12b3cb095f0c3e2cb029d` |
| [`upstream/bip143/bip-0143.mediawiki`](upstream/bip143/bip-0143.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/927b6de9915c9262615a6399de51b200f81e5aa4/bip-0143.mediawiki) | `62bc71351563e68baeb12643c68355d217953ae9eb6a6e68b2b0323275b6beec` |
| [`upstream/bitcoin-core/tx_verify.cpp`](upstream/bitcoin-core/tx_verify.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/consensus/tx_verify.cpp) | `45de31c846d87b202b46f1e314cbd07869c0b3e598cdc7b18342e2fe24de3fc0` |
| [`upstream/bitcoin-core/chain.h`](upstream/bitcoin-core/chain.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/chain.h) | `a1b863764bc6e40ed79a447805843eb7b18a09730eb3a849bf9fe43f17274aba` |
| [`upstream/mainnet/000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506.raw`](upstream/mainnet/000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506.raw) | [mempool.space API](https://mempool.space/api/block/000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506/raw) | `b02256f05ce2fc0d152394b5cd2fa28fe4ab61100844c389581c4cffad88a2b1` |
| [`upstream/mainnet/000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506.json`](upstream/mainnet/000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506.json) | [mempool.space API](https://mempool.space/api/block/000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506) | `a40baaad74146e536439774ac3966f471c6745a942423db503789547cfe2b45e` |
| [`upstream/mainnet/00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097.raw`](upstream/mainnet/00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097.raw) | [mempool.space API](https://mempool.space/api/block/00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097/raw) | `7c0d1862284dcd240a63b5bcb75e956c3645ed5d7453799abb721ce06e6b2902` |
| [`upstream/mainnet/00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097.json`](upstream/mainnet/00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097.json) | [mempool.space API](https://mempool.space/api/block/00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097) | `81682b48567be9bafe33b712930567fea33f9a18cbabc56eb89b82666d144d64` |
| [`upstream/bitcoin-core/pow.cpp`](upstream/bitcoin-core/pow.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/pow.cpp) | `cad89cf4d7b99f3f6d3e93393febe523133320a5f4e92a61c8179bb5ac526a8a` |
| [`upstream/bitcoin-core/arith_uint256.cpp`](upstream/bitcoin-core/arith_uint256.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/arith_uint256.cpp) | `0bf5e52d78e6454c566b46675903d0980e7905b4c03f91cabb1be110ad9818f5` |
| [`upstream/bitcoin-core/arith_uint256.h`](upstream/bitcoin-core/arith_uint256.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/arith_uint256.h) | `1c870b2e24743ef7765447715104aff31ccc141c15b137ef149540c202dd499e` |
| [`upstream/bitcoin-core/chain.cpp`](upstream/bitcoin-core/chain.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/chain.cpp) | `f817c51d79daa78fa3cb24506938c2523409c0703a2e0384794515d2cfc879a5` |
| [`upstream/bitcoin-core/block.h`](upstream/bitcoin-core/block.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/primitives/block.h) | `07aa61de74a4aa8b9c9f67d87a6a9a1cf2729bb452e3642d95facec23fe0fd27` |
| [`upstream/bitcoin-core/pow_tests.cpp`](upstream/bitcoin-core/pow_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/pow_tests.cpp) | `e17827a1e84f9bbeb759e99566060e167bdc8e11643eab5d20918f63f5304306` |
| [`upstream/bitcoin-core/arith_uint256_tests.cpp`](upstream/bitcoin-core/arith_uint256_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/arith_uint256_tests.cpp) | `765260cf3f73c1c2934247819726c93f91d98f468343fb697aff03f6dd900e2a` |
| [`upstream/mainnet/000000000000000000010538edbfd2d5b809a33dd83f284aeea41c6d0d96968a.header`](upstream/mainnet/000000000000000000010538edbfd2d5b809a33dd83f284aeea41c6d0d96968a.header) | [mempool.space API](https://mempool.space/api/block/000000000000000000010538edbfd2d5b809a33dd83f284aeea41c6d0d96968a/header) | `79abf5bbdf58748d3d51d7a96fbb7afb6720f78f98a4ccb3ec67d131ec9930bf` |
| [`upstream/mainnet/000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f.header`](upstream/mainnet/000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f.header) | [mempool.space API](https://mempool.space/api/block/000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f/header) | `67991e515cf93949369ff472bba7fd07686e8587dc877149e5a25019609285fb` |
| [`upstream/mainnet/0000000007bfbe7ec5510ccf80c5a291bf8fc603dd79f5d6a69fae6039f8a35e.header`](upstream/mainnet/0000000007bfbe7ec5510ccf80c5a291bf8fc603dd79f5d6a69fae6039f8a35e.header) | [mempool.space API](https://mempool.space/api/block/0000000007bfbe7ec5510ccf80c5a291bf8fc603dd79f5d6a69fae6039f8a35e/header) | `19b17e9e8cb9c29a3590d288696f7b1cef72a57f18f47bc5d76778ab6dc29471` |
| [`upstream/mainnet/000000000e27ee2a3899aec79a8aa08f138e320a66fa7fa3f2e4b9fd05e42a67.header`](upstream/mainnet/000000000e27ee2a3899aec79a8aa08f138e320a66fa7fa3f2e4b9fd05e42a67.header) | [mempool.space API](https://mempool.space/api/block/000000000e27ee2a3899aec79a8aa08f138e320a66fa7fa3f2e4b9fd05e42a67/header) | `d250c2d84d158c866b0bae8f59fb7c99fc5cb8c8f85ae3c35e84c3d36948730c` |
| [`upstream/mainnet/000000002931f8c7a9fe1a6f122dd5777d09d81dc107e051d7f78efef09c1719.header`](upstream/mainnet/000000002931f8c7a9fe1a6f122dd5777d09d81dc107e051d7f78efef09c1719.header) | [mempool.space API](https://mempool.space/api/block/000000002931f8c7a9fe1a6f122dd5777d09d81dc107e051d7f78efef09c1719/header) | `1a85eb8f9f82c9359f42d548e586692754f90aaf339e19cfcd49660f4ac51635` |
| [`upstream/mainnet/00000000360d826e18e72b3f5f3a05a43989131e512319a3b24aaa24b1010aa9.header`](upstream/mainnet/00000000360d826e18e72b3f5f3a05a43989131e512319a3b24aaa24b1010aa9.header) | [mempool.space API](https://mempool.space/api/block/00000000360d826e18e72b3f5f3a05a43989131e512319a3b24aaa24b1010aa9/header) | `a0fa0ce865f13539961d93a4f9959fe12a458a09b41561ae794f1507b0b3229c` |
| [`upstream/mainnet/000000003d5eb2328f71644f2540cf6acc343f90d1594b2ef916ad4d4a7f5d2d.header`](upstream/mainnet/000000003d5eb2328f71644f2540cf6acc343f90d1594b2ef916ad4d4a7f5d2d.header) | [mempool.space API](https://mempool.space/api/block/000000003d5eb2328f71644f2540cf6acc343f90d1594b2ef916ad4d4a7f5d2d/header) | `02ca7bd9c850d875508586de55c1a884dad17577dd9401dac8b3e69f4209ce0b` |
| [`upstream/mainnet/000000004f2886a170adb7204cb0c7a824217dd24d11a74423d564c4e0904967.header`](upstream/mainnet/000000004f2886a170adb7204cb0c7a824217dd24d11a74423d564c4e0904967.header) | [mempool.space API](https://mempool.space/api/block/000000004f2886a170adb7204cb0c7a824217dd24d11a74423d564c4e0904967/header) | `dd3a54aca0c6a10ca16a5700c10fb42b4e28447a403711b18140d27947864f9a` |
| [`upstream/mainnet/000000006baebaa74cecde6c6787c26ee0a616a3c333261bff36653babdac149.header`](upstream/mainnet/000000006baebaa74cecde6c6787c26ee0a616a3c333261bff36653babdac149.header) | [mempool.space API](https://mempool.space/api/block/000000006baebaa74cecde6c6787c26ee0a616a3c333261bff36653babdac149/header) | `48403dd4a894635c26bc0f53e02dd41935d6696472b179e6048416d4044ad91a` |
| [`upstream/mainnet/000000006e35246abda2e8f2ab945780ea7619c74a3ee5db5ca6ff0f4d5b88c6.header`](upstream/mainnet/000000006e35246abda2e8f2ab945780ea7619c74a3ee5db5ca6ff0f4d5b88c6.header) | [mempool.space API](https://mempool.space/api/block/000000006e35246abda2e8f2ab945780ea7619c74a3ee5db5ca6ff0f4d5b88c6/header) | `3c76ec1316163eed9b338f63b2bb985a0acbdd3c95b38d401fb6c0d99c8ab50d` |
| [`upstream/mainnet/0000000078bb40a0a5aae645f86e144cab79209726b98d74e2f311c6d31aa52e.header`](upstream/mainnet/0000000078bb40a0a5aae645f86e144cab79209726b98d74e2f311c6d31aa52e.header) | [mempool.space API](https://mempool.space/api/block/0000000078bb40a0a5aae645f86e144cab79209726b98d74e2f311c6d31aa52e/header) | `e741166721df7d1f4d28a4ee16e933189214266b1a437f1b6e8fa0153d93a5e4` |
| [`upstream/mainnet/000000008a5b32a0610b2b0eeb5390e30e157324bf28c09ab83ccbb99184c38b.header`](upstream/mainnet/000000008a5b32a0610b2b0eeb5390e30e157324bf28c09ab83ccbb99184c38b.header) | [mempool.space API](https://mempool.space/api/block/000000008a5b32a0610b2b0eeb5390e30e157324bf28c09ab83ccbb99184c38b/header) | `1c9f0331e9623fee87ab7e5686d39b06af48736f202dea3951cc0ffbc20f0e40` |
| [`upstream/mainnet/00000000984f962134a7291e3693075ae03e521f0ee33378ec30a334d860034b.header`](upstream/mainnet/00000000984f962134a7291e3693075ae03e521f0ee33378ec30a334d860034b.header) | [mempool.space API](https://mempool.space/api/block/00000000984f962134a7291e3693075ae03e521f0ee33378ec30a334d860034b/header) | `89ca08988ec28cc68f54e77972e8afb157c8ddc5d06a12ef545c5135a0d963cf` |
| [`upstream/bitcoin-core/script_error.h`](upstream/bitcoin-core/script_error.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/script/script_error.h) | `a2ea50346edfa59b979747b5d7573027989fabdad90af103119d23ff9db46bb9` |
| [`upstream/bitcoin-core/script_error.cpp`](upstream/bitcoin-core/script_error.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/script/script_error.cpp) | `92404c34ad460931639e6fb05c615d06c2a3410d7ac357084226c59ca72382c3` |
| [`upstream/bitcoin-core/script_tests.cpp`](upstream/bitcoin-core/script_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/script_tests.cpp) | `8ecf8bfc15190cce1b1f6893329c828b4ce732179a200147009fafb0e8e30bc9` |
| [`upstream/bitcoin-core/transaction_tests.cpp`](upstream/bitcoin-core/transaction_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/transaction_tests.cpp) | `eb0b3f38018ec8fa383e993ee5f0df4887d5ff2784cb80b4f648011e24ded5d5` |
| [`upstream/bitcoin-core/interpreter.h`](upstream/bitcoin-core/interpreter.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/script/interpreter.h) | `e22719ed2cdd2cd381ae183bb0b16c8204d7b8b1da103435d0f2e084fa809e96` |
| [`upstream/bitcoin-core/transaction_utils.cpp`](upstream/bitcoin-core/transaction_utils.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/util/transaction_utils.cpp) | `d90dcb4affb043b6b738c673c507318821f3d1252bfe6665680af7594efc977f` |
| [`upstream/bitcoin-core/sighash.json`](upstream/bitcoin-core/sighash.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/data/sighash.json) | `52cf23c2076e7f129c71d5508631d3e5ae3be1b1cb0585c0e23bbb4bb373e924` |
| [`upstream/mainnet/retarget-boundaries.txt`](upstream/mainnet/retarget-boundaries.txt) | [mempool.space API](https://mempool.space/api/v1/blocks/32256), assembled | `06c7e555e0812abb2a12246ea1326ec79f6c0c379cc88a83bb31a5c6fb297a2e` |
| [`upstream/mainnet/headers-0-2016.txt`](upstream/mainnet/headers-0-2016.txt) | [mempool.space API](https://mempool.space/api/v1/blocks/2016), assembled | `a938d6efa4a87e3cb4e14f4868f46a59e7c8119db9a3ee50a16be2c09e0218b8` |
| [`upstream/bitcoin-core/protocol.h`](upstream/bitcoin-core/protocol.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/protocol.h) | `92d6290df90eaea49b4a110b518ff34be914666471a5dee7e5eaa26832986738` |
| [`upstream/bitcoin-core/protocol.cpp`](upstream/bitcoin-core/protocol.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/protocol.cpp) | `dd933f035e3d104bea8db6f83970787912357f8aebf6a01c8afc6992e93c6dfe` |
| [`upstream/bitcoin-core/net.h`](upstream/bitcoin-core/net.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/net.h) | `c268325ae03f4f80e8427af80eb794265b08e3686923b7a5a8b850416bed213b` |
| [`upstream/bitcoin-core/net.cpp`](upstream/bitcoin-core/net.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/net.cpp) | `05e8ce20762e9c23e060d428fb5218cb32fea599afbd40b2417b9bd2a236d7a6` |
| [`upstream/bitcoin-core/net_processing.cpp`](upstream/bitcoin-core/net_processing.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/net_processing.cpp) | `bcd69aba37869cd68dfe03cce37b1400d1488b59ef1faafdc319ddcb51ebea36` |
| [`upstream/bitcoin-core/netaddress.h`](upstream/bitcoin-core/netaddress.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/netaddress.h) | `4590086cfafa37ec56949b59cab8c151fb0fac91161186f985d5048c0e0d83e4` |
| [`upstream/bitcoin-core/netaddress.cpp`](upstream/bitcoin-core/netaddress.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/netaddress.cpp) | `d8714d78569b92e8ce49b8cf845f8e3008ed739e20e44b025526e0ef92ae7c34` |
| [`upstream/bitcoin-core/messages.py`](upstream/bitcoin-core/messages.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/test_framework/messages.py) | `44e3b8c702916c8ab7820db65cf38af801dd5b5a457e91cabdff95eba5d412a5` |
| [`upstream/bip155/bip-0155.mediawiki`](upstream/bip155/bip-0155.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/927b6de9915c9262615a6399de51b200f81e5aa4/bip-0155.mediawiki) | `acf8a589041612260ec073dd7d74e4842928cb78204c4a9c8656e1b0618cf174` |
| [`upstream/developer-bitcoin-org/p2p_networking.rst`](upstream/developer-bitcoin-org/p2p_networking.rst) | [pinned source](https://raw.githubusercontent.com/bitcoin-dot-org/developer.bitcoin.org/813ba3fb5eae85cfdfffe91d12f2df653ea8b725/reference/p2p_networking.rst) | `6284673665cdf619238a828d1df15cffbe35f8ab1026583a280fb81a105ac379` |
| [`upstream/bip324/bip-0324.mediawiki`](upstream/bip324/bip-0324.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/927b6de9915c9262615a6399de51b200f81e5aa4/bip-0324.mediawiki) | `14de484a709946348effa8264991cf124a34ecdaf0a3f9d68a42313287625ad8` |
| [`upstream/bip324/ellswift_decode_test_vectors.csv`](upstream/bip324/ellswift_decode_test_vectors.csv) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/927b6de9915c9262615a6399de51b200f81e5aa4/bip-0324/ellswift_decode_test_vectors.csv) | `a238a66b093bed3c571d5b57cb2bb1f69487788f0f872f7ea7501b5b98be20b0` |
| [`upstream/bip324/xswiftec_inv_test_vectors.csv`](upstream/bip324/xswiftec_inv_test_vectors.csv) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/927b6de9915c9262615a6399de51b200f81e5aa4/bip-0324/xswiftec_inv_test_vectors.csv) | `b5e8e0e9edfdee14c3ec4db3c6763543a9f17290a84a93de0ae2671c16f4432a` |
| [`upstream/bip324/packet_encoding_test_vectors.csv`](upstream/bip324/packet_encoding_test_vectors.csv) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/927b6de9915c9262615a6399de51b200f81e5aa4/bip-0324/packet_encoding_test_vectors.csv) | `61704507945e2fa99e6c799fab888593eb7a2c3d2018f42ee4a3c3abe0684269` |
| [`upstream/bip324/reference.py`](upstream/bip324/reference.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/927b6de9915c9262615a6399de51b200f81e5aa4/bip-0324/reference.py) | `cb074b4e64bcb096ff4a2f1ee3a0580c383bb1d02662b7dccdd4968bc6b7af4f` |
| [`upstream/bitcoin-core/bip324.cpp`](upstream/bitcoin-core/bip324.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/bip324.cpp) | `58aaf7d2a2ab98679bbb1e1d4c40d0c3e4021822fd1f628099c1a178e4494685` |
| [`upstream/bitcoin-core/script.py`](upstream/bitcoin-core/script.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/test_framework/script.py) | `82e250fadd393b972ae9d5d508f458fc872ea0dd947ca162693a006c3e488bbf` |
| [`upstream/bitcoin-core/protocol_version.h`](upstream/bitcoin-core/protocol_version.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/node/protocol_version.h) | `ce87f506fc0db48f9cdb74e58a7ee396261b543fe9a7f423ec8eab1f2f82aa7a` |
| [`upstream/bitcoin-core/muhash.cpp`](upstream/bitcoin-core/muhash.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/crypto/muhash.cpp) | `4dc2a5440ab0060b20f0f598e9fd738b963d252befb51d6cd97fcef71de6838c` |
| [`upstream/bitcoin-core/muhash.h`](upstream/bitcoin-core/muhash.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/crypto/muhash.h) | `41819366201e3694eebc62c675771faf7b3eaa102657b27347d50794be0e45aa` |
| [`upstream/bitcoin-core/crypto_tests.cpp`](upstream/bitcoin-core/crypto_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/crypto_tests.cpp) | `6eefd34081d99a0ca68ce357a7eeef15864439a566b8563031c0df1fc2ffaa40` |
| [`upstream/bitcoin-core/compressor.cpp`](upstream/bitcoin-core/compressor.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/compressor.cpp) | `9d6a07d3b4b69a2e0580e5ba795725455b19e7a1e29490a186d79cb3a4d9e301` |
| [`upstream/bitcoin-core/compressor.h`](upstream/bitcoin-core/compressor.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/compressor.h) | `c203dba72780287682396607cc60a71631aa1fee5f6c35ac87853a206ea86aa4` |
| [`upstream/bitcoin-core/compress_tests.cpp`](upstream/bitcoin-core/compress_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/compress_tests.cpp) | `dba41e6bea1fc0c19d1611bb27d87964da75399b8d7edfa73df1c31433a4ded6` |
| [`upstream/bitcoin-core/coins.h`](upstream/bitcoin-core/coins.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/coins.h) | `dcda5f551c4aad640b2d8c2105f66ba40a1fc51e7c416a0088568fd4a6ae86a0` |
| [`upstream/bitcoin-core/coinstats.cpp`](upstream/bitcoin-core/coinstats.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/kernel/coinstats.cpp) | `c64eebd7291bbe78f7dd5f63e95c0ca04fa3bf7c6f3bf10b32ac3d3267367540` |
| [`upstream/bitcoin-core/txdb.cpp`](upstream/bitcoin-core/txdb.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/txdb.cpp) | `fcd265ad7d8a6b77cc121f37c6541df860e1a9d24725e6158ff1a83e722faef3` |
| [`upstream/bitcoin-core/utxo_snapshot.h`](upstream/bitcoin-core/utxo_snapshot.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/node/utxo_snapshot.h) | `9da2c30d41e8ed13a8f96eba826324dec3cc4b0e8398b665561c07400cfb16f7` |
| [`upstream/bitcoin-core/miner.cpp`](upstream/bitcoin-core/miner.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/node/miner.cpp) | `13fee188aaf49b13545d583b334b04517dad04bf43fa62691528a8abbcf0d559` |
| [`upstream/bitcoin-core/feature_utxo_set_hash.py`](upstream/bitcoin-core/feature_utxo_set_hash.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/feature_utxo_set_hash.py) | `39882a860fc1bfaaf135bc82067542486c18beaf79397c749edfbe77bde835bc` |
| [`upstream/bitcoin-core/rpc_dumptxoutset.py`](upstream/bitcoin-core/rpc_dumptxoutset.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/rpc_dumptxoutset.py) | `78005ac31b4df70964714dcc407fb5571a89350a9fc8d78381d2a8b979dba487` |
| [`upstream/bitcoin-core/muhash.py`](upstream/bitcoin-core/muhash.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/test_framework/crypto/muhash.py) | `af45c2451c06abd530ae7d65f8778f7e1ebb76f7b68dcded3459fdcee376405c` |
| [`upstream/bitcoin-core/chacha20.py`](upstream/bitcoin-core/chacha20.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/test_framework/crypto/chacha20.py) | `c0b383e8c1ee47cd3d0cb02630ad316cd8e915b857fa19193a7111d79cbe4878` |
| [`upstream/bitcoin-core/wallet.py`](upstream/bitcoin-core/wallet.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/test_framework/wallet.py) | `6787e3d41e80676260692946aaa448de71de0c772d6b7bb1d85fbf6c9535fbe4` |
| [`upstream/bitcoin-core/address.py`](upstream/bitcoin-core/address.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/test_framework/address.py) | `10ff2a6ad2cb58200741fd345163d36315fe57b2583da0e919e72bdd825346d3` |
| [`upstream/bitcoin-core/test_node.py`](upstream/bitcoin-core/test_node.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/test_framework/test_node.py) | `e00688c4aa2fe2991376c4835941804421ca21fe05700052b2118286a1e92a20` |
| [`upstream/bip141/bip-0141.mediawiki`](upstream/bip141/bip-0141.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/927b6de9915c9262615a6399de51b200f81e5aa4/bip-0141.mediawiki) | `d72409335872a32494f75e7547589abf4b3b23b6beebe6bc6999c7a84b135e2c` |
| [`upstream/bitcoin-core/consensus_validation.h`](upstream/bitcoin-core/consensus_validation.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/consensus/validation.h) | `2875af77e8e0d4db5b210bc716f37713bd0bd8922dde60713e611b33cdf1806a` |
| [`upstream/bitcoin-core/blocktools.py`](upstream/bitcoin-core/blocktools.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/test_framework/blocktools.py) | `13de0034eb8f41e3435e8f2996ef217221b4450f21f7c24aef1d6e20bc1590df` |
| [`upstream/bitcoin-core/obfuscation.h`](upstream/bitcoin-core/obfuscation.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/util/obfuscation.h) | `c9a37543d90e6859577f85ab1a46df76cdb5bb6931056764d8a9ed5ce79f6cc6` |
| [`upstream/bitcoin-core/dbwrapper.cpp`](upstream/bitcoin-core/dbwrapper.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/dbwrapper.cpp) | `3ecd43525a45dbf0318d5a00a6fadbd47393241a8204cb714b86893b7f20727f` |
| [`upstream/bitcoin-core/dbwrapper.h`](upstream/bitcoin-core/dbwrapper.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/dbwrapper.h) | `ec747f471d4467b710c41db11337dd93f834775cd08fdb94c8fb24efd0fca531` |
| [`upstream/bip152/bip-0152.mediawiki`](upstream/bip152/bip-0152.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/927b6de9915c9262615a6399de51b200f81e5aa4/bip-0152.mediawiki) | `b710fa0a4ab9ec92d84884adc77df25b974add79dbc23b8158bf3a791e47cd85` |
| [`upstream/bitcoin-core/blockencodings.h`](upstream/bitcoin-core/blockencodings.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/blockencodings.h) | `3b2db1f2d8987f1d83c4f92536b5467f922f992aa1e5bbd04327f11c01b30933` |
| [`upstream/bitcoin-core/blockencodings.cpp`](upstream/bitcoin-core/blockencodings.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/blockencodings.cpp) | `411f83d16f76e8e96f7f6df307812bde2101a5624ef7349c1a0569b2b97d9fe6` |
| [`upstream/bitcoin-core/blockencodings_tests.cpp`](upstream/bitcoin-core/blockencodings_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/blockencodings_tests.cpp) | `c070a461070d43d1052a3e4d14e5cdb10ed27e862b71cb393437f03a23487c54` |
| [`upstream/bitcoin-core/siphash.py`](upstream/bitcoin-core/siphash.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/test_framework/crypto/siphash.py) | `395fb94ebc7fb7d38bfe86bc3dbfb4df2e94d3c1e3cd1c41b66cc19d0d5e76e3` |
| [`upstream/bitcoin-core/blockfilter.cpp`](upstream/bitcoin-core/blockfilter.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/blockfilter.cpp) | `f9cd087f8112e8bce1882585f440c2b188a3cc8838621f6350f3800f4a197140` |
| [`upstream/bitcoin-core/golombrice.h`](upstream/bitcoin-core/golombrice.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/util/golombrice.h) | `e7646591a651d60d15703fb0b443a9af9770d3fc37a65f51d0ea1667374b15dd` |
| [`upstream/bitcoin-core/blockfilter_tests.cpp`](upstream/bitcoin-core/blockfilter_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/blockfilter_tests.cpp) | `1c33eed7f787fc7e57483340ebc1b6f8cd013fbadacf09df477ac4c9047692e1` |
| [`upstream/bitcoin-core/blockfilters.json`](upstream/bitcoin-core/blockfilters.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/data/blockfilters.json) | `d9049756f744e561b882a8eff507582fb7cd74ed9cf5542bdac58257449ee2a2` |
| [`upstream/bitcoin-core/blockfilter.py`](upstream/bitcoin-core/blockfilter.py) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/test/functional/test_framework/blockfilter.py) | `d759d7c22eb1fd5e517902d01a30938a991c3a34e6672c9d44a0b4f936a8fcfc` |
| [`upstream/bip158/testnet-19.json`](upstream/bip158/testnet-19.json) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0158/testnet-19.json) | `d9049756f744e561b882a8eff507582fb7cd74ed9cf5542bdac58257449ee2a2` |
| [`upstream/bip158/bip-0158.mediawiki`](upstream/bip158/bip-0158.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0158.mediawiki) | `93a560d966852e081f5e7894f43eddae55ac471ff8bcb99d93eae73f5a859954` |
| [`upstream/bip157/bip-0157.mediawiki`](upstream/bip157/bip-0157.mediawiki) | [pinned source](https://raw.githubusercontent.com/bitcoin/bips/5aadddb511a5dcfe4599531bd8ae5e5b96d0f793/bip-0157.mediawiki) | `5405acbf04ebd8a1b92ae2b7188022f4c14723ed8a0ae0d077d12a1e6908da9a` |
| [`upstream/bolts/01-messaging.md`](upstream/bolts/01-messaging.md) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/01-messaging.md) | `5801219aabcf854cf0a6dd4e98759b4351cbca238588061c680574912f6ef918` |
| [`upstream/bolts/09-features.md`](upstream/bolts/09-features.md) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/09-features.md) | `39ead9fa5743104a3e66e0f6c342e34d64966dc943dcbcc337969e543e3e3cbe` |
| [`upstream/bolts/02-peer-protocol.md`](upstream/bolts/02-peer-protocol.md) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/02-peer-protocol.md) | `75391608469812c1839d0149edb1a626420011ba2b8373135584f5afb7034245` |
| [`upstream/bolts/03-transactions.md`](upstream/bolts/03-transactions.md) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/03-transactions.md) | `27ffbf1ae436f257bff3f255b4d1e4d4bbf8597fb1e2533d7ff9b66f4fe38d43` |
| [`upstream/bolts/08-transport.md`](upstream/bolts/08-transport.md) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/08-transport.md) | `85a1d6402adb653416c5a48f3b4c8eac2d8964621c41cb1bb6e09b8e06fbde46` |
| [`upstream/bitcoin-core/addrman.cpp`](upstream/bitcoin-core/addrman.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/addrman.cpp) | `d4b7ae941920c48731fc04a6bfbefb11c65d829b1a81b45ff1084581dd7fee0a` |
| [`upstream/bitcoin-core/addrman.h`](upstream/bitcoin-core/addrman.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/addrman.h) | `198cbe03201d36dd95398dcaa8b0ca7ec7dc52faaf37e3f08367f7a868a1ff44` |
| [`upstream/bitcoin-core/addrman_impl.h`](upstream/bitcoin-core/addrman_impl.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/addrman_impl.h) | `a5eadae6360c12ddcb416dcef42642ae8af07b0d35f5472c18be0161b8204c40` |
| [`upstream/bitcoin-core/addrman_tests.cpp`](upstream/bitcoin-core/addrman_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/addrman_tests.cpp) | `c42ee5f8f1d7756b18c1d52f6ce66223669f83a1551a359ddcf420f7eef084f1` |
| [`upstream/bitcoin-core/netgroup.cpp`](upstream/bitcoin-core/netgroup.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/netgroup.cpp) | `81d2a13a3801dae67f48ae45fc604c10358c4360a9dc11eccd7b6e535d162c60` |
| [`upstream/bitcoin-core/netgroup.h`](upstream/bitcoin-core/netgroup.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/netgroup.h) | `e4a239b35db4d01d2e5e78ce7188e1d0e8b4ba3a4bba1f096b41319d5e0f1aa9` |
| [`upstream/bitcoin-core/random.h`](upstream/bitcoin-core/random.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/random.h) | `5459198b3ae7962176537041ae01a8f74f7fa0a02e5b363ce378e6f5ac7ac430` |
| [`upstream/bitcoin-core/eviction.cpp`](upstream/bitcoin-core/eviction.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/node/eviction.cpp) | `f9cabeeec931cd4d3c2b50f0e9abe8fbaa87cef5c47fbd6de6ecfadc4bd9ce3c` |
| [`upstream/bitcoin-core/eviction.h`](upstream/bitcoin-core/eviction.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/node/eviction.h) | `4d140c3f4d5c7eff6ae88083d31803febcfbce29e3518aa3fc435a916832b55e` |
| [`upstream/bitcoin-core/net_peer_eviction_tests.cpp`](upstream/bitcoin-core/net_peer_eviction_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/net_peer_eviction_tests.cpp) | `02c8421823aa431341b600bb498551c1de15596b3e948c56297d1a8978305b2e` |
| [`upstream/bitcoin-core/netbase.cpp`](upstream/bitcoin-core/netbase.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/netbase.cpp) | `2cf9b0a0f94e8632fa585927c16e6714352770d31affe6ce986c19ad468146d4` |
| [`upstream/bitcoin-core/random.cpp`](upstream/bitcoin-core/random.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/random.cpp) | `36c7f0204674539fa86d78766cd0f074420a1c46578a054bc1f0ac1c0b62a090` |
| [`upstream/bitcoin-core/test_util_net.cpp`](upstream/bitcoin-core/test_util_net.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/util/net.cpp) | `432b91c123750a21097cb2efacbc5f0fca88c461a8606e4a458bb71c257dbb5e` |
| [`upstream/bitcoin-core/banman.cpp`](upstream/bitcoin-core/banman.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/banman.cpp) | `09032596d182a7ff35fb2d179ced495a96efca63635817c2f54012ad6f3839fd` |
| [`upstream/bitcoin-core/banman.h`](upstream/bitcoin-core/banman.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/banman.h) | `831066451b07a93fdbe901fe544a79ae5564ccebbe4a53f65c1e2d15d78bef23` |
| [`upstream/bitcoin-core/bloom.cpp`](upstream/bitcoin-core/bloom.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/common/bloom.cpp) | `58a6ef2ff1895ba8b3bdac8363bfa700cd780273e65a92a611eaa7dc6be0f6ee` |
| [`upstream/bitcoin-core/bloom.h`](upstream/bitcoin-core/bloom.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/common/bloom.h) | `80c3f012f00f271c345794ef37eb48d757be56d1cb2a2f6122db4ddeb2ab74f3` |
| [`upstream/bitcoin-core/hash.cpp`](upstream/bitcoin-core/hash.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/hash.cpp) | `96a57445526b9d3ee840a17bff153115fdccb54eabb066c8ec9af90402a54e55` |
| [`upstream/bitcoin-core/headerssync.cpp`](upstream/bitcoin-core/headerssync.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/headerssync.cpp) | `bc61319b39e993d07a9852e67869b479fdbf258d290c9d13bcfd8386c68e25a6` |
| [`upstream/bitcoin-core/headerssync.h`](upstream/bitcoin-core/headerssync.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/headerssync.h) | `60485439e8fcee7ac9ab1cef39fc42a114bfd40c769e918cf4fd5e88489d29aa` |
| [`upstream/bitcoin-core/net_processing.h`](upstream/bitcoin-core/net_processing.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/net_processing.h) | `67a47337271295f0b2e1aed584c72b06d7f6ee7ad34df82656e7117e1a746c1c` |
| [`upstream/bitcoin-core/txdownloadman.h`](upstream/bitcoin-core/txdownloadman.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/node/txdownloadman.h) | `2f9cba3ce05f8e1688ca26f3c3e5febebb5443866fec0ddc9e0e6000111ad174` |
| [`upstream/bitcoin-core/net_permissions.h`](upstream/bitcoin-core/net_permissions.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/net_permissions.h) | `4df2ee38f4d3d4aeedbf723e8a96f9d7aa3d092873754b335355694a68cf16e9` |
| [`upstream/bitcoin-core/denialofservice_tests.cpp`](upstream/bitcoin-core/denialofservice_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/denialofservice_tests.cpp) | `6f5fcaee5f0277b293381ba6d3fc09fbbbecc07a1539d7c13bc55a5f5abdc883` |
| [`upstream/bitcoin-core/bloom_tests.cpp`](upstream/bitcoin-core/bloom_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/bloom_tests.cpp) | `12ddab254a360858cb4f567bd2c5f1ef735464d6292eec5b9b6b7b5686a330b1` |
| [`upstream/bitcoin-core/fastrange.h`](upstream/bitcoin-core/fastrange.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/util/fastrange.h) | `01d238a309b0a7737f72549785da0a90088068a462539f36e920f70764fe144a` |
| [`upstream/bitcoin-core/txdownloadman_impl.cpp`](upstream/bitcoin-core/txdownloadman_impl.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/node/txdownloadman_impl.cpp) | `bd44b0074d6f271312a2de14697f28871087531b06c49d801ebbd988f65809c7` |
| [`upstream/bitcoin-core/blockstorage.cpp`](upstream/bitcoin-core/blockstorage.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/node/blockstorage.cpp) | `5063ae02875b3370cb0d1ca4ec41003853d102ba74ab7354a342bc3cb8dc8d6f` |
| [`upstream/bitcoin-core/skiplist_tests.cpp`](upstream/bitcoin-core/skiplist_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/skiplist_tests.cpp) | `9eec53e04cf2a5a6a4269687da834bb0c61b52396212ef1b902df22e3a46c588` |
| [`upstream/bitcoin-core/blockchain_tests.cpp`](upstream/bitcoin-core/blockchain_tests.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/test/blockchain_tests.cpp) | `613c4f9158dfddb5b6cbdbc07d71f2043d8295eec18869591ea5550b0aea1e25` |
| [`upstream/bitcoin-core/validation.h`](upstream/bitcoin-core/validation.h) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/validation.h) | `2eec09d952cd5b5e509d9ca7f41367b7ffaf6321f559c4a6d763063882fb9bad` |
| [`upstream/bitcoin-core/blockchain.cpp`](upstream/bitcoin-core/blockchain.cpp) | [pinned source](https://raw.githubusercontent.com/bitcoin/bitcoin/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/rpc/blockchain.cpp) | `2b415f9689e667529bad2465f672065c7f898fad2b7cb980c01bfda12ccd5eb0` |
| [`upstream/bolts/04-onion-routing.md`](upstream/bolts/04-onion-routing.md) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/04-onion-routing.md) | `79f4e06d334a200f62b6053cc891f2c4dd710e7a43e1c7b3691848dc4c460070` |
| [`upstream/bolts/bolt04/onion-test.json`](upstream/bolts/bolt04/onion-test.json) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/bolt04/onion-test.json) | `bff01ffc728c3fecbee133eedfdaf39a26984f35cf7e76ed80f14d6e78cb8d56` |
| [`upstream/bolts/bolt04/onion-error-test.json`](upstream/bolts/bolt04/onion-error-test.json) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/bolt04/onion-error-test.json) | `374800b8d0dd21920299f4a998c4278f9d66f3a00b5e7b396765d38a50f2ea11` |
| [`upstream/bolts/bolt04/route-blinding-test.json`](upstream/bolts/bolt04/route-blinding-test.json) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/bolt04/route-blinding-test.json) | `d2c32e0260e6cb189246ec93bfc606459ef7c8abcad666fab4c496fe9644ef66` |
| [`upstream/bolts/bolt04/blinded-payment-onion-test.json`](upstream/bolts/bolt04/blinded-payment-onion-test.json) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/bolt04/blinded-payment-onion-test.json) | `ce74ab58d12031eebf080c3fde4e259baf9c66bbd1b1cf45f098669ad58b5dab` |
| [`upstream/bolts/bolt04/blinded-onion-message-onion-test.json`](upstream/bolts/bolt04/blinded-onion-message-onion-test.json) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/bolt04/blinded-onion-message-onion-test.json) | `e5e8ac8a42d3d0ce3bb70c12c5bbd76408e022a63322dcf32b00ddd97d051f96` |
| [`upstream/bolts/11-payment-encoding.md`](upstream/bolts/11-payment-encoding.md) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/11-payment-encoding.md) | `c659da1bc08091f7fe691cd4ce661e578dfbe1f77a0b9cd9c53d1839010ae029` |
| [`upstream/bolts/bolt11/invoice-test.json`](upstream/bolts/bolt11/invoice-test.json) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/bolt11/invoice-test.json) | `5d6af32b7dfb84722e4e8a94d633d1597b9504a836c32046219b82c1edb7824a` |
| [`upstream/bolts/12-offer-encoding.md`](upstream/bolts/12-offer-encoding.md) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/12-offer-encoding.md) | `f85fbc0577536c82d94f713076221c76c32218b7728ccfed56962d59ccdcee89` |
| [`upstream/bolts/bolt12/format-string-test.json`](upstream/bolts/bolt12/format-string-test.json) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/bolt12/format-string-test.json) | `a88e3098b3afee8cd31bf1091d76f2f265f5f0e2e397f56fc2eef8da7e3f131d` |
| [`upstream/bolts/bolt12/offers-test.json`](upstream/bolts/bolt12/offers-test.json) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/bolt12/offers-test.json) | `d30991bbd31105f87b583e1d4168a9beb8b7f9cf2b23c729597428e0dce832c7` |
| [`upstream/bolts/bolt12/payer-proof-test.json`](upstream/bolts/bolt12/payer-proof-test.json) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/bolt12/payer-proof-test.json) | `bad5f365f02c3bd82fcb5d600ba941592b211e715103b134f4deed599a650ad9` |
| [`upstream/bolts/bolt12/signature-test.json`](upstream/bolts/bolt12/signature-test.json) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/bolt12/signature-test.json) | `03ddd14f5593bea0a97e5b3cbe3af9fe215d38443ae192a01bb72a840858d6a4` |
| [`upstream/bolts/07-routing-gossip.md`](upstream/bolts/07-routing-gossip.md) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/07-routing-gossip.md) | `4b7cb122c9ba5c065d779671b52b9d2ec6be18cf34dddd046a8714a12af4a5c8` |
| [`upstream/bolts/bolt07/extended-queries.json`](upstream/bolts/bolt07/extended-queries.json) | [pinned source](https://raw.githubusercontent.com/lightning/bolts/191462825ae341f0a27098765c075f01ca9bc6c2/bolt07/extended-queries.json) | `9927ae2c9e1a68ad0f99830f2e1c318a50ff1b17d51e37eb0111cdc56ba16f79` |
| [`derived/catalog.json`](derived/catalog.json) | `make_catalog.py` (derived) | `eb5be14f52c41f115153b3d9d4189b30c1c984e62ddc1278793eef6b80321d27` |
