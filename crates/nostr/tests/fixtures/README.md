# Nostr crate fixtures

These fixtures back the integration tests in `crates/nostr/tests/`. The
domain corpus that the relay shares stays in the repository-level
`tests/fixtures/`; this directory holds only what the pure crate's
independent tests need.

## `nip44/nip44.vectors.json`

The official NIP-44 v2 test vectors.

- Source: [paulmillr/nip44](https://github.com/paulmillr/nip44), file
  `nip44.vectors.json`, at commit
  `671a1f04bcfacaf125b0db68adc45bc9ce0e763b` (2024-12-04). URL:
  <https://github.com/paulmillr/nip44/blob/671a1f04bcfacaf125b0db68adc45bc9ce0e763b/nip44.vectors.json>.
- Integrity: SHA-256
  `269ed0f69e4c192512cc779e78c555090cebc7c785b609e338a62afc3ce25040`. The
  pinned specification, `nips/official/44.md`, publishes that same checksum
  under **Tests and code**, so the copy here is the one the spec names.
- License: the file carries no license header. The repository that
  publishes it ships the TypeScript reference implementation under the
  Unlicense (public domain, `javascript/LICENSE`), and its README lists the
  TypeScript lane as "Public domain", copied from
  [nostr-protocol/nips](https://github.com/nostr-protocol/nips). The
  vectors are test data and are used here unchanged.
- Divergence from the pinned spec: the vectors predate the 2026-06-28
  revision of NIP-44 (nostr-protocol/nips commit `733a047`, pull request
  1907) that added the 6-byte extended length prefix for plaintexts of
  65536 bytes or more. `invalid.encrypt_msg_lengths` lists `65536` and
  `100000` as refused; under the pinned spec they are valid, and this crate
  accepts them up to its own client bound of 256 KiB. The test records this
  in `nip44_vectors.rs` and asserts the pinned behavior for those two
  entries rather than the pre-revision one.

## `regressions/`

Minimized inputs that a property test found. `tests/properties.rs` writes
its failure seeds to `properties.proptest-regressions` here, so a failing
seed reruns first on the next `cargo test -p nostr`. Keep a seed file only
when it records a defect in the crate; a defect in a test strategy is fixed
in the test and its seed removed. The directory is empty when no property
test has found a defect; the `.gitkeep` holds the path.
