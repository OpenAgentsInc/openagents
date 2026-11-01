# 57. Media and files

Nostr events don't carry media. A note names a URL, and the image, video or
file lives on a web server. NIP-92 and NIP-94 add metadata that describes
the file at that URL, including its SHA-256. Blossom (NIP-B7) goes further:
the URL itself ends in the SHA-256, so any server can serve the file and
the client can check it. This chapter reads and writes that metadata,
checks downloaded bytes against the hash, and builds the Blossom server's
check of a signed upload authorization.

Part X, Every NIP. Result: `imeta` tags (NIP-92) and kind 1063 file
metadata (NIP-94) read, written and checked against downloaded bytes;
Blossom blob addresses, kind 10063 server lists and kind 24242 upload
authorizations (NIP-B7) checked with a fixed clock. Picture (NIP-68),
video (NIP-71) and voice (NIP-A0) events read through the same `imeta`
reader. NIP-96 is read only.

## What this chapter teaches

- NIP-92 `imeta`: a tag of `"key value"` entries, split at the first
  space, with a required `url` and at least one other field.
- NIP-94: kind 1063 with `url`, `m`, `x` and the optional `ox`, `size`,
  `dim`, `blurhash`, `alt` and `fallback` tags.
- The hash check: downloaded bytes are the file only when their SHA-256
  equals `x`. A URL proves nothing; the hash does.
- NIP-68, NIP-71 and NIP-A0: picture, video and voice events put their
  media in `imeta` tags, which the same reader reads.
- NIP-B7 Blossom: a blob's address is its SHA-256, a user's servers are
  listed in kind 10063, and a kind 24242 event authorizes one upload.
- NIP-96 is `unrecommended` in favor of Blossom. It is described, not
  built.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip94.coax](../../coax/lib/nostr/nip94.coax) | `n94_imeta(e, index)` | Reads the index'th tag as an `imeta` tag into an `N94File` (`#authority`) |
| | `n94_file(e)` | Reads a kind 1063 event's tags into an `N94File` |
| | `n94_field(f, key, value)` | Checks and keeps one field; both readers use it |
| | `n94_check(f, bytes, original)` | Refuses bytes whose SHA-256 is not `x` (or `ox`), or whose count is not `size` |
| | `n94_write(f, out, at, imeta)` | Writes the fields as an `imeta` tag or as kind 1063 tags |
| [nipb7.coax](../../coax/lib/nostr/nipb7.coax) | `nb7_address(url)`, `nb7_fetch(url, bytes)` | The SHA-256 a Blossom URL names, and the check that fetched bytes hash to it (`#authority`) |
| | `nb7_blob_url(server, hash, ext, out)` | The same blob's URL on another server |
| | `nb7_servers(e, list)` | A kind 10063 event's `server` tags |
| | `nb7_auth_tags(verb, blob, expiration, out)` | The tags of a kind 24242 authorization |
| | `nb7_admit(e, verb, blob, now)`, `nb7_upload(e, body, now)` | The server's check of an authorization, and of an upload's body |

Nothing here fetches. The caller downloads bytes, verifies events (chapter
21) and passes both in. `nb7_admit` takes the clock as `now`, so every
decision can be replayed.

## Media lives elsewhere

A note that shows a picture has the picture's URL in its content. The
event is signed, but the URL is just text: the server behind it can serve
different bytes tomorrow, or to a different reader. So the metadata that
matters is `x`, the SHA-256 of the file as served. A client that checks
the hash knows it has the bytes the author meant, whichever server sent
them.

That is why this chapter treats the hash check as the decision and the URL
as a hint. Blossom makes the same choice the center of its design.

## The imeta tag

NIP-92 attaches metadata to a URL in the event's content with a tag like
this:

```jai
["imeta",
 "url https://nostr.build/i/my-image.jpg",
 "m image/jpeg",
 "dim 3024x4032",
 "alt A scenic photo overlooking the coast of Costa Rica",
 "x <SHA-256 as 64 hex digits>",
 "fallback https://nostrcheck.me/alt1.jpg"]
```

Each entry after `"imeta"` is a key and a value separated by a space. The
value may itself hold spaces, as `alt` does, so the reader splits at the
first space only. NIP-92 says an `imeta` tag MUST have a `url` and at least
one other field, and MAY have any field NIP-94 defines.

The reader keeps the fields this book uses in an `N94File`:

<!-- source: coax/lib/nostr/nip94.coax record -->
```jai
N94File :: struct {
  status:int=N94_EVENT;
  at:int=-1; entry:int=-1;            // the refusing tag, and its entry in an imeta tag
  url:N32Text; m:N32Text; blurhash:N32Text; alt:N32Text;
  has_x:bool; x:[32]u8;               // SHA-256 of the file as served
  has_ox:bool; ox:[32]u8;             // SHA-256 of the file as uploaded
  has_size:bool; size:int;            // bytes
  has_dim:bool; width:int; height:int;
  fallback:[N94_MAX_FALLBACK]N32Text; fallbacks:int;
  others:int;                         // imeta entries with keys not kept here
  seen:u32;                           // one bit per single-valued key read
}
```

### The fields

`n94_field` checks one key and value and keeps it. Both the `imeta` reader
and the kind 1063 reader call it, so the rules are the same in both
places:

- `x` and `ox` are 64 lowercase hex digits, read through chapter 21's
  `ne_lower_hex`.
- `size` is a decimal count of at most 15 digits.
- `dim` is `<width>x<height>`, both at least 1.
- `m` is `type/subtype`: one slash with something on each side. NIP-94
  says MIME types should be lowercase; the reader doesn't refuse
  `application/x-mpegURL`, which NIP-71's own example uses.
- `url`, `m`, `x`, `ox`, `size`, `dim`, `blurhash` and `alt` may appear
  once each. A second one is refused, because two `x` values would leave
  the hash check with two answers. `fallback` may repeat, up to four.
- Any other key (`thumbhash`, `image`, `service`, NIP-71's `duration` and
  `bitrate`, NIP-A0's `waveform`) is counted in `others` and not kept.
- A kept value holds at most 256 bytes. A whole entry holds at most 1,024,
  so NIP-71's 304-byte `waveform` entry reads.

<!-- source: coax/lib/nostr/nip94.coax field -->
```jai
N94_KEYS :: [9] string.["url", "m", "x", "ox", "size", "dim", "blurhash", "alt", "fallback"];
n94_key :: (s:string)->int {
  for k:0..<N94_KEYS.count {if s==N94_KEYS[k] {return k;}}
  return -1;
}
// A decimal count of at most 15 digits, or -1.
n94_number :: (s:string)->int {
  if s.count==0 || s.count>15 {return -1;}
  v:=0;
  for c:s {if c<'0' || c>'9' {return -1;}v=v*10+cast(int)(c-'0');}
  return v;
}
// type/subtype: one slash with something on each side.
n94_mime :: (s:string)->bool {
  slash:= -1;
  for i:0..<s.count {if s[i]=='/' {if slash>=0 {return false;}slash=i;}}
  return slash>0 && slash<s.count-1;
}
// Where the first c is, or s.count when there is none.
n94_find :: (s:string,c:u8)->int {for i:0..<s.count {if s[i]==c {return i;}}return s.count;}
// What follows position at, or "" when at is the end.
n94_after :: (s:string,at:int)->string {return ifx at<s.count then s[at+1..<s.count] else "";}
// One field into f: its status is N94_OK or the refusal.
n94_field :: (f:N94File,key:string,value:string)->N94File {
  o:=f;
  if key.count==0 || value.count==0 {return n94_status(o,N94_ENTRY);}
  k:=n94_key(key);
  if k<0 {o.others+=1;return n94_status(o,N94_OK);}
  if value.count>NT_MAX_VALUE {return n94_status(o,N94_LIMIT);}
  if k==N94_FALLBACK {
    if o.fallbacks>=N94_MAX_FALLBACK {return n94_status(o,N94_FALLBACKS);}
    o.fallback[o.fallbacks]=n94_text(value);o.fallbacks+=1;return n94_status(o,N94_OK);
  }
  bit:=cast(u32)1<<cast(u32)k;
  if (o.seen&bit)!=0 {return n94_status(o,N94_DUPLICATE);}
  o.seen|=bit;
  if k==N94_URL {o.url=n94_text(value);}
  if k==N94_M {if !n94_mime(value) {return n94_status(o,N94_MIME);}o.m=n94_text(value);}
  if k==N94_X {if !ne_lower_hex(value,o.x[..]) {return n94_status(o,N94_HEX);}o.has_x=true;}
  if k==N94_OX {if !ne_lower_hex(value,o.ox[..]) {return n94_status(o,N94_HEX);}o.has_ox=true;}
  if k==N94_SIZE_KEY {o.size=n94_number(value);if o.size<0 {return n94_status(o,N94_NUMBER);}o.has_size=true;}
  if k==N94_DIM_KEY {
    cross:=n94_find(value,'x');
    o.width=n94_number(value[0..<cross]);o.height=n94_number(n94_after(value,cross));
    if o.width<=0 || o.height<=0 {return n94_status(o,N94_DIM);}
    o.has_dim=true;
  }
  if k==N94_BLURHASH {o.blurhash=n94_text(value);}
  if k==N94_ALT {o.alt=n94_text(value);}
  return n94_status(o,N94_OK);
}
```

### Reading the tag

`n94_imeta` walks to the tag at `index`, checks that its name is `imeta`,
and passes each entry to `n94_field`. An entry with no space has an empty
value, which `n94_field` refuses, so the reader needs no separate case for
it. The `url` check and the "one other field" check come after the last
entry.

<!-- source: coax/lib/nostr/nip94.coax imeta -->
```jai
// The index'th tag read as an imeta tag.
n94_imeta :: (e:NostrEvent,index:int)->N94File {
  f:N94File;j:[1]Json;
  if index<0 || !nt_start(j[..],e) {return f;}
  tok:=json_next(j[..]);k:=0;
  while tok==JSON_ARRAY && k<index {skipped:=json_skip(j[..]);k+=1;tok=json_next(j[..]);}
  if tok!=JSON_ARRAY {return f;}
  v:=json_next(j[..]);i:=0;
  while v!=JSON_ARRAY_END {
    if v!=JSON_STRING {return n94_refuse(f,N94_EVENT,index,i);}
    buf:[N94_MAX_ENTRY]u8;n:=json_unescape(json_text(j[..]),buf[..]);
    if n<0 {return n94_refuse(f,N94_LIMIT,index,i);}
    s:=cast(string)buf[0..<n];
    if i==0 {
      if s!="imeta" {return n94_refuse(f,N94_NOT_IMETA,index,0);}
    } else {
      // No space leaves the value empty, which n94_field refuses.
      sp:=n94_find(s,' ');
      f=n94_field(f,s[0..<sp],n94_after(s,sp));
      if f.status!=N94_OK {return n94_refuse(f,f.status,index,i);}
    }
    i+=1;v=json_next(j[..]);
  }
  if !n94_has(f,N94_URL) {return n94_refuse(f,N94_NO_URL,index,-1);}
  if i<3 {return n94_refuse(f,N94_ALONE,index,-1);}
  return n94_refuse(f,N94_OK,-1,-1);
}
n94_has :: (f:N94File,k:int)->bool {return (f.seen&(cast(u32)1<<cast(u32)k))!=0;}
```

Each refusal names the tag (`at`) and, inside an `imeta` tag, the entry
(`entry`, counting `"imeta"` as entry 0):

| Input | Refusal |
| --- | --- |
| `["imeta","m text/plain","x 5083..."]` | `N94_NO_URL`: no url |
| `["imeta","url https://blossom.example/a.txt"]` | `N94_ALONE`: url alone |
| `["imeta","url ...","m"]` | `N94_ENTRY` at entry 2: not a key and a value |
| `["imeta","url ...","x <sha256 hash as specified in NIP 94>"]` | `N94_HEX` at entry 2 |
| `["imeta","url a","url b"]` | `N94_DUPLICATE` at entry 2 |
| `["imeta","url ...","dim 3024"]` | `N94_DIM` |
| `["imeta","url ...","m jpeg"]` | `N94_MIME` |
| `["title","t"]` | `N94_NOT_IMETA` |

NIP-92 also says each `imeta` tag SHOULD match a URL in the content, and a
client MAY ignore those that don't. Matching is the client's choice, so the
reader doesn't do it; the example shows the URL in both places.

## Kind 1063 file metadata

NIP-94 describes a file in an event of its own, kind 1063, whose content is
a caption. Each field is a tag: `["url", ...]`, `["m", ...]`,
`["x", ...]`. NIP-94 marks `size`, `dim` and the rest as optional and lists
`url`, `m`, `x` and `ox` without that mark. This reader requires `url`, `m`
and `x`, since a file without a hash can't be checked, and treats `ox` as
optional: a server that didn't change the file has nothing to put there.
Tags with other names (`thumb`, `image`, `summary`, `magnet`, `t`) pass
without a check.

<!-- source: coax/lib/nostr/nip94.coax file -->
```jai
// A kind 1063 event's file metadata: url, m and x are required.
n94_file :: (e:NostrEvent)->N94File {
  f:N94File;if e.error!=NE_OK {return f;}
  if e.kind!=N94_KIND_FILE {return n94_at(f,N94_KIND,-1);}
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n94_at(f,N94_LIMIT,i);}
    if t.error!=NT_OK {return n94_at(f,N94_EVENT,i);}
    name:=t.v[0];value:=t.v[1];
    key:=cast(string)name.b[0..<name.n];
    if n94_key(key)>=0 {
      f=n94_field(f,key,cast(string)value.b[0..<value.n]);
      if f.status!=N94_OK {return n94_at(f,f.status,i);}
    }
    i+=1;
  }
  if !n94_has(f,N94_URL) || !n94_has(f,N94_M) || !f.has_x {return n94_at(f,N94_MISSING,-1);}
  return n94_at(f,N94_OK,-1);
}
```

### Writing the metadata

`n94_write` turns an `N94File` back into either form, in one fixed order:
`url`, `m`, `x`, `ox`, `size`, `dim`, `blurhash`, `alt`, then each
`fallback`. The example reads a note's `imeta` tag and writes the same
fields as the tags of a kind 1063 event. Values go through chapter 20's
`json_write_string`, so a quote in `alt` is escaped.

<!-- source: coax/lib/nostr/nip94.coax write -->
```jai
// One field after sep: "key value" in an imeta tag, or ["key","value"] as
// a tag.
n94_put :: (out:[]u8,at:int,sep:string,key:string,value:string,imeta:bool)->int {
  n:=json_raw(out,at,sep);
  if imeta {
    s:[NT_MAX_VALUE+16]u8;m:=json_raw(s[..],0,key);m=json_raw(s[..],m," ");m=json_raw(s[..],m,value);
    return json_write_string(out,n,cast(string)s[0..<m]);
  }
  n=json_raw(out,n,"[");n=json_write_string(out,n,key);n=json_raw(out,n,",");
  n=json_write_string(out,n,value);return json_raw(out,n,"]");
}
// f's fields from at: an imeta tag ["imeta",...] or the tags of a kind
// 1063 event [[...],...]. The new length, or -1 when it doesn't fit or f
// has no url.
n94_write :: (f:N94File,out:[]u8,at:int,imeta:bool)->int {
  if f.url.n==0 {return -1;}
  c:=f;hex:[64]u8;num:[48]u8;
  n:=ifx imeta then json_raw(out,at,"[\"imeta\"") else at;
  n=n94_put(out,n,ifx imeta then "," else "[","url",cast(string)c.url.b[0..<c.url.n],imeta);
  if c.m.n>0 {n=n94_put(out,n,",","m",cast(string)c.m.b[0..<c.m.n],imeta);}
  if c.has_x {r:=hex_encode_checked(cast(string)c.x[..],hex[..],false);n=n94_put(out,n,",","x",cast(string)hex[..],imeta);}
  if c.has_ox {r:=hex_encode_checked(cast(string)c.ox[..],hex[..],false);n=n94_put(out,n,",","ox",cast(string)hex[..],imeta);}
  if c.has_size {w:=json_write_int(num[..],0,c.size);n=n94_put(out,n,",","size",cast(string)num[0..<w],imeta);}
  if c.has_dim {
    w:=json_write_int(num[..],0,c.width);w=json_raw(num[..],w,"x");w=json_write_int(num[..],w,c.height);
    n=n94_put(out,n,",","dim",cast(string)num[0..<w],imeta);
  }
  if c.blurhash.n>0 {n=n94_put(out,n,",","blurhash",cast(string)c.blurhash.b[0..<c.blurhash.n],imeta);}
  if c.alt.n>0 {n=n94_put(out,n,",","alt",cast(string)c.alt.b[0..<c.alt.n],imeta);}
  for k:0..<c.fallbacks {n=n94_put(out,n,",","fallback",cast(string)c.fallback[k].b[0..<c.fallback[k].n],imeta);}
  return json_raw(out,n,"]");
}
```

## The hash check

`n94_check` is the one decision in this file. It hashes the bytes with
chapter 4's streaming SHA-256 and compares the result with `x`. Then, if the metadata
has a `size`, the byte count must equal it. With `original` set, the bytes
must hash to `ox` instead: a server may resize or re-encode an upload, and
`ox` names the file the author sent. `size` describes the served file, so
it isn't checked against the original.

<!-- source: coax/lib/nostr/nip94.coax check -->
```jai
// Whether bytes are the file: the SHA-256 of the bytes must equal x, then
// their count must equal size when there is one. With original, they must
// hash to ox instead, the file before the server changed it; size is about
// the served file, so it is not checked then.
n94_check :: (f:N94File,bytes:[]u8,original:bool)->int {
  want:=ifx original then f.ox else f.x;
  has:=ifx original then f.has_ox else f.has_x;
  if !has {return N94_NO_HASH;}
  h:[32]u8;
  if n94_digest(bytes,CB_MAX_BYTES,h[..])!=SHA_OK || !ns_equal32(h,want) {return N94_MISMATCH;}
  if !original && f.has_size && f.size!=bytes.count {return N94_SIZE;}
  return N94_OK;
}
// The SHA-256 of bytes of any size, streamed in pieces of chunk bytes
// (chapter 4's init, update and final); SHA_OK, or the hash's refusal.
n94_digest :: (bytes:[]u8,chunk:int,out:[]u8)->u32 {
  if chunk<1 {return SHA_BAD_INPUT;}
  state:[112]u8;
  status:=sha256_init(state[..]);
  full:=bytes.count/chunk;
  for c:0..<full {
    if status==SHA_OK {status=sha256_update_checked(state[..],bytes[c*chunk..<c*chunk+chunk],false);}
  }
  if status==SHA_OK {status=sha256_update_checked(state[..],bytes[full*chunk..<bytes.count],false);}
  if status==SHA_OK {status=sha256_final_checked(state[..],out,false);}
  return status;
}
```

The order matters for what the reader learns. A hash that matches with the
wrong size means the metadata is wrong, not the file: two different byte
strings with one SHA-256 aren't known to exist. So the size check comes
second and names its own refusal.

Chapter 4's one-shot `sha256_hash_checked` takes at most 1 MiB, and so
does one `sha256_update_checked` call. `n94_digest` feeds the bytes to
`sha256_init`, `sha256_update_checked` and `sha256_final_checked` in pieces
of `chunk` bytes, so a file of any size verifies. `n94_check` uses 1 MiB
pieces (`CB_MAX_BYTES`); the checks also hash with pieces of 1, 7, 64, 500,
999, 1,000 and 1,001 bytes and get the same digest. A hash that can't be
computed refuses as `N94_MISMATCH`.

## Picture, video and voice events

Three NIPs build on `imeta` without new metadata rules:

- NIP-68 picture events, kind 20, put each image in an `imeta` tag, with
  `title` and the description in the content. Its example adds
  `thumbhash` and `annotate-user`, which the reader counts as other keys.
- NIP-71 video events, kinds 21 and 22 and the addressable 34235 and 34236,
  give one `imeta` tag per variant: a resolution, an HLS playlist, an audio
  track. They add `duration`, `bitrate` and `waveform`.
- NIP-A0 voice messages, kinds 1222 and 1244, carry the audio URL in the
  content and may add an `imeta` tag with `waveform` and `duration`.

Each of these tags reads through `n94_imeta`, and each `x` is checked with
`n94_check`. The kinds' own tags (titles, `content-warning`, NIP-71's
`segment` and `text-track`, NIP-A0's NIP-22 reply structure) are not
read in this chapter.

## Blossom

Blossom is a set of specifications, called BUDs, for servers that store
files by their SHA-256. NIP-B7 says how a Nostr client uses them.

### A blob's address

A Blossom URL ends in the blob's SHA-256 as 64 hex digits, with or without
an extension: `https://blossom.self.hosted/<hex>.png`. When that URL fails,
NIP-B7 says to look up the author's kind 10063 list of servers and request
the same hex string from each, and to check that the downloaded bytes hash
to it.

`nb7_address` takes the URL's last path segment, stops at its first dot,
and requires exactly 64 lowercase hex digits. `nb7_fetch` is the check;
`nb7_blob_url` builds the URL on another server.

<!-- source: coax/lib/nostr/nipb7.coax address -->
```jai
// The blob a URL names: its last path segment is 64 lowercase hex digits,
// then nothing or an extension starting with a dot.
nb7_address :: (url:string)->NB7Blob {
  b:NB7Blob;from:=0;
  for i:0..<url.count {if url[i]=='/' {from=i+1;}}
  to:=from;
  while to<url.count && url[to]!='.' {to+=1;}
  b.ok=to-from==64 && ne_lower_hex(url[from..<to],b.hash[..]);
  b.ext_from=to;
  return b;
}
// Whether bytes fetched from url are the blob it names.
nb7_fetch :: (url:string,bytes:[]u8)->int {
  b:=nb7_address(url);if !b.ok {return NB7_URL;}
  h:[32]u8;
  if sha256_hash_checked(bytes,h[..],false)!=SHA_OK {return NB7_LIMIT;}
  return ifx ns_equal32(h,b.hash) then NB7_OK else NB7_MISMATCH;
}
// The blob's URL on server: server/hex then ext ("" or ".png"). The new
// length, or -1 when it doesn't fit.
nb7_blob_url :: (server:string,hash:[32]u8,ext:string,out:[]u8)->int {
  s:=server;if s.count>0 && s[s.count-1]=='/' {s=s[0..<s.count-1];}
  h:=hash;hex:[64]u8;r:=hex_encode_checked(cast(string)h[..],hex[..],false);
  n:=json_raw(out,0,s);n=json_raw(out,n,"/");n=json_raw(out,n,cast(string)hex[..]);
  return json_raw(out,n,ext);
}
```

### Server lists

A kind 10063 event lists a user's Blossom servers as `server` tags.
`nb7_servers` reads them in order and refuses a server that is not an
`http://` or `https://` URL.

<!-- source: coax/lib/nostr/nipb7.coax servers -->
```jai
// A kind 10063 event's server tags, in order, into list.
nb7_servers :: (e:NostrEvent,list:[]N32Text)->NB7Servers {
  s:NB7Servers;if e.error!=NE_OK {return s;}
  if e.kind!=NB7_KIND_SERVERS {s.status=NB7_KIND;return s;}
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    s.at=i;
    if t.error==NT_LIMIT {s.status=NB7_LIMIT;return s;}
    if t.error!=NT_OK {return s;}
    if n32_is(t.v[0],"server") {
      v:=t.v[1];u:=cast(string)v.b[0..<v.n];
      if !nb7_http(u) {s.status=NB7_SERVER;return s;}
      if s.n>=list.count {s.status=NB7_TOO_MANY;return s;}
      list[s.n]=v;s.n+=1;
    }
    i+=1;
  }
  s.status=NB7_OK;s.at= -1;return s;
}
// An http:// or https:// URL with something after the scheme.
nb7_http :: (u:string)->bool {
  if u.count>8 && u[0..<8]=="https://" {return true;}
  return u.count>7 && u[0..<7]=="http://";
}
```

### Upload authorization

To upload, delete or list blobs, a client sends a signed kind 24242 event
in the `Authorization` header, as NIP-98 (chapter 49) does with kind
27235. Kind 24242 is defined by Blossom's BUD-02, which is not pinned in
this repository: the pinned NIP-B7 links it, and the pinned kinds table
lists 24242 as "Blobs stored on mediaservers". This chapter follows BUD-02:

- The `t` tag names the verb: `get`, `upload`, `list` or `delete`. There
  is exactly one.
- The `expiration` tag is a time in the future. There is exactly one.
- `created_at` is not in the future.
- For `upload` and `delete`, an `x` tag names the blob. There may be
  several `x` tags; one must match.
- A `get` may name blobs; then one must match. A `list` names none.
- The content is a description for people, and is not checked.

`nb7_admit` makes those checks for one request. It refuses any event that
didn't pass chapter 21's verification, so a forged or altered event never
reaches the rules. `nb7_upload` hashes the body first, so the blob the
event must name is the one the server would store.

<!-- source: coax/lib/nostr/nipb7.coax auth -->
```jai
// The tags of an authorization for verb on blob until expiration: t, then
// x (not for list), then expiration. The new length, or -1.
nb7_auth_tags :: (verb:int,blob:[32]u8,expiration:int,out:[]u8)->int {
  n:=json_raw(out,0,"[[\"t\",\"");n=json_raw(out,n,NB7_VERBS[verb]);n=json_raw(out,n,"\"],");
  if verb!=NB7_LIST {
    b:=blob;hex:[64]u8;r:=hex_encode_checked(cast(string)b[..],hex[..],false);
    n=json_raw(out,n,"[\"x\",\"");n=json_raw(out,n,cast(string)hex[..]);n=json_raw(out,n,"\"],");
  }
  n=json_raw(out,n,"[\"expiration\",\"");n=json_write_int(out,n,expiration);
  return json_raw(out,n,"\"]]");
}
// Whether a verified kind 24242 event authorizes verb on blob at now.
nb7_admit :: (e:NostrEvent,verb:int,blob:[32]u8,now:int)->int {
  if e.error!=NE_OK || !e.verified {return NB7_EVENT;}
  if e.kind!=NB7_KIND_AUTH {return NB7_KIND;}
  if e.created_at>now {return NB7_FUTURE;}
  said:int;ts:=0;expiration:int;expirations:=0;has_x:=false;named:=false;
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return NB7_LIMIT;}
    if t.error!=NT_OK {return NB7_TAG;}
    v:=t.v[1];value:=cast(string)v.b[0..<v.n];
    if n32_is(t.v[0],"t") {
      ts+=1;said= -1;
      for k:0..<NB7_VERBS.count {if value==NB7_VERBS[k] {said=k;}}
    }
    if n32_is(t.v[0],"expiration") {expirations+=1;expiration=n94_number(value);}
    if n32_is(t.v[0],"x") {
      h:[32]u8;
      if !ne_lower_hex(value,h[..]) {return NB7_TAG;}
      has_x=true;if ns_equal32(h,blob) {named=true;}
    }
    i+=1;
  }
  if ts!=1 || said<0 || expirations!=1 || expiration<0 {return NB7_TAG;}
  if said!=verb {return NB7_VERB;}
  if expiration<=now {return NB7_EXPIRED;}
  if verb==NB7_LIST {return NB7_OK;}
  if named || (verb==NB7_GET && !has_x) {return NB7_OK;}
  return NB7_HASH;
}
// The server's check of an upload: the body's SHA-256 is the blob the
// event must authorize.
nb7_upload :: (e:NostrEvent,body:[]u8,now:int)->int {
  h:[32]u8;
  if sha256_hash_checked(body,h[..],false)!=SHA_OK {return NB7_LIMIT;}
  return nb7_admit(e,NB7_UPLOAD,h,now);
}
```

Two differences from NIP-98 are worth noting. NIP-98 binds a URL and a
time window of 60 seconds; a Blossom authorization binds a verb and a blob
until its own `expiration`, on any server. BUD-02 lets an optional
`server` tag narrow that; it is not pinned and not checked here. And
neither NIP stops replay inside the window by itself. Chapter 49 records
each event ID once; a Blossom server can do the same, but this chapter
doesn't build that store.

Who may upload at all, and how much, is the server's own policy. A valid
authorization only proves that the key holder asked for this verb on this
blob.

### NIP-96

NIP-96 defines an HTTP API for file servers: a
`/.well-known/nostr/nip96.json` document, uploads with a NIP-98 header,
and kind 10096 server preferences. It is marked `unrecommended` in favor
of Blossom. This chapter reads its kind 1063-style metadata through
`n94_file`, like any other, and builds nothing else from it.

## Worked example: an imeta tag and a blob address by hand

The blob is the 16 bytes `Hello, Blossom!\n`. Its SHA-256 is

```text
50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044
```

Alice's note shows it with this `imeta` tag:

```jai
["imeta",
 "url https://blossom.example/50831e9b...e044.txt",
 "m text/plain",
 "x 50831e9b...e044",
 "size 16",
 "alt A greeting"]
```

### Step 1: split each entry at its first space

| Entry | First space at | Key | Value |
| --- | ---: | --- | --- |
| 1 | 3 | `url` | `https://blossom.example/50831e9b...e044.txt` (92 bytes) |
| 2 | 1 | `m` | `text/plain`: one slash at 4, so type/subtype holds |
| 3 | 1 | `x` | 64 lowercase hex digits: 32 bytes `50 83 ... e0 44` |
| 4 | 4 | `size` | `16` |
| 5 | 3 | `alt` | `A greeting`: the second space stays in the value |

There is a `url` and four other fields, no key repeats, so the tag reads.

### Step 2: the blob's address

The URL's last `/` is at byte 23, so its last segment starts at byte 24:
`https://` is 8 bytes, `blossom.example` 15, `/` 1. The first dot after
that is at byte 24 + 64 = 88. The 64 bytes in between are hex, so the URL
names the blob `50831e9b...e044`, the same as `x`, and the extension is
`.txt`.

### Step 3: the hash check

The downloaded bytes hash to `50831e9b...e044`, equal to `x`, and there are
16 of them, equal to `size`: accepted. Change `!` to `?` and the SHA-256
becomes `649d4e40...f3a8b3`: refused. The first 15 bytes alone hash to
something else too: refused. The 16 right bytes with `size 15` declared
hash correctly, and are refused for their size.

### Step 4: the upload window

Alice signs a kind 24242 event at `created_at = T0 = 1790000000` with
`["t","upload"]`, `["x","50831e9b...e044"]` and
`["expiration","1790000300"]`. The server checks it at `now`:

| `now` | Check | Result |
| --- | --- | --- |
| T0 + 10 | created_at ≤ now, expiration > now, body hashes to `x` | admitted |
| T0 + 10, body `Hello, Blossom?\n` | the body's hash is in no `x` tag | not for this blob |
| T0 + 10, as a delete | `t` says upload | another verb |
| T0 + 300 | expiration ≤ now | expired |
| T0 − 5 | created_at > now | created in the future |

## Not built

- Hashing Blossom bodies over 1 MiB: `nb7_fetch` and `nb7_upload` still
  use the one-shot hash and refuse larger bodies as `NB7_LIMIT`.
- Matching `imeta` URLs against the content, and NIP-92's kind 0 profile
  rule (an `imeta` tag describes `picture` or `banner` when its `url` is
  equal).
- The kinds' own tags in NIP-68, NIP-71 and NIP-A0.
- Blossom's HTTP routes, BUD-02's optional `server` tag, replay protection
  for kind 24242, and uploads over the network.
- NIP-96's API.

## The example

The example is [nostr/examples/media-files.coax](../examples/media-files.coax).
Alice signs a note with the worked `imeta` tag, and the program reads it,
names the blob from its URL and runs the hash check on the right bytes and
three wrong ones. It writes the same fields as a kind 1063 event, signs and
reads it back. It signs a kind 10063 list, builds the blob's URL on the
second server and checks it there. Last, it signs the upload authorization
and runs the server's check at five moments, then shows four refusals.

<!-- source: nostr/examples/media-files.coax example -->
```jai
#load "../../coax/lib/nostr/nipb7.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

FILE_STATUS :: [18] string.["ok", "event", "not an imeta tag", "entry is not a key and a value", "no url",
  "url alone", "duplicate field", "hash is not 64 lowercase hex digits", "size is not a count",
  "dim is not WxH", "m is not type/subtype", "not kind 1063", "missing url, m or x", "too long",
  "too many fallbacks", "no hash", "SHA-256 differs", "size differs"];
BLOSSOM :: [13] string.["ok", "event not verified", "not kind 24242", "created in the future",
  "bad t, x or expiration tag", "another verb", "expired", "not for this blob", "too long",
  "URL names no SHA-256", "SHA-256 differs from the URL", "server is not http(s)", "too many servers"];
// Alice's secret key is BIP-340 test vector 1's published key. Never sign
// with a real key in an example.
ALICE :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
BLOB :: "Hello, Blossom!\n";
URL :: "https://blossom.example/50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044.txt";
IMETA :: "[[\"imeta\",\"url https://blossom.example/50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044.txt\",\"m text/plain\",\"x 50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044\",\"size 16\",\"alt A greeting\"]]";
SERVERS :: "[[\"server\",\"https://blossom.example\"],[\"server\",\"https://mirror.example\"]]";
T0 :: 1790000000;

// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
hex32 :: (id: [32] u8, out: [] u8) -> string {
  own := id; r := hex_encode_checked(cast(string) own[..], out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<64];
}
say :: (t: N32Text) { own := t; print("%", cast(string) own.b[0..<own.n]); }
bytes_of :: (s: string, out: [] u8) -> [] u8 { for i: 0..<s.count { out[i] = s[i]; } return out[0..<s.count]; }

// Sign an event by Alice into out and read it back with its ID and
// signature checked.
sign :: (created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  key: [32] u8; decode_hex(ALICE, key[..]);
  aux: [32] u8; work: [1] NostrSignWork; scratch: [8192] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [8192] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified;
  return e;
}

show_file :: (label: string, f: N94File) {
  print("%: %", label, FILE_STATUS[f.status]);
  if f.at >= 0 { print(" at tag %", f.at); }
  if f.entry >= 0 { print(" entry %", f.entry); }
  print("\n");
  if f.status != N94_OK { return; }
  hex: [64] u8;
  print("  url "); say(f.url); print("\n  m "); say(f.m);
  if f.has_x { print("\n  x %", hex32(f.x, hex[..])); }
  if f.has_size { print("\n  size %", f.size); }
  if f.alt.n > 0 { print("\n  alt "); say(f.alt); }
  print("\n  fallbacks %, other keys %\n", f.fallbacks, f.others);
}
// Download bytes for f: the hash check.
download :: (label: string, f: N94File, bytes: [] u8) {
  print("download %: %\n", label, FILE_STATUS[n94_check(f, bytes, false)]);
}

main :: () {
  blob: [16] u8; body := bytes_of(BLOB, blob[..]);
  h: [32] u8; #assert sha256_hash_checked(body, h[..], false) == SHA_OK;
  hex: [64] u8;
  print("the blob: % bytes, SHA-256 %\n", body.count, hex32(h, hex[..]));

  print("a note that shows the blob\n");
  note_buf: [8192] u8; content: [256] u8;
  n := json_raw(content[..], 0, "A greeting: "); n = json_raw(content[..], n, URL);
  note := sign(T0, 1, IMETA, cast(string) content[0..<n], note_buf[..]);
  print("note %\n", hex32(note.id, hex[..]));
  f := n94_imeta(note, 0);
  show_file("its imeta tag", f);
  a := nb7_address(URL);
  print("the url's last segment names %, extension from byte %\n", hex32(a.hash, hex[..]), a.ext_from);

  print("the hash check\n");
  download("the blob", f, body);
  wrong: [16] u8; changed := bytes_of(BLOB, wrong[..]); changed[14] = '?';
  download("with ! changed to ?", f, changed);
  download("the first 15 bytes", f, body[0..<15]);
  g := f; g.size = 15;
  download("the blob, size 15 declared", g, body);

  print("the same file as a kind 1063 event\n");
  tags: [1024] u8; t := n94_write(f, tags[..], 0, false); #assert t > 0;
  print("tags %\n", cast(string) tags[0..<t]);
  file_buf: [8192] u8;
  file := sign(T0 + 1, N94_KIND_FILE, cast(string) tags[0..<t], "A greeting", file_buf[..]);
  print("event %\n", hex32(file.id, hex[..]));
  show_file("kind 1063", n94_file(file));

  print("the url fails: Alice's other Blossom servers\n");
  servers_buf: [8192] u8;
  servers := sign(T0 + 2, NB7_KIND_SERVERS, SERVERS, "", servers_buf[..]);
  print("kind 10063 %\n", hex32(servers.id, hex[..]));
  list: [4] N32Text; s := nb7_servers(servers, list[..]);
  print("servers: %, %\n", BLOSSOM[s.status], s.n);
  mirror: [256] u8; m := nb7_blob_url(cast(string) list[1].b[0..<list[1].n], a.hash, URL[a.ext_from..<URL.count], mirror[..]);
  mirror_url := cast(string) mirror[0..<m];
  print("try %\n", mirror_url);
  print("  the blob: %\n", BLOSSOM[nb7_fetch(mirror_url, body)]);
  print("  changed bytes: %\n", BLOSSOM[nb7_fetch(mirror_url, changed)]);

  print("uploading: a kind 24242 authorization\n");
  auth_tags: [512] u8; k := nb7_auth_tags(NB7_UPLOAD, h, T0 + 300, auth_tags[..]);
  print("tags %\n", cast(string) auth_tags[0..<k]);
  auth_buf: [8192] u8;
  auth := sign(T0, NB7_KIND_AUTH, cast(string) auth_tags[0..<k], "Upload greeting.txt", auth_buf[..]);
  print("event %\n", hex32(auth.id, hex[..]));
  print("server at T0+10, upload of the blob: %\n", BLOSSOM[nb7_upload(auth, body, T0 + 10)]);
  print("  changed bytes: %\n", BLOSSOM[nb7_upload(auth, changed, T0 + 10)]);
  print("  as a delete: %\n", BLOSSOM[nb7_admit(auth, NB7_DELETE, h, T0 + 10)]);
  print("  at T0+300: %\n", BLOSSOM[nb7_upload(auth, body, T0 + 300)]);
  print("  at T0-5: %\n", BLOSSOM[nb7_upload(auth, body, T0 - 5)]);
  forged := auth; forged.verified = false;
  print("  not verified: %\n", BLOSSOM[nb7_upload(forged, body, T0 + 10)]);

  print("refusals\n");
  show_file("imeta without url", n94_imeta(nt_event("[[\"imeta\",\"m text/plain\",\"x 50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044\"]]"), 0));
  show_file("imeta with url alone", n94_imeta(nt_event("[[\"imeta\",\"url https://blossom.example/a.txt\"]]"), 0));
  show_file("NIP-92's placeholder x", n94_imeta(nt_event("[[\"imeta\",\"url https://nostr.build/i/my-image.jpg\",\"x <sha256 hash as specified in NIP 94>\"]]"), 0));
  show_file("kind 1063 without x", n94_file(n32_event(1063, "[[\"url\",\"https://blossom.example/a.txt\"],[\"m\",\"text/plain\"]]")));
}
```

```shell
coax/bin/coax run nostr/examples/media-files.coax --run-steps 250000000
```

The output, which the book's checks compare byte for byte in debug and
release:

```text
the blob: 16 bytes, SHA-256 50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044
a note that shows the blob
note 0e3a5dce8fa291df8854513d9d0b5496f42604335e07405e16200261eec88ecf
its imeta tag: ok
  url https://blossom.example/50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044.txt
  m text/plain
  x 50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044
  size 16
  alt A greeting
  fallbacks 0, other keys 0
the url's last segment names 50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044, extension from byte 88
the hash check
download the blob: ok
download with ! changed to ?: SHA-256 differs
download the first 15 bytes: SHA-256 differs
download the blob, size 15 declared: size differs
the same file as a kind 1063 event
tags [["url","https://blossom.example/50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044.txt"],["m","text/plain"],["x","50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044"],["size","16"],["alt","A greeting"]]
event a47d4c06776d1600d1000749340e94ba947feff9642d99d58175f93f08ef8637
kind 1063: ok
  url https://blossom.example/50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044.txt
  m text/plain
  x 50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044
  size 16
  alt A greeting
  fallbacks 0, other keys 0
the url fails: Alice's other Blossom servers
kind 10063 6da5d94f60316ee2b7154674a070795bdf67b13977fea53e1b1416d455cb0377
servers: ok, 2
try https://mirror.example/50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044.txt
  the blob: ok
  changed bytes: SHA-256 differs from the URL
uploading: a kind 24242 authorization
tags [["t","upload"],["x","50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044"],["expiration","1790000300"]]
event cb2d67b82e2244ac183cd66aeababc7f71011c3a24863df16fe48d048a692a9f
server at T0+10, upload of the blob: ok
  changed bytes: not for this blob
  as a delete: another verb
  at T0+300: expired
  at T0-5: created in the future
  not verified: event not verified
refusals
imeta without url: no url at tag 0
imeta with url alone: url alone at tag 0
NIP-92's placeholder x: hash is not 64 lowercase hex digits at tag 0 entry 2
kind 1063 without x: missing url, m or x
```

The program exits with status 0. The refusals are part of the transcript.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/media-files.coax](../examples/checks/media-files.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The blob's SHA-256, the changed blob's, and the ids of all four events,
  from a Python script written from NIP-01, NIP-92, NIP-94 and BUD-02
  (`hashlib.sha256` and NIP-01's serialization through `json.dumps`).
- The hand values: the five fields, the address and its extension at
  byte 88, the 1063 tags that Python hashed, the two servers, and the
  upload window from T0 to T0 + 299.
- NIP-92's example, with its placeholder `x` (`<sha256 hash as specified in
  NIP 94>`) replaced by the blob's hash. NIP-92's kind 0 example writes
  elided hashes (`x 1f2e...a9`), which are not hashes, so the reader
  refuses them at entry 3; without them the tags read.
- NIP-68's second picture tag, NIP-71's four video and audio tags and
  NIP-A0's voice tag, each with the fields and other-key counts the NIPs
  show.
- NIP-B7's kind 10063 example. Its stated id, `e4bee088...`, is not the
  SHA-256 of its serialization: Python and Coax both compute
  `bb431f3950fe3dcd5b5ba3630cc1c88135047d7f171459ba4c0af554f914aff0`.
  So the event is refused as it stands (`NE_ID`); its tags still read.

<!-- source: nostr/examples/checks/media-files.coax check -->
```jai
// From a Python script written from NIP-01, NIP-92, NIP-94 and BUD-02 (it
// shares no code with Coax): hashlib.sha256 of the blob, and each event's
// id as hashlib.sha256 of json.dumps([0, pubkey, created_at, kind, tags,
// content], separators=(",", ":"), ensure_ascii=False).
PY_BLOB :: "50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044";
PY_CHANGED :: "649d4e402861596fd83b14f5379479f18185e913749fcb8fb0d4a1e347f3a8b3";
PY_ID :: [4] string.[
  "0e3a5dce8fa291df8854513d9d0b5496f42604335e07405e16200261eec88ecf",   // the note
  "a47d4c06776d1600d1000749340e94ba947feff9642d99d58175f93f08ef8637",   // kind 1063
  "6da5d94f60316ee2b7154674a070795bdf67b13977fea53e1b1416d455cb0377",   // kind 10063
  "cb2d67b82e2244ac183cd66aeababc7f71011c3a24863df16fe48d048a692a9f"];  // kind 24242
AT :: [4] int.[0, 1, 2, 0];
KIND :: [4] int.[1, 1063, 10063, 24242];
TAGS :: [4] string.[IMETA,
  "[[\"url\",\"https://blossom.example/50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044.txt\"],[\"m\",\"text/plain\"],[\"x\",\"50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044\"],[\"size\",\"16\"],[\"alt\",\"A greeting\"]]",
  SERVERS,
  "[[\"t\",\"upload\"],[\"x\",\"50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044\"],[\"expiration\",\"1790000300\"]]"];
CONTENT :: [4] string.[
  "A greeting: https://blossom.example/50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044.txt",
  "A greeting", "", "Upload greeting.txt"];
PUB :: "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";
ZERO_SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

// Event k with Python's id and a zero signature, read with its id
// recomputed; the signature is not checked.
py :: (k: int, out: [] u8, scratch: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, PY_ID[k]);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, T0 + AT[k]);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, KIND[k]);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, TAGS[k]);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, CONTENT[k]);
  n = json_raw(out, n, ",\"sig\":\""); n = json_raw(out, n, ZERO_SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}

#test "the blob's SHA-256 and the four ids are Python's" {
  blob: [16] u8; body := bytes_of(BLOB, blob[..]); h: [32] u8; hex: [64] u8;
  #assert sha256_hash_checked(body, h[..], false) == SHA_OK && hex32(h, hex[..]) == PY_BLOB;
  body[14] = '?'; #assert sha256_hash_checked(body, h[..], false) == SHA_OK && hex32(h, hex[..]) == PY_CHANGED;
  buf: [2048] u8; scratch: [4096] u8;
  for k: 0..<4 { e := py(k, buf[..], scratch[..]); #assert e.error == NE_OK && ns_equal32(e.computed_id, e.id); }
}

// The imeta tag by hand: each entry splits at its first space. "url" then
// the URL, "m" then text/plain, "x" then 64 hex digits, "size" then 16,
// "alt" then "A greeting", which keeps its space. The URL's last segment
// starts at byte 24 (https:// is 8, blossom.example 15, / 1), so the hex
// runs to byte 88, where ".txt" starts.
#test "the imeta tag and the address match the hand values" {
  buf: [2048] u8; scratch: [4096] u8;
  f := n94_imeta(py(0, buf[..], scratch[..]), 0);
  #assert f.status == N94_OK && n32_is(f.url, URL) && n32_is(f.m, "text/plain") && f.size == 16 && n32_is(f.alt, "A greeting");
  x: [32] u8; decode_hex(PY_BLOB, x[..]); #assert f.has_x && ns_equal32(f.x, x) && f.others == 0 && f.fallbacks == 0;
  a := nb7_address(URL); #assert a.ok && a.ext_from == 88 && ns_equal32(a.hash, x) && URL[a.ext_from..<URL.count] == ".txt";
  // The 1063 tags the writer gives are the ones Python hashed.
  out: [1024] u8; n := n94_write(f, out[..], 0, false); #assert cast(string) out[0..<n] == TAGS[1];
  g := n94_file(py(1, buf[..], scratch[..])); #assert g.status == N94_OK && ns_equal32(g.x, x) && g.size == 16;
  list: [2] N32Text; s := nb7_servers(py(2, buf[..], scratch[..]), list[..]);
  #assert s.status == NB7_OK && s.n == 2 && n32_is(list[1], "https://mirror.example");
}

// The authorization by hand: created_at T0, expiration T0 + 300. It
// admits an upload of the blob from T0 to T0 + 299: at T0 + 300 it has
// expired, and before T0 it was not yet made.
#test "the authorization's window matches the hand trace" {
  buf: [2048] u8; scratch: [4096] u8;
  e := py(3, buf[..], scratch[..]); e.verified = true;   // the id is Python's; the signature is the example's
  blob: [16] u8; body := bytes_of(BLOB, blob[..]);
  #assert nb7_upload(e, body, T0) == NB7_OK && nb7_upload(e, body, T0 + 299) == NB7_OK;
  #assert nb7_upload(e, body, T0 + 300) == NB7_EXPIRED && nb7_upload(e, body, T0 - 1) == NB7_FUTURE;
  x: [32] u8; decode_hex(PY_BLOB, x[..]);
  #assert nb7_admit(e, NB7_GET, x, T0) == NB7_VERB && nb7_admit(e, NB7_UPLOAD, x, T0) == NB7_OK;
}

// NIP-92's example, with its placeholder x ("<sha256 hash as specified in
// NIP 94>") replaced by the blob's hash. Its kind 0 example writes elided
// hashes ("1f2e...a9"), which are not hashes: the reader refuses them.
#test "NIP-92's examples" {
  f := n94_imeta(nt_event("[[\"imeta\",\"url https://nostr.build/i/my-image.jpg\",\"m image/jpeg\",\"blurhash eVF$^OI:${M{o#*0-nNFxakD-?xVM}WEWB%iNKxvR-oetmo#R-aen$\",\"dim 3024x4032\",\"alt A scenic photo overlooking the coast of Costa Rica\",\"x 50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044\",\"fallback https://nostrcheck.me/alt1.jpg\",\"fallback https://void.cat/alt1.jpg\"]]"), 0);
  #assert f.status == N94_OK && n32_is(f.url, "https://nostr.build/i/my-image.jpg") && n32_is(f.m, "image/jpeg") && f.width == 3024 && f.height == 4032;
  #assert n32_is(f.alt, "A scenic photo overlooking the coast of Costa Rica") && f.fallbacks == 2 && n32_is(f.fallback[1], "https://void.cat/alt1.jpg") && f.has_x;
  f = n94_imeta(nt_event("[[\"imeta\",\"url https://blossom.example/1f2e...a9.jpg\",\"m image/jpeg\",\"x 1f2e...a9\",\"dim 400x400\",\"blurhash LEHV6nWB2yk8pyo0adR*.7kCMdnj\",\"fallback https://mirror.example/1f2e...a9.jpg\"]]"), 0);
  #assert f.status == N94_HEX && f.entry == 3;
  f = n94_imeta(nt_event("[[\"imeta\",\"url https://blossom.example/7c3d...04.webp\",\"m image/webp\",\"x 7c3d...04\",\"dim 1500x500\"]]"), 0);
  #assert f.status == N94_HEX && f.entry == 3;
  // Without the elided x, the kind 0 tags read; the URLs name no blob.
  f = n94_imeta(nt_event("[[\"imeta\",\"url https://blossom.example/7c3d...04.webp\",\"m image/webp\",\"dim 1500x500\"]]"), 0);
  #assert f.status == N94_OK && f.width == 1500 && !nb7_address("https://blossom.example/7c3d...04.webp").ok;
}

// NIP-68's two picture imeta tags (x placeholders replaced, annotate-user
// counted as another key), NIP-71's four video and audio tags, and
// NIP-A0's voice message tag.
#test "NIP-68, NIP-71 and NIP-A0 imeta tags" {
  f := n94_imeta(nt_event("[[\"imeta\",\"url https://nostr.build/i/my-image2.jpg\",\"m image/jpeg\",\"thumbhash zOcFFIIKmXgzmWaXWIcmcFQEGA\",\"dim 3024x4032\",\"alt Another scenic photo overlooking the coast of Costa Rica\",\"x 50831e9b89b8f27c88d119fe397e7251027f5c34a421d2c89c194b9b8591e044\",\"fallback https://nostrcheck.me/alt2.jpg\",\"fallback https://void.cat/alt2.jpg\",\"annotate-user dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659:10:20\"]]"), 0);
  #assert f.status == N94_OK && f.others == 2 && f.fallbacks == 2;
  f = n94_imeta(nt_event("[[\"imeta\",\"dim 1920x1080\",\"url https://myvideo.com/1080/12345.mp4\",\"x 3093509d1e0bc604ff60cb9286f4cd7c781553bc8991937befaacfdc28ec5cdc\",\"m video/mp4\",\"image https://myvideo.com/1080/12345.jpg\",\"image https://myotherserver.com/1080/12345.jpg\",\"fallback https://myotherserver.com/1080/12345.mp4\",\"fallback https://andanotherserver.com/1080/12345.mp4\",\"service nip96\",\"bitrate 3000000\",\"duration 29.223\"]]"), 0);
  #assert f.status == N94_OK && f.width == 1920 && f.height == 1080 && f.x[0] == 0x30 && f.x[31] == 0xdc && n32_is(f.m, "video/mp4") && f.fallbacks == 2 && f.others == 5;
  f = n94_imeta(nt_event("[[\"imeta\",\"dim 1280x720\",\"url https://myvideo.com/720/12345.mp4\",\"x e1d4f808dae475ed32fb23ce52ef8ac82e3cc760702fca10d62d382d2da3697d\",\"m video/mp4\",\"image https://myvideo.com/720/12345.jpg\",\"image https://myotherserver.com/720/12345.jpg\",\"fallback https://myotherserver.com/720/12345.mp4\",\"fallback https://andanotherserver.com/720/12345.mp4\",\"service nip96\",\"bitrate 2000000\",\"duration 29.24\"]]"), 0);
  #assert f.status == N94_OK && f.height == 720 && f.x[0] == 0xe1 && f.others == 5;
  f = n94_imeta(nt_event("[[\"imeta\",\"dim 1280x720\",\"url https://myvideo.com/720/12345.m3u8\",\"x 704e720af2697f5d6a198ad377789d462054b6e8d790f8a3903afbc1e044014f\",\"m application/x-mpegURL\",\"image https://myvideo.com/720/12345.jpg\",\"image https://myotherserver.com/720/12345.jpg\",\"fallback https://myotherserver.com/720/12345.m3u8\",\"fallback https://andanotherserver.com/720/12345.m3u8\",\"service nip96\",\"duration 29.21\"]]"), 0);
  #assert f.status == N94_OK && n32_is(f.m, "application/x-mpegURL") && f.x[31] == 0x4f && f.others == 4;
  f = n94_imeta(nt_event("[[\"imeta\",\"url https://myaudio.com/audio/en/12345.mp3\",\"x b2e0a7a82ac9f3f3a71f1d9a78c381d5be9d1cf19dce258765c17c8a76287c93\",\"m audio/mp3\",\"waveform 0 7 35 8 100 100 49 8 4 16 8 10 7 2 20 10 100 100 100 100 100 100 15 100 100 100 25 60 5 4 3 1 0 100 100 15 100 29 88 0 33 11 39 100 100 19 4 100 42 35 5 0 1 5 0 0 11 38 100 94 17 11 44 58 5 100 100 100 55 14 72 100 100 57 6 1 14 2 16 100 100 40 16 100 100 6 32 14 13 41 36 16 14 6 3 0 1 2 1 6 0\",\"l en ISO-639-1 ov\",\"fallback https://myotherserver.com/audio/en/12345.mp3\",\"fallback https://andanotherserver.com/audio/en/12345.mp3\",\"service nip96\",\"bitrate  320000\",\"duration 29.24\"]]"), 0);
  #assert f.status == N94_OK && !f.has_dim && f.x[0] == 0xb2 && f.fallbacks == 2 && f.others == 5;
  f = n94_imeta(nt_event("[[\"imeta\",\"url https://blossom.primal.net/5fe7df0e46ee6b14b5a8b8b92939e84e3ca5e3950eb630299742325d5ed9891b.mp4\",\"waveform 0 7 35 8 100 100 49 8 4 16 8 10 7 2 20 10 100 100 100 100 100 100 15 100 100 100 25 60 5 4 3 1 0 100 100 15 100 29 88 0 33 11 39 100 100 19 4 100 42 35 5 0 1 5 0 0 11 38 100 94 17 11 44 58 5 100 100 100 55 14 72 100 100 57 6 1 14 2 16 100 100 40 16 100 100 6 32 14 13 41 36 16 14 6 3 0 1 2 1 6 0\",\"duration 8\"]]"), 0);
  #assert f.status == N94_OK && f.others == 2 && !f.has_x;
  // NIP-A0's URL names its blob, so a client can still check the bytes.
  a := nb7_address("https://blossom.primal.net/5fe7df0e46ee6b14b5a8b8b92939e84e3ca5e3950eb630299742325d5ed9891b.mp4");
  #assert a.ok && a.hash[0] == 0x5f && a.hash[31] == 0x1b;
}

// NIP-B7's kind 10063 example. Its stated id (e4bee088...) is not the
// SHA-256 of its serialization; Python and Coax both compute bb431f39...
// So the event is refused as it stands; its tags still read.
#test "NIP-B7's server list example" {
  text := "{\"id\":\"e4bee088334cb5d38cff1616e964369c37b6081be997962ab289d6c671975d71\",\"pubkey\":\"781208004e09102d7da3b7345e64fd193cd1bc3fce8fdae6008d77f9cabcd036\",\"content\":\"\",\"kind\":10063,\"created_at\":1708774162,\"tags\":[[\"server\",\"https://blossom.self.hosted\"],[\"server\",\"https://cdn.blossom.cloud\"]],\"sig\":\"cc5efa74f59e80622c77cacf4dd62076bcb7581b45e9acff471e7963a1f4d8b3406adab5ee1ac9673487480e57d20e523428e60ffcc7e7a904ac882cfccfc653\"}";
  scratch: [4096] u8; hex: [64] u8;
  e := nostr_event_read(text, scratch[..], false);
  #assert e.error == NE_ID && hex32(e.computed_id, hex[..]) == "bb431f3950fe3dcd5b5ba3630cc1c88135047d7f171459ba4c0af554f914aff0";
  list: [2] N32Text;
  s := nb7_servers(n32_event(10063, "[[\"server\",\"https://blossom.self.hosted\"],[\"server\",\"https://cdn.blossom.cloud\"]]"), list[..]);
  #assert s.status == NB7_OK && s.n == 2 && n32_is(list[0], "https://blossom.self.hosted");
}
```

```shell
coax/bin/coax test nostr/examples/checks/media-files.coax --run-steps 250000000
```

`coax test` runs 372 checks, including the loaded files' own checks; all
pass.

### The library's own checks

`nip94.coax` and `nipb7.coax` have their own `#test` blocks: NIP-92's
example, every refusal and the tag and entry it names, the 256-byte value
and 1,024-byte entry bounds at their edges, kind 1063 with each required
tag missing, the hash check with `x`, `ox` and `size`, a 1,100,000-byte
file hashed in two pieces against Python's `hashlib`, a 1,000-byte one
hashed with seven piece sizes, the writers
round-tripping through the readers, blob URLs with and without
extensions, server lists, and the authorization rules at each edge of the
window, for each verb, with one and several `x` tags.

```shell
coax/bin/coax test coax/lib/nostr/nip94.coax
coax/bin/coax test coax/lib/nostr/nipb7.coax
coax/bin/coax mutate coax/lib/nostr/nip94.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nipb7.coax --root-only
```

On 2026-11-01, `coax test` passed all 209 and 217 checks (with the loaded
files'). `coax mutate --root-only` killed all 323 mutants of `nip94.coax`
and all 215 of `nipb7.coax`, with none surviving and none marked
equivalent. After the streaming hash, `nip94.coax` passed 211 checks and
`coax mutate --root-only` (two shards, merged) killed all 343 of its
mutants, none marked equivalent. The 1,100,000-byte check takes about 11
seconds. The full mutation run (`COAX_MUTATION_FULL=1`) and a mutation
run of the example were not run.

## Summary

- An `imeta` tag is `"key value"` entries split at the first space, with a
  `url` and at least one other field. `n94_imeta` reads it and names the
  refusing entry.
- A kind 1063 event carries the same fields as tags. `n94_file` requires
  `url`, `m` and `x`.
- `n94_check` accepts downloaded bytes only when their SHA-256 equals `x`
  and their count equals `size`.
- A Blossom URL names its blob. `nb7_fetch` checks bytes from any server
  against it, and `nb7_servers` lists where else to look.
- `nb7_admit` admits a verified kind 24242 event for one verb and one blob
  between its `created_at` and its `expiration`.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [49. HTTP auth](ch049-http-auth.md)

## Sources

- [NIP-92](../nips/official/92.md)
- [NIP-94](../nips/official/94.md)
- [NIP-68](../nips/official/68.md)
- [NIP-71](../nips/official/71.md)
- [NIP-A0](../nips/official/A0.md)
- [NIP-B7](../nips/official/B7.md)
- [NIP-96](../nips/official/96.md)
- The kinds table: [nostr/nips/official/README.md](../nips/official/README.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
