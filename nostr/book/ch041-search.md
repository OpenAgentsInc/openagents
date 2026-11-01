# 41. Search

NIP-50 lets a filter carry a search string. This chapter reads that string,
splits stored notes into words, indexes the words, and answers a search with
the notes that hold every word, best match first. It also records where the
answers differ from the old Rust relay's.

Part IX, A relay. Result: NIP-50's `search` filter field, a word index over
the store, results ordered by a word-count score, `include:spam`, and every
other `key:value` extension ignored.

## What this chapter teaches

- NIP-50 adds one filter key, `"search"`, whose value is a string. Its
  meaning is up to the relay. NIP-50 asks relays to match content and lets
  them choose how to rank. Clients must not assume a ranking.
- How this relay reads the string: fields split on ASCII whitespace; a field
  that holds a `:` is a `key:value` extension; every other field gives
  words.
- Words: runs of ASCII letters, ASCII digits and non-ASCII bytes. `A` to `Z`
  fold to lowercase; non-ASCII bytes are compared exactly.
- A word index (an inverted index): one entry per distinct word in each
  stored note. A search reads the entries of its rarest word, and nothing
  else.
- Matching and score: a note matches when every search word occurs in it as
  a whole word. Its score is how many times the search words occur. Results
  are ordered by score, then newest first, then lowest ID, and `limit`
  applies after that order.
- Extensions: `include:spam` is supported and changes nothing here, because
  this store has no spam filter to turn off. Every other extension
  (`language:`, `domain:`, `sentiment:`, `nsfw:` and unknown ones) is
  ignored, and the filter still runs.
- How the answers differ from the Rust relay's: it matched substrings and
  ordered by time.
- What is not built: the relay process doesn't call this code yet.
  `relay_server.coax` still reads every `REQ` filter with `filter_read`,
  which refuses `search` (chapter 37 shows the `CLOSED` it sends). Tag
  search, the `language:`, `domain:`, `sentiment:` and `nsfw:` extensions,
  stemming and Unicode case folding are not built either.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [filter_read.coax](../../coax/lib/nostr/filter_read.coax) | `filter_read_search` | new: chapter 23's reader, plus the `search` key (`#authority`) |
| [nip50.coax](../../coax/lib/nostr/nip50.coax) | `n50_next`, `n50_key`, `n50_tf` | new: words, a word's index key, and how often a word occurs |
| [nip50.coax](../../coax/lib/nostr/nip50.coax) | `n50_parse` | new: a search string into terms and extensions |
| [nip50.coax](../../coax/lib/nostr/nip50.coax) | `n50_build` | new: the word index over the store |
| [nip50.coax](../../coax/lib/nostr/nip50.coax) | `n50_query`, `n50_score`, `n50_insert` | new: the answer, best first |
| [store_index.coax](../../coax/lib/nostr/store_index.coax) | `nsi_query_limit` | chapter 36's query, used when a search has no words |

`nip50.coax` is new. Its checks were written first, and `coax mutate
--root-only` on it kills 434 of 434 mutants. It is not `#authority`, for the
same reason as `store_index.coax`: loading the store brings the relay's
whole graph under every mutation run.

One library file changed. `filter_read` refuses every key outside NIP-01's
seven, and `search` is one of them. That stays true: a relay that doesn't
answer searches must still refuse them. `filter_read_search` is a second
entry to the same reader that also accepts `search`. Both call `fr_read`,
which takes a flag. The old checks pass unchanged, a new check covers the
key, and `coax mutate coax/lib/nostr/filter_read.coax --root-only` kills
263 of 263 mutants.

## The search field

A filter with a search looks like this:

```jai
{"kinds": [1], "search": "coax nostr language:en", "limit": 20}
```

`search` must be a string, given once. `filter_read_search` decodes its
JSON escapes into the filter's storage, like every other string, and
returns it in an `FrSearch`: `has` says whether the key was there, and
`text` is the decoded string. A number, an array or a second `search` key
refuses the whole filter, as for any other key.

NIP-50 says little else. The relay interprets the string "as it wishes",
should match content and may match other fields, ranks by relevance, and
should ignore extensions it doesn't support. This relay matches content
only.

## Words

`n50_next` finds the next word in a text. A word is a maximal run of
bytes that are ASCII letters, ASCII digits, or `0x80` and above. Every
other byte (spaces, ASCII punctuation, control characters) ends a word.
`n50_lower` folds `A`–`Z` to `a`–`z` and leaves every other byte as it is.

Non-ASCII text is kept whole and compared byte for byte. `café` is one
word (`c a f 0xc3 0xa9`), and it is not the same word as `CAFÉ`, because
`É` is `0xc3 0x89`. A non-ASCII punctuation mark such as `—` (U+2014) is
not a separator, so `a—b` is one word. Unicode case folding and Unicode
word boundaries are not built.

Content is read decoded: `\n` in the event's JSON is a newline, which
separates words, not the letter `n`.

<!-- source: coax/lib/nostr/nip50.coax words -->
```jai
// The first word in text at or after at.
n50_next :: (text: string, at: int) -> N50Word {
  i := at;
  while i < text.count && !n50_word_byte(text[i]) { i += 1; }
  w := N50Word.{from = i, to = i};
  while w.to < text.count && n50_word_byte(text[w.to]) { w.to += 1; }
  return w;
}
// The key of the word text[from..<to], folded to lowercase.
n50_key :: (text: string, from: int, to: int) -> int {
  h := 1;
  for i: from..<to { h = (h * 257 + cast(int) n50_lower(text[i]) + 1) % NSI_HASH; }
  return h;
}
// How many words of text equal term (lowercase) once folded.
n50_tf :: (text: string, term: string) -> int {
  n := 0; w := n50_next(text, 0);
  while w.from < text.count {
    same := w.to - w.from == term.count;
    for k: 0..<term.count { if same && n50_lower(text[w.from + k]) != term[k] { same = false; } }
    if same { n += 1; }
    w = n50_next(text, w.to);
  }
  return n;
}
```

`n50_key` is the word's key in the index: a 31-bit hash of its folded
bytes, built like chapter 36's tag keys. `n50_tf` counts the words of a
text that equal a term, so `relay` occurs once in `relays relay`, not
twice.

## The search string

`n50_parse` reads the string in fields separated by ASCII whitespace
(space, tab, LF, VT, FF, CR):

1. A field equal to `include:spam` sets `spam`.
2. Any other field that holds a `:` is an extension. It is counted in
   `ignored` and otherwise ignored. This is the Rust relay's rule, so
   `nostr:npub1...` or `https://...` in a search is ignored too.
3. Every other field is split into words. Each word is folded into the
   caller's buffer and kept once: `Nostr nostr` gives one term.

At most 16 distinct terms fit (`N50_TERMS` refuses a seventeenth), and the
folded copies must fit the caller's buffer (`N50_SPACE`).

<!-- source: coax/lib/nostr/nip50.coax parse -->
```jai
// Reads a search string into q[0]: the distinct words of its fields, each
// copied in lowercase into buf, and its key:value extensions.
n50_parse :: (search: string, buf: [] u8, q: [] N50Query) -> int {
  q[0] = N50Query.{}; used := 0; at := 0;
  while at < search.count {
    if n50_space(search[at]) { at += 1; continue; }
    end := at; colon := false;
    while end < search.count && !n50_space(search[end]) { if search[end] == ':' { colon = true; } end += 1; }
    field := search[at..<end];
    if field == "include:spam" { q[0].spam = true; }
    else if colon { q[0].ignored += 1; }
    else {
      w := n50_next(field, 0);
      while w.from < field.count {
        k := w.to - w.from;
        if used + k > buf.count { q[0].error = N50_SPACE; return N50_SPACE; }
        for i: 0..<k { buf[used + i] = n50_lower(field[w.from + i]); }
        term := cast(string) buf[used..<used + k]; seen := false;
        for i: 0..<q[0].n { if q[0].terms[i] == term { seen = true; } }
        if !seen {
          if q[0].n == N50_MAX_TERMS { q[0].error = N50_TERMS; return N50_TERMS; }
          q[0].terms[q[0].n] = term; q[0].n += 1; used += k;
        }
        w = n50_next(field, w.to);
      }
    }
    at = end;
  }
  return N50_OK;
}
```

A search with no terms (an empty string, or extensions only) is not
refused. It selects what the rest of the filter selects.

## The word index

`n50_build` reads every stored record's content, decodes it and adds one
entry per distinct word: the word's key and the record number. Entries are
kept sorted by key, then record, so the entries of one word are adjacent
and `n50_bound` finds them by binary search.

Kinds 4 (NIP-04 messages), 1059 and 21059 (NIP-59 gift wraps) are not
indexed. Their content is ciphertext, and a search never returns them.
Kind 21059 is ephemeral, so the store never keeps it anyway.

<!-- source: coax/lib/nostr/nip50.coax build -->
```jai
// Builds the word index over every record's decoded content (buf must hold
// the longest). False, and the index refused, when an array is too short.
n50_build :: (s: [] RsdStore, x: [] N50Index, buf: [] u8) -> bool {
  x[0].ok = false; x[0].count = 0;
  for i: 0..<s[0].n {
    if n50_excluded(s[0].recs[i].kind) { continue; }
    k := json_unescape(n50_content(rsd_text(s, i)), buf);
    if k < 0 { return false; }
    text := cast(string) buf[0..<k]; w := n50_next(text, 0);
    while w.from < k {
      key := n50_key(text, w.from, w.to); at := n50_bound(x[0], key, i);
      if at == x[0].count || x[0].key[at] != key || x[0].rec[at] != i {
        if x[0].count >= x[0].key.count || x[0].count >= x[0].rec.count { return false; }
        e := x[0].count;
        while e > at { x[0].key[e] = x[0].key[e - 1]; x[0].rec[e] = x[0].rec[e - 1]; e -= 1; }
        x[0].key[at] = key; x[0].rec[at] = i; x[0].count += 1;
      }
      w = n50_next(text, w.to);
    }
  }
  x[0].n = s[0].n; x[0].used = s[0].used; x[0].ok = true;
  return true;
}
```

Like chapter 36's index, the word index is built whole. It records the
store's record count and arena use, and a query refuses it once either has
changed. There is no incremental update.

The key is a hash, so two words can share one. The index only picks
candidates. `n50_query` decodes each candidate's content again and counts
the terms exactly, so a collision costs a read and never adds a result.

## Matching, score and order

`n50_query` answers one filter and one parsed search:

1. If either index is out of date, it refuses (`n` is -1).
2. With no terms, it runs chapter 36's query for every match, drops the
   ciphertext kinds, and applies the filter's `limit`. Every score is 0, so
   the order is chapter 36's: newest first, then lowest ID.
3. With terms, it finds the term with the fewest entries (the first one, on
   a tie) and reads only that term's records.
4. Each candidate must be visible (not replaced, not expired) and match the
   filter's other fields with chapter 23's `nf_match`.
5. Its score is the sum of each term's count in the decoded content, or 0
   when any term is missing. A score of 0 doesn't match.
6. `n50_insert` keeps the results ordered by score (highest first), then
   newest, then lowest ID, and at most `rs_limit(f)` long: the filter's
   `limit`, or 127. The limit applies after scoring, so `limit: 1` returns
   the best match, not the newest one.

<!-- source: coax/lib/nostr/nip50.coax query -->
```jai
// The score of decoded content: the sum of the terms' counts, or 0 when a
// term is missing.
n50_score :: (text: string, q: N50Query) -> int {
  sum := 0;
  for i: 0..<q.n { c := n50_tf(text, q.terms[i]); if c == 0 { return 0; } sum += c; }
  return sum;
}
// Puts record i with score sc into out[0..<n] (best first, then newest,
// then lowest id), at most limit and out.count long: the new length.
n50_insert :: (s: [] RsdStore, i: int, sc: int, n: int, limit: int, out: [] int, score: [] int) -> int {
  at := n;
  while at > 0 && (sc > score[at - 1] || (sc == score[at - 1] && ns_store_newer(s[0].recs[i], s[0].recs[out[at - 1]]))) { at -= 1; }
  if at >= limit || at >= out.count { return n; }
  k := n; if k >= limit || k >= out.count { k -= 1; }
  m := k + 1;
  while k > at { out[k] = out[k - 1]; score[k] = score[k - 1]; k -= 1; }
  out[at] = i; score[at] = sc;
  return m;
}
// Up to rs_limit(f) (and out.count) visible records that match f and every
// term of q, best first, with their scores in score. buf holds one decoded
// content. Refused while either index is out of date.
n50_query :: (s: [] RsdStore, ix: [] NsiIndex, x: [] N50Index, f: NostrFilter, q: N50Query, now: int, buf: [] u8, out: [] int, score: [] int) -> N50Result {
  if !x[0].ok || x[0].n != s[0].n || x[0].used != s[0].used { return N50Result.{n = -1, plan = N50_REFUSED}; }
  if q.n == 0 {
    // every match in time order, then the ciphertext kinds dropped and the limit applied
    r := nsi_query_limit(s, ix, f, now, out.count, out);
    if r.n < 0 { return N50Result.{n = -1, plan = N50_REFUSED}; }
    limit := rs_limit(f); n := 0;
    for k: 0..<r.n { if n < limit && !n50_excluded(s[0].recs[out[k]].kind) { out[n] = out[k]; score[n] = 0; n += 1; } }
    return N50Result.{n = n, plan = ifx r.plan == NSI_NONE then N50_NONE else N50_FILTER, scanned = r.scanned};
  }
  later := ns_store_seen(s, now);
  limit := rs_limit(f);
  if !nf_valid(f) || limit == 0 || out.count == 0 || (f.has_since && f.has_until && f.since > f.until) { return N50Result.{}; }
  // the term with the fewest entries gives the candidates
  res := N50Result.{plan = N50_TERM}; from: int; to: int; best := -1;
  for t: 0..<q.n {
    term := q.terms[t]; key := n50_key(term, 0, term.count);
    a := n50_bound(x[0], key, 0); b := n50_bound(x[0], key + 1, 0);
    if best < 0 || b - a < to - from { from = a; to = b; res.term = t; best = t; }
  }
  n := 0;
  for p: from..<to {
    i := x[0].rec[p]; res.scanned += 1;
    if !ns_store_visible(s, i, now) || !nf_match(f, rsd_event(s, i)) { continue; }
    k := json_unescape(n50_content(rsd_text(s, i)), buf);
    if k < 0 { res.unread += 1; continue; }
    sc := n50_score(cast(string) buf[0..<k], q);
    if sc > 0 { n = n50_insert(s, i, sc, n, limit, out, score); }
  }
  res.n = n;
  return res;
}
```

`unread` counts candidates whose decoded content didn't fit the caller's
buffer. Give it 65,536 bytes (the longest string chapter 21's reader
accepts) and none are unread.

This score is a word count, not a model of relevance. It doesn't weigh rare
words higher (no inverse document frequency), and it doesn't divide by the
note's length, so a long note that repeats a word scores high. NIP-50 lets
each relay choose, and this rule is short, deterministic and checkable by
hand.

## Worked example: three notes

The three notes are signed 300, 200 and 100 seconds before the relay's
clock, `NOW`:

| Note | Content | Words |
| --- | --- | --- |
| n1 | `Coax compiles Coax.` | coax, compiles, coax |
| n2 | `Nostr relays, written in Coax` | nostr, relays, written, in, coax |
| n3 | `nostr relay, nostr client, in COAX` | nostr, relay, nostr, client, in, coax |

Distinct words per note: n1 has 2, n2 has 5, n3 has 5, so the index has 12
entries. `coax` has three entries (n1, n2, n3) and `nostr` has two (n2,
n3).

Search `coax nostr language:en`:

1. Fields: `coax`, `nostr`, `language:en`. The third holds a `:`, so it is an
   extension, ignored: terms `coax`, `nostr`; `ignored` is 1.
2. `nostr` has fewer entries (2 against 3), so n2 and n3 are read.
3. n2: `coax` 1 + `nostr` 1 = 2. n3: `coax` 1 + `nostr` 2 = 3.
4. Order: n3 (3), then n2 (2). n1 has no `nostr` and was never read.

Search `Coax`:

1. One term, `coax`; its three entries are read.
2. n1 scores 2. n2 and n3 score 1 each.
3. Order: n1 (2); then the tie at 1, newest first: n3 (`NOW-100`), then n2
   (`NOW-200`).
4. With `"limit": 1`, the answer is n1 alone. A time-ordered relay would
   return n3.

Search `relay`: one entry, n3, which scores 1. n2 has `relays`, which is a
different word.

## The difference from the Rust relay

The Rust relay that served relay.openagents.com before the reset (removed in
`0b401561a0`) implemented NIP-50 in `crates/nostr/src/domain/filter.rs` and
in its SQL replay query:

- terms are whitespace fields with no `:`, with `A`–`Z` folded;
- a note matches when each term occurs as a substring of its folded
  content (`position(term IN lower(content))`);
- results are ordered newest first, then lowest ID, with no ranking;
- private kinds never match: gift wraps (1059, 21059) and 13 OpenAgents
  kinds such as 30078 and 44200.

This chapter keeps its extension rule and its ASCII-only case folding. The
differences, on the example's notes and on the nine-note corpus in the
checks:

| Search | This chapter | The Rust relay | Why |
| --- | --- | --- | --- |
| `relay` | n3 | n3, n2 | `relays` holds `relay` as a substring; here words must be equal |
| `Nostr, Coax!` | n3, n2 | nothing | there the fields keep their punctuation, and no note holds `coax!` |
| `Coax` | n1, n3, n2 | n3, n2, n1 | the same notes; here a higher score comes first |
| `cat` on `catwalk: the cat sat` | matches (1) | matches | the same answer; here `catwalk` doesn't add to the score |

Two more differences come from the rules, not from a query:

- With no terms, the Rust relay's replay query matched every note (an empty
  list of terms, all present), while its live matcher `search_matches`
  matched none. This chapter follows the replay query.
- The Rust relay didn't exclude kind 4, and it excluded OpenAgents'
  private kinds. Those kinds and the access rules around them belong to
  chapter 103, which replaces relay.openagents.com.

The Rust relay also had a query cost with a factor for search (chapter
44's `rs_cost` notes it). That is not part of this chapter.

## The example

The example is [nostr/examples/search.coax](../examples/search.coax). It:

1. prints the words of the three notes;
2. signs the notes with the BIP-340 vector 1 test key, admits them through
   chapter 34's read, policy and signature steps, and builds both indexes;
3. sends seven filters through `ask`, and for four of them prints what the
   Rust relay's rule would answer;
4. shows the refusals: chapter 23's `filter_read` refusing `search`, a
   number as `search`, `search` twice, 17 terms, and a fourth note admitted
   after the index was built.

Each filter goes through `ask`:

<!-- source: nostr/examples/search.coax ask -->
```jai
// One filter with a search through the relay's reader, the search string's
// parser and the word index; the answer with scores.
ask :: (r: [] Relay, text: string) {
  strs: [64] string; ints: [16] int; tags: [8] NostrTagFilter; bytes: [1024] u8;
  sp: [1] FrSpace; sp[0] = FrSpace.{strs = strs[..], ints = ints[..], tags = tags[..], bytes = bytes[..]};
  f: [1] NostrFilter; srch: [1] FrSearch;
  print("-> %\n", text);
  e := filter_read_search(text, f[..], sp[..], srch[..]);
  if e != FR_OK { print("    refused by filter_read_search: %\n", FR_NAMES[e]); return; }
  qbuf: [1024] u8; q: [1] N50Query;
  pe := n50_parse(srch[0].text, qbuf[..], q[..]);
  if pe != N50_OK { print("    refused by n50_parse: %\n", N50_NAMES[pe]); return; }
  print("    terms:"); for i: 0..<q[0].n { print(" %", q[0].terms[i]); }
  if q[0].n == 0 { print(" (none)"); }
  print("; include:spam %; extensions ignored %\n", q[0].spam, q[0].ignored);
  out: [64] int; score: [64] int;
  res := n50_query(r[0].s[..], r[0].ix[..], r[0].x[..], f[0], q[0], NOW, r[0].buf[..], out[..], score[..]);
  if res.n < 0 { print("    refused: the word index is out of date\n"); return; }
  plan := ifx res.plan == N50_TERM then "the entries of one term" else "chapter 36's query";
  print("    % result(s), read through %, % record(s) read\n", res.n, plan, res.scanned);
  for k: 0..<res.n { i := out[k]; print("    score % n% at NOW-%: %\n", score[k], i + 1, NOW - r[0].recs[i].created, text_of(r, i)); }
}
```

The Rust relay's rule, transcribed in Coax for the transcript:

<!-- source: nostr/examples/search.coax rust -->
```jai
// The Rust relay's rule (crates/nostr/src/domain/filter.rs, search_terms,
// and the replay query): whitespace fields without a ':', A-Z folded; each
// must be a substring of the folded content; newest first, then lowest id.
rust_relay :: (r: [] Relay, search: string) {
  print("    the Rust relay's rule would answer:");
  hits: [64] int; n := 0;
  for i: 0..<r[0].s[0].n {
    text := text_of(r, i); all := true; at := 0;
    while at < search.count {
      if search[at] == ' ' { at += 1; continue; }
      end := at; colon := false;
      while end < search.count && search[end] != ' ' { if search[end] == ':' { colon = true; } end += 1; }
      found := colon;
      for p: 0..<text.count - (end - at) + 1 {
        same := true;
        for k: 0..<end - at { if n50_lower(text[p + k]) != n50_lower(search[at + k]) { same = false; } }
        if same { found = true; }
      }
      if !found { all = false; }
      at = end;
    }
    if all && ns_store_visible(r[0].s[..], i, NOW) { n = ns_store_insert(r[0].s[..], i, n, 64, hits[..]); }
  }
  for k: 0..<n { print(" n%", hits[k] + 1); }
  if n == 0 { print(" nothing"); }
  print("\n");
}
```

The whole program:

<!-- source: nostr/examples/search.coax example -->
```jai
#load "../../coax/lib/nostr/nip50.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// The relay's clock for the whole run: 2026-11-01T00:00:00Z.
NOW :: 1793491200;
// BIP-340 vector 1's published test key and aux. Never sign with a real key
// in an example.
KEY_A :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
AUX_A :: "0000000000000000000000000000000000000000000000000000000000000001";

// A store of up to 64 records, both indexes, and room to read one filter.
Relay :: struct {
  arena: [16384] u8; recs: [64] RsdRec; s: [1] RsdStore;
  by_id: [64] int; by_ak: [64] int; by_time: [64] int; tag_key: [128] int; tag_rec: [128] int;
  ix: [1] NsiIndex;
  key: [512] int; rec: [512] int; x: [1] N50Index;
  buf: [65536] u8; scratch: [4096] u8;
}
relay_init :: (r: [] Relay) {
  r[0].s[0] = RsdStore.{}; r[0].s[0].arena = r[0].arena[..]; r[0].s[0].recs = r[0].recs[..];
  r[0].ix[0] = NsiIndex.{by_id = r[0].by_id[..], by_ak = r[0].by_ak[..], by_time = r[0].by_time[..], tag_key = r[0].tag_key[..], tag_rec = r[0].tag_rec[..]};
  r[0].x[0] = N50Index.{key = r[0].key[..], rec = r[0].rec[..]};
}
// One EVENT through the relay's admission: read, signature, store.
admit :: (r: [] Relay, text: string) -> int {
  a := rs_admit_read(text, NOW, false, false, r[0].scratch[..]);
  if a.reason == RS_ACCEPT { a = rs_admit_verified(a, nostr_event_verify_vartime(text, r[0].scratch[..], false)); }
  if a.reason != RS_ACCEPT { return a.reason; }
  return rsd_admit(r[0].s[..], a, text);
}
sign :: (created_at: int, content: string, out: [] u8) -> string {
  key: [32] u8; aux: [32] u8;
  #assert hex_decode_checked(KEY_A, key[..], false).error == NC_OK;
  #assert hex_decode_checked(AUX_A, aux[..], false).error == NC_OK;
  work: [1] NostrSignWork; scratch: [8192] u8;
  n := EVENT_MORE;
  while n == EVENT_MORE { n = nostr_event_sign_step(key[..], aux[..], created_at, 1, "[]", content, work[..], scratch[..], out, false); }
  #assert n > 0;
  return cast(string) out[0..<n];
}
to_hex :: (b: [] u8, out: [] u8) -> string {
  digits := "0123456789abcdef";
  for i: 0..<b.count { out[2 * i] = digits[b[i] >> 4]; out[2 * i + 1] = digits[b[i] & 15]; }
  return cast(string) out[0..<2 * b.count];
}
// Record i's content, decoded.
text_of :: (r: [] Relay, i: int) -> string {
  k := json_unescape(n50_content(rsd_text(r[0].s[..], i)), r[0].buf[..]);
  return cast(string) r[0].buf[0..<k];
}
FR_NAMES :: [6] string.["FR_OK", "FR_JSON", "FR_FIELD", "FR_TYPE", "FR_SPACE", "FR_INVALID"];
N50_NAMES :: [3] string.["N50_OK", "N50_TERMS", "N50_SPACE"];

// book:begin ask
// One filter with a search through the relay's reader, the search string's
// parser and the word index; the answer with scores.
ask :: (r: [] Relay, text: string) {
  strs: [64] string; ints: [16] int; tags: [8] NostrTagFilter; bytes: [1024] u8;
  sp: [1] FrSpace; sp[0] = FrSpace.{strs = strs[..], ints = ints[..], tags = tags[..], bytes = bytes[..]};
  f: [1] NostrFilter; srch: [1] FrSearch;
  print("-> %\n", text);
  e := filter_read_search(text, f[..], sp[..], srch[..]);
  if e != FR_OK { print("    refused by filter_read_search: %\n", FR_NAMES[e]); return; }
  qbuf: [1024] u8; q: [1] N50Query;
  pe := n50_parse(srch[0].text, qbuf[..], q[..]);
  if pe != N50_OK { print("    refused by n50_parse: %\n", N50_NAMES[pe]); return; }
  print("    terms:"); for i: 0..<q[0].n { print(" %", q[0].terms[i]); }
  if q[0].n == 0 { print(" (none)"); }
  print("; include:spam %; extensions ignored %\n", q[0].spam, q[0].ignored);
  out: [64] int; score: [64] int;
  res := n50_query(r[0].s[..], r[0].ix[..], r[0].x[..], f[0], q[0], NOW, r[0].buf[..], out[..], score[..]);
  if res.n < 0 { print("    refused: the word index is out of date\n"); return; }
  plan := ifx res.plan == N50_TERM then "the entries of one term" else "chapter 36's query";
  print("    % result(s), read through %, % record(s) read\n", res.n, plan, res.scanned);
  for k: 0..<res.n { i := out[k]; print("    score % n% at NOW-%: %\n", score[k], i + 1, NOW - r[0].recs[i].created, text_of(r, i)); }
}
// book:end ask

// book:begin rust
// The Rust relay's rule (crates/nostr/src/domain/filter.rs, search_terms,
// and the replay query): whitespace fields without a ':', A-Z folded; each
// must be a substring of the folded content; newest first, then lowest id.
rust_relay :: (r: [] Relay, search: string) {
  print("    the Rust relay's rule would answer:");
  hits: [64] int; n := 0;
  for i: 0..<r[0].s[0].n {
    text := text_of(r, i); all := true; at := 0;
    while at < search.count {
      if search[at] == ' ' { at += 1; continue; }
      end := at; colon := false;
      while end < search.count && search[end] != ' ' { if search[end] == ':' { colon = true; } end += 1; }
      found := colon;
      for p: 0..<text.count - (end - at) + 1 {
        same := true;
        for k: 0..<end - at { if n50_lower(text[p + k]) != n50_lower(search[at + k]) { same = false; } }
        if same { found = true; }
      }
      if !found { all = false; }
      at = end;
    }
    if all && ns_store_visible(r[0].s[..], i, NOW) { n = ns_store_insert(r[0].s[..], i, n, 64, hits[..]); }
  }
  for k: 0..<n { print(" n%", hits[k] + 1); }
  if n == 0 { print(" nothing"); }
  print("\n");
}
// book:end rust

main :: () {
  r: [1] Relay; relay_init(r[..]);
  h: [64] u8;
  notes := [3] string.["Coax compiles Coax.", "Nostr relays, written in Coax", "nostr relay, nostr client, in COAX"];

  print("the words of three notes (ASCII letters and digits, A-Z folded)\n");
  for i: 0..<3 {
    print("  n%: \"%\" ->", i + 1, notes[i]);
    w := n50_next(notes[i], 0);
    while w.from < notes[i].count {
      word: [32] u8; for k: w.from..<w.to { word[k - w.from] = n50_lower(notes[i][k]); }
      print(" %", cast(string) word[0..<w.to - w.from]);
      w = n50_next(notes[i], w.to);
    }
    print("\n");
  }
  print("  counts: coax n1 2, n2 1, n3 1; nostr n2 1, n3 2; relay n3 1; relays n2 1\n\n");

  print("the store: the three notes signed by the BIP-340 vector 1 key, 300, 200 and 100 seconds old\n");
  texts: [3] string; bufs: [3] [1024] u8;
  for i: 0..<3 {
    texts[i] = sign(NOW - 300 + 100 * i, notes[i], bufs[i][..]);
    result := admit(r[..], texts[i]);
    id := r[0].recs[i].id;
    print("  n% id %...: %\n", i + 1, to_hex(id[0..<4], h[..]), ifx result == RS_ACCEPT then "accepted" else "refused");
  }
  #assert nsi_build(r[0].s[..], r[0].ix[..]) && n50_build(r[0].s[..], r[0].x[..], r[0].buf[..]);
  print("  word index: % entries (n1 2, n2 5, n3 5 distinct words)\n\n", r[0].x[0].count);

  print("searches\n");
  ask(r[..], "{\"search\":\"coax nostr language:en\"}");
  rust_relay(r[..], "coax nostr language:en");
  ask(r[..], "{\"search\":\"Coax\"}");
  rust_relay(r[..], "Coax");
  ask(r[..], "{\"search\":\"Coax\",\"limit\":1}");
  ask(r[..], "{\"search\":\"relay\"}");
  rust_relay(r[..], "relay");
  ask(r[..], "{\"search\":\"Nostr, Coax!\"}");
  rust_relay(r[..], "Nostr, Coax!");
  ask(r[..], "{\"kinds\":[1],\"search\":\"nsfw:false include:spam\"}");
  ask(r[..], "{\"search\":\"coax\",\"since\":1793491000}");
  ask(r[..], "{\"search\":\"zebra\"}");

  print("\nrefusals\n");
  strs: [8] string; ints: [4] int; tags: [2] NostrTagFilter; bytes: [64] u8;
  sp: [1] FrSpace; sp[0] = FrSpace.{strs = strs[..], ints = ints[..], tags = tags[..], bytes = bytes[..]};
  f: [1] NostrFilter;
  print("  chapter 23's filter_read on {\"search\":\"coax\"}: %\n", FR_NAMES[filter_read("{\"search\":\"coax\"}", f[..], sp[..])]);
  ask(r[..], "{\"search\":5}");
  ask(r[..], "{\"search\":\"a\",\"search\":\"b\"}");
  ask(r[..], "{\"search\":\"a b c d e f g h i j k l m n o p q\"}");
  b4: [1024] u8; e4 := sign(NOW, "a fourth note about coax", b4[..]);
  print("a fourth note admitted, word index not rebuilt: %\n", ifx admit(r[..], e4) == RS_ACCEPT then "accepted" else "refused");
  ask(r[..], "{\"search\":\"coax\"}");
}
```

Run it with `coax run nostr/examples/search.coax --run-steps 250000000`.
The transcript, `nostr/fixtures/search.out`, is the same in debug and
release:

```text
the words of three notes (ASCII letters and digits, A-Z folded)
  n1: "Coax compiles Coax." -> coax compiles coax
  n2: "Nostr relays, written in Coax" -> nostr relays written in coax
  n3: "nostr relay, nostr client, in COAX" -> nostr relay nostr client in coax
  counts: coax n1 2, n2 1, n3 1; nostr n2 1, n3 2; relay n3 1; relays n2 1

the store: the three notes signed by the BIP-340 vector 1 key, 300, 200 and 100 seconds old
  n1 id 598aeaf6...: accepted
  n2 id 97572924...: accepted
  n3 id c18ebd30...: accepted
  word index: 12 entries (n1 2, n2 5, n3 5 distinct words)

searches
-> {"search":"coax nostr language:en"}
    terms: coax nostr; include:spam false; extensions ignored 1
    2 result(s), read through the entries of one term, 2 record(s) read
    score 3 n3 at NOW-100: nostr relay, nostr client, in COAX
    score 2 n2 at NOW-200: Nostr relays, written in Coax
    the Rust relay's rule would answer: n3 n2
-> {"search":"Coax"}
    terms: coax; include:spam false; extensions ignored 0
    3 result(s), read through the entries of one term, 3 record(s) read
    score 2 n1 at NOW-300: Coax compiles Coax.
    score 1 n3 at NOW-100: nostr relay, nostr client, in COAX
    score 1 n2 at NOW-200: Nostr relays, written in Coax
    the Rust relay's rule would answer: n3 n2 n1
-> {"search":"Coax","limit":1}
    terms: coax; include:spam false; extensions ignored 0
    1 result(s), read through the entries of one term, 3 record(s) read
    score 2 n1 at NOW-300: Coax compiles Coax.
-> {"search":"relay"}
    terms: relay; include:spam false; extensions ignored 0
    1 result(s), read through the entries of one term, 1 record(s) read
    score 1 n3 at NOW-100: nostr relay, nostr client, in COAX
    the Rust relay's rule would answer: n3 n2
-> {"search":"Nostr, Coax!"}
    terms: nostr coax; include:spam false; extensions ignored 0
    2 result(s), read through the entries of one term, 2 record(s) read
    score 3 n3 at NOW-100: nostr relay, nostr client, in COAX
    score 2 n2 at NOW-200: Nostr relays, written in Coax
    the Rust relay's rule would answer: nothing
-> {"kinds":[1],"search":"nsfw:false include:spam"}
    terms: (none); include:spam true; extensions ignored 1
    3 result(s), read through chapter 36's query, 3 record(s) read
    score 0 n3 at NOW-100: nostr relay, nostr client, in COAX
    score 0 n2 at NOW-200: Nostr relays, written in Coax
    score 0 n1 at NOW-300: Coax compiles Coax.
-> {"search":"coax","since":1793491000}
    terms: coax; include:spam false; extensions ignored 0
    2 result(s), read through the entries of one term, 3 record(s) read
    score 1 n3 at NOW-100: nostr relay, nostr client, in COAX
    score 1 n2 at NOW-200: Nostr relays, written in Coax
-> {"search":"zebra"}
    terms: zebra; include:spam false; extensions ignored 0
    0 result(s), read through the entries of one term, 0 record(s) read

refusals
  chapter 23's filter_read on {"search":"coax"}: FR_FIELD
-> {"search":5}
    refused by filter_read_search: FR_TYPE
-> {"search":"a","search":"b"}
    refused by filter_read_search: FR_FIELD
-> {"search":"a b c d e f g h i j k l m n o p q"}
    refused by n50_parse: N50_TERMS
a fourth note admitted, word index not rebuilt: accepted
-> {"search":"coax"}
    terms: coax; include:spam false; extensions ignored 0
    refused: the word index is out of date
```

The filter with only `nsfw:false include:spam` runs: it has no terms, so it
returns the three kind 1 notes in time order. The refusals are
`FR_FIELD` from `filter_read`, `FR_TYPE` and `FR_FIELD` from
`filter_read_search`, `N50_TERMS` from `n50_parse`, and the out-of-date
index from `n50_query`.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/search.coax](../examples/checks/search.coax), which
loads the example, so running the example doesn't run them. They build a
nine-note corpus with records that have no signatures: the three notes
above, `Café au lait at the relay` and `CAFÉ CLUB: relay-to-relay sync`
(non-ASCII case), `catwalk: the cat sat` (substrings), a kind 1059 note,
a note with `\n` and `\t` escapes, and a kind 30023 article that mentions
`coax` and `nostr` twice each.

- Ten searches give exactly the IDs and scores of a Python transcription of
  this chapter's rule, written for this chapter and sharing no code with
  `nip50.coax`.
- The same ten searches against a Python transcription of the Rust relay's
  rule: the same set of notes for 8 of them, and for the other two the
  differences in the table above.

<!-- source: nostr/examples/checks/search.coax check -->
```jai
// Expected values from two Python transcriptions written for this chapter.
// They share no code with nip50.coax.
//
// This chapter's rule:
//
//   def words(b): return [bytes(c + 32 if 65 <= c <= 90 else c for c in w)
//                         for w in re.findall(rb'[A-Za-z0-9\x80-\xff]+', b)]
//   def terms(s):    # fields on ASCII whitespace; a field with ':' is an extension
//       out = []
//       for field in re.split(rb'[ \t\n\x0b\x0c\r]+', s.encode()):
//           if field and b':' not in field:
//               out += [w for w in words(field) if w not in out]
//       return out
//   def mine(s):     # every term a whole word; score = sum of counts;
//       ...          # score desc, created desc, id asc; kinds 4, 1059, 21059 never
//
// The Rust relay's rule (crates/nostr/src/domain/filter.rs at 0b401561a0^,
// search_terms, and the replay query in nostr-relay's statements.rs):
//
//   def rust(s):
//       t = [fold(x) for x in s.split() if ':' not in x]   # fold: A-Z only
//       hits = [e for e in C if e.kind not in RUST_EXCLUDED
//               and all(x in fold(e.content) for x in t)]
//       return sorted(hits, key=lambda e: (-e.created, e.id))
//
// over the nine-event corpus below (ids are the first byte, the rest zero).
```

### The library's own checks

`nip50.coax` has 6 checks: word boundaries at every byte class edge,
folding, non-ASCII words, whole-word counts and the key's value; the search
string's terms, extensions, the bytes that split fields, 16 terms and the
17th, and a short buffer; the three notes above with every order, score,
plan, limit and filter field worked by hand; ties by ID, the ciphertext
kinds, a forged index entry (a collision), short output arrays, expiry, an
invalid filter and a stale index; `n50_insert`'s order, shifts and limits,
including that nothing is written past the limit; and short arrays,
content that doesn't fit, empty content and content found among an event
object's keys.

On 2026-11-01 they passed, and `coax mutate coax/lib/nostr/nip50.coax
--root-only` killed 434 of 434 mutants. The first run left 39 survivors.
The fixes were checks (the field separators 8, 9, 13 and 14, a hash
reduced modulo 2^31 - 1, a sentinel past the limit, a two-place shift,
expiry at `NOW` and `NOW + 1`, empty content, a stale chapter 36 index),
the `unread` count, and rewrites: the extension test compares the whole
field with `include:spam` instead of finding the first `:`, and the
rarest-term choice starts from the first term instead of a sentinel range.

`filter_read.coax` gained one check, and `coax mutate
coax/lib/nostr/filter_read.coax --root-only` killed 263 of 263 mutants
after one reordering (the first run left 2 survivors on a range test that
the `search` branch made unreachable).

Chapter 40 changed `store_index.coax` (`nsi_query_limit`). Its root-only
mutation was rerun for this chapter on the file at `f758016c52` in two
shards: 294 of 294 and 294 of 294 killed, 588 of 588 in all, about 40
seconds per shard. `store.coax`, `relay_server.coax` and the other files in
the graph did not change, and their records were not rerun:
[2026-10-29-coax-relay-r1.md](../../docs/verification/2026-10-29-coax-relay-r1.md).

## Summary

- NIP-50 adds `"search"` to filters. `filter_read_search` reads it;
  `filter_read`, which the relay uses today, still refuses it.
- Words are runs of ASCII letters, digits and non-ASCII bytes, with `A`–`Z`
  folded. A field with a `:` is an extension; `include:spam` changes
  nothing here, and the others are ignored.
- The word index holds one entry per distinct word in each note. A search
  reads its rarest term's entries and counts every term exactly.
- A note matches when it holds every term as a whole word. Score is the
  total count; order is score, then newest, then lowest ID; `limit` applies
  last.
- The Rust relay matched substrings and ordered by time. The differences
  are recorded above and checked.

## Dependencies

- [23. Filters](ch023-filters.md)
- [36. Match filters with indexes](ch036-match-filters-with-indexes.md)

## Sources

- [NIP-50](../nips/official/50.md)
- [NIP-01](../nips/official/01.md), for filters and `limit`
- [NIP-04](../nips/official/04.md) and [NIP-59](../nips/official/59.md),
  for the ciphertext kinds
- The Rust relay's search, read with `git show
  0b401561a0^:crates/nostr/src/domain/filter.rs` (`search_terms`,
  `search_matches`) and `0b401561a0^:crates/nostr-relay/src/store/statements.rs`
  (`QUERY_FILTER_SQL`).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
