# 23. Filters

A subscription asks for events with filters. A relay and a client must agree
exactly on what matches. This chapter builds filter parsing and matching,
including the single-letter tag queries that NIP-12 introduced.

Part VI, The event model. Result: NIP-01 filters: `ids`, `authors`, `kinds`,
`#<letter>` tags, `since`, `until` and `limit`; AND across fields, OR within a
list, inclusive times, and absent lists distinct from empty ones.

## What this chapter teaches

- Fields and their meaning; `limit` applies only to the initial query.
- AND across fields, OR within a list, OR across filters in one `REQ`.
- Tag filters match any value of a single-letter tag; NIP-01 says only
  single-letter tags are indexed.
- An absent list matches everything; an explicitly empty list matches nothing.
- `since <= created_at <= until`.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [filter.coax](../../coax/lib/nostr/filter.coax) | `nf_valid(f)` | Whether a `NostrFilter` is inside the bounded profile: lowercase hex, kinds 0–65535, at most 64 entries per list, one ASCII letter per tag name with no repeats, tag values at most 256 UTF-8 bytes, times and limit within 2^53 (`#authority`) |
| | `nf_match(f, event)` | Whether a parsed event passes every condition of one valid filter |
| | `nf_write(f, out, overlap)` | The filter as JSON, in the field order `ids`, `authors`, `kinds`, tags, `since`, `until`, `limit` |
| [filter_read.coax](../../coax/lib/nostr/filter_read.coax) | `filter_read(text, f, space)` | One filter object's JSON into a `NostrFilter` whose lists point into caller storage (`FrSpace`); returns `FR_OK` or a refusal (`#authority`) |

`filter_read.coax` loads `filter.coax`, which loads chapter 21's
`event.coax`. A filter has no heap: `filter_read` decodes each string into
the caller's `bytes` and appends views to `strs`, `ints` and `tags`, and it
refuses with `FR_SPACE` when they are full. The relay (chapters 33 to 36)
reads every `REQ` filter with `filter_read` and matches stored and live
events with `nf_match`.

`nf_match` decides one filter and one event. It does not order events or
apply `limit`: that is the initial query's job, which the relay's store does
(chapter 35). The example below writes a small initial query with chapter
22's `ns_newer`.

## The filter object

NIP-01 defines seven attributes:

| Key | Value | An event passes when |
| --- | --- | --- |
| `ids` | list of 64-digit lowercase hex event IDs | its `id` is in the list |
| `authors` | list of 64-digit lowercase hex pubkeys | its `pubkey` is in the list |
| `kinds` | list of kind numbers | its `kind` is in the list |
| `#<letter>` | list of strings | one of its tags named `<letter>` has a first value in the list |
| `since` | integer | `since <= created_at` |
| `until` | integer | `created_at <= until` |
| `limit` | integer `>= 0` | (no per-event condition) |

`NostrFilter` keeps each list with a `has_` flag beside it, because NIP-01
gives an absent list and an empty list different meanings:

- An absent list sets no condition. The filter `{}` matches every event.
- An empty list can't be satisfied: "the filter MUST NOT match any events.
  An empty list MUST NOT be treated as if the attribute were absent."

A tag filter has no flag. It exists in `f.tags` only when the JSON had its
key, so a `#t` with no values is an empty list and matches nothing.

`nf_match` is the conditions in table order. Every condition that is present
must hold (AND across fields); inside a list, one equal value is enough (OR
within a list):

```jai
nf_match :: (f:NostrFilter,e:NostrEvent)->bool {
  if !nf_valid(f) || e.error!=NE_OK || e.created_at<0 || e.created_at>NF_SAFE || e.kind<0 || e.kind>65535 {return false;}
  id:=e.id; key:=e.public_key;
  if f.has_ids && !nf_hex_match(f.ids,id[..]) {return false;}
  if f.has_authors && !nf_hex_match(f.authors,key[..]) {return false;}
  if f.has_kinds {found:=false;for k:f.kinds {if k==e.kind {found=true;}}if !found {return false;}}
  if f.has_since && e.created_at<f.since {return false;}
  if f.has_until && e.created_at>f.until {return false;}
  for tag:f.tags {
    if !nf_tag_span_valid(e.original.count,e.tags_from,e.tags_to) {return false;}
    if !nf_tag_match(tag,e.original[e.tags_from..<e.tags_to]) {return false;}
  }
  // limit applies to the initial query's result count, not each event.
  return true;
}
```

An empty `ids` list fails the first test, because `nf_hex_match` finds no
equal entry in a list with no entries. `nf_hex_match` writes the event's 32
ID bytes as 64 lowercase hex digits and compares strings, so a filter value
in uppercase never matches; `nf_valid` refuses it first.

`nf_match` also returns false for an event record that chapter 21's reader
didn't accept (`e.error != NE_OK`) and for a filter that `nf_valid` refuses.
A match says nothing about the signature. A relay matches only events it has
verified; a client must verify what it receives (chapter 21).

## Tag filters

NIP-01 says relays index every single-letter tag, `a`–`z` and `A`–`Z`, and
that "only the first value in any given tag is indexed". A key `#e` asks for
events with an `e` tag whose first value is in the list. `nf_tag_match`
reads the event's tags array from the signed original and, for each tag
whose name is the letter, compares the second string (index 1) with each
filter value after decoding JSON escapes:

- `["t","nostr","later"]` matches `{"#t":["nostr"]}` and not
  `{"#t":["later"]}`.
- Tag names are case-sensitive: `#E` asks for `E` tags, not `e` tags.
- `#e` and `#p` values must be 64-digit lowercase hex, as `ids` and
  `authors` must; other letters take any string up to 256 bytes.
- Two tag filters, say `#e` and `#p`, must both hold (AND), like any two
  fields.

`nf_tag_match` scans the whole tags array and refuses one that isn't an
array of arrays of strings, has more than 2,000 tags, or has a tag with more
than 16 values. A malformed tail can't make a match.

## Time and limit

`since` and `until` are inclusive: an event passes when
`since <= created_at <= until`. A filter with `since` greater than `until`
is valid, and it matches nothing. `filter_read` doesn't refuse it, because
NIP-01 doesn't.

`limit` is not a condition on an event. NIP-01 says it applies to the
initial query only: the relay returns at most `limit` stored events, "the
last `n` events ordered by the `created_at`. Newer events should appear
first, and in the case of ties the event with the lowest id (first in
lexical order) should be first." `limit: 0` asks for no stored events, only
live ones. After the stored events the relay sends `EOSE`, and from then on
it sends each new matching event with no limit.

The order is chapter 22's replacement order: a later `created_at`, then on
a tie the lower ID. The example's `query` uses `ns_newer` to sort.

## One REQ, several filters

A `REQ` carries a subscription id and one or more filters:
`["REQ", <subscription_id>, <filter1>, <filter2>, ...]`. An event is wanted
when it matches any of them (OR across filters). Each filter's `limit`
applies to that filter's own initial query. Chapter 24 reads and writes the
`REQ` frame; chapter 30 runs subscriptions to `EOSE` and beyond.

## Worked example: four events and one REQ

Four events, with authors `A` (`79be...1798`, the pubkey of the secret key
1) and `B` (`f723...e9ca`, NIP-01's example pubkey), and `R` the ID of
NIP-01's example event (`5c83da77...6f36`):

| Event | id | pubkey | kind | created_at | tags |
| --- | --- | --- | --- | --- | --- |
| E1 | `1111...` | A | 1 | 1761686400 | `["e",R]` |
| E2 | `2222...` | B | 1 | 1761686460 | `["e",R]`, `["p",A]` |
| E3 | `3333...` | A | 7 | 1761686520 | `["e",R]`, `["t","nostr","later"]` |
| E4 | `2020...` | B | 1 | 1761686460 | `["e",R,"wss://nostr.example.com"]` |

The `REQ` has two filters:

```text
F1 = {"authors":[A,B],"kinds":[1],"#e":[R],"since":1761686400,"until":1761686460,"limit":2}
F2 = {"ids":["3333..."],"#t":["nostr"]}
```

Decide F1 for each event, field by field. A field fails when no list value
equals the event's value:

| F1 condition | E1 | E2 | E3 | E4 |
| --- | --- | --- | --- | --- |
| `authors` has A or B | A: yes | B: yes | A: yes | B: yes |
| `kinds` has 1 | yes | yes | 7: **no** | yes |
| an `e` tag's first value is R | yes | yes | yes | yes (the relay is the second value) |
| `1761686400 <= created_at` | equal: yes | yes | yes | yes |
| `created_at <= 1761686460` | yes | equal: yes | 1761686520: **no** | equal: yes |
| matches F1 | yes | yes | no | yes |

E1 passes `since` and E2 and E4 pass `until` only because both bounds are
inclusive. E3 fails two conditions; one is enough.

Now the initial query for F1. Sort E1, E2 and E4 newest first:

1. E2 and E4 have the same `created_at`, 1761686460. Compare their IDs:
   byte 0 is `0x22` in E2 and `0x20` in E4, so E4 is lower and comes first.
2. E1, at 1761686400, is older than both and comes last.

The order is E4, E2, E1, and `limit: 2` cuts E1. F1 sends E4, E2.

F2 has two conditions. Only E3 has ID `3333...`, and E3's `t` tag has first
value `nostr`, so F2 sends E3. Under `{"#t":["later"]}` instead, E3 would
fail: `later` is its tag's second value.

The REQ's initial answer is F1's events and then F2's: E4, E2, E3. Then the
relay sends `EOSE`. An event that arrives later is sent when it matches F1 or
F2, and `limit` no longer applies, so a new event like E1 would be sent.

## The example

The example is [nostr/examples/filters.coax](../examples/filters.coax). It
builds the four events of the worked example, reads F1 with `filter_read`,
writes it back with `nf_write`, decides each event with `nf_match`, runs the
initial queries for F1 (with and without its limit) and F2, and then the
whole REQ. It shows eight edge cases and nineteen readings that end in
`filter_read`'s status, eighteen of them refusals.

The events are not signed. Their IDs are chosen, not hashed, and they are
only parsed with chapter 21's `ne_parse`: `nf_match` reads the parsed
fields, and verifying signatures is a separate step. `query` is a small
initial query: it keeps the events `nf_match` accepts, inserts each in
`ns_newer` order, and cuts the result to `limit`.

<!-- source: nostr/examples/filters.coax example -->
```jai
#load "../../coax/lib/nostr/filter_read.coax";
#load "../../coax/lib/nostr/store_policy.coax";
#import "Basic";

READ :: [6] string.["ok", "json", "field", "type", "space", "invalid"];
NAME :: [4] string.["E1", "E2", "E3", "E4"];

// NIP-01's example event id, and two authors.
R :: "5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36";
A :: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
B :: "f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca";
ID1 :: "1111111111111111111111111111111111111111111111111111111111111111";
ID2 :: "2222222222222222222222222222222222222222222222222222222222222222";
ID3 :: "3333333333333333333333333333333333333333333333333333333333333333";
ID4 :: "2020202020202020202020202020202020202020202020202020202020202020";
SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

// The event's JSON, written into out, then parsed (not verified: matching
// reads a parsed event's fields, and the IDs here are chosen, not hashed).
make_event :: (out: [] u8, id: string, pubkey: string, created: int, kind: int, tags: string) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"");
  n = json_raw(out, n, id);
  n = json_raw(out, n, "\",\"pubkey\":\"");
  n = json_raw(out, n, pubkey);
  n = json_raw(out, n, "\",\"created_at\":");
  n = json_write_int(out, n, created);
  n = json_raw(out, n, ",\"kind\":");
  n = json_write_int(out, n, kind);
  n = json_raw(out, n, ",\"tags\":");
  n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":\"\",\"sig\":\"");
  n = json_raw(out, n, SIG);
  n = json_raw(out, n, "\"}");
  #assert n > 0;
  e := ne_parse(cast(string) out[0..<n]);
  #assert e.error == NE_OK;
  return e;
}

// The fields NIP-01 orders by: created_at, then the id.
meta :: (e: NostrEvent) -> NsMeta {
  m: NsMeta; m.id = e.id; m.author = e.public_key; m.kind = e.kind; m.created = e.created_at;
  return m;
}

// A filter's initial query over a small event set: the indexes of the
// matching events, newest first and the lowest id first on a tie, cut to
// limit when the filter has one. Returns the count written to out.
query :: (f: NostrFilter, events: [] NostrEvent, out: [] int) -> int {
  n := 0;
  for i: 0..<events.count {
    if !nf_match(f, events[i]) { continue; }
    // Insert i after every kept event that comes before it.
    at := n;
    while at > 0 && ns_newer(meta(events[i]), meta(events[out[at - 1]])) { out[at] = out[at - 1]; at -= 1; }
    out[at] = i; n += 1;
  }
  if f.has_limit && f.limit < n { n = f.limit; }
  return n;
}

// Storage that filter_read puts one filter's lists in.
Space :: struct { strs: [80] string; ints: [80] int; tags: [8] NostrTagFilter; bytes: [8192] u8; }
read :: (text: string, f: [] NostrFilter, s: [] Space) -> int {
  sp: [1] FrSpace;
  sp[0] = FrSpace.{strs = s[0].strs[..], ints = s[0].ints[..], tags = s[0].tags[..], bytes = s[0].bytes[..]};
  return filter_read(text, f, sp[..]);
}

print_list :: (label: string, idx: [] int) {
  print("%:", label);
  if idx.count == 0 { print(" none"); }
  for i: idx { print(" %", NAME[i]); }
  print("\n");
}

// Read a filter and print its initial query, or its refusal.
run :: (label: string, text: string, events: [] NostrEvent) {
  s: [1] Space; f: [1] NostrFilter;
  r := read(text, f[..], s[..]);
  if r != FR_OK { print("%: refused, %\n", label, READ[r]); return; }
  idx: [4] int; n := query(f[0], events[..], idx[..]);
  print_list(label, idx[0..<n]);
}
refuse :: (label: string, text: string) {
  s: [1] Space; f: [1] NostrFilter;
  print("%: %\n", label, READ[read(text, f[..], s[..])]);
}

// A JSON filter {"<key>":[v,v,...]} with count copies of v.
repeat :: (out: [] u8, key: string, v: string, count: int) -> string {
  n := json_raw(out, 0, "{\"");
  n = json_raw(out, n, key);
  n = json_raw(out, n, "\":[");
  for i: 0..<count { if i > 0 { n = json_raw(out, n, ","); } n = json_raw(out, n, v); }
  n = json_raw(out, n, "]}");
  #assert n > 0;
  return cast(string) out[0..<n];
}

TAGS1 :: "[[\"e\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\"]]";
TAGS2 :: "[[\"e\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\"],[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\"]]";
TAGS3 :: "[[\"e\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\"],[\"t\",\"nostr\",\"later\"]]";
TAGS4 :: "[[\"e\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\",\"wss://nostr.example.com\"]]";

// The two filters of the worked REQ.
F1 :: "{\"authors\":[\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca\"],\"kinds\":[1],\"#e\":[\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\"],\"since\":1761686400,\"until\":1761686460,\"limit\":2}";
F2 :: "{\"ids\":[\"3333333333333333333333333333333333333333333333333333333333333333\"],\"#t\":[\"nostr\"]}";

main :: () {
  b1: [1024] u8; b2: [1024] u8; b3: [1024] u8; b4: [1024] u8;
  events: [4] NostrEvent;
  events[0] = make_event(b1[..], ID1, A, 1761686400, 1, TAGS1);
  events[1] = make_event(b2[..], ID2, B, 1761686460, 1, TAGS2);
  events[2] = make_event(b3[..], ID3, A, 1761686520, 7, TAGS3);
  events[3] = make_event(b4[..], ID4, B, 1761686460, 1, TAGS4);
  print("events\n");
  for i: 0..<4 {
    e := events[i];
    print("%: id %..., kind %, created_at %, % tags\n", NAME[i], cast(string) e.original[7..<11], e.kind, e.created_at, e.tags);
  }

  print("filter F1\n");
  s: [1] Space; f: [1] NostrFilter;
  r := read(F1, f[..], s[..]);
  out: [1024] u8; w := nf_write(f[0], out[..], false);
  print("read: %, written back %\n", READ[r], ifx w == F1.count && cast(string) out[0..<w] == F1 then "byte for byte" else "differently");
  print("authors %, kinds %, tag filters %, limit %\n", f[0].authors.count, f[0].kinds.count, f[0].tags.count, f[0].limit);
  for i: 0..<4 { print("% matches F1: %\n", NAME[i], nf_match(f[0], events[i])); }
  run("F1 initial query", F1, events[..]);
  run("F1 without limit", "{\"authors\":[\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca\"],\"kinds\":[1],\"#e\":[\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\"],\"since\":1761686400,\"until\":1761686460}", events[..]);

  print("filter F2\n");
  run("F2 initial query", F2, events[..]);

  print("one REQ with F1 and F2\n");
  s2: [1] Space; g: [1] NostrFilter;
  #assert read(F2, g[..], s2[..]) == FR_OK;
  sent: [8] int; n := query(f[0], events[..], sent[..]);
  n += query(g[0], events[..], sent[n..<8]);
  print_list("initial", sent[0..<n]);
  print("then EOSE; later events are sent when F1 or F2 matches, with no limit\n");
  for i: 0..<4 { print("% would be sent live: %\n", NAME[i], nf_match(f[0], events[i]) || nf_match(g[0], events[i])); }

  print("absent, empty and edge cases\n");
  run("{}", "{}", events[..]);
  run("kinds []", "{\"kinds\":[]}", events[..]);
  run("#t later (a second value)", "{\"#t\":[\"later\"]}", events[..]);
  run("#E 5c83... (uppercase E)", "{\"#E\":[\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\"]}", events[..]);
  run("since 1761686460 until 1761686460", "{\"since\":1761686460,\"until\":1761686460}", events[..]);
  run("since 1761686500 until 1761686400", "{\"since\":1761686500,\"until\":1761686400}", events[..]);
  run("limit 0", "{\"limit\":0}", events[..]);
  run("limit 3", "{\"limit\":3}", events[..]);

  print("refusals\n");
  refuse("ids 5c83", "{\"ids\":[\"5c83\"]}");
  refuse("ids uppercase", "{\"ids\":[\"5C83DA77AF1DEC6D7289834998AD7AAFBD9E2191396D75EC3CC27F5A77226F36\"]}");
  refuse("#p 64 x", "{\"#p\":[\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"]}");
  refuse("kinds 65536", "{\"kinds\":[65536]}");
  refuse("limit -1", "{\"limit\":-1}");
  refuse("since 2^53", "{\"since\":9007199254740992}");
  refuse("search", "{\"search\":\"nostr\"}");
  refuse("#ab", "{\"#ab\":[\"x\"]}");
  refuse("#1", "{\"#1\":[\"x\"]}");
  refuse("kinds twice", "{\"kinds\":[1],\"kinds\":[7]}");
  refuse("kinds 1", "{\"kinds\":1}");
  refuse("kinds [\"1\"]", "{\"kinds\":[\"1\"]}");
  refuse("kinds [1.5]", "{\"kinds\":[1.5]}");
  refuse("since \"1\"", "{\"since\":\"1\"}");
  refuse("an array", "[{\"kinds\":[1]}]");
  refuse("no closing brace", "{\"kinds\":[1]");
  text: [512] u8;
  refuse("64 kinds", repeat(text[..], "kinds", "1", 64));
  refuse("65 kinds", repeat(text[..], "kinds", "1", 65));
  small: [2] int; sp: [1] FrSpace; sp[0] = FrSpace.{strs = s[0].strs[..], ints = small[..], tags = s[0].tags[..], bytes = s[0].bytes[..]};
  print("3 kinds into room for 2: %\n", READ[filter_read("{\"kinds\":[1,7,30023]}", f[..], sp[..])]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/filters.coax
```

The program prints:

```text
events
E1: id 1111..., kind 1, created_at 1761686400, 1 tags
E2: id 2222..., kind 1, created_at 1761686460, 2 tags
E3: id 3333..., kind 7, created_at 1761686520, 2 tags
E4: id 2020..., kind 1, created_at 1761686460, 1 tags
filter F1
read: ok, written back byte for byte
authors 2, kinds 1, tag filters 1, limit 2
E1 matches F1: true
E2 matches F1: true
E3 matches F1: false
E4 matches F1: true
F1 initial query: E4 E2
F1 without limit: E4 E2 E1
filter F2
F2 initial query: E3
one REQ with F1 and F2
initial: E4 E2 E3
then EOSE; later events are sent when F1 or F2 matches, with no limit
E1 would be sent live: true
E2 would be sent live: true
E3 would be sent live: true
E4 would be sent live: true
absent, empty and edge cases
{}: E3 E4 E2 E1
kinds []: none
#t later (a second value): none
#E 5c83... (uppercase E): none
since 1761686460 until 1761686460: E4 E2
since 1761686500 until 1761686400: none
limit 0: none
limit 3: E3 E4 E2
refusals
ids 5c83: invalid
ids uppercase: invalid
#p 64 x: invalid
kinds 65536: invalid
limit -1: invalid
since 2^53: invalid
search: field
#ab: field
#1: field
kinds twice: field
kinds 1: type
kinds ["1"]: type
kinds [1.5]: type
since "1": type
an array: json
no closing brace: json
64 kinds: ok
65 kinds: invalid
3 kinds into room for 2: space
```

What the output shows:

- F1 is read and written back byte for byte, so `filter_read` and
  `nf_write` agree on every field, including the order of keys.
- E1, E2 and E4 match F1 and E3 doesn't, as in the hand table. The initial
  query is E4, E2 (the tie broken by the lower ID), and without the limit
  E4, E2, E1.
- F2 sends E3. The REQ's initial answer is E4, E2, E3; afterwards every
  one of the four events would be sent live, E1 included, because it
  matches F1 and `limit` no longer applies.
- `{}` matches all four, newest first. `kinds []` matches none, which is
  NIP-01's empty-list rule. `#t later` matches none because `later` is a
  second value. `#E` matches none because tag names are case-sensitive.
- `since` and `until` both 1761686460 match the two events at exactly that
  second. `since` 1761686500 with `until` 1761686400 is accepted and matches
  nothing. `limit 0` sends no stored events; `limit 3` sends the newest
  three.
- Each refusal has a status from `filter_read`:

| Input | Status | Why |
| --- | --- | --- |
| `ids` `"5c83"`, uppercase `ids`, `#p` of 64 `x` | `FR_INVALID` | not 64 lowercase hex digits; `nf_valid` refuses after reading |
| `kinds [65536]`, `limit -1`, `since 9007199254740992` | `FR_INVALID` | kind above 65535, negative limit, time beyond 2^53 − 1 |
| `search`, `#ab`, `#1` | `FR_FIELD` | not one of the seven keys; a tag key is `#` and one ASCII letter. NIP-50's `search` is not in this profile; chapter 41's `filter_read_search` reads it |
| `kinds` twice | `FR_FIELD` | a repeated key refuses the filter instead of keeping one value |
| `kinds 1`, `kinds ["1"]`, `kinds [1.5]`, `since "1"` | `FR_TYPE` | a list must be a list of integers; a time must be an integer |
| an array, no closing brace | `FR_JSON` | a filter is one complete JSON object |
| 65 kinds | `FR_INVALID` | more than 64 entries; 64 are accepted |
| 3 kinds into room for 2 | `FR_SPACE` | the caller's storage is full; the filter is refused, not cut |

The last two are the plan's "over-limit filters refuse instead of
truncating": a list one entry over the profile's bound, or over the
caller's storage, refuses the whole filter. A relay answers such a `REQ` with
`CLOSED` (chapter 34); it never runs a shorter filter than the one it was
sent.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/filters.out](../fixtures/filters.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/filters.coax](../examples/checks/filters.coax),
which loads the example. They compare the code with values from outside it:

- a second matcher, `nip01_match`, written from NIP-01's filter sentences
  over small integers instead of JSON and bytes, and sharing no code with
  `filter.coax`. It is compared with `filter_read` and `nf_match` on 4,000
  event and filter pairs drawn by a fixed generator from a space of 96
  events and 2,304 filters (221,184 pairs). The space varies every list
  among absent, empty, one value and two values, `since` and `until` across
  the inclusive boundaries, and the `e` tag among none, `R`, another ID and
  `R` as a second value. Every pair with an empty list must fail;
- the worked example's matches and orders, by hand;
- one input for each refusal status, the 2^53 − 1 boundary, and 64 against
  65 kinds.

<!-- source: nostr/examples/checks/filters.coax check -->
```jai
// A second matcher, written from NIP-01's filter sentences over small
// integers instead of JSON and bytes. It shares no code with filter.coax.
// A list field is one of four choices: 0 absent, 1 [], 2 [first value],
// 3 [second value, first value]. An event field holds 0 (the first value)
// or 1 (the second value).
in_list :: (choice: int, v: int) -> bool {
  if choice == 0 { return true; }    // absent: no condition
  if choice == 1 { return false; }   // []: "no event can satisfy that condition"
  if choice == 2 { return v == 0; }
  return v == 0 || v == 1;
}
// The event's e tag: 0 none, 1 ["e",R], 2 ["e",S], 3 ["e",S,R]. Only the
// first value of a tag is indexed, so case 3 holds S and not R.
e_value :: (etag: int) -> int { return ifx etag == 1 then 0 else ifx etag >= 2 then 1 else -1; }
// since: 0 absent, 1 since 10, 2 since 11. until: 0 absent, 1 until 9,
// 2 until 10.
nip01_match :: (ids: int, authors: int, kinds: int, e: int, since: int, until: int,
              id: int, author: int, kind: int, created: int, etag: int) -> bool {
  if !in_list(ids, id) || !in_list(authors, author) || !in_list(kinds, kind) { return false; }
  if e != 0 && (e_value(etag) < 0 || !in_list(e, e_value(etag))) { return false; }
  if since != 0 && created < 9 + since { return false; }   // since <= created_at
  if until != 0 && created > 8 + until { return false; }   // created_at <= until
  return true;
}

// A key, after a comma unless it is the object's first.
key_json :: (out: [] u8, at: int, key: string) -> int {
  n := at; if out[n - 1] != '{' { n = json_raw(out, n, ","); }
  n = json_raw(out, n, "\""); n = json_raw(out, n, key); return json_raw(out, n, "\":");
}
list_json :: (out: [] u8, at: int, key: string, choice: int, first: string, second: string) -> int {
  if choice == 0 { return at; }
  n := key_json(out, at, key); n = json_raw(out, n, "[");
  if choice == 3 { n = json_raw(out, n, second); n = json_raw(out, n, ","); }
  if choice >= 2 { n = json_raw(out, n, first); }
  return json_raw(out, n, "]");
}
filter_text :: (out: [] u8, fi: int, fa: int, fk: int, fe: int, since: int, until: int) -> string {
  n := json_raw(out, 0, "{");
  n = list_json(out, n, "ids", fi, "\"1111111111111111111111111111111111111111111111111111111111111111\"", "\"2222222222222222222222222222222222222222222222222222222222222222\"");
  n = list_json(out, n, "authors", fa, "\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\"", "\"f7234bd4c1394dda46d09f35bd384dd30cc552ad5541990f98844fb06676e9ca\"");
  n = list_json(out, n, "kinds", fk, "1", "7");
  n = list_json(out, n, "#e", fe, "\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\"", "\"0000000000000000000000000000000000000000000000000000000000000001\"");
  if since != 0 { n = key_json(out, n, "since"); n = json_write_int(out, n, 9 + since); }
  if until != 0 { n = key_json(out, n, "until"); n = json_write_int(out, n, 8 + until); }
  n = json_raw(out, n, "}");
  return cast(string) out[0..<n];
}

// The corpus space: events with one of 2 ids, 2 authors, kinds 1 and 7,
// created_at 9, 10 or 11 and the four e tags (96 events), against filters
// with four choices for each of ids, authors, kinds and #e, since absent,
// 10 or 11 and until absent, 9 or 10 (2,304 filters): 221,184 pairs. The
// check takes PAIRS of them from a fixed linear congruential generator, so
// every run checks the same pairs. Event records are set field by field, as
// filter.coax's own checks set them; the worked example uses parsed events.
PAIRS :: 4000;
ETAGS :: [4] string.["[]", TAGS1,
  "[[\"e\",\"0000000000000000000000000000000000000000000000000000000000000001\"]]",
  "[[\"e\",\"0000000000000000000000000000000000000000000000000000000000000001\",\"5c83da77af1dec6d7289834998ad7aafbd9e2191396d75ec3cc27f5a77226f36\"]]"];
#test "matching agrees with the second matcher on 4,000 generated pairs" #steps 1000000000 {
  ids := [2] string.[ID1, ID2]; authors := [2] string.[A, B]; kinds := [2] int.[1, 7];
  id_bytes: [2] [32] u8; key_bytes: [2] [32] u8;
  for i: 0..<2 { #assert ne_lower_hex(ids[i], id_bytes[i][..]) && ne_lower_hex(authors[i], key_bytes[i][..]); }
  seed := 23; matched := 0; empty := 0;
  for p: 0..<PAIRS {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff; r := seed >> 4;
    fi := r & 3; fa := (r >> 2) & 3; fk := (r >> 4) & 3; fe := (r >> 6) & 3;
    fs := ((r >> 8) & 3) % 3; fu := ((r >> 10) & 3) % 3;
    id := (r >> 12) & 1; au := (r >> 13) & 1; ki := (r >> 14) & 1; cr := 9 + ((r >> 15) & 3) % 3; et := (r >> 17) & 3;
    text: [1024] u8; s: [1] Space; f: [1] NostrFilter;
    #assert read(filter_text(text[..], fi, fa, fk, fe, fs, fu), f[..], s[..]) == FR_OK;
    e: NostrEvent; e.error = NE_OK; e.id = id_bytes[id]; e.public_key = key_bytes[au];
    e.kind = kinds[ki]; e.created_at = cr; e.original = ETAGS[et]; e.tags_from = 0; e.tags_to = e.original.count;
    got := nf_match(f[0], e);
    #assert got == nip01_match(fi, fa, fk, fe, fs, fu, id, au, ki, cr, et);
    if got { matched += 1; }
    if fi == 1 || fa == 1 || fk == 1 || fe == 1 { empty += 1; #assert !got; }
  }
  #assert matched > 100 && empty > 1000;
}

#test "the worked example, decided by hand" {
  b: [4096] u8; ev: [4] NostrEvent;
  ev[0] = make_event(b[0..<1024], ID1, A, 1761686400, 1, TAGS1);
  ev[1] = make_event(b[1024..<2048], ID2, B, 1761686460, 1, TAGS2);
  ev[2] = make_event(b[2048..<3072], ID3, A, 1761686520, 7, TAGS3);
  ev[3] = make_event(b[3072..<4096], ID4, B, 1761686460, 1, TAGS4);
  s: [1] Space; f: [1] NostrFilter; out: [4] int;
  #assert read(F1, f[..], s[..]) == FR_OK;
  #assert nf_match(f[0], ev[0]) && nf_match(f[0], ev[1]) && !nf_match(f[0], ev[2]) && nf_match(f[0], ev[3]);
  // E2 and E4 share created_at 1761686460; 0x20 < 0x22, so E4 comes first. Limit 2 cuts E1.
  #assert query(f[0], ev[..], out[..]) == 2 && out[0] == 3 && out[1] == 1;
  f[0].has_limit = false;
  #assert query(f[0], ev[..], out[..]) == 3 && out[0] == 3 && out[1] == 1 && out[2] == 0;
  s2: [1] Space; g: [1] NostrFilter;
  #assert read(F2, g[..], s2[..]) == FR_OK && query(g[0], ev[..], out[..]) == 1 && out[0] == 2;
  s3: [1] Space;
  #assert read("{}", g[..], s3[..]) == FR_OK && query(g[0], ev[..], out[..]) == 4;
  #assert out[0] == 2 && out[1] == 3 && out[2] == 1 && out[3] == 0;
  #assert read("{\"#t\":[\"later\"]}", g[..], s3[..]) == FR_OK && query(g[0], ev[..], out[..]) == 0;
  #assert read("{\"limit\":0}", g[..], s3[..]) == FR_OK && query(g[0], ev[..], out[..]) == 0;
  #assert read("{\"since\":1761686500,\"until\":1761686400}", g[..], s3[..]) == FR_OK && query(g[0], ev[..], out[..]) == 0;
}

#test "each refusal has its own status, and over-limit lists refuse" {
  s: [1] Space; f: [1] NostrFilter; text: [512] u8;
  #assert read("{\"ids\":[\"5c83\"]}", f[..], s[..]) == FR_INVALID;
  #assert read("{\"kinds\":[65536]}", f[..], s[..]) == FR_INVALID && read("{\"limit\":-1}", f[..], s[..]) == FR_INVALID;
  #assert read("{\"since\":9007199254740992}", f[..], s[..]) == FR_INVALID && read("{\"since\":9007199254740991}", f[..], s[..]) == FR_OK;
  #assert read("{\"search\":\"nostr\"}", f[..], s[..]) == FR_FIELD && read("{\"#ab\":[\"x\"]}", f[..], s[..]) == FR_FIELD;
  #assert read("{\"kinds\":[1],\"kinds\":[7]}", f[..], s[..]) == FR_FIELD;
  #assert read("{\"kinds\":1}", f[..], s[..]) == FR_TYPE && read("{\"kinds\":[1.5]}", f[..], s[..]) == FR_TYPE;
  #assert read("[{\"kinds\":[1]}]", f[..], s[..]) == FR_JSON && read("{\"kinds\":[1]", f[..], s[..]) == FR_JSON;
  #assert read(repeat(text[..], "kinds", "1", 64), f[..], s[..]) == FR_OK && f[0].kinds.count == 64;
  #assert read(repeat(text[..], "kinds", "1", 65), f[..], s[..]) == FR_INVALID;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/filters.coax
coax/bin/coax mutate nostr/examples/checks/filters.coax --root-only
```

On 2026-11-01, `coax test` ran 213 checks, including the loaded files' own
checks; the generated-pairs check took 3 seconds in the checker's
interpreter. Root-only mutation of the checks file killed 96 of 96 mutants,
among them each boundary of the second matcher. The example's procedures and
the library were not mutated for this chapter.

### The library's own checks

[filter.coax](../../coax/lib/nostr/filter.coax) checks absent against empty
lists, AND across fields and inclusive times, the first tag value with JSON
escapes, every byte value as a hex digit and as a tag name, the 64-entry and
256-byte bounds, all 52 tag names, forged tag spans, and a parsed fixture
event. [filter_read.coax](../../coax/lib/nostr/filter_read.coax) checks the
round trip through `nf_write`, every refusal status and the storage limits.

[coax/tests/nostr/FILTER.md](../../coax/tests/nostr/FILTER.md) records, for
`filter.coax` at SHA-256 `e3642586...04cf2` (the file is unchanged today),
199 of 199 checks and 3,891 of 3,891 mutants killed, and a native comparison
(`coax/tests/nostr/filter_native.py`) of 4,096 match cases against separate
Boolean formulas in checked, release, ASan/UBSan and WASI builds. The
[2026-10-29 relay record](../../docs/verification/2026-10-29-coax-relay-r1.md)
reports 4,131 of 4,131 mutants killed for `filter_read.coax` with
`filter.coax`, `event.coax` and `json.coax`, none marked equivalent. Those
runs were not repeated for this chapter.

## Summary

- A filter's present conditions must all hold; one equal value in a list is
  enough; one matching filter in a `REQ` is enough.
- An absent list sets no condition; an empty list matches nothing.
- A `#<letter>` filter compares the first value of each tag with that name,
  case-sensitively. `#e` and `#p` values are lowercase hex.
- `since <= created_at <= until`, both inclusive. `since > until` is valid
  and matches nothing.
- `limit` cuts only the initial query, newest first and the lowest ID on a
  tie. After `EOSE` every match is sent.
- Unknown keys, repeated keys, wrong types, bad hex and lists over 64
  entries refuse the whole filter; nothing is truncated.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Sources

- [NIP-01](../nips/official/01.md), the filter attributes, the empty-list
  rule, the tag index, `since` and `until`, `limit` and the `REQ` message.
- [NIP-12](../nips/official/12.md), where generic tag queries were first
  defined (now merged into NIP-01).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
