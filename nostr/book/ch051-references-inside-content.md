# 51. References inside content

Content can mention people and events inline. This chapter finds those
references by byte range, writes the tags they name, and matches each one
to its tag for display without trusting it.

Part VI, Every NIP. Result: NIP-27 `nostr:` references in content, the
deprecated NIP-08 `#[index]` mentions, NIP-30 `:shortcode:` custom emoji
with `emoji` tags, and hashtags with lowercase `t` tags.

## What this chapter teaches

- NIP-27: find `nostr:npub…`, `nostr:nprofile…`, `nostr:note…`,
  `nostr:nevent…` and `nostr:naddr…` in content, with their byte ranges.
- The optional tags those references ask for: `p` for a profile, NIP-18's
  `q` for an event or an address.
- NIP-30: `["emoji", shortcode, url]`, with shortcodes of letters, digits,
  hyphens and underscores. A `:name:` with any other byte is plain text.
- Hashtags: `#word` in the content and `["t", "word"]` in lowercase.
- NIP-08: `#[0]` refers to tag 0, and only when that tag is `e` or `p`.
- Why `nostr:nsec…` in content is refused and never decoded.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip19-core.coax](../../coax/lib/nostr/nip19-core.coax) | `nip21_decode_checked(input, out, overlap)` | Decodes one `nostr:` URI and refuses `nsec` (chapter 16) |
| [nip27.coax](../../coax/lib/nostr/nip27.coax) | `n27_scan(content, refs)` | Every reference in the content, in order, with its byte range |
| [nip27.coax](../../coax/lib/nostr/nip27.coax) | `n27_tags(content, set, out)` | The `p`, `q`, `t` and `emoji` tags the content asks for, each once |
| [nip27.coax](../../coax/lib/nostr/nip27.coax) | `n27_check(e, text, refs)` | Each reference in an event's content with the index of the tag that names it |

`nip27.coax` is new with this chapter. It builds on chapter 16's decoder,
chapter 22's tag reader and chapter 50's tags, and changes no existing
code. It loads `nip18.coax` for NIP-18 and for the unsigned test events its
checks use. Like chapter 50's threads, a reference is a reading of the
content: `n27_check` fetches nothing and verifies no signature, and a
matching tag names the same thing without proving it exists.

## References in content

NIP-27 puts a NIP-21 URI in the middle of the text:

```text
hello nostr:nprofile1qqszclxx9f5haga8sfjjrulaxncvkfekj097t6f3pu65f86rvg49ehqj6f9dh
```

A reader decodes the URI, looks up the profile and shows a name such as
`@mattn` in its place. The NIP makes the matching tags optional. A client
adds them when it wants the mentioned profile notified or the referenced
event to see the mention. NIP-27 names NIP-18's quote tag for that:

```text
["q", "<event-id> or <event-address>", "<relay-url>", "<pubkey-if-a-regular-event>"]
```

The NIP's own example uses a `p` tag for the profile. This chapter writes
`p` for `npub` and `nprofile`, and `q` for `note`, `nevent` and `naddr`.
When it checks an event, it also accepts an `e` tag for an event and an `a`
tag for an address, because older clients wrote those.

Content can contain the same characters for other reasons. A URL can hold
`nostr:` in its path, `#` starts a URL fragment, and a time such as
`12:30:45` holds `:30:`. NIP-27 and NIP-30 don't say how to tell these
apart, so this chapter uses one rule for every form: a reference starts at
the start of the content or after a byte that is not an ASCII letter, digit
or `/`.

<!-- source: coax/lib/nostr/nip27.coax edge -->
```jai
// A reference starts at the start or after a byte that can't end a word or
// a URL path: not an ASCII letter, digit or "/".
n27_edge :: (s:string,i:int)->bool {
  if i==0 {return true;}
  c:=s[i-1];return !n27_alnum(c) && c!='/';
}
```

The four forms each return the end of a reference that starts at `i`, or
`-1`:

<!-- source: coax/lib/nostr/nip27.coax forms -->
```jai
n27_uri_end :: (s:string,i:int)->int {
  scheme:="nostr:";
  for k:0..<scheme.count {if i+k>=s.count || b32_lower(s[i+k])!=scheme[k] {return -1;}}
  j:=i+scheme.count;while j<s.count && n27_alnum(s[j]) {j+=1;}
  return ifx j>i+scheme.count then j else -1;
}
n27_legacy_end :: (s:string,i:int)->int {
  if i+4>s.count || s[i]!='#' || s[i+1]!='[' {return -1;}
  j:=i+2;while j<s.count && j<i+6 && n27_digit(s[j]) {j+=1;}
  if j==i+2 || j>=s.count || s[j]!=']' {return -1;}
  return j+1;
}
n27_hashtag_end :: (s:string,i:int)->int {
  if s[i]!='#' {return -1;}
  j:=i+1;letters:=false;
  while j<s.count && n27_tag_byte(s[j]) {if !n27_digit(s[j]) {letters=true;}j+=1;}
  if !letters || j-i-1>N27_MAX_NAME {return -1;}
  return j;
}
n27_emoji_end :: (s:string,i:int)->int {
  if s[i]!=':' {return -1;}
  j:=i+1;while j<s.count && n27_name_byte(s[j]) {j+=1;}
  if j==i+1 || j-i-1>N27_MAX_NAME || j>=s.count || s[j]!=':' {return -1;}
  return j+1;
}
```

- `nostr:` is matched in any case, then the longest run of ASCII letters
  and digits. Bech32 uses only those, so punctuation after a URI, such as
  a period or a comma, ends it.
- `#[n]` takes one to four digits.
- A hashtag takes letters, digits, `_` and bytes 0x80 and over, so
  `#Teeé` is one hashtag. It must hold a byte that is not a digit, so
  `issue #12` has none, and at most 64 bytes.
- A shortcode takes 1 to 64 letters, digits, `-` and `_` between colons.

NIP-27 defines no code spans, so text between backticks is scanned like
other text.

### Worked example: a two-line note

Take this note of 118 bytes. The first line ends with a newline byte:

```text
Tea with nostr:nprofile1qqszclxx9f5haga8sfjjrulaxncvkfekj097t6f3pu65f86rvg49ehqj6f9dh :soapbox:
Brewing #GreenTea now.
```

Scan it byte by byte from offset 0:

1. Bytes 0 to 8 are `Tea with `. No byte there starts a form: `T`, `w`
   and the letters after them are no `#`, `:` or `nostr:`.
2. Byte 9 follows a space, so the edge rule holds. Bytes 9 to 14 are
   `nostr:`. The run of letters and digits after it is the 70 bytes of
   `nprofile1qqsz…ehqj6f9dh`, bytes 15 to 84. It stops at the space at
   byte 85. The reference is bytes [9, 85), 6 + 70 = 76 bytes.
3. Chapter 16's decoder reads `nprofile` with one TLV field: type 0,
   length 32, the pubkey `2c7cc62a…622a5cdc`. It has no relay field.
4. Byte 86 is `:` after a space. The name bytes are `soapbox`, bytes 87 to
   93, and byte 94 is `:`. The shortcode is bytes [86, 95).
5. Byte 95 is the newline. Bytes 96 to 103 are `Brewing `.
6. Byte 104 is `#` after a space. `GreenTea` is bytes 105 to 112, all
   letters, and byte 113 is a space. The hashtag is bytes [104, 113).
7. Bytes 113 to 117 are ` now.`. The `.` follows a letter, and nothing
   else starts a form. The scan ends at byte 118 with three references.

With an emoji set that has `soapbox`, the tags are, in the order of first
mention:

```text
[["p","2c7cc62a697ea3a7826521f3fd34f0cb273693cbe5e9310f35449f43622a5cdc"],
 ["emoji","soapbox","https://gleasonator.com/emoji/Gleasonator/soapbox.png"],
 ["t","greentea"]]
```

The `t` tag holds `greentea`, the hashtag's name in lowercase. The example
prints the same ranges and tags.

## Decoding a `nostr:` reference

`n27_scan` fills one record per reference:

<!-- source: coax/lib/nostr/nip27.coax record -->
```jai
N27Ref :: struct {
  type:int; from:int; to:int;            // the bytes content[from..<to]
  status:int;                            // a URI's decoding: ok, nsec or bech32
  entity:u32;                            // N19_NPUB .. N19_ADDR
  key:[32]u8;                            // the pubkey, the event id, or naddr's author
  has_author:bool; author:[32]u8;        // nevent's author
  kind:int=-1;                           // nevent's or naddr's kind
  d:[256]u8; d_n:int;                    // naddr's identifier
  relay:[NT_MAX_VALUE]u8; relay_n:int;   // the first ws:// or wss:// hint
  index:int;                             // #[index]
  tag_at:int=-1;                         // the matching tag, set by n27_check
}
```

A URI goes to chapter 16's `nip21_decode_checked`, which checks the
checksum, the padding and every TLV field. `n27_decode` then copies what a
tag needs: the pubkey or event id, an `nevent`'s author, a kind, an
`naddr`'s identifier and author, and the first relay hint that is a
`ws://` or `wss://` URL. Chapter 22's tag reader refuses any other relay in
a tag, so `n27_decode` leaves those hints out.

NIP-21 forbids `nsec` in a URI. `nip21_decode_checked` refuses it from its
prefix before it decodes a byte, and `n27_decode` then records the status
`nsec` instead of `bech32`. No secret key byte reaches the record or the
output. A URI with a bad checksum, an unknown prefix such as `nrelay`, or
malformed fields gets `bech32`.

## Scanning

<!-- source: coax/lib/nostr/nip27.coax scan -->
```jai
// Every reference in content, in order, with its byte range.
n27_scan :: (content:string,refs:[]N27Ref)->N27Scan {
  r:N27Scan;i:=0;
  while i<content.count {
    type:=N27_URI;to:= -1;
    if n27_edge(content,i) {
      to=n27_uri_end(content,i);
      if to== -1 {to=n27_legacy_end(content,i);type=N27_LEGACY;}
      if to== -1 {to=n27_hashtag_end(content,i);type=N27_HASHTAG;}
      if to== -1 {to=n27_emoji_end(content,i);type=N27_EMOJI;}
    }
    if to== -1 {i+=1;continue;}
    if r.n==refs.count {r.status=N27_LIMIT;return r;}
    ref:N27Ref;ref.type=type;ref.from=i;ref.to=to;
    if type==N27_URI {ref=n27_decode(ref,content[i..<to]);}
    if type==N27_LEGACY {for k:i+2..<to-1 {ref.index=ref.index*10+cast(int)(content[k]-'0');}}
    refs[r.n]=ref;r.n+=1;i=to;
  }
  return r;
}
```

At each byte where the edge rule holds, the forms are tried in order:
`nostr:`, `#[n]`, hashtag, shortcode. A reference ends where its form ends,
and the scan goes on from there. A content with more references than the
`refs` array holds is refused with `limit`; `n27_tags` and the example use
32.

## Custom emoji

NIP-30 adds `["emoji", shortcode, image-url, set-address]` tags to kinds 0,
1, 7, 1111 and 30315. A client replaces each `:shortcode:` that has a tag
with the image. The shortcode "MUST be comprised of only alphanumeric
characters, hyphens, and underscores." The chapter's earlier draft left out
the hyphen; the code follows the NIP.

A shortcode with any other byte is no shortcode. `:bad.name:` has a period,
so `n27_emoji_end` stops at it and finds no closing colon, and the scan
reports nothing. A reader then shows the text as it is, even when the event
has a `["emoji", "bad.name", url]` tag. `n27_tags` refuses an emoji set
entry whose shortcode has such a byte, or whose URL is empty, with
`shortcode`.

A `:name:` without an `emoji` tag is text too, so a stray `:30:` never
becomes an image.
The fourth value, a kind 30030 emoji set's address, is kept in the signed
tags and not read.

## NIP-08 mentions

NIP-08, deprecated in favor of NIP-27, wrote `#[index]` for the tag at that
index. A reader may replace it only when the tag exists and is an `e` or
`p` tag; otherwise it MUST show the text. `n27_check` reads the tag with
chapter 22's `nostr_tag` and sets `tag_at` only in that case. `n27_tags`
writes nothing for `#[n]`: new content uses `nostr:` URIs.

## Writing the tags

`n27_want` says which tag a reference asks for and its first value:

<!-- source: coax/lib/nostr/nip27.coax want -->
```jai
// The tag a reference asks for and its first value, written into value.
n27_want :: (content:string,ref:N27Ref,value:[]u8)->N27Want {
  w:N27Want;
  if ref.type==N27_URI && ref.status==N27_OK {
    if ref.entity==N19_NPUB || ref.entity==N19_PROFILE {w.name="p";w.n=n27_hex(value,0,ref.key);}
    else if ref.entity==N19_ADDR {
      w.name="q";w.alt="a";
      n:=json_write_int(value,0,ref.kind);n=json_raw(value,n,":");n=n27_hex(value,n,ref.key);n=json_raw(value,n,":");
      own:=ref;w.n=json_raw(value,n,cast(string)own.d[0..<own.d_n]);
    } else {w.name="q";w.alt="e";w.n=n27_hex(value,0,ref.key);}
  } else if ref.type==N27_EMOJI {
    w.name="emoji";w.n=json_raw(value,0,content[ref.from+1..<ref.to-1]);
  } else if ref.type==N27_HASHTAG {
    w.name="t";w.n=json_raw(value,0,content[ref.from+1..<ref.to]);
    for k:0..<w.n {value[k]=b32_lower(value[k]);}
  }
  return w;
}
```

An address's value is `kind:pubkey:identifier`, as NIP-01 writes it.
`n27_tags` writes each tag once, at the reference that first asks for it:

<!-- source: coax/lib/nostr/nip27.coax tags -->
```jai
// The tags a new event's content asks for, in the order of first mention,
// each once: p for a profile, q for an event or an address (NIP-18), t for
// a hashtag in lowercase, emoji for a shortcode the set has. #[n] adds
// nothing. nsec or a malformed URI refuses, at the reference's index.
n27_tags :: (content:string,set:[]N27Emoji,out:[]u8)->N27Tags {
  r:N27Tags;
  for k:0..<set.count {
    if !n27_shortcode_ok(set[k].shortcode) || set[k].url.count==0 {r.status=N27_SHORTCODE;r.at=k;return r;}
  }
  refs:[N27_MAX_REFS]N27Ref;s:=n27_scan(content,refs[..]);
  if s.status!=N27_OK {r.status=s.status;return r;}
  for k:0..<s.n {if refs[k].type==N27_URI && refs[k].status!=N27_OK {r.status=refs[k].status;r.at=k;return r;}}
  n:=json_raw(out,0,"[");
  for k:0..<s.n {
    value:[N27_MAX_VALUE]u8;w:=n27_want(content,refs[k],value[..]);
    v:=cast(string)value[0..<w.n];url:= -1;
    if w.name=="emoji" {url=n27_emoji_url(set,v);if url<0 {continue;}}
    if w.name.count==0 || n27_seen(content,refs[..],k,w,v) {continue;}
    if r.tags>0 {n=json_raw(out,n,",");}
    n=json_raw(out,n,"[");n=json_write_string(out,n,w.name);n=json_raw(out,n,",");n=json_write_string(out,n,v);
    ref:=refs[k];
    if url>=0 {n=json_raw(out,n,",");n=json_write_string(out,n,set[url].url);}
    if w.name!="t" && (ref.relay_n!=0 || ref.has_author) {
      n=json_raw(out,n,",");n=json_write_string(out,n,cast(string)ref.relay[0..<ref.relay_n]);
    }
    if ref.has_author {n=json_raw(out,n,",\"");n=n27_hex(out,n,ref.author);n=json_raw(out,n,"\"");}
    n=json_raw(out,n,"]");r.tags+=1;
  }
  n=json_raw(out,n,"]");
  if n== -1 {r.status=N27_OUTPUT;r.tags=0;return r;}
  r.n=n;return r;
}
```

- A profile mentioned as `npub` and again as `nprofile` gets one `p` tag.
- `#Tea` and `#tea` get one `["t", "tea"]`. Only ASCII letters are
  lowercased; other bytes are kept as written.
- A `q` tag carries the relay hint, and an `nevent`'s author as the fourth
  value. With an author and no relay, the relay's place holds `""`.
- A shortcode not in the set gets no tag; it stays text.
- An `nsec` or malformed URI refuses the whole list with its status and
  the reference's index. A client must not publish a secret key.
- A buffer too small for the list refuses with `output`.

## Checking an event

`n27_find` looks for the first tag with the wanted name, or the other name
it accepts, and the wanted first value. An `emoji` tag also needs a URL:

<!-- source: coax/lib/nostr/nip27.coax find -->
```jai
// The index of the first tag named name (or alt) whose first value is value;
// an emoji tag also needs a URL. -1 when there is none.
n27_find :: (e:NostrEvent,w:N27Want,value:string)->int {
  j:[1]Json;if !nt_start(j[..],e) {return -1;}
  k:=0;tok:=json_next(j[..]);
  while tok==JSON_ARRAY {
    buf:[N27_MAX_VALUE]u8;hit:=false;v:=json_next(j[..]);
    if v==JSON_STRING {
      name:=json_text(j[..]);
      if json_is(name,w.name) || (w.alt.count>0 && json_is(name,w.alt)) {
        v=json_next(j[..]);
        if v==JSON_STRING {
          n:=json_unescape(json_text(j[..]),buf[..]);
          hit=n==value.count && cast(string)buf[0..<n]==value;
          v=json_next(j[..]);
          if w.name=="emoji" {hit=hit && v==JSON_STRING && json_text(j[..]).count>0;}
        }
      }
    }
    if hit {return k;}
    while v==JSON_STRING {v=json_next(j[..]);}
    if v!=JSON_ARRAY_END {return -1;}
    k+=1;tok=json_next(j[..]);
  }
  return -1;
}
```

`n27_check` unescapes the event's content, scans it and finds each
reference's tag:

<!-- source: coax/lib/nostr/nip27.coax check -->
```jai
// The references in e's content, each with the tag that names it: a
// mention without its p, q, e or a tag, a hashtag without its t tag or a
// shortcode without its emoji tag is missing; #[n] matches only an e or p
// tag at index n (NIP-08); an nsec or malformed URI is refused.
n27_check :: (e:NostrEvent,text:[]u8,refs:[]N27Ref)->N27Check {
  c:N27Check;
  if e.error!=NE_OK || e.content_from<0 || e.content_to>e.original.count || e.content_from>e.content_to {return c;}
  n:=json_unescape(e.original[e.content_from..<e.content_to],text);
  if n<0 {c.status=N27_LIMIT;return c;}
  content:=cast(string)text[0..<n];
  s:=n27_scan(content,refs);c.status=s.status;c.n=s.n;
  if s.status!=N27_OK {return c;}
  for k:0..<s.n {
    ref:=refs[k];
    if ref.type==N27_URI && ref.status!=N27_OK {c.refused+=1;continue;}
    if ref.type==N27_LEGACY {
      t:=nostr_tag(e,ref.index);
      if t.error==NT_OK && (t.type==NT_E || t.type==NT_P) {refs[k].tag_at=ref.index;}
    } else {
      value:[N27_MAX_VALUE]u8;w:=n27_want(content,ref,value[..]);
      refs[k].tag_at=n27_find(e,w,cast(string)value[0..<w.n]);
    }
    if refs[k].tag_at>=0 {c.matched+=1;} else {c.missing+=1;}
  }
  return c;
}
```

A reference with no tag is `missing`. That is not an error: NIP-27 makes
the tags optional. For display, it means a mention that didn't notify
anyone, or a shortcode or `#[n]` that stays text. An `nsec` or malformed
URI is counted as `refused` and shown as text.

## What is not built

- Kind 30030 emoji sets themselves, NIP-51 lists of `emoji` tags. An emoji
  tag's set address is kept and not read.
- Kind 0 emojification of the `name` and `about` fields inside the
  profile's JSON. `n27_scan` takes any text, so a client can pass it those
  fields.
- Writing `e` or `a` mention tags. `n27_tags` writes NIP-18's `q`;
  `n27_check` accepts both.
- Relay hints after the first, and Unicode lowercasing of hashtags.

## The example

The example is [nostr/examples/references.coax](../examples/references.coax).
It scans the worked note, writes its tags, signs it with BIP-340 test vector
0's key, and checks it. It then checks the pinned NIP-27 and NIP-30
examples, the worked note without tags, an invalid shortcode, NIP-08
mentions and text that looks like references, and shows the refusals.
The `shown:` lines replace each reference that has a tag: a profile with
`@` and its first 4 bytes, a shortcode with `<img name>` and a hashtag with
`<a #name>`.

<!-- source: nostr/examples/references.coax example -->
```jai
#load "../../coax/lib/nostr/nip27.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

STATUS :: [7] string.["ok", "event", "nsec", "bech32", "limit", "shortcode", "output"];
TYPE :: [5] string.["", "nostr", "#[n]", "hashtag", "emoji"];
ENTITY :: [7] string.["", "npub", "nsec", "note", "nprofile", "nevent", "naddr"];

// Print the first 4 bytes of a key or id in hex.
show_short :: (bytes: [32] u8) {
  own := bytes; text: [8] u8;
  r := hex_encode_checked(cast(string) own[0..<4], text[..], false);
  #assert r.error == NC_OK;
  print("%", cast(string) text[..]);
}
// Print each reference: its byte range, form and text, what a URI decodes
// to, and, after n27_check, the index of the tag that names it.
show_refs :: (content: string, refs: [] N27Ref, n: int, checked: bool) {
  for k: 0..<n {
    r := refs[k];
    print("  [%, %) % \"%\"", r.from, r.to, TYPE[r.type], content[r.from..<r.to]);
    if r.type == N27_URI {
      if r.status != N27_OK { print(", refused: %", STATUS[r.status]); }
      else { print(", % ", ENTITY[r.entity]); show_short(r.key); print("..."); }
      if r.relay_n > 0 { print(", relay %", cast(string) r.relay[0..<r.relay_n]); }
    }
    if r.type == N27_LEGACY { print(", index %", r.index); }
    if checked { if r.tag_at >= 0 { print(", tag %", r.tag_at); } else { print(", no tag"); } }
    print("\n");
  }
}
// The content as a reader shows it: a reference with its tag becomes a
// link, an image or a mention; one without stays the text it is.
render :: (content: string, refs: [] N27Ref, n: int) {
  print("  shown: ");
  at := 0;
  for k: 0..<n {
    r := refs[k];
    if r.tag_at < 0 { continue; }
    print("%", content[at..<r.from]);
    if r.type == N27_URI && (r.entity == N19_NPUB || r.entity == N19_PROFILE) { print("@"); show_short(r.key); }
    else if r.type == N27_URI { print("[% ", ENTITY[r.entity]); show_short(r.key); print("]"); }
    else if r.type == N27_LEGACY { print("[tag %]", r.index); }
    else if r.type == N27_EMOJI { print("<img %>", content[r.from + 1..<r.to - 1]); }
    else { print("<a %>", content[r.from..<r.to]); }
    at = r.to;
  }
  print("%\n", content[at..<content.count]);
}
// Read e's references, match them to its tags, print them and the result.
show_check :: (label: string, e: NostrEvent) {
  text: [2048] u8; refs: [N27_MAX_REFS] N27Ref;
  c := n27_check(e, text[..], refs[..]);
  print("%: %, % references, % matched, % missing, % refused\n", label, STATUS[c.status], c.n, c.matched, c.missing, c.refused);
  if c.status != N27_OK { return; }
  n := json_unescape(e.original[e.content_from..<e.content_to], text[..]);
  content := cast(string) text[0..<n];
  show_refs(content, refs[..], c.n, true);
  render(content, refs[..], c.n);
}
// The tags n27_tags writes for content, or its refusal.
build :: (label: string, content: string, set: [] N27Emoji, out: [] u8) -> string {
  t := n27_tags(content, set, out);
  print("%: %", label, STATUS[t.status]);
  if t.status != N27_OK { if t.at >= 0 { print(" at %", t.at); } print("\n"); return ""; }
  print(", % tags\n  %\n", t.tags, cast(string) out[0..<t.n]);
  return cast(string) out[0..<t.n];
}

// Sign a kind 1 note into out and read it back verified.
sign_note :: (key: [] u8, created_at: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  aux: [32] u8; work: [1] NostrSignWork; scratch: [4096] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key, aux[..], created_at, 1, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [4096] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified;
  return e;
}

// The worked example: two lines, a profile, a shortcode and a hashtag.
WORKED :: "Tea with nostr:nprofile1qqszclxx9f5haga8sfjjrulaxncvkfekj097t6f3pu65f86rvg49ehqj6f9dh :soapbox:\nBrewing #GreenTea now.";
SOAPBOX :: "https://gleasonator.com/emoji/Gleasonator/soapbox.png";
// The NIP-27 example event exactly as the NIP prints it.
NIP27_EVENT :: "{\"content\":\"hello nostr:nprofile1qqszclxx9f5haga8sfjjrulaxncvkfekj097t6f3pu65f86rvg49ehqj6f9dh\",\"created_at\":1679790774,\"id\":\"f39e9b451a73d62abc5016cffdd294b1a904e2f34536a208874fe5e22bbd47cf\",\"kind\":1,\"pubkey\":\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"sig\":\"f8c8bab1b90cc3d2ae1ad999e6af8af449ad8bb4edf64807386493163e29162b5852a796a8f474d6b1001cddbaac0de4392838574f5366f03cc94cf5dfb43f4d\",\"tags\":[[\"p\",\"2c7cc62a697ea3a7826521f3fd34f0cb273693cbe5e9310f35449f43622a5cdc\"]]}";
// The NIP-30 kind 1 example's tags and content.
NIP30_TAGS :: "[[\"emoji\",\"ablobcatrainbow\",\"https://gleasonator.com/emoji/blobcat/ablobcatrainbow.png\",\"30030:79c2cae114ea28a981e7559b4fe7854a473521a8d22a66bbab9fa248eb820ff6:blobcats\"],[\"emoji\",\"disputed\",\"https://gleasonator.com/emoji/Fun/disputed.png\"],[\"emoji\",\"gleasonator\",\"https://gleasonator.com/emoji/Gleasonator/gleasonator.png\"]]";
NIP30_CONTENT :: "Hello :gleasonator: 😂 :ablobcatrainbow: :disputed: yolo";
// BIP-340 test vector 0's published secret key. Never sign with a real key
// in an example.
ALICE :: "0000000000000000000000000000000000000000000000000000000000000003";
T0 :: 1761686400;

main :: () {
  refs: [N27_MAX_REFS] N27Ref;
  print("the worked note, % bytes\n", WORKED.count);
  s := n27_scan(WORKED, refs[..]);
  print("scan: %, % references\n", STATUS[s.status], s.n);
  show_refs(WORKED, refs[..], s.n, false);

  set := [1] N27Emoji.[N27Emoji.{shortcode = "soapbox", url = SOAPBOX}];
  buf: [2048] u8;
  tags := build("its tags", WORKED, set[..], buf[..]);
  alice: [32] u8; r := hex_decode_checked(ALICE, alice[..], false); #assert r.error == NC_OK;
  note: [4096] u8;
  e := sign_note(alice[..], T0, tags, WORKED, note[..]);
  id: [64] u8; own := e.id; h := hex_encode_checked(cast(string) own[..], id[..], false);
  print("signed: id %, verified %\n", cast(string) id[..], e.verified);
  show_check("read back", e);

  print("the pinned examples\n");
  scratch: [4096] u8;
  nip27 := nostr_event_read(NIP27_EVENT, scratch[..], false);
  print("NIP-27: published id "); show_short(nip27.id); print("..., recomputed "); show_short(nip27.computed_id); print("...\n");
  // The published id is not the hash of these fields, so the event is
  // parsed without the id check: a reading for display needs no signature.
  show_check("NIP-27", ne_parse(NIP27_EVENT));
  ev: [4096] u8;
  show_check("NIP-30 kind 1", n18_event(1, NIP30_TAGS, NIP30_CONTENT, ev[..]));

  print("missing tags\n");
  show_check("the worked note, no tags", n18_event(1, "[]", "Tea with nostr:nprofile1qqszclxx9f5haga8sfjjrulaxncvkfekj097t6f3pu65f86rvg49ehqj6f9dh :soapbox:\\nBrewing #GreenTea now.", ev[..]));
  show_check("an invalid shortcode", n18_event(1, "[[\"emoji\",\"bad.name\",\"https://e.example/b.png\"],[\"emoji\",\"good\",\"https://e.example/g.png\"]]", ":bad.name: and :good:", ev[..]));
  show_check("NIP-08 #[n]", n18_event(1, "[[\"p\",\"2c7cc62a697ea3a7826521f3fd34f0cb273693cbe5e9310f35449f43622a5cdc\"],[\"t\",\"tea\"]]", "#[0] likes #[1]", ev[..]));
  show_check("URLs, times and words", n18_event(1, "[]", "https://njump.me/nostr:npub1937vv2nf06360qn9y8el6d8sevnndy7tuh5nzre4gj05xc32tnwqauhaj6 at 12:30:45, page#part, issue #12", ev[..]));

  print("refusals\n");
  show_check("an nsec in content", n18_event(1, "[]", "my key nostr:nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5", ev[..]));
  b1 := build("tags for an nsec", "#tea and nostr:nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5", set[..], buf[..]);
  b2 := build("tags for a bad checksum", "nostr:npub1937vv2nf06360qn9y8el6d8sevnndy7tuh5nzre4gj05xc32tnwqauhaj7", set[..], buf[..]);
  bad := [1] N27Emoji.[N27Emoji.{shortcode = "bad.name", url = "https://e.example/b.png"}];
  b3 := build("tags with shortcode bad.name", ":bad.name:", bad[..], buf[..]);
  out: [64] u8;
  t := n27_tags(WORKED, set[..], out[..]);
  print("tags into 64 bytes: %\n", STATUS[t.status]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/references.coax --run-steps 250000000
```

It prints:

```text
the worked note, 118 bytes
scan: ok, 3 references
  [9, 85) nostr "nostr:nprofile1qqszclxx9f5haga8sfjjrulaxncvkfekj097t6f3pu65f86rvg49ehqj6f9dh", nprofile 2c7cc62a...
  [86, 95) emoji ":soapbox:"
  [104, 113) hashtag "#GreenTea"
its tags: ok, 3 tags
  [["p","2c7cc62a697ea3a7826521f3fd34f0cb273693cbe5e9310f35449f43622a5cdc"],["emoji","soapbox","https://gleasonator.com/emoji/Gleasonator/soapbox.png"],["t","greentea"]]
signed: id 9eaf79f6302af203371f8cf748e1b99dfe1175da232a535acb67e9c3a1bbf268, verified true
read back: ok, 3 references, 3 matched, 0 missing, 0 refused
  [9, 85) nostr "nostr:nprofile1qqszclxx9f5haga8sfjjrulaxncvkfekj097t6f3pu65f86rvg49ehqj6f9dh", nprofile 2c7cc62a..., tag 0
  [86, 95) emoji ":soapbox:", tag 1
  [104, 113) hashtag "#GreenTea", tag 2
  shown: Tea with @2c7cc62a <img soapbox>
Brewing <a #GreenTea> now.
the pinned examples
NIP-27: published id f39e9b45..., recomputed 8bf8afab...
NIP-27: ok, 1 references, 1 matched, 0 missing, 0 refused
  [6, 82) nostr "nostr:nprofile1qqszclxx9f5haga8sfjjrulaxncvkfekj097t6f3pu65f86rvg49ehqj6f9dh", nprofile 2c7cc62a..., tag 0
  shown: hello @2c7cc62a
NIP-30 kind 1: ok, 3 references, 3 matched, 0 missing, 0 refused
  [6, 19) emoji ":gleasonator:", tag 2
  [25, 42) emoji ":ablobcatrainbow:", tag 0
  [43, 53) emoji ":disputed:", tag 1
  shown: Hello <img gleasonator> 😂 <img ablobcatrainbow> <img disputed> yolo
missing tags
the worked note, no tags: ok, 3 references, 0 matched, 3 missing, 0 refused
  [9, 85) nostr "nostr:nprofile1qqszclxx9f5haga8sfjjrulaxncvkfekj097t6f3pu65f86rvg49ehqj6f9dh", nprofile 2c7cc62a..., no tag
  [86, 95) emoji ":soapbox:", no tag
  [104, 113) hashtag "#GreenTea", no tag
  shown: Tea with nostr:nprofile1qqszclxx9f5haga8sfjjrulaxncvkfekj097t6f3pu65f86rvg49ehqj6f9dh :soapbox:
Brewing #GreenTea now.
an invalid shortcode: ok, 1 references, 1 matched, 0 missing, 0 refused
  [15, 21) emoji ":good:", tag 1
  shown: :bad.name: and <img good>
NIP-08 #[n]: ok, 2 references, 1 matched, 1 missing, 0 refused
  [0, 4) #[n] "#[0]", index 0, tag 0
  [11, 15) #[n] "#[1]", index 1, no tag
  shown: [tag 0] likes #[1]
URLs, times and words: ok, 0 references, 0 matched, 0 missing, 0 refused
  shown: https://njump.me/nostr:npub1937vv2nf06360qn9y8el6d8sevnndy7tuh5nzre4gj05xc32tnwqauhaj6 at 12:30:45, page#part, issue #12
refusals
an nsec in content: ok, 1 references, 0 matched, 0 missing, 1 refused
  [7, 76) nostr "nostr:nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5", refused: nsec, no tag
  shown: my key nostr:nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5
tags for an nsec: nsec at 1
tags for a bad checksum: bech32 at 0
tags with shortcode bad.name: shortcode at 0
tags into 64 bytes: output
```

The ranges and tags of the worked note are the ones worked out by hand.
The signed note reads back with all three references matched to tags 0, 1
and 2.

The NIP-27 example's `id` is not the SHA-256 of its fields: the published
id starts `f39e9b45` and the recomputed one starts `8bf8afab`, and its
pubkey is the generator point's. It is an illustration, so the example
parses it without the id check. Its one reference, bytes [6, 82), matches
its `p` tag. NIP-30's three shortcodes sit around a 4-byte emoji, so the
second starts at byte 25, not 22, and each matches its `emoji` tag.

The refusals and the text left alone:

- The worked note without tags: all three references are `missing` and
  the content is shown as it is.
- `:bad.name:` has a period, so it is no shortcode and stays text, though
  the event has an `emoji` tag for it. `:good:` is replaced.
- `#[0]` points to a `p` tag and is replaced; `#[1]` points to a `t` tag
  and stays text, as NIP-08 requires.
- The URL path, the time, the fragment and `issue #12` hold no reference.
- An `nsec` URI in content is `refused: nsec` and never decoded. Building
  tags for content with one refuses with `nsec at 1`, the index of the
  reference after `#tea`.
- A changed last character breaks the checksum: `bech32 at 0`.
- An emoji set entry `bad.name`: `shortcode at 0`.
- 64 bytes is too small for the worked note's tags: `output`.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/references.out](../fixtures/references.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/references.coax](../examples/checks/references.coax),
which loads the example, so running the example doesn't run them. They
compare the code with a Python script that shares no code with it. The
script has its own BIP-173 Bech32 decoder written from the BIP, one regular
expression for the edge rule and the four forms, and a tag builder written
from NIP-27, NIP-18 and NIP-30:

- The byte ranges of the worked note, the NIP-27 example, the NIP-30
  example and a note with an `nprofile` with two relays, the NIP-21
  `nevent` and `naddr` examples, `#Nostr` and `#nostr`.
- The tags of the worked note and of that mixed note, byte for byte.
- The worked note's id, `hashlib.sha256` of NIP-01's serialization with
  Python's tags. The check rebuilds the note unsigned with that id,
  recomputes it and checks the note.
- The NIP-27 example's own `p` tag, which is the pubkey its `nprofile`
  decodes to.

<!-- source: nostr/examples/checks/references.coax check -->
```jai
// Byte ranges, tags and an id from a Python script written from NIP-19
// (its own BIP-173 bech32 decoder), NIP-27, NIP-18, NIP-30 and the chapter's
// edge rule; it shares no code with Coax. The scanner is one regular
// expression, (?<![A-Za-z0-9/]) then nostr:[A-Za-z0-9]+, #\[[0-9]{1,4}\],
// #[A-Za-z0-9_\x80-\xff]+ (not all digits, at most 64) or :[A-Za-z0-9_-]+:
// (at most 64), over the UTF-8 bytes. The id is hashlib.sha256 of
// json.dumps([0, pubkey, created_at, 1, tags, content],
// separators=(",", ":"), ensure_ascii=False).
PY_WORKED :: [9] int.[N27_URI, 9, 85, N27_EMOJI, 86, 95, N27_HASHTAG, 104, 113];
PY_NIP27 :: [3] int.[N27_URI, 6, 82];
PY_NIP30 :: [9] int.[N27_EMOJI, 6, 19, N27_EMOJI, 25, 42, N27_EMOJI, 43, 53];
PY_MIX_RANGES :: [15] int.[N27_URI, 3, 140, N27_URI, 146, 317, N27_URI, 322, 451, N27_HASHTAG, 452, 458, N27_HASHTAG, 459, 465];
PY_WORKED_TAGS :: "[[\"p\",\"2c7cc62a697ea3a7826521f3fd34f0cb273693cbe5e9310f35449f43622a5cdc\"],[\"emoji\",\"soapbox\",\"https://gleasonator.com/emoji/Gleasonator/soapbox.png\"],[\"t\",\"greentea\"]]";
PY_WORKED_ID :: "9eaf79f6302af203371f8cf748e1b99dfe1175da232a535acb67e9c3a1bbf268";
MIX :: "Hi nostr:nprofile1qqsrhuxx8l9ex335q7he0f09aej04zpazpl0ne2cgukyawd24mayt8gpp4mhxue69uhhytnc9e3k7mgpz4mhxue69uhkg6nzv9ejuumpv34kytnrdaksjlyr9p, see nostr:nevent1qqstna2yrezu5wghjvswqqculvvwxsrcvu7uc0f78gan4xqhvz49d9spr3mhxue69uhkummnw3ez6un9d3shjtn4de6x2argwghx6egpr4mhxue69uhkummnw3ez6ur4vgh8wetvd3hhyer9wghxuet5nxnepm and nostr:naddr1qqyrzwrxvc6ngvfkqyghwumn8ghj7enfv96x5ctx9e3k7mgzyqalp33lewf5vdq847t6te0wvnags0gs0mu72kz8938tn24wlfze6qcyqqq823cph95ag #Nostr #nostr";
PY_MIX_TAGS :: "[[\"p\",\"3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d\",\"wss://r.x.com\"],[\"q\",\"b9f5441e45ca39179320e0031cfb18e34078673dcc3d3e3a3b3a981760aa5696\",\"wss://nostr-relay.untethr.me\"],[\"q\",\"30023:3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d:18ff5416\",\"wss://fiatjaf.com\"],[\"t\",\"nostr\"]]";
// BIP-340 test vector 0's published public key, the worked note's author.
PY_PUB :: "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";

// want holds form, from and to for each reference in turn.
same_ranges :: (content: string, want: [] int) -> bool {
  refs: [N27_MAX_REFS] N27Ref;
  s := n27_scan(content, refs[..]);
  if s.status != N27_OK || s.n * 3 != want.count { return false; }
  for k: 0..<s.n {
    if refs[k].type != want[3 * k] || refs[k].from != want[3 * k + 1] || refs[k].to != want[3 * k + 2] { return false; }
  }
  return true;
}
same_tags :: (content: string, set: [] N27Emoji, want: string) -> bool {
  out: [2048] u8; t := n27_tags(content, set, out[..]);
  return t.status == N27_OK && cast(string) out[0..<t.n] == want;
}

#test "every reference's byte range is Python's" {
  worked := PY_WORKED; nip27 := PY_NIP27; nip30 := PY_NIP30; mix := PY_MIX_RANGES;
  #assert same_ranges(WORKED, worked[..]);
  #assert same_ranges("hello nostr:nprofile1qqszclxx9f5haga8sfjjrulaxncvkfekj097t6f3pu65f86rvg49ehqj6f9dh", nip27[..]);
  #assert same_ranges(NIP30_CONTENT, nip30[..]);
  #assert same_ranges(MIX, mix[..]);
}
#test "the tags are Python's, and so is the worked note's id" {
  set := [1] N27Emoji.[N27Emoji.{shortcode = "soapbox", url = SOAPBOX}];
  none: [1] N27Emoji;
  #assert same_tags(WORKED, set[..], PY_WORKED_TAGS);
  #assert same_tags(MIX, none[0..<0], PY_MIX_TAGS);
  text: [2048] u8; scratch: [4096] u8; out: [2048] u8;
  n := json_raw(text[..], 0, "{\"id\":\"");
  n = json_raw(text[..], n, PY_WORKED_ID);
  n = json_raw(text[..], n, "\",\"pubkey\":\"");
  n = json_raw(text[..], n, PY_PUB);
  n = json_raw(text[..], n, "\",\"created_at\":");
  n = json_write_int(text[..], n, T0);
  n = json_raw(text[..], n, ",\"kind\":1,\"tags\":");
  n = json_raw(text[..], n, PY_WORKED_TAGS);
  n = json_raw(text[..], n, ",\"content\":");
  n = json_write_string(text[..], n, WORKED);
  n = json_raw(text[..], n, ",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  e := nostr_event_read(cast(string) text[0..<n], out[..], false);
  #assert e.error == NE_OK && ns_equal32(e.computed_id, e.id);
  refs: [N27_MAX_REFS] N27Ref; content: [256] u8;
  c := n27_check(e, content[..], refs[..]);
  #assert c.status == N27_OK && c.matched == 3 && c.missing == 0;
}
#test "the NIP-27 example's own p tag is the pubkey its nprofile decodes to" {
  refs: [N27_MAX_REFS] N27Ref; content: [256] u8;
  c := n27_check(ne_parse(NIP27_EVENT), content[..], refs[..]);
  #assert c.status == N27_OK && c.n == 1 && c.matched == 1 && refs[0].tag_at == 0 && refs[0].entity == N19_PROFILE;
  hex: [64] u8; key := refs[0].key; r := hex_encode_checked(cast(string) key[..], hex[..], false);
  #assert cast(string) hex[..] == "2c7cc62a697ea3a7826521f3fd34f0cb273693cbe5e9310f35449f43622a5cdc";
}
```

```shell
coax/bin/coax test nostr/examples/checks/references.coax --run-steps 250000000
```

On 2026-11-01, `coax test` ran 386 checks, including the loaded files'
own checks, and all passed.

### The library's own checks

`nip27.coax` has its own `#test` blocks: the NIP-27 and NIP-30 examples by
byte range, `nevent` and `naddr` fields, the edge rule against URLs, words,
times, fragments and numbers, the name and digit bounds, `nsec` prefixes,
malformed Bech32, the reference limit, the exact tags with duplicates,
every buffer size short of a whole tag list, each refusal of `n27_tags`,
and `n27_check` with missing tags, wrong tag names, `#[n]` on `e`, `p` and
`t` tags, forged content spans and unreadable tags.

```shell
coax/bin/coax test coax/lib/nostr/nip27.coax
coax/bin/coax mutate coax/lib/nostr/nip27.coax --root-only
```

On 2026-11-01, `coax test` passed all 234 checks (with the loaded files'),
and `coax mutate --root-only` killed all 526 mutants, with none surviving
and none marked equivalent. The file decides display, not money, admission
or policy, so it is not `#authority`. The full mutation run of the loaded
files was not repeated; their records are in their own chapters.

## Summary

- `n27_scan` finds `nostr:` URIs, `#[n]` mentions, hashtags and
  `:shortcode:` emoji with their byte ranges. A reference starts only at
  the start or after a byte that is not a letter, digit or `/`.
- A `nostr:nsec…` reference is refused and never decoded; a malformed one
  is refused as `bech32`.
- A shortcode holds only letters, digits, `-` and `_`. One with any other
  byte is not a shortcode and is never replaced.
- `n27_tags` writes `p`, `q`, lowercase `t` and `emoji` tags, each once,
  and refuses content with an `nsec` or malformed URI.
- `n27_check` matches each reference to the tag that names it, and reports
  the ones without a tag, which stay text.

## Dependencies

- [16. NIP-19 entities and nostr:
  URIs](ch016-nip-19-entities-and-nostr-uris.md)
- [50. Notes, threads and subjects](ch050-notes-threads-and-subjects.md)

## Sources

- [NIP-27](../nips/official/27.md)
- [NIP-08](../nips/official/08.md)
- [NIP-30](../nips/official/30.md)
- [NIP-18](../nips/official/18.md), for `q` tags.
- [NIP-21](../nips/official/21.md), for the `nevent` and `naddr` examples.
- [BIP-173](https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki),
  for the Python check's decoder.
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
  test vector 0, for the example's key.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
