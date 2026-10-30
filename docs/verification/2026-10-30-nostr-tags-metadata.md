# NIP-01 standard tags and kind 0 metadata (2026-10-30)

Board: #6015 `nostr-nip01-messages` (the relay messages in `message.coax`
land separately). These are new `#authority` units over the shared event
reader.

## `coax/lib/nostr/tags.coax`

`nostr_tag(event, index)` reads the index'th tag of a parsed event, and
`nostr_tag_d(event)` reads the first d tag.
- **e:** the event id, an optional `ws://`/`wss://` relay, then an author
  pubkey or a NIP-10 marker (root, reply, mention) with an author after it.
- **p:** the pubkey and an optional relay.
- **a:** `kind:pubkey:d`, where the kind must be replaceable (with an empty
  d) or addressable. A sixth kind digit refuses as `NT_LIMIT`, and leading
  zeros are refused.
- **d:** the identifier, up to 256 bytes decoded.

Any other tag reports only its value count. A value over 256 bytes refuses
only if it is a value we read. Unread values (another tag's, or a standard
tag's past the fifth) stay in the signed original. A forged record or span
refuses as `NT_EVENT`. The fixtures are in `coax/tests/nostr/tags_fixtures.coax`.

## `coax/lib/nostr/metadata.coax`

`nostr_metadata_read(event, buf, overlap)` reads a kind 0 event's content and
`nostr_metadata_parse(content)` parses it. The content must be one JSON
object of at most 65,536 bytes. Duplicate keys refuse under the shared JSON
policy. `name`, `about` and `picture` must be strings when present; other
fields are kept in the original and not read.

## Evidence

- **Checks:** `tags.coax` passes 198 of 198 and `metadata.coax` 189 of 189,
  including the suites they load. Both pass on `origin/main` when this landed.
- **Mutation:** zero survivors, none marked equivalent. Both ran as 60 shards
  with `--jobs 1`, merged.
  - `tags.coax`: 3,970 mutants, 3,970 killed.
  - `metadata.coax`: 3,680 mutants, 3,680 killed.

  Earlier runs left survivors. They were fixed with sharper tests, or by
  rewriting checks that couldn't be observed, never by exemption:
  - A `kind>65535` test that could never fail was removed.
  - The kind bound now refuses with its own error code.
  - Span checks became minimum-length checks.
  - The NIP-10 author no longer re-tests the tag type its marker implies.
- **Build used:** the checks and mutation ran on a snapshot whose shared
  crypto predates `7ca28e856e` (#4828). That commit replaced `*` with
  `mul_wrap` in `bytes.coax` (`cb_mask`) and `secp_scalar.coax`, with the
  same arithmetic and its own gates. Every other loaded unit is
  byte-identical to main.

## Not done

- **Native, Wasm and iOS runs** of these readers.
- **No consumer yet:** the project pages still read tags themselves. Moving
  them onto these readers is #b8cd `project-fold-typed`.
