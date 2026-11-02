# 54. Lists, sets and bookmarks

Lists hold a person's mutes, pins, bookmarks and groupings. NIP-51 puts
public items in an event's tags and private items, encrypted to the list's
owner, in its content. This chapter reads both, checks every item against
the tags its kind may hold, writes private items with NIP-44, and answers
whether a mute list mutes a pubkey, an event, a hashtag or a word.

Part VI, Every NIP. Result: NIP-51 standard lists (10000 mutes, 10001 pins,
10003 bookmarks and the rest of its table) and sets (30000 follow sets and
the rest), with public tags and private items encrypted to self, and NIP-B0
kind 39701 web bookmarks.

## What this chapter teaches

- Standard lists are replaceable kinds (10000 to 10992); sets are
  addressable kinds (30000 to 39092), one per `d` tag.
- Each kind holds a fixed set of tag names. A tag outside that set is
  refused, in public and in private items alike.
- Private items are a JSON tags array encrypted with NIP-44 to the owner's
  own key: the conversation key of their secret and their own public key.
- NIP-04 private items, from older clients, are read only when the caller
  allows it, and the next write moves them to NIP-44.
- A mute list mutes pubkeys and events by exact hex, hashtags and words
  with ASCII letters folded.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip51.coax](../../coax/lib/nostr/nip51.coax) | `nl_allowed(kind)` | the tag names a kind holds, as bits |
| | `nl_check(e, allowed, is_private)` | every tag of a tags array against those names |
| | `nl_read(e)` | a list's public items and how its content is encrypted |
| | `nl_item(l, e, index)` | item `index`, public ones first |
| | `nl_muted(l, e, what, value)` | the first item that mutes `value`, or -1 |
| [nip51-private.coax](../../coax/lib/nostr/nip51-private.coax) | `nl_self(secret, work, out)` | the owner's public key and both keys to self |
| | `nl_open(l, e, me, allow_legacy, work, plain)` | the private items, decrypted and checked |
| | `nl_seal(kind, me, public, private, nonce, work, out)` | a list's content: private items checked and encrypted |

Both files are `#authority;`: a mute list decides what a client shows. Kind
3 follow lists are chapter 45's (`follows.coax`) and kind 10002 relay lists
are chapter 32's (`nip65.coax`); `nl_allowed` returns 0 for them.

## Lists and sets

NIP-51 has two tables. A *standard list* is a replaceable event, so a
person has one of each kind: one mute list, one pin list, one bookmark
list. A *set* is an addressable event, so a person has as many as they
have `d` tags: one follow set per group of people, one bookmark set per
topic. Every set may also carry `title`, `image` and `description`. NIP-B0's
web bookmark, kind 39701, is addressable too: its `d` is the bookmarked URI
with `https://` left off, and its content is a description, not private
items.

`nl_allowed` is the two tables as code. Each tag name is one bit:

<!-- source: coax/lib/nostr/nip51.coax table -->
```jai
// The tag names each kind holds: NIP-51's tables of standard lists and sets,
// and NIP-B0. 0 is not a list kind.
nl_allowed :: (kind: int) -> int {
  if kind == 10000 { return NL_P | NL_T | NL_WORD | NL_E; }      // mutes
  if kind == 10001 || kind == 10005 { return NL_E; }             // pins, public chats
  if kind == 10003 { return NL_E | NL_A; }                       // bookmarks
  if kind == 10004 || kind == 10018 || kind == 10021 || kind == 10992 { return NL_A; }
  if kind == 10006 || kind == 10007 || kind == 10013 || kind == 10050 || kind == 10102 { return NL_RELAY; }
  if kind == 10008 { return NL_A | NL_E; }                       // profile badges
  if kind == 10009 { return NL_GROUP | NL_R; }                   // simple groups
  if kind == 10012 { return NL_RELAY | NL_A; }                   // relay feeds
  if kind == 10015 { return NL_T | NL_A; }                       // interests
  if kind == 10017 || kind == 10020 || kind == 10064 || kind == 10101 { return NL_P; }
  if kind == 10030 { return NL_EMOJI | NL_A; }                   // emojis
  if kind == 10054 { return NL_P | NL_URL; }                     // favorite podcasts
  if kind == 10063 { return NL_SERVER; }                         // Blossom servers
  // Sets: one per d tag, each with optional title, image and description.
  if kind == 30000 || kind == 30007 || kind == 39089 || kind == 39092 { return NL_SET_META | NL_P; }
  if kind == 30002 { return NL_SET_META | NL_RELAY; }
  if kind == 30003 || kind == 30004 || kind == 30008 || kind == 30063 { return NL_SET_META | NL_E | NL_A; }
  if kind == 30005 || kind == 30006 { return NL_SET_META | NL_E; }
  if kind == 30015 { return NL_SET_META | NL_T; }
  if kind == 30030 { return NL_SET_META | NL_EMOJI; }
  if kind == 30267 || kind == 31924 { return NL_SET_META | NL_A; }
  // NIP-B0: one bookmark per d (the URI), with title, published_at and t.
  if kind == NL_KIND_WEB_BOOKMARK { return NL_D_TAG | NL_TITLE | NL_PUBLISHED_AT | NL_T; }
  return 0;
}
```

Two sets have rules for their `d`: kind 30007 (kind mute sets) needs the
muted kind as a decimal string, and kind 39701 refuses a `d` that starts
with `https://`. `nl_d_ok` checks both.

### The check

`nl_check` reads each tag with chapter 22's `nostr_tag`, so `p`, `e` and
`a` values are already checked as lowercase hex and addresses. It then
looks up the name's bit and checks the value: every held tag has a nonempty
value, a `word` is lowercase ("lowercase string" in NIP-51's table), and a
`relay` is a `ws://` or `wss://` URL. A `d` tag is refused among private
items, because a set's address must stay readable by everyone.

<!-- source: coax/lib/nostr/nip51.coax check -->
```jai
// Every tag of a tags array (e's) against the kind's names. Private items
// may not hold a d tag: the address must stay readable by everyone.
nl_check :: (e: NostrEvent, allowed: int, is_private: bool) -> NlCheck {
  c: NlCheck; name: [NL_MAX_NAME] u8; value: [NT_MAX_VALUE] u8;
  k := 0;
  while true {
    t := nostr_tag(e, k);
    if t.error == NT_INDEX { break; }
    c.bad = k; c.tag_error = t.error;
    if t.error != NT_OK { c.error = ifx t.error == NT_EVENT then NL_EVENT else NL_SHAPE; return c; }
    bit := nl_bit(name[..], nl_text(e, k, 0, name[..]));
    if (allowed & bit) == 0 || (is_private && bit == NL_D_TAG) { c.error = NL_TAG; return c; }
    // Every held tag has a value; nostr_tag already checked p, e and a.
    v := nl_text(e, k, 1, value[..]);
    if v < 1 { c.error = NL_SHAPE; return c; }
    s := cast(string) value[0..<v];
    if bit == NL_WORD && nl_has_upper(s) { c.error = NL_SHAPE; return c; }   // "lowercase string"
    if bit == NL_RELAY && !nt_relay_ok(s) { c.error = NL_SHAPE; return c; }
    if bit == NL_D_TAG && !c.has_d { c.has_d = true; for i: 0..<v { c.d[i] = value[i]; } c.d_n = v; }
    k += 1;
  }
  c.bad = -1; c.n = k;
  return c;
}
```

`nl_read` runs the check over the public tags, requires a set's `d`, and
records how the content is encrypted:

<!-- source: coax/lib/nostr/nip51.coax read -->
```jai
// A list event's public items, checked against its kind, and how its
// private items are encrypted. nl_open (nip51-private.coax) reads those.
nl_read :: (e: NostrEvent) -> NlList {
  l: NlList;
  if e.error != NE_OK || e.content_from < 0 || e.content_to > e.original.count || e.content_from > e.content_to { return l; }
  l.kind = e.kind; l.allowed = nl_allowed(e.kind); l.set = e.kind >= 30000;
  if l.allowed == 0 { return nl_fail(l, NL_KIND); }
  c := nl_check(e, l.allowed, false);
  if c.error != NL_OK { return nl_checked(l, c, 0, false); }
  l.public_n = c.n;
  if l.set && (!c.has_d || !nl_d_ok(e.kind, cast(string) c.d[0..<c.d_n])) { return nl_fail(l, NL_D); }
  l.cipher = nl_cipher(e.original[e.content_from..<e.content_to]);
  // NIP-B0's content is a description, not private items.
  if e.kind == NL_KIND_WEB_BOOKMARK { l.cipher = NL_C_NONE; }
  l.opened = l.cipher == NL_C_NONE;
  return nl_fail(l, NL_OK);
}
```

### Telling NIP-04 from NIP-44

NIP-51 says clients can tell the two by searching the content for `"iv"`.
That search alone is wrong: NIP-44's base64 payload can contain the letters
`iv`. NIP-04 content is `<base64>?iv=<base64>`, and base64 never contains
`?`, so `nl_cipher` searches for `?iv=`. An empty content has no private
items.

## Private items

Private items mimic the tags array: `[["word","giveaway"]]` is one private
`word` item. The writer serializes them as JSON and encrypts that text with
NIP-44 (chapter 20) under the conversation key of the owner's secret `d` and
the owner's own public key `P = d·G`. The shared x is the x coordinate of
`d·P = d²·G`. Only the owner can compute it: anyone else would need `d`.

`nl_self` computes the owner's public key, the shared x and the
conversation key from one secret. One ECDH gives both keys: the unhashed
shared x is NIP-04's AES key, and HKDF-extract of it is NIP-44's
conversation key.

<!-- source: coax/lib/nostr/nip51-private.coax self -->
```jai
// The owner's public key and the two keys to self from their secret: the
// unhashed shared x (NIP-04's AES key) and the NIP-44 conversation key.
// One ECDH of the secret with its own public key gives both.
nl_self :: (secret: [] u8, work: [] EcdhScratch, out: [] NlSelf) -> int {
  if secret.count != 32 || work.count != 1 || out.count != 1 { return NL_KEY; }
  out[0] = NlSelf.{}; pub33: [33] u8;
  if bip32_public(secret, pub33[..]) {
    for i: 0..<32 { out[0].pub[i] = pub33[i + 1]; }
    if ecdh_stage(secret, out[0].pub[..], work) == EC_OK {
      out[0].shared_x = work[0].staged;
      out[0].ok = nip44_conversation_key_checked(work[0].staged[..], out[0].ck[..], false) == NIP44_OK;
    }
  }
  work[0] = EcdhScratch.{};
  return ifx out[0].ok then NL_OK else NL_KEY;
}
```

### Worked example: a mute list by hand

The owner is test key 3, BIP-340 test vector 0's secret. Its public key is
`f9308a01…36f9`. The list has two public items and one private item:

| Item | Where | Tag |
| --- | --- | --- |
| 0 | tags | `["p","07caba28…a3a9"]`, a pubkey (NIP-51's own example key) |
| 1 | tags | `["t","airdrop"]`, a hashtag |
| 2 | content | `["word","giveaway"]`, a word |

1. The private JSON before encryption is the tags array of item 2 alone:
   `[["word","giveaway"]]`. Count its bytes: `[[` is 2, `"word"` is 6, the
   comma is 1, `"giveaway"` is 10 and `]]` is 2, so 21 bytes.
2. The owner's shared x with themself is the x of `3·(3·G) = 9·G`:
   `acd484e2…ccbe`. HKDF-extract with salt `nip44-v2` gives the
   conversation key `fc5e31fe…c300`.
3. NIP-44 pads 21 bytes to 32 (its smallest padded length) and puts a
   2-byte big-endian length in front: `00 15`, the 21 bytes, then 11 zero
   bytes. That is 34 bytes.
4. The payload is the version byte, the 32-byte nonce, the 34 encrypted
   bytes and a 32-byte MAC: `1 + 32 + 34 + 32 = 99` bytes. Base64 writes 3
   bytes as 4 characters, so 99 bytes are `4 · 33 = 132` characters, the
   smallest NIP-44 payload there is.
5. The example uses the test nonce `00 01 … 1f`, so the payload starts
   with the version byte 2 and the nonce: `AgABAgMEBQYH…`. A real write
   takes 32 fresh random bytes; a reused nonce under one key leaks the
   XOR of two plaintexts.
6. The event's tags are the two public items. Its content is the
   132-character payload.

A reader without the key sees items 0 and 1 and a content it cannot open.
The owner sees all three, numbered public first: item 2 is the private
`word`.

### Opening

`nl_open` refuses NIP-04 content before it uses any key, unless the caller
passes `allow_legacy`. It then requires the key to be the list's author and
decrypts. A wrong key fails NIP-44's MAC, which `nl_open` reports as
`NL_NOT_SELF`: the items were not encrypted to the owner. Under NIP-04 a
wrong key almost always leaves bad PKCS#7 padding, reported the same way.
The opened items go through the same `nl_check` as public ones, numbered
after them.

<!-- source: coax/lib/nostr/nip51-private.coax open -->
```jai
// The private items of a list nl_read returned, decrypted into plain with
// the owner's keys. NIP-04 content is refused unless allow_legacy.
nl_open :: (l: NlList, e: NostrEvent, me: NlSelf, allow_legacy: bool, work: [] NlWork, plain: [] u8) -> NlList {
  if l.error != NL_OK || l.opened { return l; }
  if l.cipher == NL_C_NIP04 && !allow_legacy { return nl_fail(l, NL_LEGACY); }
  if !me.ok || work.count != 1 { return nl_fail(l, NL_KEY); }
  if !nl_same32(me.pub, e.public_key) { return nl_fail(l, NL_NOT_OWNER); }
  r := nl_decrypt(l, e, me, work, plain);
  work[0] = NlWork.{};
  return r;
}
nl_decrypt :: (l: NlList, e: NostrEvent, me: NlSelf, work: [] NlWork, plain: [] u8) -> NlList {
  n := json_unescape(e.original[e.content_from..<e.content_to], work[0].text[..]);
  if n == -1 { return nl_fail(l, NL_LIMIT); }
  text := cast(string) work[0].text[0..<n];
  if l.cipher == NL_C_NIP04 {
    key := me.shared_x;
    r := nip04_decrypt_checked(key[..], text, plain, work[0].nip04[..], false);
    // A wrong key leaves bad padding (almost always) or bytes that are not JSON.
    if r.error == NIP04_PADDING { return nl_fail(l, NL_NOT_SELF); }
    if r.error == NIP04_OUTPUT { return nl_fail(l, NL_OUTPUT); }
    if r.error != NIP04_OK { return nl_fail(l, NL_CIPHER); }
    return nl_opened(l, plain, r.written);
  }
  ck := me.ck;
  r := nip44_decrypt_checked(NIP44_COMPAT, ck[..], text, work[0].cipher[..], plain, false);
  if r.error == NIP44_MAC { return nl_fail(l, NL_NOT_SELF); }
  if r.error == NIP44_OUTPUT { return nl_fail(l, NL_OUTPUT); }
  if r.error != NIP44_OK { return nl_fail(l, NL_CIPHER); }
  return nl_opened(l, plain, r.written);
}
```

### Sealing

`nl_seal` writes a list's content. It checks the public tags and the
private tags against the kind before it encrypts anything, so a list that
would not read back is never written. It writes only NIP-44, which is how
a list read with `allow_legacy` moves to NIP-44 on its next write. The
caller owns the `NlWork` record; `nl_open` and `nl_seal` leave it zero,
because it held the plaintext.

<!-- source: coax/lib/nostr/nip51-private.coax seal -->
```jai
// A list's content: private_tags (a JSON tags array, "[]" for none)
// checked against kind and encrypted with NIP-44 to the owner. public_tags
// are checked too, so a list that would not read back is never written.
// nonce is 32 fresh random bytes (entropy.coax); never reuse one.
nl_seal :: (kind: int, me: NlSelf, public_tags: string, private_tags: string, nonce: [] u8, work: [] NlWork, out: [] u8) -> NlSealed {
  allowed := nl_allowed(kind);
  if allowed == 0 { return nl_sealed(NL_KIND); }
  if !me.ok || nonce.count != 32 || work.count != 1 { return nl_sealed(NL_KEY); }
  if !json_valid(public_tags) { return nl_sealed(NL_EVENT); }
  p := nl_check(nt_event(public_tags), allowed, false);
  if p.error != NL_OK { return nl_sealed_at(p.error, p.bad); }
  if kind >= 30000 && (!p.has_d || !nl_d_ok(kind, cast(string) p.d[0..<p.d_n])) { return nl_sealed(NL_D); }
  if !json_valid(private_tags) { return nl_sealed(NL_PRIVATE); }
  if private_tags.count > NL_MAX_PRIVATE { return nl_sealed(NL_LIMIT); }
  q := nl_check(nt_event(private_tags), allowed, true);
  if q.error == NL_EVENT { return nl_sealed_at(NL_PRIVATE, p.n + q.bad); }
  if q.error != NL_OK { return nl_sealed_at(q.error, p.n + q.bad); }
  // NIP-B0's content is a description; it holds no private items.
  if kind == NL_KIND_WEB_BOOKMARK && q.n > 0 { return nl_sealed_at(NL_TAG, p.n); }
  if q.n == 0 { return NlSealed.{error = NL_OK}; }
  for i: 0..<private_tags.count { work[0].text[i] = private_tags[i]; }
  ck := me.ck;
  r := nip44_encrypt_checked(NIP44_COMPAT, ck[..], nonce, work[0].text[0..<private_tags.count], work[0].cipher[..], out, false);
  work[0] = NlWork.{};
  // The key, nonce, size and work are admitted above, so a short out is
  // the one refusal left.
  if r.error != NIP44_OK { return nl_sealed(NL_OUTPUT); }
  return NlSealed.{error = NL_OK, written = r.written, private_n = q.n};
}
```

## Muting

`nl_muted` walks the opened items, public then private, and returns the
first that mutes the value. Pubkeys and event ids are compared as
lowercase hex, exactly. Hashtags are compared with ASCII letters folded,
so `#AirDrop` matches `["t","airdrop"]`. A note's text is muted when it
contains a `word` item, folded the same way. A list that is not opened
answers `NL_NOT_MUTE_LIST`, so a caller cannot mistake "I could not read
the private items" for "nothing is muted".

<!-- source: coax/lib/nostr/nip51.coax muted -->
```jai
// The first item of a read mute list that mutes value, or -1. A pubkey or
// event id is its lowercase hex and must match exactly; a hashtag (without
// '#') matches a t item with ASCII case folded; a note's text is muted when
// it contains a word item, folded the same way.
nl_muted :: (l: NlList, e: NostrEvent, what: int, value: string) -> int {
  if l.error != NL_OK || l.kind != NL_KIND_MUTE || !l.opened { return NL_NOT_MUTE_LIST; }
  name := ifx what == NL_MUTE_PUBKEY then "p" else ifx what == NL_MUTE_EVENT then "e" else ifx what == NL_MUTE_HASHTAG then "t" else "word";
  for i: 0..<l.public_n + l.private_n {
    item_ := nl_item(l, e, i);
    if cast(string) item_.name[0..<item_.name_n] != name { continue; }
    item := cast(string) item_.value[0..<item_.value_n];
    hit := ifx what == NL_MUTE_WORD then nl_contains_folded(value, item)
      else ifx what == NL_MUTE_HASHTAG then nl_same_folded(value, item) else value == item;
    if hit { return i; }
  }
  return -1;
}
```

## NIP-51's own examples

NIP-51's mute list example has NIP-04 content (`…?iv=S3rFeFr1…`), and the
id it prints, `a92a316b…bcd0`, is not the id of its fields: those hash to
`a326f2cc…c02c`. NIP-B0's example bookmark prints `d7a92714…3ec7`; its
fields hash to `2453e802…1f8b`. The tags of both, and of NIP-51's curation
set of yaks, read under the tables. The NIP-51 mute list's private items
cannot be opened here: they belong to a key whose secret is not published.

## The example

The example is [nostr/examples/lists-sets.coax](../examples/lists-sets.coax).
It prints the tags a few kinds hold and reads NIP-51's yaks and NIP-B0's
bookmark. It derives key 3's keys, seals the worked mute list, signs it, and
reads it back first as a stranger and then as the owner. It asks the list
about two pubkeys, a hashtag and two notes. It then opens a list a NIP-04
client wrote (made with `openssl`), refuses it, opens it as legacy, and
writes it again with NIP-44. Last come NIP-51's own example and seven
broken lists.

<!-- source: nostr/examples/lists-sets.coax example -->
```jai
#load "../../coax/lib/nostr/nip51-private.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// Test keys, never real ones. Key 3 is BIP-340 test vector 0's secret and
// owns the lists; key 1's public key is G's x.
KEY_3 :: "0000000000000000000000000000000000000000000000000000000000000003";
AUX :: "0000000000000000000000000000000000000000000000000000000000000000";
NOW :: 1793214821;   // 2026-10-28T19:13:41Z, as in chapters 38 and 42
SPAMMER :: "07caba282f76441955b695551c3c5c742e5b9202a3784780f8086fdcdc1da3a9";
FRIEND :: "a55c15f5e41d5aebd236eca5e0142789c5385703f1a7485aa4b38d94fd18dcc4";
// The worked mute list: two public items, one private.
PUBLIC :: "[[\"p\",\"07caba282f76441955b695551c3c5c742e5b9202a3784780f8086fdcdc1da3a9\"],[\"t\",\"airdrop\"]]";
PRIVATE :: "[[\"word\",\"giveaway\"]]";
// A legacy client's private items for the same list: NIP-04 (AES-256-CBC
// under the unhashed shared x to self), made with openssl and iv 10 11 .. 1f.
LEGACY :: "Oqf7zWp1+jpQG1snq/n0kjns/f89zrtWAUGEYeb1PWS45KViy+0qxvwRmNEKMOSwaLVLxD2Z6dWuBBCnyW39D3uc6Ib3t5MOMDMPJcOwyaM=?iv=EBESExQVFhcYGRobHB0eHw==";
// NIP-51's own mute list example, compacted.
NIP51 :: "{\"id\":\"a92a316b75e44cfdc19986c634049158d4206fcc0b7b9c7ccbcdabe28beebcd0\",\"pubkey\":\"854043ae8f1f97430ca8c1f1a090bdde6488bd5115c7a45307a2a212750ae4cb\",\"created_at\":1699597889,\"kind\":10000,\"tags\":[[\"p\",\"07caba282f76441955b695551c3c5c742e5b9202a3784780f8086fdcdc1da3a9\"],[\"p\",\"a55c15f5e41d5aebd236eca5e0142789c5385703f1a7485aa4b38d94fd18dcc4\"]],\"content\":\"TJob1dQrf2ndsmdbeGU+05HT5GMnBSx3fx8QdDY/g3NvCa7klfzgaQCmRZuo1d3WQjHDOjzSY1+MgTK5WjewFFumCcOZniWtOMSga9tJk1ky00tLoUUzyLnb1v9x95h/iT/KpkICJyAwUZ+LoJBUzLrK52wNTMt8M5jSLvCkRx8C0BmEwA/00pjOp4eRndy19H4WUUehhjfV2/VV/k4hMAjJ7Bb5Hp9xdmzmCLX9+64+MyeIQQjQAHPj8dkSsRahP7KS3MgMpjaF8nL48Bg5suZMxJayXGVp3BLtgRZx5z5nOk9xyrYk+71e2tnP9IDvSMkiSe76BcMct+m7kGVrRcavDI4n62goNNh25IpghT+a1OjjkpXt9me5wmaL7fxffV1pchdm+A7KJKIUU3kLC7QbUifF22EucRA9xiEyxETusNludBXN24O3llTbOy4vYFsq35BeZl4v1Cse7n2htZicVkItMz3wjzj1q1I1VqbnorNXFgllkRZn4/YXfTG/RMnoK/bDogRapOV+XToZ+IvsN0BqwKSUDx+ydKpci6htDRF2WDRkU+VQMqwM0CoLzy2H6A2cqyMMMD9SLRRzBg==?iv=S3rFeFr1gsYqmQA7bNnNTQ==\",\"sig\":\"1173822c53261f8cffe7efbf43ba4a97a9198b3e402c2a1df130f42a8985a2d0d3430f4de350db184141e45ca844ab4e5364ea80f11d720e36357e1853dba6ca\"}";
// NIP-51's curation set of yaks and NIP-B0's bookmark: their tags.
YAKS :: "[[\"d\",\"jvdy9i4\"],[\"title\",\"Yaks\"],[\"image\",\"https://cdn.britannica.com/40/188540-050-9AC748DE/Yak-Himalayas-Nepal.jpg\"],[\"a\",\"30023:26dc95542e18b8b7aec2f14610f55c335abebec76f3db9e58c254661d0593a0c:95ODQzw3ajNoZ8SyMDOzQ\"],[\"e\",\"d78ba0d5dce22bfff9db0a9e996c9ef27e2c91051de0c4e1da340e0326b4941e\"]]";
BOOKMARK :: "[[\"d\",\"alice.blog/post\"],[\"published_at\",\"1738863000\"],[\"title\",\"Blog insights by Alice\"],[\"t\",\"post\"],[\"t\",\"insight\"]]";

text_of :: (n: int, out: [] u8) -> string { #assert n > 0; return cast(string) out[0..<n]; }
hex_of :: (bytes: [32] u8, out: [] u8) -> string {
  b := bytes; #assert hex_encode_checked(cast(string) b[..], out, false).error == NC_OK;
  return cast(string) out[0..<64];
}
key_of :: (hex: string) -> [32] u8 { k: [32] u8; #assert hex_decode_checked(hex, k[..], false).error == NC_OK; return k; }
// A test nonce, 00 01 .. 1f, so the output is fixed. A real write takes 32
// fresh random bytes.
test_nonce :: () -> [32] u8 { n: [32] u8; for i: 0..<32 { n[i] = cast(u8) i; } return n; }
// Signs one event with key 3 into out.
sign :: (created_at: int, kind: int, tags: string, content: string, out: [] u8) -> string {
  key := key_of(KEY_3); aux := key_of(AUX);
  work: [1] NostrSignWork; scratch: [4096] u8; k := EVENT_MORE;
  while k == EVENT_MORE {
    k = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  return text_of(k, out);
}
NAMES :: [16] string.["p", "e", "a", "t", "word", "relay", "group", "r", "emoji", "server", "url", "d", "title", "image", "description", "published_at"];
print_names :: (allowed: int) {
  for i: 0..<16 { if (allowed & (1 << i)) != 0 { print(" %", NAMES[i]); } }
  print("\n");
}
ERRORS :: [14] string.["ok", "not a parsed event", "not a list kind", "tag not held by this kind", "malformed value", "missing or refused d tag",
  "NIP-04 content refused", "not the list's owner", "not addressed to the owner's own key", "malformed payload",
  "private items are not a tags array", "key", "output too small", "too large"];
why :: (code: int) -> string { return ERRORS[code]; }
// Every item of a read list, one per line.
print_items :: (l: NlList, e: NostrEvent) {
  for i: 0..<l.public_n + l.private_n {
    item := nl_item(l, e, i);
    print("  %  % %  %\n", i, ifx item.is_private then "private" else "public ", cast(string) item.name[0..<item.name_n], cast(string) item.value[0..<item.value_n]);
  }
}
yes :: (i: int) -> string { return ifx i >= 0 then "muted" else "not muted"; }

main :: () {
  print("== 1. The tags each kind holds\n");
  kinds := [7] int.[10000, 10001, 10003, 10006, 30000, 30004, 39701];
  for k: kinds { print("  %:", k); print_names(nl_allowed(k)); }
  print("  3, 10002 and 1: % (follows.coax, nip65.coax)\n", nl_allowed(3) + nl_allowed(10002) + nl_allowed(1));
  b: [4096] u8;
  yaks := nl_read(nl_event(30004, YAKS, "", b[..]));
  print("NIP-51's curation set of yaks: %, % items, set %\n", why(yaks.error), yaks.public_n, yaks.set);
  bm := nl_read(nl_event(39701, BOOKMARK, "A marvelous insight by Alice about the nature of blogs and posts.", b[..]));
  print("NIP-B0's web bookmark: %, % items, content is private items: %\n", why(bm.error), bm.public_n, bm.cipher != NL_C_NONE);

  print("\n== 2. The worked mute list, sealed and signed by key 3\n");
  w: [1] NlWork;
  secret := key_of(KEY_3); me: [1] NlSelf; ew: [1] EcdhScratch;
  #assert nl_self(secret[..], ew[..], me[..]) == NL_OK;
  hx: [64] u8; h2: [64] u8; h3: [64] u8;
  print("owner        %\nshared x     %\nconversation %\n", hex_of(me[0].pub, hx[..]), hex_of(me[0].shared_x, h2[..]), hex_of(me[0].ck, h3[..]));
  print("public tags  %\nprivate JSON % (% bytes, padded to %)\n", PUBLIC, PRIVATE, PRIVATE.count, nip44_calc_padded_len(PRIVATE.count));
  nonce := test_nonce(); content: [512] u8;
  s := nl_seal(10000, me[0], PUBLIC, PRIVATE, nonce[..], w[..], content[..]);
  sealed := cast(string) content[0..<s.written];
  print("nl_seal: %, % private item, % characters\n  %\n", why(s.error), s.private_n, s.written, sealed);
  eb: [1024] u8; event := sign(NOW, NL_KIND_MUTE, PUBLIC, sealed, eb[..]);
  print("event\n  %\n", event);

  print("\n== 3. Reading it\n");
  sc: [4096] u8; e := nostr_event_verify_vartime(event, sc[..], false);
  print("signature verifies: %\n", e.error == NE_OK && e.verified);
  l := nl_read(e);
  print("anyone: %, % public items, private items encrypted (NIP-44 %), opened %\n", why(l.error), l.public_n, l.cipher == NL_C_NIP44, l.opened);
  plain: [NL_MAX_PRIVATE] u8;
  l = nl_open(l, e, me[0], false, w[..], plain[..]);
  print("the owner: %, opened %, % public + % private\n", why(l.error), l.opened, l.public_n, l.private_n);
  print_items(l, e);

  print("\n== 4. What the list mutes\n");
  print("  pubkey 07caba..a3a9          %\n", yes(nl_muted(l, e, NL_MUTE_PUBKEY, SPAMMER)));
  print("  pubkey a55c15..dcc4          %\n", yes(nl_muted(l, e, NL_MUTE_PUBKEY, FRIEND)));
  print("  hashtag #AirDrop             % (item %)\n", yes(nl_muted(l, e, NL_MUTE_HASHTAG, "AirDrop")), nl_muted(l, e, NL_MUTE_HASHTAG, "AirDrop"));
  print("  note \"Huge GIVEAWAY tonight\" % (item %)\n", yes(nl_muted(l, e, NL_MUTE_WORD, "Huge GIVEAWAY tonight")), nl_muted(l, e, NL_MUTE_WORD, "Huge GIVEAWAY tonight"));
  print("  note \"good morning\"          %\n", yes(nl_muted(l, e, NL_MUTE_WORD, "good morning")));
  stranger := nl_read(e);
  print("  a reader without the key: note \"Huge GIVEAWAY tonight\" % (NL_NOT_MUTE_LIST: the list is not opened)\n", nl_muted(stranger, e, NL_MUTE_WORD, "Huge GIVEAWAY tonight"));

  print("\n== 5. A list a NIP-04 client wrote\n");
  ob: [1024] u8; old := sign(NOW, NL_KIND_MUTE, PUBLIC, LEGACY, ob[..]);
  o := nostr_event_read(old, sc[..], false); ol := nl_read(o);
  print("cipher NIP-04: %\n", ol.cipher == NL_C_NIP04);
  print("nl_open, legacy not allowed: %\n", why(nl_open(ol, o, me[0], false, w[..], plain[..]).error));
  ol = nl_open(ol, o, me[0], true, w[..], plain[..]);
  print("nl_open, legacy allowed: %, legacy %, private items %\n", why(ol.error), ol.legacy, ol.hidden);
  mc: [512] u8; m := nl_seal(10000, me[0], PUBLIC, ol.hidden, nonce[..], w[..], mc[..]);
  nb: [1024] u8; moved := sign(NOW + 60, NL_KIND_MUTE, PUBLIC, cast(string) mc[0..<m.written], nb[..]);
  n := nostr_event_read(moved, sc[..], false); nl := nl_open(nl_read(n), n, me[0], false, w[..], plain[..]);
  print("the next write: NIP-44 %, %, legacy %, id %\n", nl.cipher == NL_C_NIP44, why(nl.error), nl.legacy, hex_of(n.id, hx[..]));

  print("\n== 6. NIP-51's own example\n");
  x := nostr_event_read(NIP51, sc[..], false);
  print("claimed id  a92a316b75e44cfdc19986c634049158d4206fcc0b7b9c7ccbcdabe28beebcd0\ncomputed id %\n", hex_of(x.computed_id, hx[..]));
  xp := ne_parse(NIP51); xl := nl_read(xp);
  print("its fields: %, % public items, cipher NIP-04 %\n", why(xl.error), xl.public_n, xl.cipher == NL_C_NIP04);
  print("nl_open, legacy not allowed: %\n", why(nl_open(xl, xp, me[0], false, w[..], plain[..]).error));
  print("nl_open by key 3, legacy allowed: %\n", why(nl_open(xl, xp, me[0], true, w[..], plain[..]).error));

  print("\n== 7. Broken lists\n");
  r := nl_read(nl_event(10000, "[[\"t\",\"airdrop\"],[\"relay\",\"wss://relay.example.com\"]]", "", b[..]));
  print("a relay tag in a mute list: % (item %)\n", why(r.error), r.bad);
  r = nl_read(nl_event(10000, "[[\"word\",\"GiveAway\"]]", "", b[..]));
  print("an uppercase word: %\n", why(r.error));
  r = nl_read(nl_event(30000, "[[\"p\",\"07caba282f76441955b695551c3c5c742e5b9202a3784780f8086fdcdc1da3a9\"]]", "", b[..]));
  print("a follow set without d: %\n", why(r.error));
  zc: [512] u8;
  z := nl_seal(10000, me[0], PUBLIC, "[[\"relay\",\"wss://relay.example.com\"]]", nonce[..], w[..], zc[..]);
  print("nl_seal, a private relay tag: % (item %)\n", why(z.error), z.bad);
  z = nl_seal(30000, me[0], "[[\"d\",\"friends\"]]", "[[\"d\",\"secret\"]]", nonce[..], w[..], zc[..]);
  print("nl_seal, a private d tag: %\n", why(z.error));
  // The worked items encrypted to key 1 instead of to self.
  ck1: [32] u8; g := NIP44_KEY_GENERATOR_X;
  #assert nip44_conversation_key_from_keys_checked(secret[..], g[..], ew[..], ck1[..], false) == NIP44_OK;
  pt: [32] u8; for i: 0..<PRIVATE.count { pt[i] = PRIVATE[i]; }
  wk: [1024] u8; tc: [512] u8;
  t := nip44_encrypt_checked(NIP44_COMPAT, ck1[..], nonce[..], pt[0..<PRIVATE.count], wk[..], tc[..], false);
  ab: [1024] u8; aimed := sign(NOW, NL_KIND_MUTE, PUBLIC, cast(string) tc[0..<t.written], ab[..]);
  a := nostr_event_read(aimed, sc[..], false);
  print("private items encrypted to key 1: %\n", why(nl_open(nl_read(a), a, me[0], false, w[..], plain[..]).error));
  other := me[0]; other.pub = NIP44_KEY_GENERATOR_X;
  print("the worked list opened with key 1's keys: %\n", why(nl_open(nl_read(e), e, other, false, w[..], plain[..]).error));
  at := 0; while event[at] != 'A' || event[at + 1] != 'g' { at += 1; }
  tb: [1024] u8; for i: 0..<event.count { tb[i] = event[i]; }
  tb[at + 60] = ifx tb[at + 60] == 'B' then cast(u8) 'C' else cast(u8) 'B';
  tp := ne_parse(cast(string) tb[0..<event.count]);
  print("one payload character changed (id not checked): %\n", why(nl_open(nl_read(tp), tp, me[0], false, w[..], plain[..]).error));
  nl_self_forget(me[..]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/lists-sets.coax --run-steps 250000000
```

It prints this, and exits with status 0:

```text
== 1. The tags each kind holds
  10000: p e t word
  10001: e
  10003: e a
  10006: relay
  30000: p d title image description
  30004: e a d title image description
  39701: t d title published_at
  3, 10002 and 1: 0 (follows.coax, nip65.coax)
NIP-51's curation set of yaks: ok, 5 items, set true
NIP-B0's web bookmark: ok, 5 items, content is private items: false

== 2. The worked mute list, sealed and signed by key 3
owner        f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
shared x     acd484e2f0c7f65309ad178a9f559abde09796974c57e714c35f110dfc27ccbe
conversation fc5e31fe0006369674bb81fb7aab0a54241d5ac42f631399d51a8ed0f888c300
public tags  [["p","07caba282f76441955b695551c3c5c742e5b9202a3784780f8086fdcdc1da3a9"],["t","airdrop"]]
private JSON [["word","giveaway"]] (21 bytes, padded to 32)
nl_seal: ok, 1 private item, 132 characters
  AgABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fRYMpntbEm8lIxWDVpUjP7vsvi6K1v7d9t27sUHTyfzxD09VOvRljzDfiH+hJ3HQg74dTqv+PxewzBEhTBNjlbVa5
event
  {"id":"99c1756a16fbb38b86079650a2707df4ce66d57e0365b1ab57986e341961e3af","pubkey":"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","created_at":1793214821,"kind":10000,"tags":[["p","07caba282f76441955b695551c3c5c742e5b9202a3784780f8086fdcdc1da3a9"],["t","airdrop"]],"content":"AgABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fRYMpntbEm8lIxWDVpUjP7vsvi6K1v7d9t27sUHTyfzxD09VOvRljzDfiH+hJ3HQg74dTqv+PxewzBEhTBNjlbVa5","sig":"28e44b017c71ea98641b0ede5d28d5e2cdc68b3380a0d2afdce4c06d6262d451536ca0785ea4df4d6d9ee1d6625734cd6d41f3f106ce3ed154c4656c430ddd2f"}

== 3. Reading it
signature verifies: true
anyone: ok, 2 public items, private items encrypted (NIP-44 true), opened false
the owner: ok, opened true, 2 public + 1 private
  0  public  p  07caba282f76441955b695551c3c5c742e5b9202a3784780f8086fdcdc1da3a9
  1  public  t  airdrop
  2  private word  giveaway

== 4. What the list mutes
  pubkey 07caba..a3a9          muted
  pubkey a55c15..dcc4          not muted
  hashtag #AirDrop             muted (item 1)
  note "Huge GIVEAWAY tonight" muted (item 2)
  note "good morning"          not muted
  a reader without the key: note "Huge GIVEAWAY tonight" -2 (NL_NOT_MUTE_LIST: the list is not opened)

== 5. A list a NIP-04 client wrote
cipher NIP-04: true
nl_open, legacy not allowed: NIP-04 content refused
nl_open, legacy allowed: ok, legacy true, private items [["p","a55c15f5e41d5aebd236eca5e0142789c5385703f1a7485aa4b38d94fd18dcc4"]]
the next write: NIP-44 true, ok, legacy false, id 8baefb88e1a7cdf398321b797a753276892290fb0c5279df76e6fb0df336eca8

== 6. NIP-51's own example
claimed id  a92a316b75e44cfdc19986c634049158d4206fcc0b7b9c7ccbcdabe28beebcd0
computed id a326f2cc4f9524270a008b99634c4ca5d76e763120dd282bc37b32edb4a2c02c
its fields: ok, 2 public items, cipher NIP-04 true
nl_open, legacy not allowed: NIP-04 content refused
nl_open by key 3, legacy allowed: not the list's owner

== 7. Broken lists
a relay tag in a mute list: tag not held by this kind (item 1)
an uppercase word: malformed value
a follow set without d: missing or refused d tag
nl_seal, a private relay tag: tag not held by this kind (item 2)
nl_seal, a private d tag: tag not held by this kind
private items encrypted to key 1: not addressed to the owner's own key
the worked list opened with key 1's keys: not the list's owner
one payload character changed (id not checked): not addressed to the owner's own key
```

The broken cases and their refusals:

| Input | Refusal |
| --- | --- |
| a `relay` tag in a mute list | `NL_TAG`, item 1 |
| `["word","GiveAway"]` | `NL_SHAPE`: a word is lowercase |
| a follow set without `d` | `NL_D` |
| a private `relay` tag passed to `nl_seal` | `NL_TAG`, item 2 (after the two public items) |
| a private `d` tag in a follow set | `NL_TAG` |
| NIP-04 content without `allow_legacy` | `NL_LEGACY` |
| items encrypted to key 1 instead of to self | `NL_NOT_SELF` (the MAC fails) |
| the worked list opened with key 1's keys | `NL_NOT_OWNER` |
| one payload character changed | `NL_NOT_SELF` (the MAC fails) |

## How the code is checked

### Checks in the library

`nip51.coax`'s checks cover the whole table, kind by kind, and the names
it holds and refuses; NIP-51's curation set of yaks and NIP-B0's bookmark;
every refusal of `nl_read`; the `d` rules of kinds 30007 and 39701; the
`?iv=` test on contents that contain `iv` without it; and every mute case,
including uppercase hex, a pubkey asked as an event, and words that are
prefixes of a muted word.

`nip51-private.coax`'s checks compare key 3's public key, shared x and
conversation key with values from Python's integers and `hmac`. They seal
the worked list and compare the payload with Python's, byte for byte; open
it; read openssl's NIP-04 list only with `allow_legacy`; and seal it again
to Python's NIP-44 payload. They refuse items encrypted to key 1, a key
whose first byte differs from the owner's, an unready key, a short output,
a malformed payload, content over the bound, a private `d` in a set, opened
items that are not one JSON tags array, and the size bound at exactly
16384 bytes and one more. They check that every work record is zero after
each call.

Root-only mutation, run on 2026-11-01: `nip51.coax` 398 of 398 killed,
`nip51-private.coax` 200 of 200 killed, and the checks file 8 of 8.

### Checks in the example

The checks file is
[nostr/examples/checks/lists-sets.coax](../examples/checks/lists-sets.coax).
Its values come from a Python script that shares no code with Coax:
secp256k1 in Python integers, NIP-44 from `hashlib`, `hmac` and a ChaCha20
written from RFC 8439, NIP-04 from `openssl enc -aes-256-cbc`, and event
ids from `hashlib` over the NIP-01 serialization.

<!-- source: nostr/examples/checks/lists-sets.coax check -->
```jai
// Values from a Python script that shares no code with Coax: secp256k1 in
// Python integers, NIP-44 v2 from hashlib, hmac and a ChaCha20 written from
// RFC 8439, NIP-04 from the openssl command line (enc -aes-256-cbc), and ids
// from hashlib over json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False).
PY_PADDED :: "00155b5b22776f7264222c226769766561776179225d5d0000000000000000000000";  // 2-byte length, 21 bytes, 11 zeros
PY_C44 :: "AgABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fRYMpntbEm8lIxWDVpUjP7vsvi6K1v7d9t27sUHTyfzxD09VOvRljzDfiH+hJ3HQg74dTqv+PxewzBEhTBNjlbVa5";
PY_ID_WORKED :: "99c1756a16fbb38b86079650a2707df4ce66d57e0365b1ab57986e341961e3af";    // section 2's event
PY_ID_LEGACY :: "5ecdf12a19341f77d16b9a25515cbacfe41936fd4722c38c2ee6b72cad0268a7";    // section 5's NIP-04 event
PY_ID_MOVED :: "8baefb88e1a7cdf398321b797a753276892290fb0c5279df76e6fb0df336eca8";     // section 5's next write
PY_ID_NIP51 :: "a326f2cc4f9524270a008b99634c4ca5d76e763120dd282bc37b32edb4a2c02c";     // NIP-51's mute list fields
PY_ID_B0 :: "2453e8020757db78d541c5b34fbbef393ecc9d571f55cf1e229153e8f4f41f8b";        // NIP-B0's bookmark fields
// The ids the two NIPs print for their examples, copied from
// nostr/nips/official/51.md and B0.md. Neither is the id of its fields.
NIP51_CLAIMED :: "a92a316b75e44cfdc19986c634049158d4206fcc0b7b9c7ccbcdabe28beebcd0";
B0_CLAIMED :: "d7a92714f81d0f712e715556aee69ea6da6bfb287e6baf794a095d301d603ec7";
B0_EVENT :: "{\"kind\":39701,\"id\":\"d7a92714f81d0f712e715556aee69ea6da6bfb287e6baf794a095d301d603ec7\",\"pubkey\":\"2729620da105979b22acfdfe9585274a78c282869b493abfa4120d3af2061298\",\"created_at\":1738869705,\"tags\":[[\"d\",\"alice.blog/post\"],[\"published_at\",\"1738863000\"],[\"title\",\"Blog insights by Alice\"],[\"t\",\"post\"],[\"t\",\"insight\"]],\"content\":\"A marvelous insight by Alice about the nature of blogs and posts.\",\"sig\":\"36d34e6448fe0223e9999361c39c492a208bc423d2fcdfc2a3404e04df7c22dc65bbbd62dbe8a4373c62e4d29aac285b5aa4bb9b4b8053bd6207a8b45fbd0c98\"}";

same_hex :: (bytes: [32] u8, text: string) -> bool {
  want := key_of(text);
  for i: 0..<32 { if bytes[i] != want[i] { return false; } }
  return true;
}
key3 :: (me: [] NlSelf) {
  secret := key_of(KEY_3); ew: [1] EcdhScratch;
  #assert nl_self(secret[..], ew[..], me) == NL_OK;
}

#test "the worked numbers by hand: 21 bytes pad to 32, so the payload is 99 bytes, 132 characters" {
  #assert PRIVATE.count == 21 && nip44_calc_padded_len(21) == 32;
  #assert 1 + 32 + (2 + 32) + 32 == 99 && 4 * ((99 + 2) / 3) == 132;
  #assert PY_PADDED.count == 68 && PY_C44.count == 132;
  // The padded plaintext's bytes 2..22 are the private JSON itself.
  b: [34] u8; #assert hex_decode_checked(PY_PADDED, b[..], false).error == NC_OK;
  #assert b[0] == 0 && b[1] == 21 && cast(string) b[2..<23] == PRIVATE && b[23] == 0 && b[33] == 0;
}
#test "section 2: Python's NIP-44 payload and Python's event id" #steps 2000000000 {
  me: [1] NlSelf; w: [1] NlWork; key3(me[..]);
  nonce := test_nonce(); out: [512] u8;
  s := nl_seal(10000, me[0], PUBLIC, PRIVATE, nonce[..], w[..], out[..]);
  #assert s.error == NL_OK && cast(string) out[0..<s.written] == PY_C44;
  eb: [1024] u8; sc: [4096] u8;
  e := nostr_event_verify_vartime(sign(NOW, 10000, PUBLIC, PY_C44, eb[..]), sc[..], false);
  #assert e.error == NE_OK && e.verified && same_hex(e.id, PY_ID_WORKED);
  plain: [256] u8; l := nl_open(nl_read(e), e, me[0], false, w[..], plain[..]);
  #assert l.error == NL_OK && l.public_n == 2 && l.private_n == 1 && l.hidden == PRIVATE;
}
#test "section 5: openssl's NIP-04 items open; the migrated list has Python's id" #steps 2000000000 {
  me: [1] NlSelf; w: [1] NlWork; key3(me[..]);
  eb: [1024] u8; sc: [4096] u8; plain: [256] u8;
  o := nostr_event_read(sign(NOW, 10000, PUBLIC, LEGACY, eb[..]), sc[..], false);
  #assert o.error == NE_OK && same_hex(o.id, PY_ID_LEGACY);
  l := nl_open(nl_read(o), o, me[0], true, w[..], plain[..]);
  #assert l.error == NL_OK && l.legacy && l.hidden == "[[\"p\",\"a55c15f5e41d5aebd236eca5e0142789c5385703f1a7485aa4b38d94fd18dcc4\"]]";
  nonce := test_nonce(); mc: [512] u8; m := nl_seal(10000, me[0], PUBLIC, l.hidden, nonce[..], w[..], mc[..]);
  nb: [1024] u8; n := nostr_event_read(sign(NOW + 60, 10000, PUBLIC, cast(string) mc[0..<m.written], nb[..]), sc[..], false);
  #assert n.error == NE_OK && same_hex(n.id, PY_ID_MOVED);
}
#test "NIP-51's and NIP-B0's examples: their fields hash to Python's ids, not the ones printed" {
  sc: [4096] u8;
  x := nostr_event_read(NIP51, sc[..], false);
  #assert x.error == NE_ID && same_hex(x.computed_id, PY_ID_NIP51) && same_hex(x.id, NIP51_CLAIMED);
  y := nostr_event_read(B0_EVENT, sc[..], false);
  #assert y.error == NE_ID && same_hex(y.computed_id, PY_ID_B0) && same_hex(y.id, B0_CLAIMED);
  // Their tags read under NIP-51's and NIP-B0's tables all the same.
  #assert nl_read(ne_parse(NIP51)).error == NL_OK && nl_read(ne_parse(B0_EVENT)).public_n == 5;
}
#test "same_hex notices a change in the first or last byte" {
  id: [32] u8; zero := "0000000000000000000000000000000000000000000000000000000000000000";
  #assert same_hex(id, zero);
  id[0] = 1; #assert !same_hex(id, zero);
  id[0] = 0; id[31] = 1; #assert !same_hex(id, zero);
}
```

Run them with:

```shell
coax/bin/coax test nostr/examples/checks/lists-sets.coax --run-steps 250000000
```

### What is not built

- Fetching what an item points at. A pin's `e` tag is not checked to be a
  kind 1 note, nor a bookmark's `a` tag to be a kind 30023 article.
- The deprecated kind 30001 lists (`d` of `pin`, `bookmark` or
  `communities`). `nl_allowed` refuses kind 30001; a client that still
  reads them would map each to its standard list.
- Item order on write. NIP-51 asks clients to append new items at the
  end; `nl_seal` encrypts the items in the order the caller gives.
- Values past the first. `nl_item` returns the name and first value; a
  `group` tag's relay and name, or an `emoji` tag's URL, stay in the
  signed tags for the chapters of those NIPs.

## Summary

- NIP-51 lists are replaceable kinds, one per person; sets are
  addressable kinds, one per `d` tag. NIP-B0 bookmarks are addressable by
  their URI.
- Each kind holds a fixed set of tag names, checked in public and private
  items alike.
- Private items are a JSON tags array encrypted with NIP-44 under the
  conversation key of the owner's secret and own public key. A key that is
  not the owner's fails the MAC.
- NIP-04 private items are read only on request and are written again
  with NIP-44.
- The worked mute list's 21-byte private array pads to 32 bytes and
  encrypts to a 132-character payload, the same bytes Python computes.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)

## Sources

- [NIP-51](../nips/official/51.md), pinned, with its tables and examples.
- [NIP-B0](../nips/official/B0.md), pinned.
- [NIP-44](../nips/official/44.md) and [NIP-04](../nips/official/04.md)
  for the two encryptions.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
