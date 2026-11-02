# 70. Retired and experimental NIPs

Thirteen of the pinned official NIPs open with the same warning:
`unrecommended`, followed by a short reason. A client still meets their
events, so it must recognize them, read the ones it needs to read, and
never write them. This chapter gathers the 13 into one table, builds a
classifier that names the retired forms an event uses, and covers the three
that no other chapter does: NIP-26 delegation, NIP-31 `alt` tags and NIP-BE
Bluetooth LE transport.

Part VI, Every NIP. Result: NIP-26 delegation tokens, the NIP-31 `alt` tag and
NIP-BE Bluetooth LE transport, each marked `unrecommended` in the pinned text,
with what this stack reads and what it never builds.

## What this chapter teaches

- The 13 NIPs the pinned texts mark `unrecommended`, each one's reason as
  written, what it points to instead, and its kinds and tags.
- NIP-26: `["delegation", pubkey, conditions, sig]`; `unrecommended`: "adds
  unnecessary burden for little gain". The token is read and verified, and
  the event is still attributed to its signer.
- NIP-31: `["alt", summary]` for unknown kinds; `unrecommended`
  ("unnecessarily bloated") but common; read for display.
- NIP-BE: a BLE GATT service for device-to-device sync; `unrecommended`:
  "only implemented once". Not built.
- A publishing check that refuses every retired form before signing.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [retired.coax](../../coax/lib/nostr/retired.coax) | `RT_NIP`, `RT_REASON`, `RT_INSTEAD`, `RT_CHAPTER` | The 13 `unrecommended` NIPs |
| [retired.coax](../../coax/lib/nostr/retired.coax) | `rt_kind(kind)` | The row whose kinds include a kind |
| [retired.coax](../../coax/lib/nostr/retired.coax) | `rt_mention(e, content)` | Whether content holds a NIP-08 position `#[i]` |
| [retired.coax](../../coax/lib/nostr/retired.coax) | `rt_forms(e, out)` | The retired forms a received event uses |
| [retired.coax](../../coax/lib/nostr/retired.coax) | `rt_write(kind, tags, content)` | Refuses an event to be signed that uses a retired form |
| [nip26.coax](../../coax/lib/nostr/nip26.coax) | `n26_conditions(text, kind, created_at)` | Reads a delegation's conditions and checks an event against them |
| [nip26.coax](../../coax/lib/nostr/nip26.coax) | `n26_claim(e)` | Reads a delegation tag and verifies its token |
| [nip26.coax](../../coax/lib/nostr/nip26.coax) | `n26_attribute(e)` | The event's author (its signer) and the claim |
| [nip26.coax](../../coax/lib/nostr/nip26.coax) | `n26_delegate(...)` | Refuses: this stack makes no delegations |
| [nip31.coax](../../coax/lib/nostr/nip31.coax) | `n31_display(e, known)` | What a timeline shows for an event: its kind, its `alt` text, or neither |

The three library files are new with this chapter. They read tags through
chapter 22's `nostr_tag` and chapter 63's `nt_tag`, which keeps a tag's first
four values as spans of the event's JSON. `retired.coax` and `nip26.coax`
are `#authority` files: one decides what this stack may sign, the other
whose event a client shows. Every procedure in the three files runs in a
check, and `coax mutate` leaves no survivors in any of them.

## The 13 unrecommended NIPs

The upstream NIP index marks a NIP `unrecommended` by putting this line at
the top of its file:

```text
> __Warning__  `unrecommended`: <reason>
```

Thirteen pinned files under `nostr/nips/official/` carry it. The table
copies each reason as written, with each Markdown link written as its label:

<!-- source: coax/lib/nostr/retired.coax table -->
```jai
RT_NIP :: [RT_N] string.["03", "04", "06", "08", "15", "26", "28", "31", "72", "90", "96", "BE", "EE"];
// The pinned texts' first lines after "`unrecommended`: ", each Markdown
// link written as its label.
RT_REASON :: [RT_N] string.[
  "vulnerable to one specific attack, needs update",
  "deprecated in favor of NIP-17",
  "prefer a single nsec",
  "deprecated in favor of NIP-27",
  "too complicated, try NIP-99 instead",
  "adds unnecessary burden for little gain",
  "try NIP-29 instead",
  "unnecessarily bloated",
  "try NIP-29 instead",
  "this got totally out of control, prefer use-case-specific microstandards",
  "deprecated in favor of NIP-B7",
  "only implemented once and unclear whether it works, requires review",
  "superseded by the Marmot Protocol"];
RT_INSTEAD :: [RT_N] string.["", "NIP-17", "one nsec", "NIP-27", "NIP-99", "", "NIP-29", "", "NIP-29",
  "use-case-specific NIPs", "NIP-B7 (Blossom)", "", "Marmot"];
RT_CHAPTER :: [RT_N] int.[68, 19, 46, 51, 66, 70, 59, 70, 59, 65, 57, 70, 60];
```

What each one is, what marks it in an event, and where the book covers it:

| NIP | What it is | Its form in an event | Chapter |
| --- | --- | --- | --- |
| 03 | OpenTimestamps attestations | kind 1040 | 68 |
| 04 | Encrypted direct messages | kind 4 | 19 (decrypt only) |
| 06 | Keys from a seed phrase | none: a key derivation | 46 |
| 08 | Positional mentions | `#[i]` in content, naming tag `i` | 51 |
| 15 | Marketplace | kinds 1021, 1022, 30017 to 30020 | 66 |
| 26 | Delegated signing | a `delegation` tag | 70 |
| 28 | Public chat channels | kinds 40 to 44 | 59 |
| 31 | Summaries for unknown kinds | an `alt` tag | 70 |
| 72 | Moderated communities | kinds 4550 and 34550 | 59 |
| 90 | Data vending machines | kinds 5000 to 7000 | 65 |
| 96 | HTTP file storage | kind 10096 | 57 |
| BE | Bluetooth LE transport | none: a transport | 70 |
| EE | MLS group messages | kinds 443, 444, 445 and 10051 | 60 |

Four NIPs merged into NIP-01 (12, 16, 20 and 33) say so in their own files
but are not marked `unrecommended`; Part II covers them.

## Recognizing a retired form

`rt_kind` maps a kind to its row. `rt_mention` looks for NIP-08's form: for
each `p` or `e` tag at index `i`, whether the content contains `#[i]`. A
`#[i]` whose tag is something else, or doesn't exist, is ordinary text, and
`#[00]` names no tag. `rt_scan` collects the rows into a list in table
order: the kind's row, NIP-08, a `delegation` tag (NIP-26) and an `alt` tag
(NIP-31). `rt_forms` does this for a received event, decoding its content
first, so an escaped `#[0]` is still `#[0]`.

<!-- source: coax/lib/nostr/retired.coax classify -->
```jai
// The row whose kinds include kind, or RT_ALLOWED.
rt_kind :: (kind:int)->int {
  if kind==1040 {return RT_03;}
  if kind==4 {return RT_04;}
  if kind==1021 || kind==1022 || (kind>=30017 && kind<=30020) {return RT_15;}
  if kind>=40 && kind<=44 {return RT_28;}
  if kind==4550 || kind==34550 {return RT_72;}
  if kind>=5000 && kind<=7000 {return RT_90;}
  if kind==10096 {return RT_96;}
  if (kind>=443 && kind<=445) || kind==10051 {return RT_EE;}
  return RT_ALLOWED;
}
// Whether content holds a NIP-08 position "#[i]" naming a p or e tag.
rt_mention :: (e:NostrEvent,content:string)->bool {
  i:=0;t:=nostr_tag(e,i);
  while t.error!=NT_INDEX && t.error!=NT_EVENT {
    if t.error==NT_OK && (t.type==NT_E || t.type==NT_P) {
      at:[16]u8;n:=json_raw(at[..],0,"#[");n=json_write_int(at[..],n,i);n=json_raw(at[..],n,"]");
      if rt_holds(content,cast(string)at[0..<n]) {return true;}
    }
    i+=1;t=nostr_tag(e,i);
  }
  return false;
}
rt_holds :: (text:string,part:string)->bool {
  for i:0..<text.count-part.count+1 {if text[i..<i+part.count]==part {return true;}}
  return false;
}
// The rows of the retired forms an event with these tags and this decoded
// content uses, into out in table order: their count (at most out.count),
// or RT_EVENT.
rt_scan :: (e:NostrEvent,content:string,out:[]int)->int {
  ev:=nt_event_of(e);
  delegations:=nt_tags(ev,"delegation");
  if delegations<0 {return RT_EVENT;}
  rows:[RT_N]bool;
  kind:=rt_kind(e.kind);if kind!=RT_ALLOWED {rows[kind]=true;}
  rows[RT_08]=rt_mention(e,content);
  rows[RT_26]=delegations>0;rows[RT_31]=nt_tags(ev,"alt")>0;
  n:=0;
  for r:0..<RT_N {if rows[r] && n<out.count {out[n]=r;n+=1;}}
  return n;
}
// The retired forms a received event uses.
rt_forms :: (e:NostrEvent,out:[]int)->int {
  if e.error!=NE_OK {return RT_EVENT;}
  decoded:[65536]u8;
  n:=json_unescape(e.original[e.content_from..<e.content_to],decoded[..]);
  if n<0 {return RT_EVENT;}
  return rt_scan(e,cast(string)decoded[0..<n],out);
}
// Before signing: RT_ALLOWED, the first retired row the event would use,
// or RT_EVENT when the tags don't read.
rt_write :: (kind:int,tags:string,content:string)->int {
  e:=nt_event(tags);e.kind=kind;out:[1]int;
  n:=rt_scan(e,content,out[..]);
  if n<0 {return n;}
  if n==0 {return RT_ALLOWED;}
  return out[0];
}
```

Reading is not writing. `rt_forms` tells a client which chapter can read an
event: chapter 19 decrypts an old kind 4 message, this chapter reports a
delegation claim and shows an `alt` summary. `rt_write` is the check before
signing: an event this stack would publish that uses any retired form
refuses, and the refusal names the row and quotes its reason. NIP-04's own
reader makes the same choice: it decrypts and refuses to encrypt.

## Delegated signing: NIP-26

NIP-26 lets one key (the delegator) authorize another (the delegatee) to
sign events for it. The delegatee adds a tag to each event:

```text
["delegation", <delegator pubkey>, <conditions>, <token>]
```

The token is the delegator's BIP-340 signature of the SHA-256 hash of the
delegation string:

```text
nostr:delegation:<delegatee pubkey>:<conditions>
```

The conditions are clauses joined by `&`: `kind=N` (the delegatee may sign
this kind), `created_at<T` and `created_at>T` (only before or after a
time). NIP-26 asks clients to show such an event "as if it was published
directly by the delegator", and relays to return it for queries by the
delegator's pubkey.

### Why this stack doesn't accept it

The pinned text marks NIP-26 `unrecommended`, and its claim is not the kind
of authority this stack accepts (INVARIANTS.md: Nostr projections cannot
create authority). A delegation can't be revoked: the token stays valid
for every event that meets its conditions, and a key the delegator wants
to stop trusting can still sign under it. Every client and relay must
implement the token check, or the claim is shown unchecked. And the
conditions are only kind and time: nothing limits what the delegatee says.

So `n26_attribute` always names the signer as the author. The claim is
read and checked, and reported next to the author, never in place of it.
A client may show "claims delegation from X"; it must not show the event
as X's, count it under X's pubkey in a filter, or let X delete it.

<!-- source: coax/lib/nostr/nip26.coax record -->
```jai
N26Claim :: struct {
  status:int=N26_EVENT;
  delegator:[32]u8;                       // whom the tag names
  token:[64]u8;                           // the delegator's claimed signature
  conditions:[N26_MAX_CONDITIONS]u8; n:int;
  hash:[32]u8;                            // sha256 of the delegation string
}
// The author is always the signer. The claim is reported, never applied.
N26Attribution :: struct {author:[32]u8; claim:N26Claim;}
```

### The conditions

`n26_conditions` splits the text at `&` and each clause at its first
operator. NIP-26's example `kind=0&kind=1` allows either kind, so `kind`
clauses are alternatives; every `created_at` clause must hold, and both
comparisons are strict. Numbers are plain decimal with at most ten digits,
no sign and no leading zero. An empty query, an empty clause, another
field or another operator refuses: an unlimited delegation is an error
here, not permission for everything.

<!-- source: coax/lib/nostr/nip26.coax conditions -->
```jai
// A decimal number of at most ten digits, no sign and no leading zero, or
// -1.
n26_number :: (s:string)->int {
  if s.count==0 || s.count>10 || (s[0]=='0' && s.count>1) {return -1;}
  v:=0;for c:s {if c<'0' || c>'9' {return -1;}v=v*10+cast(int)(c-'0');}
  return v;
}
// N26_OK when an event of this kind and time meets the conditions,
// N26_OUTSIDE when it doesn't, N26_CONDITIONS when they don't read.
n26_conditions :: (text:string,kind:int,created_at:int)->int {
  has_kind:=false;kind_ok:=false;times_ok:=true;from:=0;
  while from<=text.count {
    i:=from;while i<text.count && text[i]!='&' {i+=1;}
    c:=text[from..<i];from=i+1;
    // The field ends at the first operator.
    p:=0;while p<c.count && c[p]!='=' && c[p]!='<' && c[p]!='>' {p+=1;}
    if p==c.count {return N26_CONDITIONS;}
    field:=c[0..<p];op:=c[p];v:=n26_number(c[p+1..<c.count]);
    if v<0 {return N26_CONDITIONS;}
    if field=="kind" && op=='=' {
      if v>65535 {return N26_CONDITIONS;}
      has_kind=true;if v==kind {kind_ok=true;}
    } else if field=="created_at" && op=='<' {
      if created_at>=v {times_ok=false;}
    } else if field=="created_at" && op=='>' {
      if created_at<=v {times_ok=false;}
    } else {return N26_CONDITIONS;}
  }
  if (has_kind && !kind_ok) || !times_ok {return N26_OUTSIDE;}
  return N26_OK;
}
```

### Reading the claim

`n26_claim` takes the event's one `delegation` tag (none reads as
`N26_NONE`, two refuse), requires exactly three values, decodes the
delegator and the token as lowercase hex, unescapes the conditions and
reads them. Then it builds the delegation string from the event's own
signer, hashes it, and verifies the token with Bitcoin chapter 23's
`nostr_verify_vartime`. A token signed for another delegatee, other
conditions or by another key doesn't verify. Only then are the conditions
applied to the event's kind and `created_at`.

<!-- source: coax/lib/nostr/nip26.coax claim -->
```jai
// "nostr:delegation:<signer hex>:<conditions>" into out: its length, or -1.
n26_string :: (signer:[32]u8,conditions:string,out:[]u8)->int {
  if out.count<N26_PREFIX.count+65+conditions.count {return -1;}
  key:=signer;hex:[64]u8;h:=hex_encode_checked(cast(string)key[..],hex[..],false);
  #assert h.error==NC_OK;
  for i:0..<N26_PREFIX.count {out[i]=N26_PREFIX[i];}
  n:=N26_PREFIX.count;
  for i:0..<64 {out[n+i]=hex[i];}
  n+=64;out[n]=':';n+=1;
  for i:0..<conditions.count {out[n+i]=conditions[i];}
  return n+conditions.count;
}
n26_status :: (c:N26Claim,status:int)->N26Claim {r:=c;r.status=status;return r;}
// What the event's delegation tag claims, checked against its signer, kind
// and created_at. A claim that reads OK is still only a claim.
n26_claim :: (e:NostrEvent)->N26Claim {
  c:N26Claim;ev:=nt_event_of(e);
  count:=nt_tags(ev,"delegation");
  if count<0 {return c;}
  if count==0 {return n26_status(c,N26_NONE);}
  if count>1 {return n26_status(c,N26_REPEATED);}
  t:=nt_tag(ev,"delegation",0);
  if t.values!=3 {return n26_status(c,N26_SHAPE);}
  if !ne_lower_hex(nt_raw(ev,t.v[0]),c.delegator[..]) || !ne_lower_hex(nt_raw(ev,t.v[2]),c.token[..]) {return n26_status(c,N26_HEX);}
  c.n=json_unescape(nt_raw(ev,t.v[1]),c.conditions[..]);
  if c.n<0 {c.n=0;return n26_status(c,N26_LIMIT);}
  conditions:=cast(string)c.conditions[0..<c.n];
  held:=n26_conditions(conditions,e.kind,e.created_at);
  if held==N26_CONDITIONS {return n26_status(c,N26_CONDITIONS);}
  text:[N26_TEXT_MAX]u8;
  n:=n26_string(e.public_key,conditions,text[..]);
  if sha256_hash_checked(text[0..<n],c.hash[..],false)!=SHA_OK {return c;}
  if !nostr_verify_vartime(c.hash[..],c.delegator[..],c.token[..]) {return n26_status(c,N26_TOKEN);}
  return n26_status(c,held);
}
// The event's author is its signer, whatever its tags claim.
n26_attribute :: (e:NostrEvent)->N26Attribution {
  return N26Attribution.{author=e.public_key,claim=n26_claim(e)};
}
```

`n26_claim` does not check the event's own id or signature. The caller
passes an event from `nostr_event_verify_vartime` (chapter 21). There is no
writer: `n26_delegate` clears its output and returns `N26_REFUSED`, as
chapter 19's `nip04_encrypt` does.

## Summaries for unknown kinds: NIP-31

A custom kind that isn't meant to be read as text should carry
`["alt", summary]`, so a client that doesn't know the kind can show
something. The pinned text marks NIP-31 `unrecommended` ("unnecessarily
bloated"), but many clients write it, and a reader that shows a summary
is better than one that shows nothing.

`n31_display` takes the kinds the caller renders itself. A known kind is
shown as itself and its `alt` is ignored. An unknown kind shows its first
`alt` tag's value. Otherwise the client says only that it can't show an
event of that kind. The summary is plain text from the signer, so a value
that is empty, longer than 256 bytes or holds a control byte (below 0x20,
or 0x7f) is not shown: a newline could fake a second line, and an escape
sequence could move a terminal's cursor.

<!-- source: coax/lib/nostr/nip31.coax record -->
```jai
N31Display :: struct {
  show:int=N31_UNKNOWN;
  alt:int=N31_EVENT;                     // how the first alt tag read
  kind:int;
  text:[NT_MAX_VALUE]u8; n:int;          // the summary when show is N31_ALT
  alts:int;                              // alt tags on the event
}
```

<!-- source: coax/lib/nostr/nip31.coax display -->
```jai
n31_known :: (kind:int,known:[]int)->bool {for k:known {if k==kind {return true;}}return false;}
// What a timeline shows for e when it renders the kinds in known itself.
n31_display :: (e:NostrEvent,known:[]int)->N31Display {
  d:N31Display;d.kind=e.kind;ev:=nt_event_of(e);
  d.alts=nt_tags(ev,"alt");
  if d.alts<0 {d.alts=0;return d;}
  if d.alts==0 {d.alt=N31_NONE;}
  else {
    t:=nt_tag(ev,"alt",0);
    n:=ifx t.values==0 then 0 else json_unescape(nt_raw(ev,t.v[0]),d.text[..]);
    if n<0 {d.alt=N31_LIMIT;}
    else if n==0 {d.alt=N31_SHAPE;}
    else {
      d.alt=N31_OK;
      for i:0..<n {if d.text[i]<0x20 || d.text[i]==0x7f {d.alt=N31_CONTROL;}}
    }
    if d.alt==N31_OK {d.n=n;} else {for i:0..<d.text.count {d.text[i]=0;}}
  }
  if n31_known(e.kind,known) {d.show=N31_KIND;d.n=0;}
  else if d.alt==N31_OK {d.show=N31_ALT;}
  return d;
}
```

## Bluetooth LE transport: NIP-BE

NIP-BE carries NIP-01 messages between two nearby devices over Bluetooth
LE: a device advertises service UUID `0000180f-0000-1000-8000-00805f9b34fb`
with its device UUID, the one with the higher UUID becomes the GATT server
(the relay), and each message is DEFLATE-compressed and split into chunks
of at most 20 to 256 bytes, depending on the BLE version. The pinned text
marks it `unrecommended`: "only implemented once and unclear whether it
works, requires review".

This stack doesn't build it. It has no Bluetooth host binding, and a
transport with one implementation has nothing to check against. It also
has no event form, so `rt_forms` never names it. A NIP-BE link would carry
the same NIP-01 messages as chapter 29's relay connection, so if it is
built later, it needs only a host transport under that client.

## Worked example: three events by hand

The example signs three events and classifies them. Work them out first.

**E1** is NIP-26's example event re-signed by its delegatee (the NIP
publishes both secret keys): kind 1, `created_at` 1677426298, content
"Hello, world!", and the example's `delegation` tag, whose conditions are
`kind=1&created_at>1674834236&created_at<1677426236`.

1. Retired forms: kind 1 has no row; the content has no `#[`; there is a
   `delegation` tag and no `alt` tag. So the list is NIP-26 alone.
2. The delegation string is `nostr:delegation:477318cf…691396:` followed
   by the conditions, 132 bytes. Its SHA-256 begins `397b7519`, and the
   delegator's token verifies over it.
3. The conditions: `kind=1` holds. `created_at>1674834236` holds.
   `created_at<1677426236` fails: 1677426298 is 62 seconds later. NIP-26's
   own example event breaks its own delegation, so the claim reads "token
   verifies, event breaks the conditions".
4. The author is the signer, the delegatee, whatever the claim says.

**E2** is Alice's kind 31337 with `d` and
`alt` "Chess opening study: the Ruy Lopez".

1. Retired forms: 31337 has no row, no `#[`, no `delegation`, one `alt`:
   NIP-31.
2. Display: the timeline knows kinds 0, 1, 3, 6 and 7. 31337 is unknown,
   the first `alt` is 34 bytes with no control byte, so the timeline shows
   the summary.

**E3** is Alice's kind 1 with one `p` tag (the delegatee) and content
"thanks #[0]".

1. Retired forms: tag 0 is a `p` tag and the content contains `#[0]`:
   NIP-08. No `delegation`, no `alt`.
2. Display: kind 1 is known, so it is shown as a note.

## The example

The example is
[nostr/examples/retired-nips.coax](../examples/retired-nips.coax). It
prints the table, signs E1, E2 and E3 with test keys (NIP-26's two
published keys and BIP-340 test vector 0's), reads each back with its id
and signature checked, and classifies them. Then it signs E4, a delegated
note inside the window, whose claim holds and whose author is still the
delegatee. The refusals: NIP-26's example as published, whose id is not
the hash of its content; a delegated profile (kind 0, outside `kind=1`); the
delegator's own event carrying the tag (the string names another signer, so
the token doesn't verify); an `alt` with a newline; a request to make a
delegation; and `rt_write` on a kind 4 message, a delegation tag, an `alt`
tag and a `#[0]` mention.

<!-- source: nostr/examples/retired-nips.coax example -->
```jai
#load "../../coax/lib/nostr/nip26.coax";
#load "../../coax/lib/nostr/nip31.coax";
#load "../../coax/lib/nostr/retired.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

CLAIM :: [11] string.["token verifies, conditions hold", "event", "no delegation tag", "repeated delegation tag",
  "bad shape", "bad hex", "conditions don't read", "conditions too long", "token does not verify",
  "token verifies, event breaks the conditions", "refused: this stack makes no delegations"];
READ :: [9] string.["ok", "not JSON", "bad shape", "too long", "bad hex", "no room", "aliased", "id is not the hash of the event", "bad signature"];
ALT :: [6] string.["ok", "event", "none", "empty", "too long", "control byte"];
// NIP-26's published keys, then BIP-340 test vector 0's. They are test
// values: never sign with a real key in an example.
PEOPLE :: [3] string.["Delegator", "Delegatee", "Alice"];
PUB :: [3] string.[
  "8e0d3d3eb2881ec137a11debe736a9086715a8c8beeeda615780064d68bc25dd",
  "477318cfb5427b9cfc66a9fa376150c1ddbc62115ae27cef72417eb959691396",
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"];
SECRET :: [3] string.[
  "ee35e8bb71131c02c1d7e73231daa48e9953d329a4b701f7133c8f46dd21139c",
  "777e4f60b4aa87937e13acc84f7abcc3c93cc035cb4c1e9f7a9086dd78fffce1",
  "0000000000000000000000000000000000000000000000000000000000000003"];
DELEGATOR :: 0; DELEGATEE :: 1; ALICE :: 2;
// NIP-26's example event, byte for byte but without its whitespace.
NIP26_EVENT :: "{\"id\":\"e93c6095c3db1c31d15ac771f8fc5fb672f6e52cd25505099f62cd055523224f\",\"pubkey\":\"477318cfb5427b9cfc66a9fa376150c1ddbc62115ae27cef72417eb959691396\",\"created_at\":1677426298,\"kind\":1,\"tags\":[[\"delegation\",\"8e0d3d3eb2881ec137a11debe736a9086715a8c8beeeda615780064d68bc25dd\",\"kind=1&created_at>1674834236&created_at<1677426236\",\"6f44d7fe4f1c09f3954640fb58bd12bae8bb8ff4120853c4693106c82e920e2b898f1f9ba9bd65449a987c39c0423426ab7b53910c0c6abfb41b30bc16e5f524\"]],\"content\":\"Hello, world!\",\"sig\":\"633db60e2e7082c13a47a6b19d663d45b2a2ebdeaf0b4c35ef83be2738030c54fc7fd56d139652937cdca875ee61b51904a1d0d0588a6acd6168d7be2909d693\"}";
DELEGATION :: "[[\"delegation\",\"8e0d3d3eb2881ec137a11debe736a9086715a8c8beeeda615780064d68bc25dd\",\"kind=1&created_at>1674834236&created_at<1677426236\",\"6f44d7fe4f1c09f3954640fb58bd12bae8bb8ff4120853c4693106c82e920e2b898f1f9ba9bd65449a987c39c0423426ab7b53910c0c6abfb41b30bc16e5f524\"]]";
// The kinds this timeline renders itself.
KNOWN :: [5] int.[0, 1, 3, 6, 7];

decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
key_of :: (who: int) -> [32] u8 { k: [32] u8; decode_hex(PUB[who], k[..]); return k; }
show_key :: (key: [32] u8) {
  for k: 0..<PEOPLE.count { if ns_equal32(key_of(k), key) { print("%", PEOPLE[k]); return; } }
  print("someone else");
}

// Sign an event into out and read it back with its ID and signature checked.
sign :: (who: int, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  key: [32] u8; decode_hex(SECRET[who], key[..]);
  aux: [32] u8; work: [1] NostrSignWork; scratch: [16384] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [16384] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified;
  return e;
}

forms :: (e: NostrEvent) {
  rows: [RT_N] int; n := rt_forms(e, rows[..]);
  #assert n >= 0;
  print("  retired forms:");
  if n == 0 { print(" none"); }
  for i: 0..<n { r := rows[i]; print(" NIP-% (chapter %)", RT_NIP[r], RT_CHAPTER[r]); }
  print("\n");
}
attribute :: (label: string, e: NostrEvent) {
  a := n26_attribute(e);
  print("%: kind %, created_at %\n  author: ", label, e.kind, e.created_at); show_key(a.author);
  print("\n  delegation: %", CLAIM[a.claim.status]);
  if a.claim.status == N26_OK || a.claim.status == N26_OUTSIDE || a.claim.status == N26_TOKEN {
    print(", claims "); show_key(a.claim.delegator);
  }
  print("\n");
}
display :: (label: string, e: NostrEvent) {
  known := KNOWN; d := n31_display(e, known[..]);
  print("%: kind %, alt %: ", label, d.kind, ALT[d.alt]);
  if d.show == N31_KIND { print("shown as kind %\n", d.kind); }
  else if d.show == N31_ALT { print("\"%\"\n", cast(string) d.text[0..<d.n]); }
  else { print("an event of kind % this client can't show\n", d.kind); }
}
write :: (label: string, kind: int, tags: string, content: string) {
  r := rt_write(kind, tags, content);
  print("%: ", label);
  if r == RT_ALLOWED { print("may be signed\n"); return; }
  #assert r >= 0;
  print("refused, NIP-% is unrecommended: %\n", RT_NIP[r], RT_REASON[r]);
}

main :: () {
  print("unrecommended NIPs in the pinned texts\n");
  for r: 0..<RT_N {
    print("NIP-%  chapter %", RT_NIP[r], RT_CHAPTER[r]);
    if RT_INSTEAD[r].count > 0 { print("  instead: %", RT_INSTEAD[r]); }
    print("\n");
  }

  print("three events\n");
  scratch: [16384] u8; b1: [4096] u8; b2: [4096] u8; b3: [4096] u8;
  // NIP-26's example event, re-signed by the delegatee: as published, its
  // id is not the hash of its content (see the refusals).
  e1 := sign(DELEGATEE, 1677426298, 1, DELEGATION, "Hello, world!", b1[..]);
  attribute("E1 (NIP-26's example, re-signed)", e1); forms(e1);
  id: [64] u8; own := e1.id; r := hex_encode_checked(cast(string) own[..], id[..], false);
  #assert r.error == NC_OK;
  print("  id: %\n", cast(string) id[..]);
  e2 := sign(ALICE, 1761955200, 31337, "[[\"d\",\"ruy-lopez\"],[\"alt\",\"Chess opening study: the Ruy Lopez\"]]", "{\"moves\":\"e4 e5 Nf3 Nc6 Bb5\"}", b2[..]);
  display("E2", e2); forms(e2);
  e3 := sign(ALICE, 1761955260, 1, "[[\"p\",\"477318cfb5427b9cfc66a9fa376150c1ddbc62115ae27cef72417eb959691396\"]]", "thanks #[0]", b3[..]);
  display("E3", e3); forms(e3);

  print("a delegation inside its window\n");
  e4 := sign(DELEGATEE, 1677426000, 1, DELEGATION, "Inside the window", b1[..]);
  attribute("E4", e4);

  print("refusals\n");
  published := nostr_event_verify_vartime(NIP26_EVENT, scratch[..], false);
  print("NIP-26's example as published: %\n", READ[published.error]);
  e5 := sign(DELEGATEE, 1677426000, 0, DELEGATION, "{}", b1[..]);
  attribute("E5, a profile", e5);
  e6 := sign(DELEGATOR, 1677426000, 1, DELEGATION, "self", b1[..]);
  attribute("E6, the delegator's own", e6);
  e7 := sign(ALICE, 1761955320, 31337, "[[\"alt\",\"Chess study\\nFrom: your bank\"]]", "", b1[..]);
  display("E7", e7);
  out: [64] u8; key := key_of(DELEGATEE);
  print("make a delegation: %\n", CLAIM[n26_delegate(key[..], key[..], "kind=1&created_at<1677426236", out[..])]);
  write("kind 1 note", 1, "[[\"t\",\"nostr\"]]", "gm");
  write("kind 4 message", 4, "[[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"]]", "x?iv=y");
  write("note with a delegation tag", 1, DELEGATION, "gm");
  write("custom kind with an alt tag", 31337, "[[\"alt\",\"study\"]]", "");
  write("note mentioning #[0]", 1, "[[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"]]", "hi #[0]");
}
```

Run it:

```shell
coax/bin/coax run nostr/examples/retired-nips.coax --run-steps 250000000
```

The signer's fixed-base tables are built at compile time, so the program
needs `--run-steps 250000000` (Bitcoin chapter 16). The output, which the book's
checks compare in debug and release builds, exit status 0:

```text
unrecommended NIPs in the pinned texts
NIP-03  chapter 68
NIP-04  chapter 19  instead: NIP-17
NIP-06  chapter 46  instead: one nsec
NIP-08  chapter 51  instead: NIP-27
NIP-15  chapter 66  instead: NIP-99
NIP-26  chapter 70
NIP-28  chapter 59  instead: NIP-29
NIP-31  chapter 70
NIP-72  chapter 59  instead: NIP-29
NIP-90  chapter 65  instead: use-case-specific NIPs
NIP-96  chapter 57  instead: NIP-B7 (Blossom)
NIP-BE  chapter 70
NIP-EE  chapter 60  instead: Marmot
three events
E1 (NIP-26's example, re-signed): kind 1, created_at 1677426298
  author: Delegatee
  delegation: token verifies, event breaks the conditions, claims Delegator
  retired forms: NIP-26 (chapter 70)
  id: 61a9b5964c24d559aa961efa198d0b7c3a83b3b4c5c288ffb6e99fd562b9b99c
E2: kind 31337, alt ok: "Chess opening study: the Ruy Lopez"
  retired forms: NIP-31 (chapter 70)
E3: kind 1, alt none: shown as kind 1
  retired forms: NIP-08 (chapter 51)
a delegation inside its window
E4: kind 1, created_at 1677426000
  author: Delegatee
  delegation: token verifies, conditions hold, claims Delegator
refusals
NIP-26's example as published: id is not the hash of the event
E5, a profile: kind 0, created_at 1677426000
  author: Delegatee
  delegation: token verifies, event breaks the conditions, claims Delegator
E6, the delegator's own: kind 1, created_at 1677426000
  author: Delegator
  delegation: token does not verify, claims Delegator
E7: kind 31337, alt control byte: an event of kind 31337 this client can't show
make a delegation: refused: this stack makes no delegations
kind 1 note: may be signed
kind 4 message: refused, NIP-04 is unrecommended: deprecated in favor of NIP-17
note with a delegation tag: refused, NIP-26 is unrecommended: adds unnecessary burden for little gain
custom kind with an alt tag: refused, NIP-31 is unrecommended: unnecessarily bloated
note mentioning #[0]: refused, NIP-08 is unrecommended: deprecated in favor of NIP-27
```

E1, E2 and E3 come out as worked by hand, and E1's id is the hash Python
computes for NIP-26's example event. E4 is the event NIP-26 describes, a
delegated note inside its window with a token that verifies, and its
author is still the delegatee.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/retired-nips.coax](../examples/checks/retired-nips.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The first line of each of the 13 pinned files, copied by hand: the
  table's reasons must equal them after the warning prefix, with each link
  written as its label. A Python script
  read every pinned file under `nostr/nips/official/` on 2026-11-01 and
  found these 13 warning lines and no others.
- NIP-26's published token: Python's `hashlib.sha256` of the delegation
  string equals the claim's hash, and the token verifies.
- NIP-26's published event: Python's hash of its NIP-01 serialization is
  `61a9b596…`, not the published `e93c6095…`. `nostr_event_read` refuses
  it with that same computed id, and the published signature verifies over
  neither id. A BIP-340 verifier written from the BIP with Python integers
  gives the same three answers: the token verifies, the signature fails
  over both ids.
- NIP-26's three example condition strings, and E1's 62 seconds by hand.
- E1, E2 and E3 classified as the worked example says.

<!-- source: nostr/examples/checks/retired-nips.coax check -->
```jai
PREFIX :: "> __Warning__  `unrecommended`: ";
// A Markdown line with each link, a [label] then its (target), written as
// its label.
unlink :: (s: string, out: [] u8) -> string {
  n := 0; i := 0; label := false;
  while i < s.count {
    c := s[i];
    if c == '[' { label = true; i += 1; continue; }
    if c == ']' && label && i + 1 < s.count && s[i + 1] == '(' {
      label = false; i += 2;
      while s[i] != ')' { i += 1; }
      i += 1; continue;
    }
    out[n] = c; n += 1; i += 1;
  }
  return cast(string) out[0..<n];
}
#test "the table's reasons are the pinned texts' warnings" {
  buf: [256] u8;
  #assert unlink("a [Marmot](https://e.org/m) b [c", buf[..]) == "a Marmot b c";
  for i: 0..<RT_N {
    #assert PINNED[i][0..<PREFIX.count] == PREFIX;
    #assert unlink(PINNED[i][PREFIX.count..<PINNED[i].count], buf[..]) == RT_REASON[i];
  }
}

// From Python, sharing no code with Coax: hashlib.sha256 of the delegation
// string; hashlib.sha256 of json.dumps([0, pubkey, created_at, kind, tags,
// content], separators=(",", ":"), ensure_ascii=False) for NIP-26's example
// event; and a BIP-340 verifier written from the BIP with Python integers,
// which accepts the token and rejects the example's signature over both its
// published id and the computed one.
PY_HASH :: "397b751983c871f6e3986c6ede36c0f955ddd752c514ad5d1ff026a3e9a8b7f6";
PY_ID :: "61a9b5964c24d559aa961efa198d0b7c3a83b3b4c5c288ffb6e99fd562b9b99c";
PUBLISHED_ID :: "e93c6095c3db1c31d15ac771f8fc5fb672f6e52cd25505099f62cd055523224f";
PUBLISHED_SIG :: "633db60e2e7082c13a47a6b19d663d45b2a2ebdeaf0b4c35ef83be2738030c54fc7fd56d139652937cdca875ee61b51904a1d0d0588a6acd6168d7be2909d693";
hex32_of :: (text: string) -> [32] u8 { b: [32] u8; decode_hex(text, b[..]); return b; }

#test "NIP-26's example: the token verifies, the event's id and signature don't" #steps 2000000000 {
  scratch: [16384] u8; buf: [4096] u8;
  e := sign(DELEGATEE, 1677426298, 1, DELEGATION, "Hello, world!", buf[..]);
  #assert ns_equal32(e.id, hex32_of(PY_ID));
  c := n26_claim(e);
  #assert c.status == N26_OUTSIDE && ns_equal32(c.hash, hex32_of(PY_HASH)) && ns_equal32(c.delegator, key_of(DELEGATOR));
  published := nostr_event_read(NIP26_EVENT, scratch[..], false);
  #assert published.error == NE_ID && ns_equal32(published.computed_id, hex32_of(PY_ID)) && ns_equal32(published.id, hex32_of(PUBLISHED_ID));
  sig: [64] u8; decode_hex(PUBLISHED_SIG, sig[..]);
  pub := key_of(DELEGATEE); a := hex32_of(PUBLISHED_ID); b := hex32_of(PY_ID);
  #assert !nostr_verify_vartime(a[..], pub[..], sig[..]) && !nostr_verify_vartime(b[..], pub[..], sig[..]);
  // The published keys: each secret key signs as its public key.
  #assert ns_equal32(e.public_key, key_of(DELEGATEE));
  d := sign(DELEGATOR, 1, 1, "[]", "", buf[..]); #assert ns_equal32(d.public_key, key_of(DELEGATOR));
}

// NIP-26's three example condition strings read; the hand-worked times.
#test "NIP-26's condition examples" {
  #assert n26_conditions("kind=1&created_at<1675721813", 1, 1675721812) == N26_OK;
  #assert n26_conditions("kind=0&kind=1&created_at>1675721813", 0, 1675721814) == N26_OK;
  #assert n26_conditions("kind=1&created_at>1674777689&created_at<1675721813", 1, 1675000000) == N26_OK;
  // E1: 1677426298 is 62 seconds past created_at<1677426236.
  #assert n26_conditions("kind=1&created_at>1674834236&created_at<1677426236", 1, 1677426298) == N26_OUTSIDE;
  #assert n26_conditions("kind=1&created_at>1674834236&created_at<1677426236", 1, 1677426235) == N26_OK;
}

// The three events classified by hand in the chapter.
#test "the hand classification of E1, E2 and E3" #steps 2000000000 {
  buf: [3][4096] u8; rows: [RT_N] int; known := KNOWN;
  e1 := sign(DELEGATEE, 1677426298, 1, DELEGATION, "Hello, world!", buf[0][..]);
  #assert rt_forms(e1, rows[..]) == 1 && rows[0] == RT_26;
  a := n26_attribute(e1); #assert ns_equal32(a.author, key_of(DELEGATEE)) && a.claim.status == N26_OUTSIDE;
  e2 := sign(ALICE, 1761955200, 31337, "[[\"d\",\"ruy-lopez\"],[\"alt\",\"Chess opening study: the Ruy Lopez\"]]", "{\"moves\":\"e4 e5 Nf3 Nc6 Bb5\"}", buf[1][..]);
  #assert rt_forms(e2, rows[..]) == 1 && rows[0] == RT_31;
  d := n31_display(e2, known[..]); #assert d.show == N31_ALT && d.n == 34;
  e3 := sign(ALICE, 1761955260, 1, "[[\"p\",\"477318cfb5427b9cfc66a9fa376150c1ddbc62115ae27cef72417eb959691396\"]]", "thanks #[0]", buf[2][..]);
  #assert rt_forms(e3, rows[..]) == 1 && rows[0] == RT_08 && n31_display(e3, known[..]).show == N31_KIND;
}
```

### What the NIPs' own examples show

- NIP-26's delegation token is correct: it is the delegator's signature of
  the hash of the delegation string, and the published keys match.
- NIP-26's example event is not: its `id` is not the hash of the event,
  its signature doesn't verify, and its `created_at` (1677426298) is after
  its own `created_at<1677426236` condition. A client following the text
  must refuse it twice: once as an event, and once as a delegation.
- NIP-31 and NIP-BE publish no example events.

### The library's own checks

Each library file has its own `#test` blocks. `nip26.coax`: every
condition's edges (equal times are outside, both kind alternatives, ten
digits and eleven, `kind=65535` and `65536`), 22 condition strings that
don't read, the delegation string at its exact buffer size and one byte
short, the published token on events inside and outside the window, a
token checked against another signer, other conditions, another delegator
and a changed byte, an escaped `&` in the conditions, each refusal,
and `n26_delegate` clearing its output. `nip31.coax`: known and unknown
kinds, escapes, the first `alt` winning, empty, newline, ESC and DEL
summaries, 256 bytes and 257. `retired.coax`: every kind range at its
edges, `#[i]` forms that do and don't name a tag (including two-digit
positions, `#[00]` and broken tag arrays), content decoded before the
search, a forged record, and each publishing refusal.

```shell
coax/bin/coax test coax/lib/nostr/nip26.coax
coax/bin/coax test coax/lib/nostr/nip31.coax
coax/bin/coax test coax/lib/nostr/retired.coax
coax/bin/coax mutate coax/lib/nostr/nip26.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip31.coax --root-only
coax/bin/coax mutate coax/lib/nostr/retired.coax --root-only
coax/bin/coax test nostr/examples/checks/retired-nips.coax --run-steps 250000000
```

On 2026-11-01, `coax test` passed all 240, 236 and 238 checks of the three
files (with the loaded files'), and all 401 of the example's checks.
`coax mutate --root-only` killed all 188 mutants of `nip26.coax`, 60 of
`nip31.coax` and 139 of `retired.coax`, with none surviving and none marked
equivalent. The full mutation run (`COAX_MUTATION_FULL=1`) and a mutation
run of the example were not run.

### Not built

- Anything that applies a delegation: showing a delegated event as the
  delegator's, answering a filter's `authors` with the `delegation` tag's
  pubkey, or letting the delegator delete it. NIP-26 asks for all three;
  this stack refuses them by design.
- Making delegation tokens, writing `alt` tags, or writing any other
  retired form.
- NIP-BE: no BLE advertising, GATT service, DEFLATE chunking or role
  assignment.
- Reading the other retired NIPs' events beyond naming them: that is in
  the chapters the table names (some still design drafts).

## Summary

- Thirteen pinned NIPs are marked `unrecommended`. `RT_REASON` quotes each
  one's reason as written, and the checks compare them with the files.
- `rt_forms` names the retired forms a received event uses: its kind's row,
  a NIP-08 `#[i]` that names a `p` or `e` tag, a `delegation` tag and an
  `alt` tag. `rt_write` refuses every one of them before signing.
- `n26_claim` reads a delegation tag, verifies its token over the
  delegation string built from the event's signer, and applies the
  conditions. `n26_attribute` always names the signer as the author.
- `n31_display` shows an unknown kind's first `alt` summary, and nothing for
  a summary that is empty, too long or holds a control byte.
- NIP-26's published event fails three ways: wrong id, bad signature, and a
  `created_at` past its own delegation.

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)

## Sources

- [NIP-26](../nips/official/26.md)
- [NIP-31](../nips/official/31.md)
- [NIP-BE](../nips/official/BE.md)
- The other unrecommended NIPs: [03](../nips/official/03.md),
  [04](../nips/official/04.md), [06](../nips/official/06.md),
  [08](../nips/official/08.md), [15](../nips/official/15.md),
  [28](../nips/official/28.md), [72](../nips/official/72.md),
  [90](../nips/official/90.md), [96](../nips/official/96.md) and
  [EE](../nips/official/EE.md), and the [NIP index](../nips/official/README.md)
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki), for the delegation token
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
