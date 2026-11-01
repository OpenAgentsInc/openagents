# 69. Small content kinds

Four small NIPs each define a kind of content: a torrent, a chess game, a
geocache and a podcast. Each has a few required tags and a few rules. This
chapter reads all four with chapter 22's tag readers, refuses events that
break the rules, and checks the one rule in each that a client must not
get wrong: a chess game whose PGN doesn't read is shown as invalid, a
geocache find counts only with a proof signed by the cache's key, and a
podcast's claimed author is shown only when that author claims the podcast
back.

Part X, Every NIP. Result: NIP-35 torrents (2003), NIP-64 chess games in PGN
(64), NIP-CC geocaches (37516) and logs (7516, 7517), and NIP-F4 podcasts.

## What this chapter teaches

- NIP-35: kind 2003 with an `x` info hash, `file` and `tracker` tags, and
  `i` references with prefixes such as `imdb:`; and the magnet link built
  from them.
- NIP-64: kind 64 with PGN content, and a PGN reader that finds the first
  byte that breaks the format.
- NIP-CC: kind 37516 listings, 7516 found logs, 7517 verification events,
  37517 curation lists; the check of a verified find; and the winner of a
  first-to-find cache.
- NIP-F4: show metadata (10154), episodes (54), author lists (10164), and
  why a show's claimed author needs a counter-claim.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag(e, i)`, `nostr_tag_d(e)` | Reads and checks an `a`, `e`, `p` or `d` tag (chapter 22) |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_tag(e, i)` | Reads a tag of any name: the name and its first two values (chapter 55) |
| [nip35.coax](../../coax/lib/nostr/nip35.coax) | `n35_read(e)` | A torrent's info hash, title, files, trackers and references |
| [nip35.coax](../../coax/lib/nostr/nip35.coax) | `n35_magnet(t, out)` | The magnet link for a torrent read by `n35_read` |
| [nip64.coax](../../coax/lib/nostr/nip64.coax) | `n64_san(move)` | Whether a move is written in Standard Algebraic Notation |
| [nip64.coax](../../coax/lib/nostr/nip64.coax) | `n64_pgn(s)` | Reads a PGN database: games, moves, tag pairs, or the first bad byte |
| [nip64.coax](../../coax/lib/nostr/nip64.coax) | `n64_read(e, work)` | A kind 64 event's PGN, its content decoded first |
| [nipcc.coax](../../coax/lib/nostr/nipcc.coax) | `ncc_listing(e)`, `ncc_found(e)`, `ncc_list(e)` | Reads a listing, a found log and a curation list |
| [nipcc.coax](../../coax/lib/nostr/nipcc.coax) | `ncc_proof(found, text, scratch)` | The found log's embedded kind 7517 event, its id and signature checked |
| [nipcc.coax](../../coax/lib/nostr/nipcc.coax) | `ncc_verify(listing, found, proof)` | Whether a found log proves its author found the cache |
| [nipcc.coax](../../coax/lib/nostr/nipcc.coax) | `ncc_winner(listing, logs, proofs)` | The first-to-find winner among found logs |
| [nipf4.coax](../../coax/lib/nostr/nipf4.coax) | `nf4_show(e)`, `nf4_episode(e)`, `nf4_authored(e)` | Reads a show, an episode and an author list |
| [nipf4.coax](../../coax/lib/nostr/nipf4.coax) | `nf4_credits(show, lists)` | Which of a show's claimed authors confirm it |

The four library files are new with this chapter. `nipcc.coax` and
`nipf4.coax` are `#authority` files: one decides whether a find is proved
and who wins a cache, the other whose name a client shows on a podcast.
Every procedure in them runs in a check, and `coax mutate` leaves no
survivors in any of the four files.

Each reader returns a record with a status, the index of the tag it
refused (or -1), and what it read. `N32Text` is chapter 55's decoded string
of at most 256 bytes, and `NostrTag` is chapter 22's checked tag. Like
chapter 62's readers, these take the first tag of a name when a NIP allows
one: a second `title` or `D` is ignored.

## Torrents: NIP-35

A torrent event (kind 2003) is an index entry: enough to search for content
and to build a magnet link. No torrent file is on Nostr. Its tags:

- `x` (required): the V1 BitTorrent info hash, the SHA-1 of the torrent's
  info dictionary: 40 hex digits. The reader takes either case and the
  writer lowercases. A second `x` is refused.
- `file`: a path inside the torrent and, optionally, its size in bytes (1
  to 18 decimal digits).
- `tracker`: a tracker URL. The first four are kept.
- `i`: a reference written `prefix:value`. NIP-35 lists `tcat`, `newznab`,
  `tmdb`, `ttvdb`, `imdb`, `mal` and `anilist`. An unlisted prefix still
  reads and is counted as unknown; a value without a prefix or after it is
  refused.
- `title` and `t` (topics such as `movie` or `4k`).

<!-- source: coax/lib/nostr/nip35.coax record -->
```jai
N35Torrent :: struct {
  status:int=N35_EVENT; at:int=-1;          // the refusing tag
  hash:[20]u8;                              // the info hash
  has_title:bool; title:N32Text;
  files:int; bytes:int;                     // file tags; their sizes summed
  trackers:[N35_MAX_TRACKERS]N32Text; tracker_n:int; tracker_more:int;
  refs:int; unknown:int;                    // i tags; those with unlisted prefixes
  topics:int;                               // t tags
}
```

<!-- source: coax/lib/nostr/nip35.coax read -->
```jai
n35_read :: (e:NostrEvent)->N35Torrent {
  r:N35Torrent;if e.error!=NE_OK {return r;}
  if e.kind!=N35_KIND_TORRENT {return n35_refuse(r,N35_KIND,-1);}
  has_hash:=false;i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n35_refuse(r,N35_LIMIT,i);}
    if t.error!=NT_OK {return n35_refuse(r,N35_EVENT,i);}
    tag:=t.v[0];v:=t.v[1];
    if n32_is(tag,"x") {
      if has_hash {return n35_refuse(r,N35_REPEATED,i);}
      if v.n!=40 {return n35_refuse(r,N35_HASH,i);}
      for k:0..<20 {
        hi:=n35_nibble(v.b[2*k]);lo:=n35_nibble(v.b[2*k+1]);
        if hi<0 || lo<0 {return n35_refuse(r,N35_HASH,i);}
        r.hash[k]=cast(u8)(hi*16+lo);
      }
      has_hash=true;
    } else if n32_is(tag,"file") {
      if v.n==0 {return n35_refuse(r,N35_EMPTY,i);}
      if t.values>=3 {
        size:=n35_size(t.v[2]);
        if size<0 {return n35_refuse(r,N35_FILE,i);}
        r.bytes+=size;
      }
      r.files+=1;
    } else if n32_is(tag,"tracker") {
      if v.n==0 {return n35_refuse(r,N35_EMPTY,i);}
      if r.tracker_n<N35_MAX_TRACKERS {r.trackers[r.tracker_n]=v;r.tracker_n+=1;} else {r.tracker_more+=1;}
    } else if n32_is(tag,"i") {
      // prefix:value, the prefix one or more characters before a colon.
      s:=cast(string)v.b[0..<v.n];colon:=0;
      while colon<s.count && s[colon]!=':' {colon+=1;}
      if colon<1 || colon+1>=s.count {return n35_refuse(r,N35_PREFIX,i);}
      if !n35_prefix(cast(string)v.b[0..<v.n]) {r.unknown+=1;}
      r.refs+=1;
    } else if n32_is(tag,"title") && !r.has_title {r.has_title=true;r.title=v;}
    else if n32_is(tag,"t") {r.topics+=1;}
    i+=1;
  }
  if !has_hash {return n35_refuse(r,N35_MISSING,-1);}
  r.status=N35_OK;return r;
}
```

The magnet link is `magnet:?xt=urn:btih:` with the hash, then `&dn=` and
the title, then `&tr=` and each tracker kept. Everything but RFC 3986's
unreserved characters (letters, digits, `-`, `_`, `.`, `~`) is written as
`%XX` in uppercase hex, one per byte, so a UTF-8 title is encoded byte by
byte.

<!-- source: coax/lib/nostr/nip35.coax magnet -->
```jai
// Percent-encoding: RFC 3986's unreserved characters stay, every other
// byte is %XX in uppercase hex.
n35_escape :: (out:[]u8,at:int,s:N32Text)->int {
  c:=s;n:=at;
  for i:0..<c.n {
    b:=c.b[i];
    plain:=(b>='a' && b<='z') || (b>='A' && b<='Z') || (b>='0' && b<='9') || b=='-' || b=='_' || b=='.' || b=='~';
    one:[3]u8;one[0]=b;one[1]=N35_UPPER[b>>4];one[2]=N35_UPPER[b&15];
    if plain {n=json_raw(out,n,cast(string)one[0..<1]);}
    else {one[0]='%';n=json_raw(out,n,cast(string)one[..]);}
  }
  return n;
}
// The magnet link: xt, then dn when there is a title, then a tr for each
// tracker kept. The length written, or -1.
n35_magnet :: (t:N35Torrent,out:[]u8)->int {
  if t.status!=N35_OK {return -1;}
  hex:[40]u8;
  for k:0..<20 {hex[2*k]=N35_LOWER[t.hash[k]>>4];hex[2*k+1]=N35_LOWER[t.hash[k]&15];}
  n:=json_raw(out,0,"magnet:?xt=urn:btih:");n=json_raw(out,n,cast(string)hex[..]);
  if t.has_title {n=json_raw(out,n,"&dn=");n=n35_escape(out,n,t.title);}
  for k:0..<t.tracker_n {n=json_raw(out,n,"&tr=");n=n35_escape(out,n,t.trackers[k]);}
  return n;
}
```

Kind 2004 comments on a torrent work like kind 1 replies with NIP-10 tags
(chapter 23); nothing here reads them.

### Worked example: the magnet link of TOR

TOR has the hash `dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c`, the title
`Big Buck Bunny`, one tracker `udp://tracker.example.org:1337`, and files of
276134947 and 310380 bytes. By hand:

- The files sum to 276134947 + 310380 = 276445327 bytes.
- In the title only the spaces need encoding: a space is byte 0x20, so
  `Big%20Buck%20Bunny`.
- In the tracker, `:` is 0x3A and `/` is 0x2F: `udp%3A%2F%2Ftracker.example.org%3A1337`.
  The dots and digits stay.

```text
magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big%20Buck%20Bunny&tr=udp%3A%2F%2Ftracker.example.org%3A1337
```

## Chess games: NIP-64

A kind 64 event's content is a PGN database: one or more chess games in
Portable Game Notation. NIP-64 says clients SHOULD publish PGN in the strict
"export format" and accept the lax "import format" that people type, and
SHOULD check that the formatting is valid. This reader checks the import
format's syntax:

- Tag pairs `[Name "value"]` come before a game's moves. A name is letters,
  digits and `_`; the value may escape `"` and `\` with a backslash and may
  not span lines.
- Move numbers are digits then one or more periods: `12.` or `12...`. The
  import format allows them with or without a space after them.
- Moves are SAN, below. Annotations `$n`, `{brace comments}`, `;` comments
  to the end of the line, `%` escape lines and nested `(variations)` are
  skipped.
- Each game ends with `1-0`, `0-1`, `1/2-1/2` or `*`. A `Result` tag must
  agree with that ending, as the PGN standard requires.

A record of what was read, or the first byte that broke the format:

<!-- source: coax/lib/nostr/nip64.coax record -->
```jai
N64Pgn :: struct {
  status:int=N64_EMPTY; at:int=-1;   // the refusing byte's offset
  games:int;                         // games that end with a marker
  moves:int;                         // SAN moves, variations included
  tags:int;                          // tag pairs in all games
  result:int=N64_UNKNOWN;            // the first game's ending marker
}
```

A move in Standard Algebraic Notation (SAN) is castling (`O-O`, `O-O-O`), a
piece letter `K Q R B N` with an optional origin file and rank, `x` for a
capture and the target square (`Nbd7`, `R1e2`, `Qh4xe1`), or a pawn move:
the target square, after the pawn's file and `x` when it captures (`exd5`).
A pawn capture moves to the next file, and a pawn that reaches the first or
last rank must promote (`e8=Q` or `e8Q`); nothing else promotes. Then `+`
or `#`, then at most two of `!` and `?`.

<!-- source: coax/lib/nostr/nip64.coax san -->
```jai
// What may follow a move: + or #, then at most two of ! and ?.
n64_suffix :: (rest:string)->bool {
  i:=0;if i<rest.count && (rest[i]=='+' || rest[i]=='#') {i+=1;}
  k:=0;while k<2 && i<rest.count && (rest[i]=='!' || rest[i]=='?') {i+=1;k+=1;}
  return i==rest.count;
}
// Is s[i] a piece letter?
n64_piece :: (s:string,i:int)->bool {return i<s.count && (s[i]=='K' || s[i]=='Q' || s[i]=='R' || s[i]=='B' || s[i]=='N');}
n64_body :: (c:u8)->bool {return n64_file(c) || n64_rank(c) || c=='x';}
// One move in SAN: castling; a piece letter, then an optional origin file
// and rank, x for a capture and the target square; or a pawn's target,
// after its file and x when it captures, then its promotion. Then the
// suffix.
n64_san :: (move:string)->bool {
  n:=move.count;i:=0;
  if n>=5 && move[0..<5]=="O-O-O" {i=5;}
  else if n>=3 && move[0..<3]=="O-O" {i=3;}
  else {
    piece:=n64_piece(move,0);
    if piece {i=1;}
    from:=i;while i<n && n64_body(move[i]) {i+=1;}
    s:=move[from..<i];m:=s.count;
    if m<2 || !n64_file(s[m-2]) || !n64_rank(s[m-1]) {return false;}
    if piece {
      k:=0;e:=m-2;
      if e>0 && s[e-1]=='x' {e-=1;}
      if k<e && n64_file(s[k]) {k+=1;}
      if n64_rank(s[k]) {k+=1;}   // s[e] is x or a file
      if k!=e {return false;}
    } else {
      if m==4 && (s[1]!='x' || (s[2]!=s[0]+1 && s[2]+1!=s[0])) {return false;}
      if m!=2 && m!=4 {return false;}
      last:=s[m-1]=='1' || s[m-1]=='8';
      equals:=i<n && move[i]=='=';if equals {i+=1;}
      promote:=i<n && (move[i]=='Q' || move[i]=='R' || move[i]=='B' || move[i]=='N');
      if promote {i+=1;}
      if promote!=last || (equals && !promote) {return false;}
    }
  }
  return n64_suffix(move[i..<n]);
}
```

<!-- source: coax/lib/nostr/nip64.coax pgn -->
```jai
// A PGN database read token by token. A game is open from its first tag or
// move until its ending marker; tags may only come before its moves.
n64_pgn :: (s:string)->N64Pgn {
  p:N64Pgn;i:=0;open:=false;moved:=false;depth:=0;result:=-1;
  while i<s.count {
    c:=s[i];
    if n64_space(c) {i+=1;continue;}
    if c=='%' && (i==0 || s[i-1]=='\n') || c==';' {while i<s.count && s[i]!='\n' {i+=1;}continue;}
    if c=='{' {
      j:=i+1;while j<s.count && s[j]!='}' {j+=1;}
      if j==s.count {return n64_fail(p,N64_COMMENT,i);}
      i=j+1;continue;
    }
    if c=='[' {
      // [Name "value"]: the value is the Result tag's when the name is.
      if moved {return n64_fail(p,N64_TAG,i);}
      j:=i+1;while j<s.count && n64_space(s[j]) {j+=1;}
      from:=j;while j<s.count && n64_name(s[j]) {j+=1;}
      if j==from || j==s.count || !n64_space(s[j]) {return n64_fail(p,N64_TAG,j);}
      name:=s[from..<j];
      while j<s.count && n64_space(s[j]) {j+=1;}
      if j==s.count || s[j]!='"' {return n64_fail(p,N64_TAG,j);}
      j+=1;v:=j;
      while j<s.count && s[j]!='"' {
        if s[j]=='\n' {return n64_fail(p,N64_TAG,j);}
        if s[j]=='\\' {j+=1;}
        j+=1;
      }
      if j>=s.count {return n64_fail(p,N64_TAG,s.count);}
      value:=s[v..<j];j+=1;
      while j<s.count && n64_space(s[j]) {j+=1;}
      if j==s.count || s[j]!=']' {return n64_fail(p,N64_TAG,j);}
      if name=="Result" {result=n64_ending(value);}
      open=true;p.tags+=1;i=j+1;continue;
    }
    if c=='(' {depth+=1;open=true;i+=1;continue;}
    if c==')' {if depth==0 {return n64_fail(p,N64_VARIATION,i);}depth-=1;i+=1;continue;}
    if c=='$' {
      j:=i+1;while j<s.count && n64_digit(s[j]) {j+=1;}
      if j==i+1 {return n64_fail(p,N64_MOVE,i);}
      i=j;continue;
    }
    if !n64_token(c) && c!='*' {return n64_fail(p,N64_MOVE,i);}
    // A move number: digits then periods.
    j:=i;while j<s.count && n64_digit(s[j]) {j+=1;}
    if j<s.count && s[j]=='.' {
      while j<s.count && s[j]=='.' {j+=1;}
      open=true;i=j;continue;
    }
    j=i+1;
    if c!='*' {while j<s.count && n64_token(s[j]) {j+=1;}}
    word:=s[i..<j];end:=n64_ending(word);
    if end>=0 {
      if depth>0 {return n64_fail(p,N64_VARIATION,i);}
      if result>=0 && result!=end {return n64_fail(p,N64_MISMATCH,i);}
      if p.games==0 {p.result=end;}
      p.games+=1;open=false;moved=false;result=-1;i=j;continue;
    }
    if !n64_san(word) {return n64_fail(p,N64_MOVE,i);}
    p.moves+=1;open=true;moved=true;i=j;
  }
  if open {return n64_fail(p,N64_UNFINISHED,i);}
  if p.games==0 {return n64_fail(p,N64_EMPTY,i);}
  p.status=N64_OK;return p;
}
```

<!-- source: coax/lib/nostr/nip64.coax read -->
```jai
// A kind 64 event's PGN, its content decoded into work first.
n64_read :: (e:NostrEvent,work:[]u8)->N64Pgn {
  p:N64Pgn;
  if e.error!=NE_OK || e.content_from<0 || e.content_to>e.original.count || e.content_from>e.content_to {return n64_fail(p,N64_EVENT,-1);}
  if e.kind!=N64_KIND_CHESS {return n64_fail(p,N64_KIND,-1);}
  n:=json_unescape(e.original[e.content_from..<e.content_to],work);
  if n<0 {return n64_fail(p,N64_ROOM,-1);}
  return n64_pgn(cast(string)work[0..<n]);
}
```

NIP-64 also says a client SHOULD check that every move is legal on the
board. That needs a move generator and is not built: `1. e4 e4 *` reads
here.

### Worked example: G2 and a Result that disagrees

G2's content is `1. e4 e9 *`. Byte by byte from 0: `1.` is a move number
(bytes 0 and 1), byte 2 is a space, `e4` (bytes 3 and 4) is a pawn move to
e4, byte 5 is a space. At byte 6 the token `e9` starts: `e` is a file, but
`9` is no rank. The reader stops: bad move at byte 6.

The content `[Result "1-0"] 1. e4 0-1` has a tag pair in bytes 0 to 13. Then
come a space, `1.` at 15, a space, `e4` at 18 and 19, a space, and the
ending `0-1` at byte 21. The tag said white won and the ending says black
won: Result tag disagrees at byte 21.

## Geocaching: NIP-CC

NIP-CC defines four kinds. A listing (kind 37516, addressable) is the cache:

- `d`, `name`: required.
- `g`: one or more geohashes, 1 to 12 of the 32 geohash digits. Several
  precisions let a client search by proximity.
- `D` and `T`: difficulty and terrain, one digit 1 to 5.
- `S`: `micro`, `small`, `regular`, `large` or `other`.
- `t`: the cache type, `traditional` when absent. The value `archived`
  retires the cache instead.
- `n`: type modifiers. `first-to-find` makes the first verified find the
  only claim; `art` says the cache is a work of art. Unknown values are
  ignored, as NIP-CC asks.
- `verification`: the hex public key that signs proofs of a find. The
  matching secret key is printed at the cache, often as a QR code.
- `F`: the locked-in first-to-find winner.
- `hint`, `mission`, `image`, `r`: optional.

The reader refuses `first-to-find` without a `verification` key (a find
could never be proved) and an `F` tag without `first-to-find`.

A found log (kind 7516) has an `a` tag naming the listing
(`37516:<owner>:<d>`) and may carry a proof in a `verification` tag. A
proof is a kind 7517 event signed by the cache's key, written into the tag
as JSON. Its `a` tag is `<finder hex>:<naddr>` and its content is
`Geocache verification for <finder npub>`. A curation list (kind 37517)
groups caches with `d`, `title` and one or more `a` tags. Non-found logs
(did not find, notes, maintenance) are NIP-22 comments (kind 1111,
chapter 24); this chapter doesn't read them.

<!-- source: coax/lib/nostr/nipcc.coax record -->
```jai
NccListing :: struct {
  status:int=NCC_EVENT; at:int=-1;        // the refusing tag
  d:N32Text; name:N32Text; type:N32Text;  // type: traditional when absent
  geohashes:int;                          // g tags
  difficulty:int; terrain:int; size:int;  // size: an index into NCC_SIZES
  archived:bool;
  first_to_find:bool; art:bool;           // n modifiers
  has_key:bool; key:[32]u8;               // verification
  has_winner:bool; winner:[32]u8;         // F
  has_mission:bool; mission:N32Text;
}
NccFound :: struct {
  status:int=NCC_EVENT; at:int=-1;
  cache:NostrTag;                         // the a tag: 37516:<owner>:<d>
  proof_at:int=-1;                        // the verification tag, or -1
  images:int;
}
NccList :: struct {status:int=NCC_EVENT; at:int=-1; d:N32Text; title:N32Text; caches:int;}
NccWinner :: struct {
  status:int=NCC_EVENT;
  index:int=-1;                           // the winning log in the set, or -1
  locked:bool; finder:[32]u8;             // locked: named by the F tag
  verified:int; refused:int;              // logs with valid and invalid proofs
}
```

<!-- source: coax/lib/nostr/nipcc.coax listing -->
```jai
ncc_listing :: (e:NostrEvent)->NccListing {
  r:NccListing;if e.error!=NE_OK {return r;}
  if e.kind!=NCC_KIND_LISTING {return n_cc_refuse_l(r,NCC_KIND,-1);}
  seen:[6]bool;   // d, name, D, T, S, a cache type
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n_cc_refuse_l(r,NCC_LIMIT,i);}
    if t.error!=NT_OK {return n_cc_refuse_l(r,NCC_EVENT,i);}
    tag:=t.v[0];v:=t.v[1];
    if n32_is(tag,"d") {if !seen[0] {seen[0]=true;r.d=v;}}
    else if n32_is(tag,"name") {if !seen[1] {seen[1]=true;r.name=v;}}
    else if n32_is(tag,"g") {
      if !ncc_geohash(v) {return n_cc_refuse_l(r,NCC_GEOHASH,i);}
      r.geohashes+=1;
    } else if n32_is(tag,"D") || n32_is(tag,"T") {
      score:=ncc_score(v);if score==0 {return n_cc_refuse_l(r,NCC_SCORE,i);}
      if n32_is(tag,"D") {if !seen[2] {seen[2]=true;r.difficulty=score;}}
      else if !seen[3] {seen[3]=true;r.terrain=score;}
    } else if n32_is(tag,"S") {
      size:=-1;for k:0..<5 {if n32_is(v,NCC_SIZES[k]) {size=k;}}
      if size<0 {return n_cc_refuse_l(r,NCC_SIZE,i);}
      if !seen[4] {seen[4]=true;r.size=size;}
    } else if n32_is(tag,"t") {
      if n32_is(v,"archived") {r.archived=true;}
      else if !seen[5] {seen[5]=true;r.type=v;}
    } else if n32_is(tag,"n") {
      // Each modifier is its own category: a repeat changes nothing.
      if n32_is(v,"first-to-find") {r.first_to_find=true;}
      if n32_is(v,"art") {r.art=true;}
    } else if n32_is(tag,"verification") || n32_is(tag,"F") {
      key:[32]u8;if !ncc_key(v,key[..]) {return n_cc_refuse_l(r,NCC_KEY,i);}
      if n32_is(tag,"F") {if !r.has_winner {r.has_winner=true;r.winner=key;}}
      else if !r.has_key {r.has_key=true;r.key=key;}
    } else if n32_is(tag,"mission") && !r.has_mission {r.has_mission=true;r.mission=v;}
    i+=1;
  }
  if !seen[0] || !seen[1] || r.geohashes==0 || !seen[2] || !seen[3] || !seen[4] {return n_cc_refuse_l(r,NCC_MISSING,-1);}
  if !seen[5] {for c:"traditional" {r.type.b[r.type.n]=c;r.type.n+=1;}}
  if r.first_to_find && !r.has_key {return n_cc_refuse_l(r,NCC_UNVERIFIABLE,-1);}
  if r.has_winner && !r.first_to_find {return n_cc_refuse_l(r,NCC_WINNER,-1);}
  r.status=NCC_OK;return r;
}
```

A proof is about 600 bytes of JSON, longer than the 256 bytes `n32_tag`
decodes. `ncc_value` reads one value of a tag into a buffer of any size,
and `ncc_named` compares a tag's name. The found-log reader lets a long
value through for that reason.

<!-- source: coax/lib/nostr/nipcc.coax value -->
```jai
// The k'th string of the index'th tag (0 is its name), decoded into out
// whatever its length: its length, or -1 when the tag or value is missing
// or malformed, or the raw value is longer than out.
ncc_value :: (e:NostrEvent,index:int,k:int,out:[]u8)->int {
  j:[1]Json;if !nt_start(j[..],e) {return -1;}
  at:=0;tok:=json_next(j[..]);
  while tok==JSON_ARRAY && at<index {skipped:=json_skip(j[..]);at+=1;tok=json_next(j[..]);}
  if index<0 || tok!=JSON_ARRAY {return -1;}
  for s:0..<k {if json_next(j[..])!=JSON_STRING {return -1;}}
  if json_next(j[..])!=JSON_STRING {return -1;}
  raw:=json_text(j[..]);
  if raw.count>out.count {return -1;}   // unescaping never lengthens
  return json_unescape(raw,out);
}
// Is the index'th tag's name this one?
ncc_named :: (e:NostrEvent,index:int,name:string)->bool {
  b:[16]u8;n:=ncc_value(e,index,0,b[..]);
  return n==name.count && cast(string)b[0..<n]==name;
}
```

<!-- source: coax/lib/nostr/nipcc.coax found -->
```jai
ncc_found :: (e:NostrEvent)->NccFound {
  r:NccFound;if e.error!=NE_OK {return r;}
  if e.kind!=NCC_KIND_FOUND {return n_cc_refuse_f(r,NCC_KIND,-1);}
  has_cache:=false;i:=0;
  while true {
    // A verification value may be longer than tags.coax reads: NT_LIMIT is
    // let through here and the value read by ncc_value.
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error!=NT_OK && t.error!=NT_LIMIT {return n_cc_refuse_f(r,NCC_EVENT,i);}
    if ncc_named(e,i,"a") {
      a:=nostr_tag(e,i);
      if a.error!=NT_OK {return n_cc_refuse_f(r,NCC_TAG,i);}
      if a.address_kind!=NCC_KIND_LISTING || has_cache {return n_cc_refuse_f(r,NCC_TARGET,i);}
      has_cache=true;r.cache=a;
    } else if ncc_named(e,i,"verification") {if r.proof_at== -1 {r.proof_at=i;}}
    else if ncc_named(e,i,"image") {r.images+=1;}
    i+=1;
  }
  if !has_cache {return n_cc_refuse_f(r,NCC_MISSING,-1);}
  r.status=NCC_OK;return r;
}
```

### A verified find

NIP-CC lists four checks of a verified find: the proof is signed by the
cache's verification key, the finder in its `a` tag is the log's author,
the naddr in its `a` tag is the cache, and the signature is valid.
`ncc_proof` decodes the proof and verifies its id and signature with
chapter 21's `nostr_event_verify_vartime`. `ncc_verify` then checks the
rest, in NIP-CC's order, and also the content's npub. Each failure has its
own verdict.

<!-- source: coax/lib/nostr/nipcc.coax proof -->
```jai
// The found log's embedded verification event, read into text with its id
// and signature checked (scratch holds its serialization): error NE_OK and
// verified, or a refused event. text must outlive it.
ncc_proof :: (found:NostrEvent,text:[]u8,scratch:[]u8)->NostrEvent {
  none:NostrEvent;
  f:=ncc_found(found);if f.status!=NCC_OK {return none;}
  n:=ncc_value(found,f.proof_at,1,text);
  if n<0 {return none;}
  return nostr_event_verify_vartime(cast(string)text[0..<n],scratch,false);
}
```

<!-- source: coax/lib/nostr/nipcc.coax verify -->
```jai
// Does this found log, with the proof ncc_proof read from it, prove its
// author found this cache? NCC_OK, or the first check that fails, in
// NIP-CC's order.
ncc_verify :: (listing:NostrEvent,found:NostrEvent,p:NostrEvent)->int {
  l:=ncc_listing(listing);if l.status!=NCC_OK {return NCC_KIND;}
  f:=ncc_found(found);if f.status!=NCC_OK {return NCC_KIND;}
  d:=nostr_tag_d(listing);
  if !ns_equal32(f.cache.pubkey,listing.public_key) {return NCC_TARGET;}
  if cast(string)f.cache.d[0..<f.cache.d_n]!=cast(string)d.d[0..<d.d_n] {return NCC_TARGET;}
  if !l.has_key {return NCC_NO_KEY;}
  if f.proof_at<0 {return NCC_UNVERIFIED;}
  if p.error!=NE_OK || !p.verified || p.kind!=NCC_KIND_PROOF {return NCC_INNER;}
  // 1. Signed by the cache's verification key.
  if !ns_equal32(p.public_key,l.key) {return NCC_SIGNER;}
  // 2. Its a tag is the log author's key in hex, a colon, then an naddr.
  a:=ncc_first(p,"a");s:=cast(string)a.b[0..<a.n];finder:[32]u8;
  if s.count<65 {return NCC_FINDER;}
  if s[64]!=':' {return NCC_FINDER;}
  if !ne_lower_hex(s[0..<64],finder[..]) {return NCC_FINDER;}
  if !ns_equal32(finder,found.public_key) {return NCC_FINDER;}
  // 3. The naddr is the cache's: kind 37516, the owner and the d tag.
  bytes:[512]u8;
  x:=nip19_decode_checked(s[65..<s.count],bytes[..],false);
  if x.type!=N19_ADDR {return NCC_CACHE;}
  if x.kind!=NCC_KIND_LISTING {return NCC_CACHE;}
  owner:[32]u8;for k:0..<32 {owner[k]=bytes[x.author_at+k];}
  if !ns_equal32(owner,listing.public_key) {return NCC_CACHE;}
  if cast(string)bytes[x.special_at..<x.special_at+x.special_length]!=cast(string)d.d[0..<d.d_n] {return NCC_CACHE;}
  // The content names the finder's npub, compared as written: an escaped
  // spelling of the same text is refused.
  text:[128]u8;want:[160]u8;
  w:=nip19_encode_checked(N19_NPUB,cast(string)finder[..],text[..],false);
  m:=json_raw(want[..],0,NCC_PROOF_TEXT);m=json_raw(want[..],m,cast(string)text[0..<w.written]);
  if p.original[p.content_from..<p.content_to]!=cast(string)want[0..<m] {return NCC_CONTENT;}
  return NCC_OK;
}
```

The content is compared as written: an escaped spelling of the same text
(`npub` for `npub`) is refused. A signer that follows NIP-01 never
escapes a letter.

### First to find

On a `first-to-find` cache, the first verified find is the only claim. The
winner is the verified log with the earliest `created_at`, the lower id
first when two are equally early. Since `created_at` is the author's to
choose, the owner can lock the winner in: a new revision of the listing
with `["t","archived"]` and `["F", <winner>]`. With an `F` tag the winner is
that key, whatever the timestamps.

<!-- source: coax/lib/nostr/nipcc.coax winner -->
```jai
// The first-to-find winner among found logs: the F tag's finder when the
// listing locks one in, else the earliest verified log, the lower id first
// when two are as early. proofs[k] is ncc_proof of logs[k]. Logs that
// don't verify are counted, never chosen.
ncc_winner :: (listing:NostrEvent,logs:[]NostrEvent,proofs:[]NostrEvent)->NccWinner {
  w:NccWinner;if proofs.count!=logs.count {return w;}
  l:=ncc_listing(listing);if l.status!=NCC_OK {w.status=NCC_KIND;return w;}
  if !l.first_to_find {w.status=NCC_UNVERIFIABLE;return w;}
  w.locked=l.has_winner;if w.locked {w.finder=l.winner;}
  for k:0..<logs.count {
    if ncc_verify(listing,logs[k],proofs[k])!=NCC_OK {w.refused+=1;continue;}
    w.verified+=1;e:=logs[k];
    if w.locked {
      if w.index== -1 && ns_equal32(e.public_key,w.finder) {w.index=k;}
      continue;
    }
    better:=w.index== -1;
    if !better {
      b:=logs[w.index];
      better=e.created_at<b.created_at || (e.created_at==b.created_at && ncc_before(e.id,b.id));
    }
    if better {w.index=k;w.finder=e.public_key;}
  }
  w.status=NCC_OK;return w;
}
```

### Worked example: who found the old oak

Alice's listing L (d `oak`) is `first-to-find` with the verification key
Cache. The secret key inside the box signs two proofs: PB for Bob and PD for
Dave. Three found logs arrive:

| Log | Author | `created_at` | Proof | Verdict |
| --- | --- | --- | --- | --- |
| FC | Carol | T0+50 | PB, for Bob | proof for another finder |
| FD | Dave | T0+100 | PD, for Dave | ok |
| FB | Bob | T0+200 | PB, for Bob | ok |

FC is the earliest, but its proof names Bob, not Carol: Carol copied Bob's
proof and can't claim the find. Of the two verified logs, FD (T0+100) is
earlier than FB (T0+200), so Dave wins L. Then Alice publishes LK, the same
listing archived with `["F", <Bob>]`. With LK the winner is Bob, though
Dave's log is earlier.

## Podcasts: NIP-F4

A podcast is its own key. It publishes its show's metadata as a replaceable
kind 10154 (`title`, `image`, `description`, `website`, and `p` tags that
claim authors with an optional role `host`, `cohost` or `editor`) and its
episodes as kind 54 (`title`, one or more `audio` tags with a URL and an
optional media type, `image`, `description`). A person lists the podcasts
they author in a replaceable kind 10164 of `p` tags.

The show reader requires a title and refuses an unknown role. An episode
needs a title and an audio URL; a media type, when given, has a slash with
something on both sides. NIP-F4 puts the role in the `p` tag's third
position, where NIP-01 puts a relay URL, so the show reader reads the key
itself instead of through `nostr_tag`, which would refuse `host` as a
relay.

<!-- source: coax/lib/nostr/nipf4.coax record -->
```jai
NF4Author :: struct {key:[32]u8; role:int;}
NF4Show :: struct {
  status:int=NF4_EVENT; at:int=-1;          // the refusing tag
  title:N32Text; has_image:bool; image:N32Text; has_description:bool;
  websites:int;
  authors:[NF4_MAX_AUTHORS]NF4Author; author_n:int;   // claimed, in order
}
NF4Authored :: struct {status:int=NF4_EVENT; at:int=-1; shows:[NF4_MAX_SHOWS][32]u8; n:int;}
NF4Episode :: struct {
  status:int=NF4_EVENT; at:int=-1;
  title:N32Text; audio:N32Text; has_type:bool; type:N32Text;   // the first audio tag
  audios:int; has_image:bool; has_description:bool;
}
NF4Credits :: struct {
  status:int=NF4_EVENT;
  confirmed:[NF4_MAX_AUTHORS]bool;          // by the claimed author's newest list
  shown:int; unconfirmed:int;
}
```

<!-- source: coax/lib/nostr/nipf4.coax show -->
```jai
nf4_show :: (e:NostrEvent)->NF4Show {
  r:NF4Show;if e.error!=NE_OK {return r;}
  if e.kind!=NF4_KIND_SHOW {return nf4_refuse_s(r,NF4_KIND,-1);}
  has_title:=false;i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return nf4_refuse_s(r,NF4_LIMIT,i);}
    if t.error!=NT_OK {return nf4_refuse_s(r,NF4_EVENT,i);}
    tag:=t.v[0];
    if n32_is(tag,"title") && !has_title {has_title=true;r.title=t.v[1];}
    else if n32_is(tag,"image") && !r.has_image {r.has_image=true;r.image=t.v[1];}
    else if n32_is(tag,"description") {r.has_description=true;}
    else if n32_is(tag,"website") {r.websites+=1;}
    else if n32_is(tag,"p") {
      // The role is the tag's second value, where NIP-01 puts a relay, so
      // tags.coax would refuse it: the key is read here.
      key:[32]u8;v:=t.v[1];
      if !ne_lower_hex(cast(string)v.b[0..<v.n],key[..]) {return nf4_refuse_s(r,NF4_TAG,i);}
      role:=-1;for k:0..<4 {if n32_is(t.v[2],NF4_ROLES[k]) {role=k;}}
      if role<0 {return nf4_refuse_s(r,NF4_ROLE,i);}
      if r.author_n<NF4_MAX_AUTHORS {r.authors[r.author_n]=NF4Author.{key=key,role=role};r.author_n+=1;}
    }
    i+=1;
  }
  if !has_title || r.title.n==0 {return nf4_refuse_s(r,NF4_MISSING,-1);}
  r.status=NF4_OK;return r;
}
```

<!-- source: coax/lib/nostr/nipf4.coax episode -->
```jai
nf4_episode :: (e:NostrEvent)->NF4Episode {
  r:NF4Episode;if e.error!=NE_OK {return r;}
  if e.kind!=NF4_KIND_EPISODE {return nf4_refuse_e(r,NF4_KIND,-1);}
  has_title:=false;i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return nf4_refuse_e(r,NF4_LIMIT,i);}
    if t.error!=NT_OK {return nf4_refuse_e(r,NF4_EVENT,i);}
    tag:=t.v[0];
    if n32_is(tag,"title") && !has_title {has_title=true;r.title=t.v[1];}
    else if n32_is(tag,"audio") {
      if t.v[1].n==0 {return nf4_refuse_e(r,NF4_EMPTY,i);}
      // A media type: something, a slash, something.
      m:=t.v[2];ms:=cast(string)m.b[0..<m.n];slash:=0;
      while slash<ms.count && ms[slash]!='/' {slash+=1;}
      if ms.count!=0 && (slash==0 || slash>=ms.count-1) {return nf4_refuse_e(r,NF4_MEDIA,i);}
      if r.audios==0 {r.audio=t.v[1];r.has_type=ms.count!=0;r.type=m;}
      r.audios+=1;
    }
    else if n32_is(tag,"image") {r.has_image=true;}
    else if n32_is(tag,"description") {r.has_description=true;}
    i+=1;
  }
  if !has_title || r.title.n==0 || r.audios==0 {return nf4_refuse_e(r,NF4_MISSING,-1);}
  r.status=NF4_OK;return r;
}
```

Anyone can claim anyone as an author, so NIP-F4 says a client must check a
claim against the author's own kind 10164 before showing it.
`nf4_credits` takes each claimed author's newest list (the lower id first
at the same time) and confirms the claim when that list names the show's
key.

<!-- source: coax/lib/nostr/nipf4.coax credits -->
```jai
// Which of a show's claimed authors confirm it: each claimed author's
// newest kind 10164 list in lists (the lower id first at the same time)
// must name the show's podcast key.
nf4_credits :: (show:NostrEvent,lists:[]NostrEvent)->NF4Credits {
  c:NF4Credits;
  s:=nf4_show(show);if s.status!=NF4_OK {c.status=NF4_KIND;return c;}
  for a:0..<s.author_n {
    best:=-1;
    for k:0..<lists.count {
      e:=lists[k];
      if e.error!=NE_OK {continue;}
      if e.kind!=NF4_KIND_AUTHORED {continue;}
      if !ns_equal32(e.public_key,s.authors[a].key) {continue;}
      newer:=best== -1;
      if !newer {
        b:=lists[best];
        newer=e.created_at>b.created_at || (e.created_at==b.created_at && nf4_before(e.id,b.id));
      }
      if newer {best=k;}
    }
    if best>=0 {
      l:=nf4_authored(lists[best]);
      for k:0..<l.n {if l.status==NF4_OK && ns_equal32(l.shows[k],show.public_key) {c.confirmed[a]=true;}}
    }
    if c.confirmed[a] {c.shown+=1;} else {c.unconfirmed+=1;}
  }
  c.status=NF4_OK;return c;
}
```

NIP-F4 names the author list kind 10164 in its text and 10064 in its
example; this code takes the text's 10164. Favorite podcasts (kind 10054)
are a NIP-51 list (chapter 57); nothing here reads them.

### Worked example: whose name is on Relay Hour

Pod's show S claims Alice as host and Bob as cohost. Alice's kind 10164
list A names Pod. Bob has published no list. So Alice is confirmed and
shown, and Bob is not: shown 1, unconfirmed 1. If Bob published a list
naming Pod, he would be shown too; if he later replaced it with one that
doesn't, he would not, because only the newest list counts.

## The example

The example is
[nostr/examples/small-content.coax](../examples/small-content.coax). It
signs 14 events with BIP-340 test keys: the torrent TOR, the
games G1 (NIP-64's Ruy Lopez example) and G2, the listing L, the proofs PB
and PD, the found logs FB, FD and FC, the locked listing LK, the curation
list C, the show S, Alice's author list A and the episode E. Each is read
back with its id and signature checked, so each round-trips: written,
serialized, read, and its values printed from the reading. Then it reads
each with this chapter's code. The refusals use unsigned events, because
no signer here would write them.

<!-- source: nostr/examples/small-content.coax example -->
```jai
#load "../../coax/lib/nostr/nip35.coax";
#load "../../coax/lib/nostr/nip64.coax";
#load "../../coax/lib/nostr/nipcc.coax";
#load "../../coax/lib/nostr/nipf4.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

TORRENT :: [10] string.["ok", "event", "kind", "missing x", "bad hash", "bad file size", "empty value", "bad i prefix", "limit", "repeated x"];
PGN :: [11] string.["ok", "event", "kind", "empty", "bad tag pair", "bad move", "open comment", "unbalanced variation", "unfinished game", "Result tag disagrees", "no room"];
CACHE :: [20] string.["ok", "event", "kind", "bad tag", "missing", "bad geohash", "bad score", "bad size", "bad key", "first-to-find without a key", "F without first-to-find", "limit", "wrong target", "no key", "no proof", "proof refused", "proof signed by another key", "proof for another finder", "proof for another cache", "proof content wrong"];
PODCAST :: [9] string.["ok", "event", "kind", "bad tag", "missing", "bad role", "empty value", "bad media type", "limit"];
ENDING :: [4] string.["unknown (*)", "white wins", "black wins", "draw"];
// BIP-340 test vectors 0 to 3's public keys, then the public keys of the
// secret keys 1 and 2. The secret key 2 is the cache's verification key,
// printed on a QR code inside the box; Pod is the podcast's own key.
PEOPLE :: [6] string.["Alice", "Bob", "Carol", "Pod", "Dave", "Cache"];
PUB :: [6] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
  "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8",
  "25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517",
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"];
// Their secret keys: published test values. Never sign with a real key in
// an example.
SECRET :: [6] string.[
  "0000000000000000000000000000000000000000000000000000000000000003",
  "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef",
  "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9",
  "0b432b2677937381aef05bb02a66ecd012773062cf3fa2549e44f58ed2401710",
  "0000000000000000000000000000000000000000000000000000000000000001",
  "0000000000000000000000000000000000000000000000000000000000000002"];
ALICE :: 0; BOB :: 1; CAROL :: 2; POD :: 3; DAVE :: 4; CACHE_KEY :: 5;
T0 :: 1761686400;

decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
hex32 :: (id: [32] u8, out: [] u8) -> string {
  own := id; r := hex_encode_checked(cast(string) own[..], out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<64];
}
key_of :: (who: int) -> [32] u8 { k: [32] u8; decode_hex(PUB[who], k[..]); return k; }
show_key :: (key: [32] u8) {
  for k: 0..<PEOPLE.count { if ns_equal32(key_of(k), key) { print("%", PEOPLE[k]); return; } }
  text: [64] u8; print("%...", hex32(key, text[..])[0..<8]);
}
say :: (t: N32Text) { own := t; print("%", cast(string) own.b[0..<own.n]); }

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

// The npub of who, and the naddr of Alice's cache d (kind 37516): NIP-19.
npub_of :: (who: int, out: [] u8) -> string {
  k := key_of(who); r := nip19_encode_checked(N19_NPUB, cast(string) k[..], out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<r.written];
}
naddr_of :: (d: string, out: [] u8) -> string {
  p: [128] u8; n := 2 + d.count; p[0] = 0; p[1] = cast(u8) d.count;
  for i: 0..<d.count { p[2 + i] = d[i]; }
  k := key_of(ALICE); p[n] = 2; p[n + 1] = 32; for i: 0..<32 { p[n + 2 + i] = k[i]; } n += 34;
  p[n] = 3; p[n + 1] = 4; p[n + 2] = 0; p[n + 3] = 0; p[n + 4] = cast(u8)(37516 >> 8); p[n + 5] = cast(u8)(37516 & 255); n += 6;
  r := nip19_encode_checked(N19_ADDR, cast(string) p[0..<n], out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<r.written];
}
// A kind 7517 proof's tags and content: finder found Alice's cache d.
proof_tags :: (finder: int, d: string, out: [] u8) -> string {
  a: [256] u8;
  n := json_raw(out, 0, "[[\"a\",\""); n = json_raw(out, n, PUB[finder]); n = json_raw(out, n, ":");
  n = json_raw(out, n, naddr_of(d, a[..])); n = json_raw(out, n, "\"]]");
  #assert n > 0;
  return cast(string) out[0..<n];
}
proof_content :: (finder: int, out: [] u8) -> string {
  b: [128] u8; n := json_raw(out, 0, NCC_PROOF_TEXT); n = json_raw(out, n, npub_of(finder, b[..]));
  return cast(string) out[0..<n];
}
// A found log's tags: Alice's cache d, then the proof's JSON when given.
found_tags :: (d: string, proof: string, out: [] u8) -> string {
  n := json_raw(out, 0, "[[\"a\",\"37516:"); n = json_raw(out, n, PUB[ALICE]); n = json_raw(out, n, ":");
  n = json_raw(out, n, d); n = json_raw(out, n, "\"]");
  if proof.count > 0 { n = json_raw(out, n, ",[\"verification\","); n = json_write_string(out, n, proof); n = json_raw(out, n, "]"); }
  n = json_raw(out, n, "]");
  #assert n > 0;
  return cast(string) out[0..<n];
}
event_text :: (e: NostrEvent) -> string { return e.original; }

// The events. Templates whose tags hold proofs or whose content names an
// npub are built by tags_of and content_of.
EVENTS :: 14;
NAME :: [EVENTS] string.["TOR", "G1", "G2", "L", "PB", "PD", "FB", "FD", "FC", "LK", "C", "S", "A", "E"];
WHO :: [EVENTS] int.[ALICE, BOB, CAROL, ALICE, CACHE_KEY, CACHE_KEY, BOB, DAVE, CAROL, ALICE, CAROL, POD, ALICE, POD];
AT :: [EVENTS] int.[0, 10, 20, 30, 200, 100, 200, 100, 50, 300, 40, 60, 70, 80];
KIND :: [EVENTS] int.[2003, 64, 64, 37516, 7517, 7517, 7516, 7516, 7516, 37516, 37517, 10154, 10164, 54];
TAGS :: [EVENTS] string.[
  "[[\"title\",\"Big Buck Bunny\"],[\"x\",\"dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c\"],[\"file\",\"Big Buck Bunny/Big Buck Bunny.mp4\",\"276134947\"],[\"file\",\"Big Buck Bunny/poster.jpg\",\"310380\"],[\"tracker\",\"udp://tracker.example.org:1337\"],[\"i\",\"tcat:video,movie\"],[\"i\",\"imdb:tt1254207\"],[\"t\",\"movie\"]]",
  "[[\"alt\",\"Fischer vs. Spassky, the Ruy Lopez\"]]",
  "[[\"alt\",\"A broken game\"]]",
  "[[\"d\",\"oak\"],[\"name\",\"Old Oak\"],[\"g\",\"u4xsu6ry\"],[\"g\",\"u4x\"],[\"D\",\"3\"],[\"T\",\"2\"],[\"S\",\"small\"],[\"hint\",\"In the branches\"],[\"n\",\"first-to-find\"],[\"verification\",\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\"]]",
  "", "", "", "", "",
  "[[\"d\",\"oak\"],[\"name\",\"Old Oak\"],[\"g\",\"u4xsu6ry\"],[\"g\",\"u4x\"],[\"D\",\"3\"],[\"T\",\"2\"],[\"S\",\"small\"],[\"hint\",\"In the branches\"],[\"n\",\"first-to-find\"],[\"verification\",\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\"],[\"t\",\"archived\"],[\"F\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"]]",
  "[[\"d\",\"park-trail\"],[\"title\",\"Park trail\"],[\"a\",\"37516:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:oak\"]]",
  "[[\"title\",\"Relay Hour\"],[\"image\",\"https://example.com/cover.jpg\"],[\"description\",\"Weekly talk about relays.\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"host\"],[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\",\"cohost\"]]",
  "[[\"p\",\"25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517\"]]",
  "[[\"title\",\"Episode 1: Outboxes\"],[\"audio\",\"https://example.com/1.mp3\",\"audio/mpeg\"],[\"audio\",\"https://example.com/1.ogg\",\"audio/ogg\"]]"];
CONTENT :: [EVENTS] string.[
  "A short open movie.",
  "[White \"Fischer, Robert J.\"]\n[Black \"Spassky, Boris V.\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 {This opening is called the Ruy Lopez.} *",
  "1. e4 e9 *",
  "Under the old oak by the pond.",
  "", "", "Found it! Great hiding spot.", "First!", "Found it too.", "Under the old oak by the pond.",
  "One cache by the pond.", "", "", "We compare outbox routing on three relays."];

// The tags of event k: its template, or a proof's or found log's, which
// need the proofs signed before them.
tags_of :: (k: int, all: [] NostrEvent, out: [] u8) -> string {
  if k == 4 { return proof_tags(BOB, "oak", out); }
  if k == 5 { return proof_tags(DAVE, "oak", out); }
  if k == 6 { return found_tags("oak", event_text(all[4]), out); }
  if k == 7 { return found_tags("oak", event_text(all[5]), out); }
  if k == 8 { return found_tags("oak", event_text(all[4]), out); }   // Carol copies Bob's proof
  n := json_raw(out, 0, TAGS[k]); #assert n > 0;
  return cast(string) out[0..<n];
}
content_of :: (k: int, out: [] u8) -> string {
  if k == 4 { return proof_content(BOB, out); }
  if k == 5 { return proof_content(DAVE, out); }
  return CONTENT[k];
}

show_torrent :: (label: string, e: NostrEvent) {
  t := n35_read(e);
  print("%: %", label, TORRENT[t.status]);
  if t.at >= 0 { print(" at tag %", t.at); }
  if t.status != N35_OK { print("\n"); return; }
  print(", \""); say(t.title); print("\", files %, % bytes, trackers %, references % (unknown %), topics %\n", t.files, t.bytes, t.tracker_n, t.refs, t.unknown, t.topics);
  link: [512] u8; n := n35_magnet(t, link[..]);
  print("  magnet: %\n", cast(string) link[0..<n]);
}
show_game :: (label: string, e: NostrEvent) {
  work: [2048] u8; p := n64_read(e, work[..]);
  print("%: ", label);
  if p.status != N64_OK { print("invalid PGN, % at byte %\n", PGN[p.status], p.at); return; }
  print("valid PGN, games %, moves %, tag pairs %, result %\n", p.games, p.moves, p.tags, ENDING[p.result]);
}
show_listing :: (label: string, e: NostrEvent) {
  l := ncc_listing(e);
  print("%: %", label, CACHE[l.status]);
  if l.at >= 0 { print(" at tag %", l.at); }
  if l.status != NCC_OK { print("\n"); return; }
  print(", \""); say(l.name); print("\" (d "); say(l.d); print("), "); say(l.type);
  print(", D% T%, %, geohashes %", l.difficulty, l.terrain, NCC_SIZES[l.size], l.geohashes);
  if l.first_to_find { print(", first-to-find"); }
  if l.has_key { print(", key "); show_key(l.key); }
  if l.archived { print(", archived"); }
  if l.has_winner { print(", locked to "); show_key(l.winner); }
  print("\n");
}
show_found :: (label: string, e: NostrEvent) {
  f := ncc_found(e);
  print("%: %", label, CACHE[f.status]);
  if f.at >= 0 { print(" at tag %", f.at); }
  if f.status != NCC_OK { print("\n"); return; }
  print(", by "); show_key(e.public_key); print(" at T0+%, of % ", e.created_at - T0, f.cache.address_kind);
  print("%", cast(string) f.cache.d[0..<f.cache.d_n]); print(" by "); show_key(f.cache.pubkey);
  print(", %\n", ifx f.proof_at >= 0 then "with a proof" else "no proof");
}
show_verdict :: (label: string, listing: NostrEvent, found: NostrEvent) {
  text: [NCC_MAX_PROOF] u8; scratch: [NCC_MAX_PROOF] u8; p := ncc_proof(found, text[..], scratch[..]);
  print("%: %\n", label, CACHE[ncc_verify(listing, found, p)]);
}
show_winner :: (label: string, listing: NostrEvent, logs: [] NostrEvent) {
  texts: [3][NCC_MAX_PROOF] u8; scratch: [NCC_MAX_PROOF] u8; proofs: [3] NostrEvent;
  for k: 0..<logs.count { proofs[k] = ncc_proof(logs[k], texts[k][..], scratch[..]); }
  w := ncc_winner(listing, logs, proofs[0..<logs.count]);
  print("%: %, verified %, refused %, ", label, CACHE[w.status], w.verified, w.refused);
  if w.index >= 0 { print("winner "); show_key(w.finder); print(" (log %)", w.index); } else { print("no winner"); }
  print("%\n", ifx w.locked then ", locked by F" else "");
}
show_show :: (label: string, e: NostrEvent) {
  s := nf4_show(e);
  print("%: %", label, PODCAST[s.status]);
  if s.at >= 0 { print(" at tag %", s.at); }
  if s.status != NF4_OK { print("\n"); return; }
  print(", \""); say(s.title); print("\" by "); show_key(e.public_key); print(", claims");
  for k: 0..<s.author_n { print(" "); show_key(s.authors[k].key); print(" (%)", ifx s.authors[k].role == NF4_NONE then "no role" else NF4_ROLES[s.authors[k].role]); }
  print("\n");
}
show_episode :: (label: string, e: NostrEvent) {
  p := nf4_episode(e);
  print("%: %", label, PODCAST[p.status]);
  if p.at >= 0 { print(" at tag %", p.at); }
  if p.status != NF4_OK { print("\n"); return; }
  print(", \""); say(p.title); print("\", audio "); say(p.audio);
  if p.has_type { print(" ("); say(p.type); print(")"); }
  print(", % audio tags\n", p.audios);
}

// An unsigned event by Carol with these tags and content, parsed but not
// verified: its id and sig are zeros. Used for the refusals.
unsigned :: (kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"");
  n = json_raw(out, n, PUB[CAROL]); n = json_raw(out, n, "\",\"created_at\":1761686400,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, content);
  n = json_raw(out, n, ",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

main :: () {
  all: [EVENTS] NostrEvent; buf: [EVENTS][8192] u8; tags: [4096] u8; text: [256] u8;
  for k: 0..<EVENTS {
    all[k] = sign(WHO[k], T0 + AT[k], KIND[k], tags_of(k, all[..], tags[..]), content_of(k, text[..]), buf[k][..]);
  }
  print("each event's id, recomputed and its signature checked\n");
  for k: 0..<EVENTS { hex: [64] u8; print("%: %, verified %\n", NAME[k], hex32(all[k].id, hex[..]), all[k].verified); }

  print("NIP-35 torrent\n");
  show_torrent("TOR", all[0]);
  print("NIP-64 chess\n");
  show_game("G1", all[1]); show_game("G2", all[2]);
  print("NIP-CC geocaching\n");
  show_listing("L", all[3]);
  show_found("FB", all[6]); show_found("FD", all[7]); show_found("FC", all[8]);
  show_verdict("FB proves", all[3], all[6]);
  show_verdict("FD proves", all[3], all[7]);
  show_verdict("FC proves", all[3], all[8]);
  logs: [3] NostrEvent; logs[0] = all[6]; logs[1] = all[7]; logs[2] = all[8];
  show_winner("first to find L", all[3], logs[..]);
  show_listing("LK", all[9]);
  show_winner("first to find LK", all[9], logs[..]);
  c := ncc_list(all[10]);
  print("C: %, \"", CACHE[c.status]); say(c.title); print("\", % caches\n", c.caches);
  print("NIP-F4 podcast\n");
  show_show("S", all[11]);
  lists: [1] NostrEvent; lists[0] = all[12];
  cr := nf4_credits(all[11], lists[..]);
  s := nf4_show(all[11]);
  print("credits:");
  for k: 0..<s.author_n { print(" "); show_key(s.authors[k].key); print(" %;", ifx cr.confirmed[k] then "confirmed" else "not confirmed"); }
  print(" shown %, unconfirmed %\n", cr.shown, cr.unconfirmed);
  show_episode("E", all[13]);

  print("refusals\n");
  bad: [4096] u8;
  show_torrent("torrent without x", unsigned(2003, "[[\"title\",\"T\"],[\"file\",\"a.txt\"]]", "", bad[..]));
  show_torrent("torrent with a short info hash", unsigned(2003, "[[\"x\",\"dd8255ecdc7ca55f\"]]", "", bad[..]));
  show_game("game whose Result tag disagrees", unsigned(64, "[]", "[Result \"1-0\"] 1. e4 0-1", bad[..]));
  show_game("game with an open comment", unsigned(64, "[]", "1. e4 {unclosed *", bad[..]));
  show_listing("listing with difficulty 6", unsigned(37516, "[[\"d\",\"x\"],[\"name\",\"N\"],[\"g\",\"u4x\"],[\"D\",\"6\"],[\"T\",\"1\"],[\"S\",\"small\"]]", "", bad[..]));
  show_listing("first-to-find without a key", unsigned(37516, "[[\"d\",\"x\"],[\"name\",\"N\"],[\"g\",\"u4x\"],[\"D\",\"1\"],[\"T\",\"1\"],[\"S\",\"small\"],[\"n\",\"first-to-find\"]]", "", bad[..]));
  show_found("found log of an article", unsigned(7516, "[[\"a\",\"30023:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:oak\"]]", "", bad[..]));
  nip: [4096] u8;
  show_verdict("found log carrying NIP-CC's example proof", all[3], unsigned(7516, found_tags("oak", NCC_PROOF_NIP, nip[..]), "", bad[..]));
  show_show("show crediting a guest", unsigned(10154, "[[\"title\",\"T\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"guest\"]]", "", bad[..]));
  show_episode("episode without audio", unsigned(54, "[[\"title\",\"Episode 2\"]]", "", bad[..]));
}
// NIP-CC's verification example without its whitespace: no id, a
// placeholder signature, and an npub and naddr that don't decode.
NCC_PROOF_NIP :: "{\"kind\":7517,\"content\":\"Geocache verification for npub1qc0lc5lxnhxnfxlw2lxkv4x4vp6xsf4d5qwvlhfx6qmz6x4nfhqd8h2z3\",\"tags\":[[\"a\",\"0461fcbecc4c3374439932d6b8f11269ccdb7cc973ad7a50ae362db135a474dd:naddr1qqxnzd3e8q6n2dfk8qcnjve48qmnsw3jsqgswaehxw309aex2mrp0yhx6tpdsek6w309aex2mrp0yh56tnwdus8vatjvs6kzdrz956k7tjzw6qzypzgd2dmgxhxf34hnlw2y03nckr8f4g6mw9flxqq65v94zkp77rqfgrf8\"]],\"pubkey\":\"6805d4e5c0df48b4f76e2fdcb67a2acb1d97567b01c6fe17a236dc32f34f1c07\",\"created_at\":1672531200,\"sig\":\"a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456789012345678901234567890abcdef1234567890abcdef1234567890abcdef12\"}";
```

Run it:

```shell
coax/bin/coax run nostr/examples/small-content.coax --run-steps 250000000
```

It prints this transcript, which is kept in
[nostr/fixtures/small-content.out](../fixtures/small-content.out) and which
the book's checks compare in debug and release. It exits with status 0.

```text
each event's id, recomputed and its signature checked
TOR: 6d761b12ab026f6e2f8fd8004c944d90933106104b94659b29fa16848cb92a1e, verified true
G1: d5977b40c51709866ab8eed83044d6611b401d1c9c59d90b71e560f10f209d38, verified true
G2: 01c338bb43dca47df38128ac798489e0050b9ac1ce0f5076b6929b58a239f3c9, verified true
L: f4731b9a17ede85099cf58f457d3e74bfa6f884c8acf29d19f22bcaa686d9985, verified true
PB: 6dcb54235470b5c71d029e7b85f75631c34e3257ff6d04c3dd6c27ab7c276389, verified true
PD: 67c9cf0ea42d77dd80b5e1dbff2d7aa8a05bf5fa22b4448585df431bb816009a, verified true
FB: d0a88acb3ec102bfa8a144180c78a6d16f579d916d718ca28422aeb1ba1f17e1, verified true
FD: 5805c34d58a1439f945f241f4f2dc16b16130266066ef76209552c6dc462dd61, verified true
FC: f322b5ce5b34c796132a939792142c002bd1f5c16343c0b94b6fcb1225116ad1, verified true
LK: 8a70460ca6bee4c7bfb45afec5518fb0fa6a7166c593b74a1d39b6b925c2ef53, verified true
C: dd2d4d06a2ce7375ead761020ccb39df1fdcb2b7b8bfd825d35251882b0b55da, verified true
S: dc3f330cf1a1ba2fd7fb3b966cc08e169e8edbb8778ec8c6a98d9fe43c927f08, verified true
A: 758e8c6174931dfa4a0132090246b4c29dc502aca227ee395685816a7241fff3, verified true
E: e8c89e862675529e74dd7a4210fb51adf3191c169e2f65b841b410757380eaa9, verified true
NIP-35 torrent
TOR: ok, "Big Buck Bunny", files 2, 276445327 bytes, trackers 1, references 2 (unknown 0), topics 1
  magnet: magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big%20Buck%20Bunny&tr=udp%3A%2F%2Ftracker.example.org%3A1337
NIP-64 chess
G1: valid PGN, games 1, moves 5, tag pairs 2, result unknown (*)
G2: invalid PGN, bad move at byte 6
NIP-CC geocaching
L: ok, "Old Oak" (d oak), traditional, D3 T2, small, geohashes 2, first-to-find, key Cache
FB: ok, by Bob at T0+200, of 37516 oak by Alice, with a proof
FD: ok, by Dave at T0+100, of 37516 oak by Alice, with a proof
FC: ok, by Carol at T0+50, of 37516 oak by Alice, with a proof
FB proves: ok
FD proves: ok
FC proves: proof for another finder
first to find L: ok, verified 2, refused 1, winner Dave (log 1)
LK: ok, "Old Oak" (d oak), traditional, D3 T2, small, geohashes 2, first-to-find, key Cache, archived, locked to Bob
first to find LK: ok, verified 2, refused 1, winner Bob (log 0), locked by F
C: ok, "Park trail", 1 caches
NIP-F4 podcast
S: ok, "Relay Hour" by Pod, claims Alice (host) Bob (cohost)
credits: Alice confirmed; Bob not confirmed; shown 1, unconfirmed 1
E: ok, "Episode 1: Outboxes", audio https://example.com/1.mp3 (audio/mpeg), 2 audio tags
refusals
torrent without x: missing x
torrent with a short info hash: bad hash at tag 0
game whose Result tag disagrees: invalid PGN, Result tag disagrees at byte 21
game with an open comment: invalid PGN, open comment at byte 6
listing with difficulty 6: bad score at tag 3
first-to-find without a key: first-to-find without a key
found log of an article: wrong target at tag 0
found log carrying NIP-CC's example proof: proof refused
show crediting a guest: bad role at tag 1
episode without audio: missing
```

The magnet link, the PGN readings, the winners and the credits match the
worked examples. The refusals show each broken input and its exact
diagnostic:

- `torrent without x`: only `title` and `file` tags.
- `torrent with a short info hash`: 16 hex digits, at tag 0.
- G2, `1. e4 e9 *`: a signed event whose PGN is shown as invalid, bad move
  at byte 6.
- `game whose Result tag disagrees`: `[Result "1-0"]` and the ending `0-1`,
  at byte 21.
- `game with an open comment`: `{` at byte 6 and no `}`.
- `listing with difficulty 6`: `["D","6"]`, at tag 3.
- `first-to-find without a key`: an `n` tag `first-to-find` and no
  `verification` tag.
- `found log of an article`: the `a` tag names kind 30023, at tag 0.
- `found log carrying NIP-CC's example proof`: NIP-CC's own kind 7517
  example, which has no id, so it doesn't read.
- `show crediting a guest`: a `p` tag with the role `guest`, at tag 1.
- `episode without audio`: only a `title` tag.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/small-content.coax](../examples/checks/small-content.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The ids of all 14 events, from a Python script written from NIP-01,
  NIP-19, BIP-340 and the four NIPs, with its own bech32 for the npubs and
  naddrs and its own BIP-340 signer for the two proofs. The found logs
  carry the proofs' JSON, signature included, so their ids matching means
  Python's signatures equal the Coax signer's. The checks read each event
  with Python's id and require that the recomputed id matches, and verify
  Python's two proofs.
- The torrent's byte count and magnet link by hand (Python's
  `urllib.parse.quote` agrees).
- NIP-64's five PGN examples and its note example.
- NIP-CC's listing, found log and curation list examples, and its
  verification example refused.
- The geocaching verdicts and winners by hand, in two orders and without
  Dave's log; the podcast credits by hand.

<!-- source: nostr/examples/checks/small-content.coax check -->
```jai
// The ids from a Python script written from NIP-01, NIP-19, BIP-340,
// NIP-35, NIP-CC and NIP-F4 (it shares no code with Coax): hashlib.sha256
// of json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False), with its own bech32 for the
// npubs and naddrs and its own BIP-340 signer (auxiliary randomness zero)
// for the two proofs, whose JSON the found logs carry. The found logs' ids
// matching means the proofs' signatures match too.
PY_ID :: [EVENTS] string.[
  "6d761b12ab026f6e2f8fd8004c944d90933106104b94659b29fa16848cb92a1e",
  "d5977b40c51709866ab8eed83044d6611b401d1c9c59d90b71e560f10f209d38",
  "01c338bb43dca47df38128ac798489e0050b9ac1ce0f5076b6929b58a239f3c9",
  "f4731b9a17ede85099cf58f457d3e74bfa6f884c8acf29d19f22bcaa686d9985",
  "6dcb54235470b5c71d029e7b85f75631c34e3257ff6d04c3dd6c27ab7c276389",
  "67c9cf0ea42d77dd80b5e1dbff2d7aa8a05bf5fa22b4448585df431bb816009a",
  "d0a88acb3ec102bfa8a144180c78a6d16f579d916d718ca28422aeb1ba1f17e1",
  "5805c34d58a1439f945f241f4f2dc16b16130266066ef76209552c6dc462dd61",
  "f322b5ce5b34c796132a939792142c002bd1f5c16343c0b94b6fcb1225116ad1",
  "8a70460ca6bee4c7bfb45afec5518fb0fa6a7166c593b74a1d39b6b925c2ef53",
  "dd2d4d06a2ce7375ead761020ccb39df1fdcb2b7b8bfd825d35251882b0b55da",
  "dc3f330cf1a1ba2fd7fb3b966cc08e169e8edbb8778ec8c6a98d9fe43c927f08",
  "758e8c6174931dfa4a0132090246b4c29dc502aca227ee395685816a7241fff3",
  "e8c89e862675529e74dd7a4210fb51adf3191c169e2f65b841b410757380eaa9"];
// Python's proofs PB and PD, signed by its own BIP-340 code.
PY_PB :: "{\"id\":\"6dcb54235470b5c71d029e7b85f75631c34e3257ff6d04c3dd6c27ab7c276389\",\"pubkey\":\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\",\"created_at\":1761686600,\"kind\":7517,\"tags\":[[\"a\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659:naddr1qqpk7cttqgs0jvy2qxf93scsfy6ylp0cn4fzndf3epzcxmuekzrqrugnhnsrd7grqsqqpy5vge5j7y\"]],\"content\":\"Geocache verification for npub1mlcawle2vuw97dscxundkg6phev0atsa5t0vakzrys8hk5pt5evssm7a0a\",\"sig\":\"9e3576eaea2ea997c667cc17eb7027ed935bb3856d6e0bcff1ea0df5e7c4b06808a35e102c6fb34d2b2c615c7650f218d9a7b1516bdaac9481376ac707a8c435\"}";
PY_PD :: "{\"id\":\"67c9cf0ea42d77dd80b5e1dbff2d7aa8a05bf5fa22b4448585df431bb816009a\",\"pubkey\":\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\",\"created_at\":1761686500,\"kind\":7517,\"tags\":[[\"a\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:naddr1qqpk7cttqgs0jvy2qxf93scsfy6ylp0cn4fzndf3epzcxmuekzrqrugnhnsrd7grqsqqpy5vge5j7y\"]],\"content\":\"Geocache verification for npub10xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqpkge6d\",\"sig\":\"9e713b83f684b18c357cba4ae80f3af39d9227a0e1535b588f76a05b76bd0c20efb96e1da96312560c16af5ba663c3e2aea504d61a79aa4b42a9dde48cfbe0c4\"}";
SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

// Event k with Python's id and a zero signature, read with its id
// recomputed; the signature is not checked. The proofs are Python's.
py :: (k: int, out: [] u8, scratch: [] u8) -> NostrEvent {
  if k == 4 { return nostr_event_read(PY_PB, scratch, false); }
  if k == 5 { return nostr_event_read(PY_PD, scratch, false); }
  tags: [4096] u8; text: [256] u8; t := "";
  if k == 6 || k == 8 { t = found_tags("oak", PY_PB, tags[..]); }
  else if k == 7 { t = found_tags("oak", PY_PD, tags[..]); }
  else { n := json_raw(tags[..], 0, TAGS[k]); t = cast(string) tags[0..<n]; }
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, PY_ID[k]);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB[WHO[k]]);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, T0 + AT[k]);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, KIND[k]);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, t);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, content_of(k, text[..]));
  n = json_raw(out, n, ",\"sig\":\""); n = json_raw(out, n, SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}

#test "the ids are Python's, and Python's proofs verify" #steps 2000000000 {
  buf: [EVENTS][4096] u8; scratch: [8192] u8;
  for k: 0..<EVENTS { e := py(k, buf[k][..], scratch[..]); #assert e.error == NE_OK && ns_equal32(e.computed_id, e.id); }
  p := nostr_event_verify_vartime(PY_PB, scratch[..], false); #assert p.error == NE_OK && p.verified;
  p = nostr_event_verify_vartime(PY_PD, scratch[..], false); #assert p.error == NE_OK && p.verified;
}

// By hand: the torrent's two files are 276134947 + 310380 = 276445327
// bytes. Its magnet link percent-encodes the title's spaces as %20 and the
// tracker's : and / as %3A and %2F (Python's urllib.parse.quote with
// safe "-_.~" gives the same).
#test "the torrent and its magnet link" {
  buf: [4096] u8; scratch: [8192] u8; link: [512] u8;
  t := n35_read(py(0, buf[..], scratch[..]));
  #assert t.status == N35_OK && t.bytes == 276445327 && t.files == 2 && t.refs == 2;
  n := n35_magnet(t, link[..]);
  #assert cast(string) link[0..<n] == "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big%20Buck%20Bunny&tr=udp%3A%2F%2Ftracker.example.org%3A1337";
}

// NIP-64's five PGN examples (the first without the two // lines that
// introduce it in the NIP: they are commentary, not PGN), and the kind 64
// note example.
#test "NIP-64's examples" {
  #assert n64_pgn("*").status == N64_OK && n64_pgn("1. e4 *").status == N64_OK;
  #assert n64_pgn("[White \"Fischer, Robert J.\"]\n[Black \"Spassky, Boris V.\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 {This opening is called the Ruy Lopez.} *").moves == 5;
  f := n64_pgn(N64_T_FISCHER); #assert f.status == N64_OK && f.moves == 85 && f.result == N64_DRAW;
  two := n64_pgn(N64_T_TWO); #assert two.status == N64_OK && two.games == 2 && two.result == N64_WHITE;
  // Read literally, the first example's // lines are not PGN.
  #assert n64_pgn("// A game where nothing is known.\n*").status == N64_MOVE;
}

// NIP-CC's examples: three listings, the found log, the curation list. Its
// DNF and note logs are NIP-22 comments; its verification example has no
// id, a placeholder signature, and an npub and naddr whose bech32
// checksums fail (Python's bech32 agrees), so the find it supports is
// refused.
#test "NIP-CC's examples" {
  l := ncc_listing(n32_event(37516, "[[\"d\",\"verified-treasure-1748619568669\"],[\"name\",\"Verified Treasure\"],[\"g\",\"u4xsu6ry\"],[\"D\",\"3\"],[\"T\",\"2\"],[\"S\",\"small\"],[\"t\",\"traditional\"],[\"hint\",\"Look for the secret code\"],[\"verification\",\"6805d4e5c0df48b4f76e2fdcb67a2acb1d97567b01c6fe17a236dc32f34f1c07\"]]"));
  #assert l.status == NCC_OK && l.has_key && l.difficulty == 3;
  f := ncc_found(n32_event(7516, "[[\"a\",\"37516:0461fcbecc4c3374439932d6b8f11269ccdb7cc973ad7a50ae362db135a474dd:first-treasure-1748619568668\"]]"));
  #assert f.status == NCC_OK && f.proof_at == -1;
  b: [8] u8;
  #assert nip19_decode_checked("npub1qc0lc5lxnhxnfxlw2lxkv4x4vp6xsf4d5qwvlhfx6qmz6x4nfhqd8h2z3", b[..], false).error != NC_OK;
  text: [4096] u8; scratch: [4096] u8; logs: [4096] u8; nip: [4096] u8;
  fl := n32_event_in(7516, found_tags("oak", NCC_PROOF_NIP, nip[..]), logs[..]);
  #assert ncc_proof(fl, text[..], scratch[..]).error != NE_OK;
}

// The geocaching by hand. L is first-to-find with the cache key. FB (Bob,
// T0+200) and FD (Dave, T0+100) carry proofs signed by that key for
// themselves; FC (Carol, T0+50) carries Bob's proof, which names Bob, so it
// is refused although it is the earliest. Of the two verified logs FD is
// earlier: Dave wins. LK locks the winner to Bob with an F tag: Bob wins,
// as NIP-CC says, whatever the timestamps.
#test "the first-to-find winner matches the hand count, in any order" #steps 2000000000 {
  buf: [EVENTS][4096] u8; scratch: [8192] u8; ev: [EVENTS] NostrEvent;
  for k: 0..<EVENTS { ev[k] = py(k, buf[k][..], scratch[..]); }
  texts: [3][4096] u8; s: [4096] u8; logs: [3] NostrEvent; proofs: [3] NostrEvent;
  logs[0] = ev[8]; logs[1] = ev[6]; logs[2] = ev[7];
  for k: 0..<3 { proofs[k] = ncc_proof(logs[k], texts[k][..], s[..]); #assert proofs[k].verified; }
  #assert ncc_verify(ev[3], ev[8], proofs[0]) == NCC_FINDER && ncc_verify(ev[3], ev[6], proofs[1]) == NCC_OK;
  w := ncc_winner(ev[3], logs[..], proofs[..]);
  #assert w.status == NCC_OK && w.index == 2 && ns_equal32(w.finder, key_of(DAVE)) && w.verified == 2 && w.refused == 1;
  w = ncc_winner(ev[9], logs[..], proofs[..]);
  #assert w.locked && w.index == 1 && ns_equal32(w.finder, key_of(BOB));
  // Without Dave's log Bob wins L; reversed, the answer is the same.
  w = ncc_winner(ev[3], logs[0..<2], proofs[0..<2]); #assert w.index == 1 && ns_equal32(w.finder, key_of(BOB));
  rl: [3] NostrEvent; rp: [3] NostrEvent; for k: 0..<3 { rl[k] = logs[2 - k]; rp[k] = proofs[2 - k]; }
  w = ncc_winner(ev[3], rl[..], rp[..]); #assert w.index == 0 && ns_equal32(w.finder, key_of(DAVE));
}

// The podcast by hand: S claims Alice (host) and Bob (cohost). Alice's
// kind 10164 list A names Pod; Bob has none, so only Alice is shown.
#test "the podcast credits and episode" {
  buf: [EVENTS][4096] u8; scratch: [8192] u8; ev: [EVENTS] NostrEvent;
  for k: 0..<EVENTS { ev[k] = py(k, buf[k][..], scratch[..]); }
  c := nf4_credits(ev[11], ev[12..<13]); #assert c.confirmed[0] && !c.confirmed[1] && c.shown == 1;
  c = nf4_credits(ev[11], ev[0..<11]); #assert c.shown == 0 && c.unconfirmed == 2;
  e := nf4_episode(ev[13]); #assert e.status == NF4_OK && e.audios == 2 && n32_is(e.type, "audio/mpeg");
  // NIP-F4's examples with their placeholders filled read; its author-list
  // example is kind 10064, not the 10164 its text names, and is refused.
  s := nf4_show(n32_event(10154, "[[\"title\",\"T\"],[\"image\",\"https://x\"],[\"description\",\"D\"],[\"website\",\"https://w\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"editor\"]]"));
  #assert s.status == NF4_OK && s.authors[0].role == NF4_EDITOR;
  #assert nf4_authored(n32_event(10064, "[]")).status == NF4_KIND;
}
```

```shell
coax/bin/coax test nostr/examples/checks/small-content.coax --run-steps 250000000
```

`coax test` runs 391 checks, including the loaded files' own checks; all
pass.

### What the NIPs' own examples show

None of the four NIPs publishes a signed example with an id that could be
recomputed. What their examples do show:

- NIP-35's example event uses placeholders (`<torrent-title>`) and ends its
  tag list with a comma, which is not JSON. Filled in, it reads.
- NIP-64's first PGN example starts with two `//` lines of commentary.
  Read literally, they are not PGN (PGN's comments are `{...}` and `;`), and
  the reader says bad move at byte 0. Without them, all five read.
- NIP-CC's verification example has no `id`, a placeholder signature
  (`a1b2c3d4...`), and an npub and naddr whose bech32 checksums fail:
  Python's bech32 and Coax's NIP-19 decoder both refuse them, and the npub
  is one byte short of a key. Its listing, found log and curation list
  examples read.
- NIP-F4's author list is kind 10164 in its text and 10064 in its example.

### The library's own checks

Each library file has its own `#test` blocks: the NIPs' examples, each
refusal and the tag index or byte it reports, the first-of-each-tag rules,
every bound at its edge (40 hex digits, 18 size digits, 12 geohash
characters, scores 1 and 5, a proof exactly as long as its buffer), broken
tag arrays and events, PGN's lax forms (moves without spaces, nested
variations, escapes, comments at the end), SAN's valid and invalid forms,
each of `ncc_verify`'s verdicts with proofs that change one thing, ties on
`created_at` decided at the first and last id byte, and the newest author
list replacing an older one. The checks of `ncc_verify`'s rules use
unsigned proofs marked as verified; `ncc_proof`'s own check verifies real
signatures and refuses a changed byte and a changed signature.

```shell
coax/bin/coax test coax/lib/nostr/nip35.coax
coax/bin/coax test coax/lib/nostr/nip64.coax
coax/bin/coax test coax/lib/nostr/nipcc.coax
coax/bin/coax test coax/lib/nostr/nipf4.coax
coax/bin/coax mutate coax/lib/nostr/nip35.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip64.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nipcc.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nipf4.coax --root-only
```

On 2026-11-01, `coax test` passed all 206, 207, 223 and 206
checks (with the loaded files'). `coax mutate --root-only` killed all
266 mutants of `nip35.coax`, 536 of `nip64.coax`, 485 of
`nipcc.coax` and 264 of `nipf4.coax`, with none surviving and none
marked equivalent. The full mutation run (`COAX_MUTATION_FULL=1`) and a
mutation run of the example were not run.

### Not built

- Chess move legality: a board and a move generator, so that `1. e4 e4 *`
  is refused. NIP-64 says a client SHOULD check it.
- Drawing a chessboard, ROT13 hints, cache status from recent logs, and
  NIP-CC's geohash precision rule for submissions (8 or more characters, 9
  for micro caches).
- NIP-CC's kind 1111 comment logs (they are NIP-22 comments, chapter 24)
  and NIP-GD good deeds.
- NIP-35's kind 2004 comments, and URL mapping for `i` references.
- NIP-F4's kind 10054 favorites (a NIP-51 list), fetching events from
  relays, and picking the newest kind 10154 of a podcast. The caller passes
  the events.

## Summary

- `n35_read` requires one `x` info hash of 40 hex digits, checks file
  sizes and `i` prefixes, and `n35_magnet` percent-encodes the link.
- `n64_pgn` reads PGN's import format and reports the first byte that
  breaks it, including a `Result` tag that disagrees with the game's ending;
  `n64_read` decodes a kind 64 event's content first. A game that doesn't
  read is shown as invalid. Move legality is not checked.
- `ncc_listing` requires `d`, `name`, `g`, `D`, `T` and `S`, and refuses
  `first-to-find` without a key and `F` without `first-to-find`.
- `ncc_proof` and `ncc_verify` prove a find only with a proof signed by the
  cache's key, naming the log's author and the cache. `ncc_winner` picks
  the earliest verified log, or the `F` tag's finder.
- `nf4_show` and `nf4_episode` require a title (and an audio URL), and
  `nf4_credits` shows a claimed author only when that author's newest kind
  10164 names the podcast.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Sources

- [NIP-35](../nips/official/35.md)
- [NIP-64](../nips/official/64.md)
- [NIP-CC](../nips/official/CC.md)
- [NIP-F4](../nips/official/F4.md)
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki), for the signatures in the checks
- [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986), for the magnet link's percent-encoding
- The PGN specification: [pgn-spec-commented](https://github.com/mliebelt/pgn-spec-commented/blob/main/pgn-specification.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
