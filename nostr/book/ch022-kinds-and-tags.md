# 22. Kinds and tags

A kind says what an event means and how relays keep it. Tags point at other
events, keys and addresses. This chapter reads the kind ranges, which NIP-16
and NIP-33 first defined and NIP-01 now holds, and the standard tags.

Part II, The event model. Result: Kind ranges (regular, replaceable 0, 3 and
10000–19999, ephemeral 20000–29999, addressable 30000–39999 by `d` tag) and
the standard tags `e`, `p`, `a` and `d`.

## What this chapter teaches

- Ranges: regular (1000–9999 and the small kinds), replaceable, ephemeral and
  addressable, and what the code does with the kinds NIP-01 puts in no range.
- Replacement: the newest `created_at` wins; on a tie the lowest ID wins.
- Tags: `e` with relay hint and marker, `p`, `a` as `kind:pubkey:d`, `d`.
- Kind 0 metadata: content is a JSON object; NIP-24's extra fields are kept
  and not interpreted here.
- A tag is a reading, not evidence the referenced object exists.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [store_policy.coax](../../coax/lib/nostr/store_policy.coax) | `ns_class(kind)` | The kind's class: `NS_ORDINARY`, `NS_REPLACEABLE`, `NS_EPHEMERAL` or `NS_ADDRESSABLE` (`#authority`) |
| | `ns_same_key(a, b)` | Whether two events share one replaceable address: kind and author, and for an addressable kind the `d` value |
| | `ns_newer(a, b)` | Whether `a` replaces `b`: later `created_at`, or the same second and a lower ID |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag(event, index)` | Reads the index'th tag of a parsed event into a `NostrTag`; checks `e`, `p`, `a` and `d`, counts the values of any other tag (`#authority`) |
| | `nostr_tag_d(event)` | The first `d` tag's value, or the empty identifier when there is none |
| [metadata.coax](../../coax/lib/nostr/metadata.coax) | `nostr_metadata_read(event, buf, overlap)`, `nostr_metadata_parse(content)` | A kind 0 event's content as `name`, `about` and `picture` (`#authority`) |

`tags.coax` loads chapter 21's `event.coax` and `store_policy.coax`, so the
`a` tag reader and the store use the same `ns_class`. The store
(`store_core.coax`) calls `ns_same_key` and `ns_newer` to replace events;
chapter 23 and the relay chapters use it.

The readers allocate nothing. `nostr_tag` returns a `NostrTag` by value: the
tag's type, its value count, and the decoded id, pubkey, relay, marker,
address kind and `d` value in fixed arrays. Values are bounded at 256 bytes
(`NT_MAX_VALUE`).

## Kinds

NIP-01 gives every kind from 0 to 65535 one of four storage rules:

| Class | Kinds (NIP-01) | What a relay keeps |
| --- | --- | --- |
| Regular | `1000 <= n < 10000`, `4 <= n < 45`, `n == 1`, `n == 2` | Every event |
| Replaceable | `10000 <= n < 20000`, `n == 0`, `n == 3` | The latest event per `pubkey` and `kind` |
| Ephemeral | `20000 <= n < 30000` | Nothing; relays pass them on and forget them |
| Addressable | `30000 <= n < 40000` | The latest event per `kind`, `pubkey` and `d` tag value |

`ns_class` tests replaceable first, then ephemeral, then addressable, and
returns `NS_ORDINARY` for everything else:

```jai
ns_class :: (kind:int)->u32 #export_c {
 if kind==0 || kind==3 || (kind>=10000 && kind<20000) {return NS_REPLACEABLE;}
 if kind>=20000 && kind<30000 {return NS_EPHEMERAL;}
 if kind>=30000 && kind<40000 {return NS_ADDRESSABLE;}
 return NS_ORDINARY;
}
```

NIP-01's regular rule doesn't cover 45 to 999 or 40000 to 65535. Those
26,491 kinds are in no class in the NIP. The code puts them in
`NS_ORDINARY`, so a relay built on it stores them like regular events. Other
NIPs assign some of them, for example kind 1059 for gift wraps (chapter 25)
and kind 9735 for zap receipts, and those are regular in NIP-01's own rule.

### Worked example: six kinds by hand

Read each kind against the table, top row first:

| Kind | Test that matches | Class |
| --- | --- | --- |
| 0 | `n == 0` | replaceable (user metadata) |
| 1 | `n == 1` | regular (text note) |
| 3 | `n == 3` | replaceable (follow list) |
| 10002 | `10000 <= 10002 < 20000` | replaceable (relay list, NIP-65) |
| 20001 | `20000 <= 20001 < 30000` | ephemeral |
| 30023 | `30000 <= 30023 < 40000` | addressable (long-form article, NIP-23) |

The boundaries are where an off-by-one would show: 9999 is regular and 10000
replaceable; 19999 replaceable and 20000 ephemeral; 29999 ephemeral and
30000 addressable; 39999 addressable and 40000 back in `NS_ORDINARY`.

## Replacement

A replaceable or addressable event has an address. Two events with the same
address are versions of one thing, and a relay keeps one of them.

- For a replaceable kind the address is `(kind, pubkey)`.
- For an addressable kind it is `(kind, pubkey, d)`, where `d` is the first
  `d` tag's value, or the empty string when there is none.
- Regular and ephemeral events have no address. `ns_same_key` returns false
  for them, even for the same event.

NIP-01 picks the version to keep: the later `created_at`; when both have the
same `created_at`, the event with the lowest ID, in lexical order. Lexical
order of lowercase hex is the order of the 32 ID bytes compared from the
first. `ns_newer` is that rule:

```jai
ns_before :: (a:[32]u8,b:[32]u8)->bool {for i:0..<32 {if a[i]<b[i] {return true;}if a[i]>b[i] {return false;}}return false;}
ns_newer :: (a:NsMeta,b:NsMeta)->bool {return a.created>b.created || (a.created==b.created && ns_before(a.id,b.id));}
```

By hand, for two kind 10002 events from one author:

1. `created_at` 1761686400, ID `5c83...`, against 1761686460, ID `b701...`.
   The second is 60 seconds later, so it wins. The IDs aren't compared.
2. Both at 1761686400, IDs `5c83...` and `5c82...`. Byte 0 is `0x5c` in
   both; byte 1 is `0x83` against `0x82`. The second is lower, so it wins.

Two kind 30023 events with `d` values `abcd` and `efgh` have different
addresses, and a relay keeps both.

## Tags

A tag is a JSON array of strings. The first string is the tag's name, the
second its value. NIP-01 says relays index every single-letter tag name
(`a`–`z`, `A`–`Z`) by its first value, so a filter like `{"#e": [<id>]}`
finds the events that refer to an event. That matching is chapter 23's.

`nostr_tag` reads the name and, for the four standard names, checks up to
five values. Each standard tag has one shape:

| Tag | Values read | Checks |
| --- | --- | --- |
| `e` | id, relay, marker or author, author | id is 64 lowercase hex digits; relay, when not `""`, starts with `ws://` or `wss://` and has a host; the fourth value is `root`, `reply` or `mention`, or else an author pubkey; the fifth, after a marker, is an author pubkey |
| `p` | pubkey, relay | pubkey is 64 lowercase hex digits; relay as for `e` |
| `a` | address, relay | the address is `kind:pubkey:d` (below); relay as for `e` |
| `d` | identifier | none: any string up to 256 bytes |

A standard tag with no value refuses with `NT_SHAPE`. Any other name, such
as `t` or `alt`, is reported as `NT_OTHER` with its value count and is not
checked. Values past the fifth are left in the signed tags for other NIPs.

The `e` tag's fourth position has two meanings in the NIPs. NIP-01 puts the
referenced event's author there. NIP-10 puts a marker there and the author
fifth. The reader tells them apart by the value: one of the three marker
words is a marker, and anything else must be a pubkey. The pinned NIP-10
lists only `root` and `reply`; the reader also accepts `mention`, from
NIP-10's older text.

A tag is a claim by the event's signer. A `p` tag reads as a valid pubkey
whether or not anyone holds its key, and an `e` tag's relay hint says only
where the signer suggests looking.

### The `a` coordinate

The `a` tag's value is an address written as text:

```text
<kind in decimal>:<pubkey as 64 lowercase hex digits>:<d value>
```

For a replaceable kind the `d` part is empty, and the trailing colon stays.

By hand, NIP-01's example address for kind 30023, its example pubkey and
`d` value `abcd`:

1. The kind in decimal: `30023`, 5 bytes.
2. A colon, then the pubkey `f7234bd4...76e9ca`, 64 bytes.
3. A colon, then `abcd`, 4 bytes.

That is 5 + 1 + 64 + 1 + 4 = 75 bytes:
`30023:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:abcd`.
The same pubkey's relay list (kind 10002) is 71 bytes and ends in `:`.

`nt_read_address` reads it back in this order:

```jai
nt_read_address :: (t:NostrTag,s:string)->NostrTag {
  r:=t;i:=0;kind:=0;
  while i<s.count && s[i]>='0' && s[i]<='9' {
    if i==5 {return nt_fail(r,NT_LIMIT);}
    kind=kind*10+cast(int)(s[i]-'0');i+=1;
  }
  if i==0 || i>=s.count || s[i]!=':' || (s[0]=='0' && i>1) {return nt_fail(r,NT_ADDRESS);}
  if s.count<i+66 || s[i+65]!=':' {return nt_fail(r,NT_ADDRESS);}
  if !ne_lower_hex(s[i+1..<i+65],r.pubkey[..]) {return nt_fail(r,NT_HEX);}
  rest:=s[i+66..<s.count];class:=ns_class(kind);
  if class!=NS_ADDRESSABLE && (class!=NS_REPLACEABLE || rest.count!=0) {return nt_fail(r,NT_ADDRESS);}
  r.address_kind=kind;for k:0..<rest.count {r.d[k]=rest[k];}r.d_n=rest.count;return r;
}
```

For the 75-byte address: the digit loop reads `30023` and stops at `i = 5`
on the colon. The string has at least `5 + 66 = 71` bytes and byte 70 is a
colon. Bytes 6 to 69 decode as the pubkey. The rest, bytes 71 to 74, is
`abcd`. `ns_class(30023)` is addressable, so any rest is allowed.

Each check gives one refusal:

| Address | Stops at | Status |
| --- | --- | --- |
| `1:<pk>:` | kind 1 is regular | `NT_ADDRESS` |
| `20001:<pk>:` | kind 20001 is ephemeral | `NT_ADDRESS` |
| `10002:<pk>:x` | replaceable with a non-empty `d` | `NT_ADDRESS` |
| `10002:<pk>` | no colon at byte 70 | `NT_ADDRESS` |
| `030023:<pk>:abcd` | a sixth digit | `NT_LIMIT` |
| `03:<pk>:` | a leading zero | `NT_ADDRESS` |
| `99999:<pk>:abcd` | kind 99999 has no replaceable class | `NT_ADDRESS` |
| 63 hex digits | no colon where the 64th digit ends | `NT_ADDRESS` |
| uppercase `F` in the pubkey | not lowercase hex | `NT_HEX` |

The `d` part may itself hold colons: `30023:<pk>:a:b:` reads `d` as `a:b:`,
because everything after the pubkey's colon is the identifier.

### The `d` tag

`nostr_tag_d` returns the first `d` tag. When there is none, or the tag has
no value, it returns the empty identifier with `NT_OK`, because that is how
relays index an addressable event without one. The tag readers don't
compare tags with the event's kind: a `d` tag on a kind 1 event reads as
`ok`, and it only affects storage when the kind is addressable.

## Kind 0 metadata

Kind 0's content is a JSON object in a string. `nostr_metadata_read` unescapes
the content into the caller's buffer and `nostr_metadata_parse` reads three
fields: `name`, `about` and `picture`. Each must be a string when present.
Other fields, including NIP-24's `display_name`, `website` and `bot`, are
skipped and stay in the signed original. The content must be one object of
at most 65,536 bytes, and a repeated key refuses under chapter 17's
admission policy. Metadata is what a key says about itself; it proves
nothing about who holds the key.

## The example

The example is [nostr/examples/kinds-tags.coax](../examples/kinds-tags.coax).
It classifies the six worked kinds and fourteen boundaries, builds the two
addresses, and reads the tags of a kind 1 reply that carries NIP-01's three
example tags. It then reads the Coax project's task order (kind 30623),
captured from `wss://relay.openagents.com`, and builds the event's own
address from its kind, pubkey and `d` tag. Last, it decides four
replacements and shows fifteen refusals, three accepted edge cases and four
kind 0 readings.

The reply is not signed. Its ID and signature are zeros, and it is only
parsed with chapter 21's `ne_parse`, which is all a tag reader needs: tags
are read the same way from a verified event. The captured event goes
through `nostr_event_read`, which recomputes its ID; its signature isn't
checked here (chapter 21 verifies captured events).

<!-- source: nostr/examples/kinds-tags.coax example -->
```jai
#load "../../coax/lib/nostr/tags.coax";
#load "../../coax/lib/nostr/metadata.coax";
#import "Basic";

CLASS :: [4] string.["regular", "replaceable", "ephemeral", "addressable"];
TAG :: [5] string.["other", "e", "p", "a", "d"];
STATUS :: [8] string.["ok", "event", "index", "shape", "hex", "relay", "address", "limit"];
MARK :: [4] string.["none", "root", "reply", "mention"];
META :: [8] string.["ok", "event", "kind", "json", "shape", "duplicate key", "field", "output"];

// Print a label and bytes as lowercase hex.
print_hex :: (label: string, bytes: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}

// The address "kind:pubkey:d" of an addressable or replaceable event,
// written into out: the kind in decimal, the pubkey as lowercase hex, and
// the d value as it is (empty for a replaceable event).
build_address :: (kind: int, pubkey: [] u8, d: string, out: [] u8) -> string {
  hex: [64] u8;
  r := hex_encode_checked(cast(string) pubkey, hex[..], false);
  n := json_write_int(out, 0, kind);
  n = json_raw(out, n, ":");
  n = json_raw(out, n, cast(string) hex[0..<r.written]);
  n = json_raw(out, n, ":");
  n = json_raw(out, n, d);
  #assert n > 0;
  return cast(string) out[0..<n];
}

// An unsigned event with these tags, parsed but not verified: its id and
// sig are zeros. A tag reader needs a parsed event, not a verified one.
EVENT_HEAD :: "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"created_at\":1761686400,\"kind\":";
EVENT_TAIL :: ",\"content\":\"\",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}";
event_with :: (kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, EVENT_HEAD);
  n = json_write_int(out, n, kind);
  n = json_raw(out, n, ",\"tags\":");
  n = json_raw(out, n, tags);
  n = json_raw(out, n, EVENT_TAIL);
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

// Print what nostr_tag read from one tag.
show_tag :: (e: NostrEvent, index: int) {
  t := nostr_tag(e, index);
  print("tag %: %, %", index, TAG[t.type], STATUS[t.error]);
  if t.error != NT_OK { print("\n"); return; }
  print(", % values\n", t.values);
  if t.type == NT_E {
    print_hex("  id", t.id[..]);
    print("  marker: %\n", MARK[t.marker]);
    if t.has_author { print_hex("  author", t.author[..]); }
  }
  if t.type == NT_P { print_hex("  pubkey", t.pubkey[..]); }
  if t.type == NT_A {
    print("  kind: % (%)\n", t.address_kind, CLASS[ns_class(t.address_kind)]);
    print_hex("  pubkey", t.pubkey[..]);
  }
  if t.type == NT_A || t.type == NT_D { print("  d: \"%\"\n", cast(string) t.d[0..<t.d_n]); }
  if t.has_relay { print("  relay: %\n", cast(string) t.relay[0..<t.relay_n]); }
}

// One tag, read as tag 0 of an unsigned kind 1 event.
read_one :: (tag: string) -> NostrTag {
  buf: [1024] u8; tags: [512] u8;
  n := json_raw(tags[..], 0, "[");
  n = json_raw(tags[..], n, tag);
  n = json_raw(tags[..], n, "]");
  #assert n > 0;
  return nostr_tag(event_with(1, cast(string) tags[0..<n], buf[..]), 0);
}
// An a tag with this value, read the same way.
read_a :: (value: string) -> NostrTag {
  tag: [512] u8;
  n := json_raw(tag[..], 0, "[\"a\",");
  n = json_write_string(tag[..], n, value);
  n = json_raw(tag[..], n, "]");
  #assert n > 0;
  return read_one(cast(string) tag[0..<n]);
}
one_tag :: (label: string, tag: string) { print("%: %\n", label, STATUS[read_one(tag).error]); }
a_value :: (label: string, value: string) { print("%: %\n", label, STATUS[read_a(value).error]); }

// Which of two versions a relay keeps, by NIP-01's rule: one address (kind,
// pubkey and, for an addressable kind, the d value) keeps one version, the
// later created_at, and on a tie the lower id.
keep :: (label: string, a: NsMeta, b: NsMeta) {
  if ns_class(a.kind) == NS_EPHEMERAL { print("%: ephemeral, store neither\n", label); return; }
  if !ns_same_key(a, b) { print("%: not one address, keep both\n", label); return; }
  w := ifx ns_newer(a, b) then a else b;
  print("%: one address, keep the %, created_at %, ", label,
        ifx ns_newer(a, b) then "first" else "second", w.created);
  print_hex("id begins", w.id[0..<2]);
}

PK_HEX :: "f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca";
// A kind 1 reply: NIP-01's three example tags, with the e tag given a NIP-10
// root marker and the author's pubkey, and a t tag that is no standard one.
REPLY_TAGS :: "[[\"e\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\",\"wss://nostr.example.com\",\"root\",\"f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca\"],[\"p\",\"f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca\"],[\"a\",\"30023:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:abcd\",\"wss://nostr.example.com\"],[\"t\",\"nostr\"]]";

main :: () {
  print("kind ranges\n");
  kinds := [6] int.[0, 1, 3, 10002, 20001, 30023];
  for k: kinds { print("%: %\n", k, CLASS[ns_class(k)]); }
  print("boundaries\n");
  edges := [14] int.[2, 44, 45, 999, 1000, 9999, 10000, 19999, 20000, 29999, 30000, 39999, 40000, 65535];
  for k: edges { print("%: %\n", k, CLASS[ns_class(k)]); }

  print("an address\n");
  pk: [32] u8; decode_hex(PK_HEX, pk[..]);
  text: [128] u8;
  print("article: %\n", build_address(30023, pk[..], "abcd", text[..]));
  print("relay list: %\n", build_address(10002, pk[..], "", text[..]));

  print("a reply's tags\n");
  buf: [2048] u8;
  e := event_with(1, REPLY_TAGS, buf[..]);
  print("parsed: %, kind %, tags %\n", ifx e.error == NE_OK then "ok" else "refused", e.kind, e.tags);
  for i: 0..<e.tags { show_tag(e, i); }
  show_tag(e, e.tags);
  d := nostr_tag_d(e);
  print("first d: %, \"%\"\n", STATUS[d.error], cast(string) d.d[0..<d.d_n]);

  print("a captured addressable event\n");
  scratch: [16384] u8;
  p := nostr_event_read(NT_PROJECT_EVENT, scratch[..], false);
  print("read: %, kind % (%), tags %\n", ifx p.error == NE_OK then "ok, ID recomputed" else "refused",
        p.kind, CLASS[ns_class(p.kind)], p.tags);
  d = nostr_tag_d(p);
  print("first d: %, \"%\"\n", STATUS[d.error], cast(string) d.d[0..<d.d_n]);
  show_tag(p, 1);
  show_tag(p, 2);
  print("address: %\n", build_address(p.kind, p.public_key[..], cast(string) d.d[0..<d.d_n], text[..]));

  print("which version is kept\n");
  older := NsMeta.{kind = 10002, created = 1761686400};
  older.id[0] = 0x5c; older.id[1] = 0x83;
  newer := older; newer.created = 1761686460; newer.id[0] = 0xb7; newer.id[1] = 0x01;
  keep("10002, 60 s apart", older, newer);
  tie := older; tie.id[1] = 0x82;
  keep("10002, same second", older, tie);
  draft := NsMeta.{kind = 30023, created = 1761686400, identifier = "abcd"};
  edit := draft; edit.created = 1761686460; edit.id[0] = 0x01;
  keep("30023, d abcd twice", draft, edit);
  other := draft; other.identifier = "efgh"; other.created = 1761686460;
  keep("30023, d abcd and d efgh", draft, other);
  note := NsMeta.{kind = 1, created = 1761686400};
  keep("1, two notes", note, note);
  keep("20001, two signals", NsMeta.{kind = 20001}, NsMeta.{kind = 20001});

  print("refusals\n");
  a_value("a, kind 1", "1:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:");
  a_value("a, kind 20001", "20001:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:");
  a_value("a, kind 10002 with d x", "10002:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:x");
  a_value("a, no trailing colon", "10002:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca");
  a_value("a, kind 030023", "030023:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:abcd");
  a_value("a, kind 03", "03:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:");
  a_value("a, kind 99999", "99999:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:abcd");
  a_value("a, pubkey 63 digits", "30023:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9c:abcd");
  a_value("a, pubkey uppercase F", "30023:F7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:abcd");
  one_tag("e, no id", "[\"e\"]");
  one_tag("e, id 4 digits", "[\"e\",\"5c83\"]");
  one_tag("e, relay https://x", "[\"e\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\",\"https://x\"]");
  one_tag("e, marker quote", "[\"e\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\",\"\",\"quote\"]");
  one_tag("p, empty pubkey", "[\"p\",\"\"]");
  one_tag("tag value 1", "[\"p\",1]");
  print("accepted\n");
  one_tag("d on a kind 1 event", "[\"d\",\"abcd\"]");
  a_value("a, kind 30023 with d a:b:", "30023:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:a:b:");
  one_tag("e, relay \"\" and marker reply", "[\"e\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\",\"\",\"reply\"]");

  print("kind 0 metadata\n");
  m := nostr_metadata_parse("{\"name\":\"Ada\",\"about\":\"writes proofs\",\"website\":\"https://example.com\"}");
  print("parse: %, name \"%\", about \"%\", picture %\n", META[m.error],
        nmd_text(m, NMD_NAME), nmd_text(m, NMD_ABOUT), ifx m.has[NMD_PICTURE] then "present" else "absent");
  print("name twice: %\n", META[nostr_metadata_parse("{\"name\":\"a\",\"name\":\"b\"}").error]);
  print("name 1: %\n", META[nostr_metadata_parse("{\"name\":1}").error]);
  print("an array: %\n", META[nostr_metadata_parse("[]").error]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/kinds-tags.coax
```

The program prints:

```text
kind ranges
0: replaceable
1: regular
3: replaceable
10002: replaceable
20001: ephemeral
30023: addressable
boundaries
2: regular
44: regular
45: regular
999: regular
1000: regular
9999: regular
10000: replaceable
19999: replaceable
20000: ephemeral
29999: ephemeral
30000: addressable
39999: addressable
40000: regular
65535: regular
an address
article: 30023:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:abcd
relay list: 10002:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:
a reply's tags
parsed: ok, kind 1, tags 4
tag 0: e, ok, 5 values
  id: 5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36
  marker: root
  author: f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca
  relay: wss://nostr.example.com
tag 1: p, ok, 2 values
  pubkey: f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca
tag 2: a, ok, 3 values
  kind: 30023 (addressable)
  pubkey: f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca
  d: "abcd"
  relay: wss://nostr.example.com
tag 3: other, ok, 2 values
tag 4: other, index
first d: ok, ""
a captured addressable event
read: ok, ID recomputed, kind 30623 (addressable), tags 108
first d: ok, "coax"
tag 1: a, ok, 3 values
  kind: 30621 (addressable)
  pubkey: 9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575
  d: "coax"
  relay: wss://relay.openagents.com
tag 2: other, ok, 3 values
address: 30623:9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575:coax
which version is kept
10002, 60 s apart: one address, keep the second, created_at 1761686460, id begins: b701
10002, same second: one address, keep the second, created_at 1761686400, id begins: 5c82
30023, d abcd twice: one address, keep the second, created_at 1761686460, id begins: 0100
30023, d abcd and d efgh: not one address, keep both
1, two notes: not one address, keep both
20001, two signals: ephemeral, store neither
refusals
a, kind 1: address
a, kind 20001: address
a, kind 10002 with d x: address
a, no trailing colon: address
a, kind 030023: limit
a, kind 03: address
a, kind 99999: address
a, pubkey 63 digits: address
a, pubkey uppercase F: hex
e, no id: shape
e, id 4 digits: hex
e, relay https://x: relay
e, marker quote: hex
p, empty pubkey: hex
tag value 1: event
accepted
d on a kind 1 event: ok
a, kind 30023 with d a:b:: ok
e, relay "" and marker reply: ok
kind 0 metadata
parse: ok, name "Ada", about "writes proofs", picture absent
name twice: duplicate key
name 1: field
an array: shape
```

What the output shows:

- The six kinds and the boundaries have the classes worked out above. Kinds
  45, 999 and 65535, which NIP-01 puts in no class, print `regular`.
- The built addresses are the 75 and 71 bytes worked out above.
- The reply's `e` tag reads with its relay, the `root` marker and the
  author. `t` reads as `other` with two values. Tag 4 is past the end
  (`NT_INDEX`), and the reply has no `d` tag, so its identifier is empty.
- The captured event's ID recomputes. Its first `d` tag is `coax`, its `a`
  tag points at the project's kind 30621 event by the same author, and its
  third tag, an `item` with three values, is no standard one. The address
  built from its own fields is the one other events use to refer to it.
- The replacements follow the two hand cases. A later edit of the same
  article (`d abcd twice`) replaces it whatever its ID. Different `d`
  values, and regular notes, are kept side by side. Ephemeral events are
  stored by neither rule.
- Each refusal has the status in the table above. `tag value 1` is
  `NT_EVENT`: chapter 21's reader refused the event (`NE_SHAPE`) before any
  tag was read, and `nostr_tag` reads only a parsed event.
- A `d` tag on kind 1, a `d` value with colons, and an empty relay before a
  `reply` marker are accepted.
- The kind 0 content reads `name` and `about`; `website` is skipped and
  `picture` is absent. A repeated `name`, a number as `name`, and an array
  instead of an object refuse.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/kinds-tags.out](../fixtures/kinds-tags.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/kinds-tags.coax](../examples/checks/kinds-tags.coax),
which loads the example. They compare the code with values from outside it:

- the class of every kind from 0 to 65535, counted, against counts from a
  Python script written from NIP-01's four range sentences, and every kind
  NIP-01 calls regular against `NS_ORDINARY`;
- the six worked kinds and the eight boundaries from the plan (9999, 10000,
  19999, 20000, 29999, 30000, 39999, 40000), by hand;
- the built address against NIP-01's published example `a` value, and NIP-01's
  example tags against what the NIP says they hold;
- the captured event's address against its fields;
- the two hand replacements and the address rules;
- one malformed tag for each refusal status.

<!-- source: nostr/examples/checks/kinds-tags.coax check -->
```jai
// Class counts over every kind 0..65535, from a Python script written from
// NIP-01's four kind-range sentences (it shares no code with Coax):
// regular 9043 (1, 2, 4..44, 1000..9999), replaceable 10002, ephemeral
// 10000, addressable 10000, and 26491 kinds NIP-01 puts in no class
// (45..999 and 40000..65535). ns_class stores those as regular.
PY_REGULAR :: 9043; PY_UNCLASSED :: 26491;
PY_REPLACEABLE :: 10002; PY_EPHEMERAL :: 10000; PY_ADDRESSABLE :: 10000;

// NIP-01's regular rule, as its text states it.
nip01_regular :: (n: int) -> bool { return (1000 <= n && n < 10000) || (4 <= n && n < 45) || n == 1 || n == 2; }

#test "every kind's class matches the counts from NIP-01's text" {
  count: [4] int; regular := 0;
  for k: 0..<65536 {
    c := ns_class(k); count[c] += 1;
    if nip01_regular(k) { regular += 1; #assert c == NS_ORDINARY; }
  }
  #assert regular == PY_REGULAR && count[NS_ORDINARY] == PY_REGULAR + PY_UNCLASSED;
  #assert count[NS_REPLACEABLE] == PY_REPLACEABLE && count[NS_EPHEMERAL] == PY_EPHEMERAL;
  #assert count[NS_ADDRESSABLE] == PY_ADDRESSABLE;
}
#test "the worked kinds and every range boundary, by hand" {
  #assert ns_class(0) == NS_REPLACEABLE && ns_class(1) == NS_ORDINARY && ns_class(3) == NS_REPLACEABLE;
  #assert ns_class(10002) == NS_REPLACEABLE && ns_class(20001) == NS_EPHEMERAL && ns_class(30023) == NS_ADDRESSABLE;
  #assert ns_class(9999) == NS_ORDINARY && ns_class(10000) == NS_REPLACEABLE;
  #assert ns_class(19999) == NS_REPLACEABLE && ns_class(20000) == NS_EPHEMERAL;
  #assert ns_class(29999) == NS_EPHEMERAL && ns_class(30000) == NS_ADDRESSABLE;
  #assert ns_class(39999) == NS_ADDRESSABLE && ns_class(40000) == NS_ORDINARY;
}
#test "the built address is NIP-01's example a tag value" {
  pk: [32] u8; decode_hex(PK_HEX, pk[..]); text: [128] u8;
  #assert build_address(30023, pk[..], "abcd", text[..]) == "30023:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:abcd";
  #assert build_address(10002, pk[..], "", text[..]) == "10002:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:";
}
#test "NIP-01's example tags read as the NIP describes them" {
  buf: [2048] u8; e := event_with(1, REPLY_TAGS, buf[..]);
  #assert e.error == NE_OK && e.tags == 4;
  t := nostr_tag(e, 0); id: [32] u8; pk: [32] u8;
  decode_hex("5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36", id[..]); decode_hex(PK_HEX, pk[..]);
  #assert t.error == NT_OK && t.type == NT_E && ns_equal32(t.id, id) && t.marker == NT_MARK_ROOT && t.has_author && ns_equal32(t.author, pk);
  #assert t.has_relay && cast(string) t.relay[0..<t.relay_n] == "wss://nostr.example.com";
  t = nostr_tag(e, 1); #assert t.error == NT_OK && t.type == NT_P && ns_equal32(t.pubkey, pk) && !t.has_relay;
  t = nostr_tag(e, 2);
  #assert t.error == NT_OK && t.type == NT_A && t.address_kind == 30023 && ns_equal32(t.pubkey, pk) && cast(string) t.d[0..<t.d_n] == "abcd";
  t = nostr_tag(e, 3); #assert t.error == NT_OK && t.type == NT_OTHER && t.values == 2;
  #assert nostr_tag(e, 4).error == NT_INDEX && nostr_tag_d(e).d_n == 0;
}
#test "the captured event's address is its own kind, pubkey and d" {
  scratch: [16384] u8; p := nostr_event_read(NT_PROJECT_EVENT, scratch[..], false);
  #assert p.error == NE_OK && p.kind == 30623 && p.tags == 108;
  d := nostr_tag_d(p); text: [128] u8;
  #assert build_address(p.kind, p.public_key[..], cast(string) d.d[0..<d.d_n], text[..]) == "30623:9ef92e341b43ebc5617a75cf7eca7e2e30fae41e08a0614a344d851d98731575:coax";
  a := nostr_tag(p, 1); #assert a.address_kind == 30621 && ns_equal32(a.pubkey, p.public_key);
}
#test "replacement: later created_at, then the lower id; addresses by kind, author, d" {
  a := NsMeta.{kind = 10002, created = 1761686400}; a.id[0] = 0x5c; a.id[1] = 0x83;
  b := a; b.created = 1761686460; b.id[0] = 0xb7;
  #assert ns_same_key(a, b) && ns_newer(b, a) && !ns_newer(a, b);
  b = a; b.id[1] = 0x82; #assert ns_newer(b, a) && !ns_newer(a, b);
  b = a; b.author[0] = 1; #assert !ns_same_key(a, b);
  x := NsMeta.{kind = 30023, identifier = "abcd"}; y := x; #assert ns_same_key(x, y);
  y.identifier = "efgh"; #assert !ns_same_key(x, y);
  n := NsMeta.{kind = 1}; #assert !ns_same_key(n, n);
}
#test "each broken tag refuses with its own status" {
  #assert read_a("1:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:").error == NT_ADDRESS;
  #assert read_a("20001:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:").error == NT_ADDRESS;
  #assert read_a("10002:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:x").error == NT_ADDRESS;
  #assert read_a("10002:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca").error == NT_ADDRESS;
  #assert read_a("030023:f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:abcd").error == NT_LIMIT;
  #assert read_a("30023:F7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca:abcd").error == NT_HEX;
  #assert read_one("[\"e\"]").error == NT_SHAPE && read_one("[\"p\",\"\"]").error == NT_HEX;
  #assert read_one("[\"e\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\",\"https://x\"]").error == NT_RELAY;
  #assert read_one("[\"p\",1]").error == NT_EVENT && read_one("[\"d\",\"abcd\"]").error == NT_OK;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/kinds-tags.coax
coax/bin/coax mutate nostr/examples/checks/kinds-tags.coax --root-only
```

On 2026-11-01, `coax test` ran 209 checks, including the loaded files' own
checks. Root-only mutation of the checks file killed 17 of 17 mutants. The
example's procedures and the library were not mutated for this chapter.

### The library's own checks

[tags.coax](../../coax/lib/nostr/tags.coax) checks the captured event, every
`e` shape with and without relay, marker and author, `p` relays, 23 `a`
addresses at each refusal and bound, the first `d` tag, values at 256 and
257 bytes, forged event spans, and every byte value after `ws://`.
[store_policy.coax](../../coax/lib/nostr/store_policy.coax) checks the
range boundaries, the address identity and the newest-then-lowest-ID rule.
[metadata.coax](../../coax/lib/nostr/metadata.coax) checks optional and
unknown fields, the content bound at 65,536 bytes, duplicate keys and
non-string fields.

The [2026-10-30 record](../../docs/verification/2026-10-30-nostr-tags-metadata.md)
reports 198 of 198 checks for `tags.coax` and 189 of 189 for
`metadata.coax`, and zero mutation survivors: 3,970 of 3,970 mutants killed
in `tags.coax` and 3,680 of 3,680 in `metadata.coax`, none marked
equivalent. Those runs were not repeated for this chapter. The record also
says what is not done: native, Wasm and iOS runs of these readers, and
moving the project pages onto them.

## Summary

- Every kind is regular, replaceable (0, 3, 10000–19999), ephemeral
  (20000–29999) or addressable (30000–39999). The kinds NIP-01 leaves out,
  45–999 and 40000–65535, are stored as regular.
- A replaceable event's address is its kind and pubkey; an addressable
  event's adds the first `d` value. Of two versions, the later `created_at`
  wins, and on a tie the lower ID.
- `nostr_tag` checks `e`, `p`, `a` and `d` and counts the values of other
  tags. Hex must be lowercase, relays must be `ws://` or `wss://`, and an
  `a` address must name a replaceable or addressable kind.
- An `a` value is `kind:pubkey:d`; a replaceable kind has an empty `d` and
  keeps the trailing colon.
- A tag is what the signer wrote. Reading it proves nothing about the event,
  key or relay it names.

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)

## Sources

- [NIP-01](../nips/official/01.md), the kinds, the ranges, the replacement
  rule and the standard tags.
- [NIP-10](../nips/official/10.md), the `e` tag's markers.
- [NIP-16](../nips/official/16.md) and [NIP-33](../nips/official/33.md),
  where the ranges were first defined.
- [NIP-24](../nips/official/24.md), the extra kind 0 fields.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
