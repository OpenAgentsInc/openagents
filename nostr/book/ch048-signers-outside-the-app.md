# 48. Signers outside the app

Many people keep their Nostr key in another program: a browser extension,
an Android signer app, or a remote signer (a "bunker") on another device.
The app asks that program to sign and never holds the key. This chapter
describes the browser and Android interfaces and builds NIP-46 remote
signing in Coax: both connection URIs, the request and response messages,
their NIP-44 encryption, the client's table of waiting requests, the
connect secret, and the remote signer's permission check.

Part X, Every NIP. Result: NIP-07's `window.nostr`, NIP-46 remote signing
over kind 24133 with `bunker://` and `nostrconnect://` URIs, and NIP-55's
Android signer intents.

## What this chapter teaches

- NIP-07: `getPublicKey()`, `signEvent()`, and `nip44.encrypt`/`decrypt`;
  the page never sees the key.
- NIP-46: requests and responses as NIP-44-encrypted kind 24133 events;
  the methods `connect`, `sign_event`, `get_public_key`, `nip44_encrypt`,
  `nip44_decrypt`, `ping`, `switch_relays` and `logout`.
- Three distinct keys: the client's, the remote signer's and the user's.
  The client calls `get_public_key` after `connect` to learn the user's
  key.
- A one-use connect secret, responses matched to requests by id with a
  fixed deadline, an `auth_url` challenge, and permissions by method and
  kind, decided in an `#authority` file.
- NIP-55: Android intents and the content resolver, described only.
- The legacy NIP-04 methods are refused with an explicit error.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip46.coax](../../coax/lib/nostr/nip46.coax) | `n46_uri(text)`, `n46_forget(u)` | Reads a `bunker://` or `nostrconnect://` URI; wipes it |
| [nip46.coax](../../coax/lib/nostr/nip46.coax) | `n46_perms(text)`, `n46_allow`, `n46_meet` | Reads a perms list; checks one call against it; intersects what is offered and asked |
| [nip46.coax](../../coax/lib/nostr/nip46.coax) | `n46_request(text)`, `n46_response(text)`, `n46_template(text)`, `n46_auth(r)` | Read a request's and a response's plaintext, `sign_event`'s template, and an auth challenge |
| [nip46.coax](../../coax/lib/nostr/nip46.coax) | `n46_request_json`, `n46_response_json`, `n46_sign_params` | Write them |
| [nip46.coax](../../coax/lib/nostr/nip46.coax) | `n46_tags`, `n46_event`, `n46_key`, `n46_seal`, `n46_open` | The `p` tag, the reading of an event, the conversation key, NIP-44 v2 encryption and decryption |
| [nip46.coax](../../coax/lib/nostr/nip46.coax) | `n46_client`, `n46_ask`, `n46_take`, `n46_expire` | The client's state and its table of waiting requests |
| [nip46.coax](../../coax/lib/nostr/nip46.coax) | `n46_admit`, `n46_accept`, `n46_session` | The remote signer's decision on each request, and the session of a pasted `nostrconnect://` URI |
| [keys.coax](../../coax/lib/nostr/keys.coax) | `signer_ask`, `signer_approve`, `signer_reply` | Chapter 12's signer-request contract: the expected event id, and the check of a signer's reply |
| [nip44-key.coax](../../coax/lib/nostr/nip44-key.coax) | `nip44_conversation_key_from_keys_checked` | ECDH and HKDF-extract (chapter 20) |

`nip46.coax` is new with this chapter. It is an `#authority` file, because
`n46_admit` decides what a client may have signed or encrypted with the
user's key. Every procedure runs in a check, and `coax mutate --root-only`
leaves no survivors. `coax/tests/nostr/nip46_fixtures.coax` holds the
checks' test records, so that mutation changes the library and not the
checks.

Nothing in `nip46.coax` opens a socket, verifies a signature or signs. The
caller publishes and subscribes (chapter 31), verifies events (chapter 21),
hands a `sign_event` result to chapter 12's `signer_reply`, and its signer
signs.

## Three ways to reach a signer

All three interfaces offer the same small set of operations: tell me the
user's public key, sign this unsigned event, and encrypt or decrypt with
the user's key. They differ in the transport.

| Interface | Transport | Who builds it here |
| --- | --- | --- |
| NIP-07 | A JavaScript object, `window.nostr`, injected by a browser extension | The host: `coax/web/host.js` calls it; chapter 12's `signer_template` writes its input and `signer_reply` checks its output |
| NIP-55 | Android intents and content-resolver queries to a signer app | Not built. Described below |
| NIP-46 | Encrypted kind 24133 events over relays | This chapter, in Coax |

### NIP-07: the browser's `window.nostr`

A browser extension defines `window.nostr` with two required methods:

```jai
async window.nostr.getPublicKey(): string
async window.nostr.signEvent(event: {created_at, kind, tags, content}): Event
```

`signEvent` takes an unsigned event and returns it with `id`, `pubkey` and
`sig` added. The optional `nip44.encrypt(pubkey, plaintext)` and
`nip44.decrypt(pubkey, ciphertext)` encrypt with the user's key. The
`nip04` methods are deprecated; this stack doesn't call them.

Calling a JavaScript object is a host effect, so the Coax side is only the
contract. Chapter 12's `signer_template` writes the
`{"kind","created_at","tags","content"}` object that
`coaxial_nip07_sign` in `coax/web/host.js` passes to `signEvent`. When the
extension answers, `signer_reply` accepts the event only if its id is the
one the request fixed, its pubkey is the requested identity and its
signature verifies. An extension that changed the content or signed with
another key is refused by name.

### NIP-55: Android signer apps

On Android, a client reaches a signer app in one of three ways:

- **Intents.** The client starts an activity with a `nostrsigner:` URI
  whose data is the payload (for example, the event JSON) and whose extras
  carry `type` (`get_public_key`, `sign_event`, `nip44_encrypt`,
  `nip44_decrypt`, `decrypt_zap_event`, ...), `pubkey` and `current_user`.
  The answer comes back as extras: `result`, `event` for `sign_event`, or
  `rejected` set to `true` when the user said no.
- **Content resolver.** For requests the user pre-approved, the client
  queries `content://<package>.<TYPE>` with the payload in `selectionArgs`
  and reads a `result` column without showing the signer. A `rejected`
  column means "always reject": the client must not fall back to an
  intent.
- **Web applications.** A web page opens a `nostrsigner:` URL with a
  `callbackUrl`, and the signer appends the result to it.

The first call is `get_public_key`, which returns the user's key and the
signer's package name; later calls go to that package. Permissions are
objects such as `{"type":"sign_event","kind":22242}`. None of this is built
here: it is all host effects on a platform this repository has no app for.
A future Android host would hand the `sign_event` result to the same
`signer_reply` check.

## NIP-46: the keys and the flow

NIP-46 uses three key pairs:

- The **client key**, made by the app for this connection. The app
  encrypts its requests with it and deletes it on logout.
- The **remote signer key**, used by the bunker to encrypt its responses.
- The **user key**, which signs the user's events. The bunker holds it. It
  may equal the remote signer key, but the client must not assume so.

The flow:

1. A connection is made with one of the two URIs below. The bunker learns
   the client key, and the client learns the remote signer key.
2. The client sends requests: kind 24133 events signed by the client key,
   with one `p` tag naming the remote signer key, and the request JSON in
   the content, NIP-44 encrypted to the remote signer key.
3. The bunker answers with kind 24133 events signed by the remote signer
   key, `p`-tagging the client key, encrypted back.
4. The client calls `get_public_key` to learn the user key.

The methods, their params (an array of strings) and results:

| Method | Params | Result |
| --- | --- | --- |
| `connect` | the remote signer key, the secret, requested perms, client metadata (the last three optional) | `"ack"` or the secret |
| `sign_event` | the unsigned event, as a JSON string | the signed event, as a JSON string |
| `ping` | none | `"pong"` |
| `get_public_key` | none | the user key |
| `nip44_encrypt`, `nip44_decrypt` | a third party's key, the text | the ciphertext or the plaintext |
| `switch_relays` | none | the bunker's relay list, or null |
| `logout` | none | `"ack"` |
| `nip04_encrypt`, `nip04_decrypt` | a third party's key, the text | refused here |

The names, the param counts and the client's states as the code keeps them:

<!-- source: coax/lib/nostr/nip46.coax names -->
```jai
// The methods, in the NIP's table order.
M_CONNECT :: 0; M_SIGN :: 1; M_PING :: 2; M_PUBKEY :: 3; M_NIP04_ENC :: 4; M_NIP04_DEC :: 5;
M_NIP44_ENC :: 6; M_NIP44_DEC :: 7; M_SWITCH :: 8; M_LOGOUT :: 9;
N46_METHODS :: [10] string.["connect", "sign_event", "ping", "get_public_key", "nip04_encrypt",
  "nip04_decrypt", "nip44_encrypt", "nip44_decrypt", "switch_relays", "logout"];
// How many string params each method takes: connect takes the signer's key
// and up to three optional ones (secret, perms, client metadata).
N46_LEAST :: [10] int.[1, 1, 0, 0, 2, 2, 2, 2, 0, 0];
N46_MOST :: [10] int.[4, 1, 0, 0, 2, 2, 2, 2, 0, 0];
// The two flows: the remote signer's bunker:// token, or the client's
// nostrconnect:// token.
N46_BUNKER :: 1; N46_NOSTRCONNECT :: 2;
// The client's states.
C_IDLE :: 0; C_CONNECTING :: 1; C_CONNECTED :: 2; C_READY :: 3; C_CLOSED :: 4;
// A waiting request's.
P_FREE :: 0; P_WAIT :: 1; P_AUTH :: 2;
```

And the records: a URI, a request, a response, the client and the bunker.

<!-- source: coax/lib/nostr/nip46.coax record -->
```jai
N46Str :: struct {b:[N46_MAX_STR]u8; n:int;}
N46Id :: struct {b:[N46_MAX_ID]u8; n:int;}
// Permissions: methods allowed for any params (one bit per N46_METHODS
// entry; the sign_event bit means any kind), and the kinds sign_event may
// have signed besides. unknown counts entries this file doesn't grant.
N46Perms :: struct {error:int; methods:int; kinds:[N46_MAX_KINDS]int; nkinds:int; unknown:int;}
// A connection token. peer is the remote signer's key in a bunker:// URI
// and the client's key in a nostrconnect:// one. The secret is kept, never
// written anywhere but the connect message.
N46Uri :: struct {
  error:int=N46_SCHEME; flow:int; peer:[32]u8;
  relays:[N46_MAX_RELAYS]N46Str; nrelays:int;
  secret:N46Id; perms:N46Perms; name:N46Str;
}
// A request: {"id", "method", "params": [strings]}.
N46Call :: struct {error:int=N46_JSON; id:N46Id; method:int=-1; params:[4]N46Str; nparams:int;}
// A response: {"id", "result", "error"}; error present means it failed.
N46Reply :: struct {error:int=N46_JSON; id:N46Id; result:N46Str; has_result:bool; failed:bool; message:N46Str;}
// An unsigned event as sign_event's param carries it: spans into its text.
N46Template :: struct {error:int=N46_FIELD; kind:int; created_at:int; tags_from:int; tags_to:int; content_from:int; content_to:int;}
N46Out :: struct {error:int; n:int;}
// The client: its flow, the signer once known, the user's key once
// get_public_key answered, and the requests waiting for answers.
N46Pending :: struct {state:int; id:N46Id; method:int; deadline:int;}
N46Client :: struct {
  state:int; flow:int; signer:[32]u8; user:[32]u8; secret:N46Id; deadline:int;
  pending:[N46_SLOTS]N46Pending;
}
N46Take :: struct {error:int; method:int=-1; slot:int=-1;}
// The remote signer: its own key, the one-use secret of its bunker:// URI,
// what its user offers a client, and the connected clients.
N46Session :: struct {live:bool; client:[32]u8; perms:N46Perms;}
N46Bunker :: struct {key:[32]u8; secret:N46Id; used:bool; offer:N46Perms; sessions:[N46_SESSIONS]N46Session;}
N46Admit :: struct {error:int; method:int=-1; kind:int=-1; session:int=-1;}
```

## The two connection URIs

The bunker gives the user a token to paste into the client:

```jai
bunker://<remote signer key>?relay=<url>&relay=<url>&secret=<secret>
```

Or the client shows the user a token to paste into the bunker:

```jai
nostrconnect://<client key>?relay=<url>&secret=<secret>&perms=<list>&name=<name>
```

- The key is 64 lowercase hex digits.
- `relay` is required and may repeat; this code keeps up to four. Values
  are percent-encoded, and `+` reads as a space (the NIP's example writes
  `name=My+Client`).
- `secret` is a short random string. In a `nostrconnect://` URI it is
  required: the bunker returns it as the result of its `connect` response,
  and the client checks it, so that nobody else can answer as the bunker.
  In a `bunker://` URI the NIP makes it optional and single use. This code
  requires it in both, because a `bunker://` token without one lets anyone
  who sees it connect.
- `perms` (only in `nostrconnect://`) and `name` are optional. Other
  parameters, such as `url` and `image`, are skipped.

The secret is kept in the record and written only into the `connect`
message. The example prints its length, never its value; `n46_forget`
wipes the record.

<!-- source: coax/lib/nostr/nip46.coax uri -->
```jai
// One query value with %XX escapes and + (a space) decoded: its length,
// -1 for a broken escape, -2 when it doesn't fit.
n46_unescape :: (s:string,out:[]u8)->int {
  n:=0;i:=0;
  while i<s.count {
    c:=s[i];
    if c=='+' {c=' ';}
    if c=='%' {
      if i+2>=s.count {return -1;}
      h:=json_hex(s[i+1]);l:=json_hex(s[i+2]);
      if h<0 || l<0 {return -1;}
      c=cast(u8)(h*16+l);i+=2;
    }
    if n>=out.count {return -2;}
    out[n]=c;n+=1;i+=1;
  }
  return n;
}
// bunker://<remote-signer key>?relay=...&secret=... or
// nostrconnect://<client key>?relay=...&secret=...&perms=...&name=...:
// the key as 64 lowercase hex digits, one to four ws:// or wss:// relays,
// one secret (both flows here), and for nostrconnect optional perms and
// name. Other parameters are skipped.
n46_uri :: (text:string)->N46Uri {
  u:N46Uri;p:=0;
  if text.count>=73 && text[0..<9]=="bunker://" {u.flow=N46_BUNKER;p=9;}
  if text.count>=79 && text[0..<15]=="nostrconnect://" {u.flow=N46_NOSTRCONNECT;p=15;}
  if p==0 {return u;}
  u.error=N46_KEY;
  if !ne_lower_hex(text[p..<p+64],u.peer[..]) {return u;}
  if text.count>p+64 && text[p+64]!='?' {return u;}
  has_secret:=false;has_perms:=false;has_name:=false;from:=p+65;
  buf:[N46_MAX_STR]u8;
  for i:from..<text.count+1 {
    if i<text.count && text[i]!='&' {continue;}
    part:=text[from..<i];from=i+1;eq:=0;
    while eq<part.count && part[eq]!='=' {eq+=1;}
    key:=part[0..<eq];
    n:=n46_unescape(part[ifx eq<part.count then eq+1 else eq..<part.count],buf[..]);
    known:=key=="relay" || key=="secret" || (u.flow==N46_NOSTRCONNECT && (key=="perms" || key=="name"));
    if n<0 {
      u.error=ifx n== -1 then N46_ESCAPE else N46_LIMIT;
      if known {return u;}
      continue;
    }
    value:=cast(string)buf[0..<n];
    if key=="relay" {
      u.error=N46_LIMIT;
      if u.nrelays==N46_MAX_RELAYS {return u;}
      u.error=N46_RELAY;
      if !nt_relay_ok(value) {return u;}
      for k:0..<n {u.relays[u.nrelays].b[k]=buf[k];}
      u.relays[u.nrelays].n=n;u.nrelays+=1;
    } else if key=="secret" {
      u.error=N46_REPEAT;
      if has_secret {return u;}
      has_secret=true;u.error=N46_SECRET;
      s:[1]N46Id;
      if !n46_token(value,s[..]) {return u;}
      u.secret=s[0];
    } else if key=="perms" && u.flow==N46_NOSTRCONNECT {
      u.error=N46_REPEAT;
      if has_perms {return u;}
      has_perms=true;u.perms=n46_perms(value);u.error=u.perms.error;
      if u.error!=N46_OK {return u;}
    } else if key=="name" && u.flow==N46_NOSTRCONNECT {
      u.error=N46_REPEAT;
      if has_name {return u;}
      has_name=true;
      for k:0..<n {u.name.b[k]=buf[k];}
      u.name.n=n;
    }
  }
  u.error=N46_NO_RELAY;
  if u.nrelays==0 {return u;}
  u.error=ifx has_secret then N46_OK else N46_NO_SECRET;
  return u;
}
// Wipes a token, its secret with it, when the connection is removed.
n46_forget :: (u:[]N46Uri) {for i:0..<u.count {u[i]=N46Uri.{};}}
```

## Requests, responses and the template

A request's plaintext is `{"id", "method", "params"}`: a random id string,
the method name and an array of strings. A response is `{"id", "result",
"error"}`: the same id, a result string, and an error string when the call
failed. Two details:

- The reader keeps the id even when the method is unknown, because the
  NIP says a bunker must answer an unknown method with an error, and the
  error must carry the request's id.
- An **auth challenge** is a response whose result is `"auth_url"` and
  whose error is a URL. The client shows the URL, and the bunker answers the
  same id again once the user has authenticated there.

`sign_event`'s one param is the unsigned event as a JSON string:
`{"kind","created_at","tags","content"}` in any order. `n46_template`
reads it into spans of its text.

<!-- source: coax/lib/nostr/nip46.coax read -->
```jai
// A request's plaintext. The id is read even when the method is unknown,
// so that the remote signer can answer with an error under that id.
n46_request :: (text:string)->N46Call {
  c:N46Call;j:[1]Json;json_start(j[..],text);has_id:=false;has_method:=false;has_params:=false;
  if json_next(j[..])!=JSON_OBJECT {return c;}
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);raw:=json_text(j[..]);
    if json_is(key,"id") {
      c.error=N46_REPEAT;
      if has_id {return c;}
      has_id=true;c.error=N46_ID;c.id.n=json_unescape(raw,c.id.b[..]);
      if v!=JSON_STRING || !n46_printable(c.id) {return c;}
    } else if json_is(key,"method") {
      c.error=N46_REPEAT;
      if has_method {return c;}
      has_method=true;c.error=N46_METHOD;
      if v!=JSON_STRING {return c;}
      c.method=n46_word(raw);
    } else if json_is(key,"params") {
      c.error=N46_REPEAT;
      if has_params {return c;}
      has_params=true;c.error=N46_PARAMS;
      if v!=JSON_ARRAY {return c;}
      u:=json_next(j[..]);
      while u!=JSON_ARRAY_END {
        c.error=N46_PARAMS;
        if u!=JSON_STRING || c.nparams==4 {return c;}
        c.error=N46_LIMIT;
        n:=json_unescape(json_text(j[..]),c.params[c.nparams].b[..]);
        if n<0 {return c;}
        c.params[c.nparams].n=n;c.nparams+=1;
        u=json_next(j[..]);
      }
    } else if !json_skip(j[..]) {c.error=N46_JSON;return c;}
    t=json_next(j[..]);
  }
  c.error=N46_JSON;
  if t!=JSON_OBJECT_END || json_next(j[..])!=JSON_END {return c;}
  c.error=N46_ID;
  if !has_id {return c;}
  c.error=N46_METHOD;
  if c.method<0 {return c;}
  c.error=N46_PARAMS;
  if !has_params || c.nparams<N46_LEAST[c.method] || c.nparams>N46_MOST[c.method] {return c;}
  c.error=N46_OK;
  return c;
}
// A response's plaintext: result a string (or null, absent), error a
// string when the call failed. One of the two must be there.
n46_response :: (text:string)->N46Reply {
  r:N46Reply;j:[1]Json;json_start(j[..],text);has_id:=false;seen_result:=false;seen_error:=false;
  if json_next(j[..])!=JSON_OBJECT {return r;}
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);raw:=json_text(j[..]);
    r.error=N46_REPEAT;
    if json_is(key,"id") {
      if has_id {return r;}
      has_id=true;r.error=N46_ID;r.id.n=json_unescape(raw,r.id.b[..]);
      if v!=JSON_STRING || !n46_printable(r.id) {return r;}
    } else if json_is(key,"result") {
      if seen_result {return r;}
      seen_result=true;r.error=N46_FIELD;
      if v!=JSON_STRING && v!=JSON_NULL {return r;}
      if v==JSON_STRING {
        r.has_result=true;r.error=N46_LIMIT;r.result.n=json_unescape(raw,r.result.b[..]);
        if r.result.n<0 {r.result.n=0;return r;}
      }
    } else if json_is(key,"error") {
      if seen_error {return r;}
      seen_error=true;r.error=N46_FIELD;
      if v!=JSON_STRING && v!=JSON_NULL {return r;}
      if v==JSON_STRING {
        r.failed=true;r.error=N46_LIMIT;r.message.n=json_unescape(raw,r.message.b[..]);
        if r.message.n<0 {r.message.n=0;return r;}
      }
    } else if !json_skip(j[..]) {r.error=N46_JSON;return r;}
    t=json_next(j[..]);
  }
  r.error=N46_JSON;
  if t!=JSON_OBJECT_END || json_next(j[..])!=JSON_END {return r;}
  r.error=N46_ID;
  if !has_id {return r;}
  r.error=ifx r.has_result || r.failed then N46_OK else N46_FIELD;
  return r;
}
// An auth challenge: result "auth_url", the URL to show in error.
n46_auth :: (r:N46Reply)->bool {
  own:=r.result;
  return r.error==N46_OK && r.failed && r.has_result && cast(string)own.b[0..<own.n]=="auth_url";
}
// sign_event's param: {"kind", "created_at", "tags", "content"} in any
// order, other keys skipped.
n46_template :: (text:string)->N46Template {
  m:N46Template;j:[1]Json;json_start(j[..],text);have:=0;
  if json_next(j[..])!=JSON_OBJECT {return m;}
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);x:=json_int(json_text(j[..]));bit:=0;
    if json_is(key,"kind") {bit=1;if v!=JSON_NUMBER || !x.ok || x.value<0 || x.value>65535 {return m;} m.kind=x.value;}
    else if json_is(key,"created_at") {bit=2;if v!=JSON_NUMBER || !x.ok || x.value<0 {return m;} m.created_at=x.value;}
    else if json_is(key,"tags") {bit=4;s:=ne_tags(j[..]);if !s.ok {return m;} m.tags_from=s.from;m.tags_to=s.to;}
    else if json_is(key,"content") {bit=8;if v!=JSON_STRING {return m;} m.content_from=j[0].from;m.content_to=j[0].to;}
    else if !json_skip(j[..]) {return m;}
    if (have&bit)!=0 {return m;}
    have|=bit;
    t=json_next(j[..]);
  }
  if t!=JSON_OBJECT_END || json_next(j[..])!=JSON_END || have!=15 {return m;}
  m.error=N46_OK;
  return m;
}
```

The writers emit the same bytes as Python's `json.dumps` with compact
separators and the keys in the order shown. `n46_sign_params` writes the
template in the order chapter 12's `signer_template` uses for NIP-07.

<!-- source: coax/lib/nostr/nip46.coax write -->
```jai
// A request's plaintext, or -1 when it isn't one n46_request would accept
// or out is too small.
n46_request_json :: (c:N46Call,out:[]u8)->int {
  if c.method<0 || c.id.n==0 || c.nparams<N46_LEAST[c.method] || c.nparams>N46_MOST[c.method] {return -1;}
  id:=c.id;
  n:=json_raw(out,0,"{\"id\":");n=json_write_string(out,n,cast(string)id.b[0..<id.n]);
  n=json_raw(out,n,",\"method\":");n=json_write_string(out,n,N46_METHODS[c.method]);
  n=json_raw(out,n,",\"params\":[");
  for k:0..<c.nparams {
    if k>0 {n=json_raw(out,n,",");}
    p:=c.params[k];n=json_write_string(out,n,cast(string)p.b[0..<p.n]);
  }
  return json_raw(out,n,"]}");
}
// A response's plaintext: id, then result and error when present.
n46_response_json :: (r:N46Reply,out:[]u8)->int {
  if r.id.n==0 || (!r.has_result && !r.failed) {return -1;}
  id:=r.id;res:=r.result;msg:=r.message;
  n:=json_raw(out,0,"{\"id\":");n=json_write_string(out,n,cast(string)id.b[0..<id.n]);
  if r.has_result {n=json_raw(out,n,",\"result\":");n=json_write_string(out,n,cast(string)res.b[0..<res.n]);}
  if r.failed {n=json_raw(out,n,",\"error\":");n=json_write_string(out,n,cast(string)msg.b[0..<msg.n]);}
  return json_raw(out,n,"}");
}
// sign_event's param for an event: kind, created_at, tags, content, the
// order chapter 12's signer_template writes for NIP-07.
n46_sign_params :: (kind:int,created_at:int,tags:string,content:string,out:[]u8)->int {
  decoded:[N46_MAX_STR]u8;
  if kind<0 || kind>65535 || created_at<0 {return -1;}
  n:=json_raw(out,0,"{\"kind\":");n=json_write_int(out,n,kind);
  n=json_raw(out,n,",\"created_at\":");n=json_write_int(out,n,created_at);
  n=json_raw(out,n,",\"tags\":");n=ne_write_tags(tags,out,n,decoded[..]);
  n=json_raw(out,n,",\"content\":");n=json_write_string(out,n,content);
  return json_raw(out,n,"}");
}
```

## The events and their encryption

Both directions are kind 24133 with one `p` tag. The conversation key is
NIP-44's: the client's secret with the remote signer key, or the remote
signer's secret with the client key, which gives the same 32 bytes. A
NIP-04 payload (one with `?iv=`) is refused before anything is decrypted.

<!-- source: coax/lib/nostr/nip46.coax events -->
```jai
// Both directions carry one p tag: the key the event is for.
n46_tags :: (peer:[32]u8,out:[]u8)->int {
  hex:[64]u8;own:=peer;h:=hex_encode_checked(cast(string)own[..],hex[..],false);
  n:=json_raw(out,0,"[[\"p\",\"");n=json_raw(out,n,cast(string)hex[..]);
  return json_raw(out,n,"\"]]");
}
// The reading of an event by the key me: kind 24133 and a p tag naming me.
n46_event :: (e:NostrEvent,me:[32]u8)->int {
  if e.error!=NE_OK || e.kind!=N46_EVENT {return N46_KIND;}
  for i:0..<e.tags {
    t:=nostr_tag(e,i);
    if t.error==NT_OK && t.type==NT_P && n46_same(t.pubkey,me) {return N46_OK;}
  }
  return N46_P;
}
// The conversation key of this side's secret and the other side's key.
n46_key :: (secret:[]u8,peer:[32]u8,out:[]u8)->int {
  work:[1]EcdhScratch;pub:=peer;
  r:=nip44_conversation_key_from_keys_checked(secret,pub[..],work[..],out,false);
  return ifx r==NIP44_OK then N46_OK else N46_KEY;
}
// plain encrypted as NIP-44 v2 with the caller's fresh nonce.
n46_seal :: (key:[]u8,nonce:[]u8,plain:string,out:[]u8)->N46Out {
  o:N46Out;o.error=N46_LIMIT;
  if plain.count>N46_MAX_PLAIN {return o;}
  work:[N46_WORK]u8;bytes:[N46_MAX_PLAIN]u8;
  for i:0..<plain.count {bytes[i]=plain[i];}
  r:=nip44_encrypt_checked(NIP44_COMPAT,key,nonce,bytes[0..<plain.count],work[..],out,false);
  o.error=ifx r.error==NIP44_OK then N46_OK else N46_KEY;o.n=r.written;
  return o;
}
// An event's content opened. A NIP-04 payload (its ?iv= suffix) is
// refused; NIP-44's MAC is checked before decryption.
n46_open :: (key:[]u8,payload:string,out:[]u8)->N46Out {
  o:N46Out;o.error=N46_ENCRYPTION;
  for c:payload {if c=='?' {return o;}}
  work:[N46_WORK]u8;
  r:=nip44_decrypt_checked(NIP44_COMPAT,key,payload,work[..],out,false);
  o.error=ifx r.error==NIP44_OK then N46_OK else N46_CIPHER;o.n=r.written;
  return o;
}
```

## The client: waiting requests, deadlines and the secret

The client keeps up to eight requests waiting for answers. Each has the
caller's random id, its method and a deadline fixed when it is sent: the
caller's clock plus a wait of 1 to 300 seconds. A response counts only if:

1. It is opened from an event `p`-tagged to the client and written by the
   remote signer key.
2. Its id is the id of a waiting request. Ids are compared as strings; a
   second response to an answered id is `no request has this id`.
3. It arrives before that request's deadline. A late response frees the
   slot and is refused.

Then an auth challenge keeps the request waiting, an error frees it, and a
result frees it after a check per method: `connect` must return `"ack"` or
the secret, `get_public_key` a 64-digit lowercase key, `ping` `"pong"`,
and `logout` `"ack"`. A `sign_event` result is handed to chapter 12's
`signer_reply`, which accepts it only if it is the requested event, signed
by the user key.

The client's state decides what it may send. `connect` only before
connecting with a `bunker://` token; `get_public_key`, `ping`,
`switch_relays` and `logout` once connected; `sign_event` and `nip44_*`
only after `get_public_key` has answered. The NIP-04 methods are never
sent.

For its own `nostrconnect://` token the client sends nothing first. It
waits until its deadline for a response from any key whose result is the
token's secret. That response's author becomes the remote signer key.

<!-- source: coax/lib/nostr/nip46.coax client -->
```jai
// A client for a token: for bunker:// it knows the signer and sends
// connect; for its own nostrconnect:// it waits, until now + wait, for the
// signer's connect response.
n46_client :: (u:N46Uri,now:int,wait:int)->N46Client {
  c:N46Client;c.state=C_CLOSED;
  if u.error!=N46_OK || wait<1 || wait>N46_MAX_WAIT {return c;}
  c.flow=u.flow;c.secret=u.secret;c.state=C_IDLE;
  if u.flow==N46_BUNKER {c.signer=u.peer;return c;}
  c.state=C_CONNECTING;c.deadline=now+wait;
  return c;
}
// Records a request with the caller's random id, answered by now + wait.
// connect only from a bunker:// token before connecting; get_public_key,
// ping, switch_relays and logout once connected; sign_event and nip44_*
// once the user's key is known; the NIP-04 methods never.
n46_ask :: (c:[]N46Client,method:int,id:string,now:int,wait:int)->int {
  if method<0 || method>=N46_METHODS.count || wait<1 || wait>N46_MAX_WAIT {return N46_WAIT;}
  if n46_legacy(method) {return N46_LEGACY;}
  s:=c[0].state;
  need:=ifx method==M_SIGN || method==M_NIP44_ENC || method==M_NIP44_DEC then s==C_READY else s==C_CONNECTED || s==C_READY;
  if method==M_CONNECT {need=s==C_IDLE && c[0].flow==N46_BUNKER;}
  if !need {return N46_STATE;}
  t:[1]N46Id;
  if !n46_token(id,t[..]) {return N46_ID;}
  free:=-1;
  for k:0..<N46_SLOTS {
    q:=c[0].pending[k];
    if q.state!=P_FREE && n46_eq(q.id,t[0]) {return N46_REPEAT;}
    if q.state==P_FREE && free<0 {free=k;}
  }
  if free<0 {return N46_FULL;}
  c[0].pending[free]=N46Pending.{state=P_WAIT,id=t[0],method=method,deadline=now+wait};
  if method==M_CONNECT {c[0].state=C_CONNECTING;}
  return N46_OK;
}
// Frees a slot; a connect that ended without success leaves the client idle.
n46_end :: (c:[]N46Client,k:int) {
  if c[0].pending[k].method==M_CONNECT {c[0].state=C_IDLE;}
  c[0].pending[k]=N46Pending.{};
}
// Marks every request past its deadline expired; returns how many.
n46_expire :: (c:[]N46Client,now:int)->int {
  n:=0;
  for k:0..<N46_SLOTS {if c[0].pending[k].state!=P_FREE && now>c[0].pending[k].deadline {n46_end(c,k);n+=1;}}
  if c[0].flow==N46_NOSTRCONNECT && c[0].state==C_CONNECTING && now>c[0].deadline {c[0].state=C_CLOSED;n+=1;}
  return n;
}
// An opened response from author at time now. The waiting request with its
// id takes it: an auth challenge keeps it waiting (N46_AUTH), an error
// frees it (N46_ERROR), a result frees it. connect must answer "ack" or
// the secret; for a nostrconnect:// token the response comes unasked and
// must echo the secret, and its author becomes the signer.
n46_take :: (c:[]N46Client,author:[32]u8,r:N46Reply,now:int)->N46Take {
  k:N46Take;k.error=r.error;
  if r.error!=N46_OK {return k;}
  res:=r.result;result:=cast(string)res.b[0..<res.n];sec:=c[0].secret;
  echo:=r.has_result && !r.failed && result==cast(string)sec.b[0..<sec.n];
  if c[0].flow==N46_NOSTRCONNECT && c[0].state==C_CONNECTING {
    k.error=N46_LATE;
    if now>c[0].deadline {c[0].state=C_CLOSED;return k;}
    k.error=N46_ECHO;
    if !echo {return k;}
    c[0].signer=author;c[0].state=C_CONNECTED;k.error=N46_OK;k.method=M_CONNECT;
    return k;
  }
  k.error=N46_AUTHOR;
  if c[0].state==C_CLOSED || !n46_same(author,c[0].signer) {return k;}
  for i:0..<N46_SLOTS {if c[0].pending[i].state!=P_FREE && n46_eq(c[0].pending[i].id,r.id) {k.slot=i;}}
  k.error=N46_UNKNOWN;
  if k.slot<0 {return k;}
  q:=c[0].pending[k.slot];k.method=q.method;k.error=N46_LATE;
  if now>q.deadline {n46_end(c,k.slot);return k;}
  k.error=N46_AUTH;
  if n46_auth(r) {c[0].pending[k.slot].state=P_AUTH;return k;}
  k.error=N46_ERROR;
  if r.failed {n46_end(c,k.slot);return k;}
  k.error=N46_FIELD;
  if q.method==M_CONNECT && !echo && result!="ack" {n46_end(c,k.slot);return k;}
  if q.method==M_PUBKEY && !ne_lower_hex(result,c[0].user[..]) {n46_end(c,k.slot);return k;}
  if q.method==M_PING && result!="pong" {n46_end(c,k.slot);return k;}
  if q.method==M_LOGOUT && result!="ack" {n46_end(c,k.slot);return k;}
  c[0].pending[k.slot]=N46Pending.{};
  if q.method==M_CONNECT {c[0].state=C_CONNECTED;}
  if q.method==M_PUBKEY {c[0].state=C_READY;}
  if q.method==M_LOGOUT {c[0].state=C_CLOSED;}
  k.error=N46_OK;
  return k;
}
```

## The remote signer: the secret and permissions

The bunker decides every request with `n46_admit`. A refusal is answered
with an error string under the request's id.

- `connect` must name the bunker's own key and carry the secret of its
  `bunker://` token. The secret works once: after a successful connect, the
  same secret from any client is `the secret was used`. The session gets
  what the user offers *and* the client asked for, by method and by kind.
- Every other method needs a session for the requesting client key.
- `sign_event` reads the template and checks its kind. `nip44_*` checks
  the third party's key and the method's permission. `ping`,
  `get_public_key`, `switch_relays` and `logout` need only the session;
  `logout` ends it.
- The NIP-04 methods are refused for every client.

A perms list is `method[:param]` separated by commas. `sign_event:13`
grants kind 13; a bare `sign_event` grants every kind; a bare method grants
it with any params. Unknown methods, and params on methods other than
`sign_event`, are counted and not granted. The NIP-46 metadata (`name`,
`url`, `image`) is a display hint only; nothing here authorizes by it.

<!-- source: coax/lib/nostr/nip46.coax perms -->
```jai
// A comma-separated perms list, "method[:param]": sign_event:<kind> grants
// one kind, a bare method grants it for any params. Unknown methods and
// params on other methods are counted, not granted; the NIP-04 methods are
// never granted. An empty list grants nothing.
n46_perms :: (text:string)->N46Perms {
  p:N46Perms;from:=0;
  if text.count==0 {return p;}
  for i:0..<text.count+1 {
    if i<text.count && text[i]!=',' {continue;}
    entry:=text[from..<i];from=i+1;colon:=0;
    while colon<entry.count && entry[colon]!=':' {colon+=1;}
    name:=entry[0..<colon];m:=-1;
    for k:0..<N46_METHODS.count {if name==N46_METHODS[k] {m=k;}}
    p.error=N46_PERMS;
    if colon==0 {return p;}
    if colon==entry.count && m>=0 && !n46_legacy(m) {p.methods|=1<<m;}
    else if m==M_SIGN {
      x:=json_int(entry[colon+1..<entry.count]);
      if !x.ok || x.value<0 || x.value>65535 {return p;}
      known:=false;
      for k:0..<p.nkinds {if p.kinds[k]==x.value {known=true;}}
      p.error=N46_LIMIT;
      if !known && p.nkinds==N46_MAX_KINDS {return p;}
      if !known {p.kinds[p.nkinds]=x.value;p.nkinds+=1;}
    } else {p.unknown+=1;}
    p.error=N46_OK;
  }
  return p;
}
// Whether perms let a session call method (with kind, for sign_event).
// ping, get_public_key, switch_relays and logout need only a session.
n46_allow :: (p:N46Perms,method:int,kind:int)->bool {
  if method==M_PING || method==M_PUBKEY || method==M_SWITCH || method==M_LOGOUT {return true;}
  if (p.methods&(1<<method))!=0 {return true;}
  if method!=M_SIGN {return false;}
  for k:0..<p.nkinds {if p.kinds[k]==kind {return true;}}
  return false;
}
// What a session gets: what the user offers and the client asked for.
n46_meet :: (offer:N46Perms,asked:N46Perms)->N46Perms {
  g:N46Perms;g.methods=offer.methods&asked.methods;
  for k:0..<offer.nkinds {
    // An offered kind the client asked for (by kind, or by any kind).
    if n46_allow(asked,M_SIGN,offer.kinds[k]) {g.kinds[g.nkinds]=offer.kinds[k];g.nkinds+=1;}
  }
  if (g.methods&(1<<M_SIGN))!=0 {return g;}
  for k:0..<asked.nkinds {
    // An asked kind the user offers by any kind and not yet granted.
    if (offer.methods&(1<<M_SIGN))!=0 && !n46_allow(g,M_SIGN,asked.kinds[k]) {g.kinds[g.nkinds]=asked.kinds[k];g.nkinds+=1;}
  }
  return g;
}
```

<!-- source: coax/lib/nostr/nip46.coax signer -->
```jai
// The session of a client key, or -1.
n46_session :: (b:N46Bunker,client:[32]u8)->int {
  for i:0..<N46_SESSIONS {if b.sessions[i].live && n46_same(b.sessions[i].client,client) {return i;}}
  return -1;
}
// A session for client with perms: its old one replaced, else a free one.
n46_open_session :: (b:[]N46Bunker,client:[32]u8,perms:N46Perms)->int {
  at:=n46_session(b[0],client);
  for i:0..<N46_SESSIONS {if at<0 && !b[0].sessions[i].live {at=i;}}
  if at>=0 {b[0].sessions[at]=N46Session.{live=true,client=client,perms=perms};}
  return at;
}
// The remote signer's decision on a request from client: connect checks
// the signer's key and the one-use secret and opens a session with what
// the user offers and the client asked for; every other method needs a
// session and its permission; logout ends the session; the NIP-04 methods
// are refused. A refusal is answered with an error under the request's id.
n46_admit :: (b:[]N46Bunker,client:[32]u8,c:N46Call)->N46Admit {
  a:N46Admit;a.error=c.error;a.method=c.method;
  if c.error!=N46_OK {return a;}
  a.error=N46_LEGACY;
  if n46_legacy(c.method) {return a;}
  p0:=c.params[0];first:=cast(string)p0.b[0..<p0.n];
  if c.method==M_CONNECT {
    key:[32]u8;a.error=N46_KEY;
    if !ne_lower_hex(first,key[..]) || !n46_same(key,b[0].key) {return a;}
    // Params past nparams are empty: the reader leaves them so.
    p1:=c.params[1];a.error=N46_NO_SECRET;
    if p1.n==0 {return a;}
    sec:=b[0].secret;a.error=N46_SECRET;
    if cast(string)p1.b[0..<p1.n]!=cast(string)sec.b[0..<sec.n] {return a;}
    a.error=N46_USED;
    if b[0].used {return a;}
    p2:=c.params[2];asked:=n46_perms(cast(string)p2.b[0..<p2.n]);a.error=asked.error;
    if asked.error!=N46_OK {return a;}
    a.session=n46_open_session(b,client,n46_meet(b[0].offer,asked));a.error=N46_FULL;
    if a.session<0 {return a;}
    b[0].used=true;a.error=N46_OK;
    return a;
  }
  a.session=n46_session(b[0],client);a.error=N46_DENIED;
  if a.session<0 {return a;}
  if c.method==M_SIGN {
    m:=n46_template(first);a.error=N46_PARAMS;
    if m.error!=N46_OK {return a;}
    a.kind=m.kind;
  }
  if c.method==M_NIP44_ENC || c.method==M_NIP44_DEC {
    peer:[32]u8;a.error=N46_PARAMS;
    if !ne_lower_hex(first,peer[..]) {return a;}
  }
  a.error=N46_DENIED;
  if !n46_allow(b[0].sessions[a.session].perms,c.method,a.kind) {return a;}
  if c.method==M_LOGOUT {b[0].sessions[a.session]=N46Session.{};}
  a.error=N46_OK;
  return a;
}
// The signer's side of a nostrconnect:// token its user pasted: a session
// for the client's key with what the user offers and the token asked for.
// The signer then answers connect with the token's secret as its result.
n46_accept :: (b:[]N46Bunker,u:N46Uri)->int {
  if u.error!=N46_OK || u.flow!=N46_NOSTRCONNECT {return N46_SCHEME;}
  return ifx n46_open_session(b,u.peer,n46_meet(b[0].offer,u.perms))<0 then N46_FULL else N46_OK;
}
```

## The worked example, by hand

The example uses three published test keys. Never use them for anything
real.

| Role | Secret key | Public key |
| --- | --- | --- |
| Client | 1 | `79be667e…16f81798` |
| Remote signer | 2 | `c6047f94…709ee5` |
| User | 3 | `f9308a01…bce036f9` |

The bunker's token is
`bunker://c6047f94…709ee5?relay=wss%3A%2F%2Frelay.example.com&secret=<6 bytes>`.
Its user offers `sign_event:1,sign_event:7,nip44_encrypt`. The client
connects asking for `sign_event:1,sign_event:4`, so the session gets kind 1
only: offered and asked. Then `get_public_key` returns `f9308a01…`, which is
neither the client's key nor the remote signer's.

One `sign_event` round trip, at T0 = 1762000000:

1. **Chapter 12 fixes the event.** `signer_ask` with target
   `SIGNER_PHONE`, identity `f9308a01…`, kind 1, created_at T0, tags `[]`
   and content `hello from a bunker` hashes
   `[0,"f9308a01…",1762000000,1,[],"hello from a bunker"]`. The expected
   id is
   `c12bdb20e95e5fef04d90dc20dbf353d840152939c9ba32d017bfd133caac907`.
2. **Write the template and the request.** The template is
   `{"kind":1,"created_at":1762000000,"tags":[],"content":"hello from a bunker"}`.
   As a JSON string inside the request, its quotes are escaped:
   `{"id":"s1","method":"sign_event","params":["{\"kind\":1,...}"]}`, 133
   bytes.
3. **Derive the conversation key.** The client's secret is 1, so the
   shared point is the remote signer's point itself, and the shared x is
   `c6047f94…709ee5`. HKDF-extract with salt `nip44-v2` gives
   `c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d`,
   NIP-44's published conversation key for keys 1 and 2.
4. **Derive the message keys.** With a nonce of 32 bytes 0x46 (fixed for
   the example; a client takes 32 fresh random bytes), HKDF-expand gives the
   ChaCha20 key `1dc39822…325dd9db`, the ChaCha20 nonce
   `7557c544fed256b621631bab` and the HMAC key `97ab69e7…6879772b`.
5. **Pad and encrypt.** 133 bytes: the next power of two is 256, so the
   chunk is 32, and 5 chunks are 160 bytes. With the 2-byte length prefix,
   162 bytes are encrypted. The payload is 1 + 32 + 162 + 32 = 227 bytes,
   which is 4 × ⌈227 / 3⌉ = 304 base64 characters starting `AkZGRkZG`:
   `0x02` and the first 0x46 bytes read as `AkZG`.
6. **Sign the request event.** Kind 24133 at T0, tags
   `[["p","c6047f94…709ee5"]]`, signed with key 1: id
   `574edf34e6bf0d063cccb4f9c12fc90f7373f09af3d8ef0052daffdf3fd66513`.
7. **The bunker answers.** It opens the payload with its own secret and
   the client's key, the session allows kind 1, and it signs the template
   with the user's key. The signed event has id `c12bdb20…`, the id step 1
   expected. The response `{"id":"s1","result":"<the signed event JSON>"}`
   is 406 bytes, sealed with nonce 0x47 and signed with key 2 at T0 + 1:
   id `4d46758978ffcdbe707d6ee2104f0ee1d36818997b8b37c5a65bd74846439f31`.
8. **The client takes it.** The event is `p`-tagged to the client and
   written by the remote signer; id `s1` is waiting; T0 + 1 is before the
   deadline T0 + 60. `signer_reply` finds the result's id is its own and
   the expected one, its pubkey is the user's, and its signature verifies.

## The example

The example, [remote-signer.coax](../../nostr/examples/remote-signer.coax),
plays both sides of a `bunker://` connection: `connect`,
`get_public_key` and the `sign_event` round trip worked out above, each
event signed and sealed. Then the same client sends request `s2` and takes
the response an independent bunker wrote: a Python script that shares no
code with Coax. Then it runs the client's own `nostrconnect://` flow and
the refusals.

<!-- source: nostr/examples/remote-signer.coax example -->
```jai
#load "../../coax/lib/nostr/nip46.coax";
#load "../../coax/lib/nostr/keys.coax";
#import "Basic";

E46 :: [32] string.["ok", "not a bunker:// or nostrconnect:// URI", "bad key", "no relay", "bad relay",
  "no secret", "bad secret", "bad escape", "repeated", "limit", "bad perms", "not JSON", "bad id",
  "unknown method", "bad params", "bad field", "kind", "p names another key", "another author",
  "can't decrypt", "not NIP-44 v2", "not in this state", "table full", "no request has this id",
  "late", "the secret is not echoed", "auth challenge: wait", "the signer answered an error", "bad wait",
  "the secret was used", "not allowed", "NIP-04 methods are refused"];
SR :: [16] string.["ok", "input", "scope", "full", "unknown", "unapproved", "duplicate", "late", "closed",
  "malformed", "tampered", "signed by another key", "another body", "bad signature", "target", "state"];
// Secret keys 1 (the client), 2 (the remote signer) and 3 (the user):
// published test values. Never put a real secret in an example, and never
// print one.
CLIENT :: 1; SIGNER :: 2; USER :: 3;
PUB :: [4] string.["",
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"];
BUNKER :: "bunker://c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5?relay=wss%3A%2F%2Frelay.example.com&secret=s3cret";
CONNECT :: "nostrconnect://79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798?relay=wss%3A%2F%2Frelay.example.com&secret=0s8j2djs&perms=sign_event%3A1&name=Coax";
T0 :: 1762000000;
// The independent bunker's answer to request s2: a Python script written
// from NIP-01, NIP-44, NIP-46 and BIP-340 signed the user's event (secret 3)
// and the response (secret 2) and sealed it with nonce 0x50 repeated.
PY_BUNKER_EVENT :: "{\"id\":\"c24cfeb25376bab1ad4223ddc91a35c6764c40c43396d2d535c513a72ee05ea2\",\"pubkey\":\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\",\"created_at\":1762000003,\"kind\":24133,\"tags\":[[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\"]],\"content\":\"AlBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQ4BMgW2giNOp+ZBgqpgTDGSe6k6rpi+KhDLmlmFu+JuXg3N2xuwbPwcBdfcVZ+WlSXA18czGmogiMCgEO9COSg/chapvY+P/MBiQ3+WyqCJ/xIw7N0ZL76PFPw22t1cMiv6U7ktyys/dIZxeHW9CfhMEIgKJGMQC54T8pm+/cdc2NOkaluDygDrd7FHu4otJZrmH4f1gNX8kSyk/+yw4EZApXNtwDEZwX7+RAPes/9LfUqE7b7LM5lJRGnTLysWa6DM3MUCkKwnEoAEhl2GGhhd44H0/1CWfcvTpj649MZ8/ysh7n0gXrbgn057wqxNWeBUR9+XJBHuC2XlUh7TR1RDW4Afs+x8bARvaZaAisfZUFS39amvUa4EAwmV4xPPRFx+j1bzn3Z50VcQpAh1UQAGuhX9yoDXVYFdsGSb+X7tz5Aa10aDjcybndkzFrI0G73NXIKqK3QzRhvvoHi2paAoq85rLEGHPpBFBCcLXmbyGQDPYn7B5LxobV8Nq0uYsTAfwTP8V+xTN1L7lFbmTv8YE44q2Y2n4PQXgRFwy+7kXCYEKqmBa0/lIWoDMa52zCpkgCTkwXKSlff+N+pox8ZuQKonYwbtaoV118lr1qwFY2s6107/nkCq++oESU9Y5M2GQ=\",\"sig\":\"2fd6c0d9c25228076842b870e26cd2e270a340731e86736c1ae6e3534caec3587d8fe9a9f35e9cf700483ca488b5ab123df52cccf2e8586ed00aaa8decadadb2\"}";

key_of :: (who: int) -> [32] u8 { k: [32] u8; r := hex_decode_checked(PUB[who], k[..], false); return k; }
secret_of :: (who: int) -> [32] u8 { k: [32] u8; k[31] = cast(u8) who; return k; }
hex32 :: (id: [32] u8, out: [] u8) -> string {
  own := id; r := hex_encode_checked(cast(string) own[..], out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<64];
}
say :: (s: N46Str) { own := s; print("%", cast(string) own.b[0..<own.n]); }
of :: (t: string) -> N46Str { s: N46Str; for i: 0..<t.count { s.b[i] = t[i]; } s.n = t.count; return s; }
// A nonce of 32 equal bytes, for a reproducible example only: each side
// takes 32 fresh random bytes for every message.
nonce_of :: (b: u8) -> [32] u8 { n: [32] u8; for i: 0..<32 { n[i] = b; } return n; }
content_of :: (e: NostrEvent) -> string { return e.original[e.content_from..<e.content_to]; }

// Sign an event with secret key who (aux all zero) and read it back with
// its id and signature checked.
sign :: (who: int, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  key := secret_of(who); aux: [32] u8; work: [1] NostrSignWork; scratch: [16384] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [16384] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified && n46_same(e.public_key, key_of(who));
  return e;
}

// One kind 24133 message: its plaintext, payload and signed event.
Msg :: struct { plain: [8192] u8; np: int; sealed: [12288] u8; ns: int; buf: [16384] u8; event: NostrEvent; }
// Seal plain from secret key from to key to with nonce byte nb, and sign
// the event at time t.
seal :: (from: int, to: [32] u8, plain: string, t: int, nb: u8, m: [] Msg) {
  for i: 0..<plain.count { m[0].plain[i] = plain[i]; }
  m[0].np = plain.count;
  ck: [32] u8; sk := secret_of(from); nonce := nonce_of(nb);
  #assert n46_key(sk[..], to, ck[..]) == N46_OK;
  o := n46_seal(ck[..], nonce[..], plain, m[0].sealed[..]);
  #assert o.error == N46_OK;
  m[0].ns = o.n;
  tags: [256] u8; tn := n46_tags(to, tags[..]);
  m[0].event = sign(from, t, N46_EVENT, cast(string) tags[0..<tn], cast(string) m[0].sealed[0..<o.n], m[0].buf[..]);
}
// The client records request id and sends it with params at time t.
send :: (c: [] N46Client, method: int, id: string, params: [] string, t: int, nb: u8, m: [] Msg) -> int {
  e := n46_ask(c, method, id, t, 60);
  if e != N46_OK { return e; }
  call: N46Call; call.error = N46_OK; call.method = method; call.nparams = params.count;
  for i: 0..<id.count { call.id.b[i] = id[i]; }
  call.id.n = id.count;
  for k: 0..<params.count { call.params[k] = of(params[k]); }
  plain: [8192] u8; n := n46_request_json(call, plain[..]);
  seal(CLIENT, c[0].signer, cast(string) plain[0..<n], t, nb, m);
  return N46_OK;
}

// The remote signer reads a request event, opens it, decides, does what is
// allowed and answers at t + 1. A refusal is answered with an error.
serve :: (b: [] N46Bunker, req: NostrEvent, t: int, nb: u8, m: [] Msg) -> N46Admit {
  a: N46Admit; a.error = n46_event(req, b[0].key);
  if a.error != N46_OK { return a; }
  ck: [32] u8; sk := secret_of(SIGNER); k := n46_key(sk[..], req.public_key, ck[..]);
  opened: [8192] u8; o := n46_open(ck[..], content_of(req), opened[..]);
  call := n46_request(cast(string) opened[0..<o.n]);
  a = n46_admit(b, req.public_key, call);
  r: N46Reply; r.id = call.id; r.has_result = a.error == N46_OK; r.failed = !r.has_result;
  inner: [4096] u8; hex: [64] u8;
  if a.error != N46_OK { r.message = of(E46[a.error]); }
  else if a.method == M_CONNECT || a.method == M_LOGOUT { r.result = of("ack"); }
  else if a.method == M_PING { r.result = of("pong"); }
  else if a.method == M_PUBKEY { r.result = of(hex32(key_of(USER), hex[..])); }
  else if a.method == M_SIGN {
    // Sign the template's event with the user's key.
    own := call.params[0]; p := cast(string) own.b[0..<own.n]; tm := n46_template(p); content: [4096] u8;
    cn := json_unescape(p[tm.content_from..<tm.content_to], content[..]);
    e := sign(USER, tm.created_at, tm.kind, p[tm.tags_from..<tm.tags_to], cast(string) content[0..<cn], inner[..]);
    r.result = of(e.original);
  }
  plain: [8192] u8; n := n46_response_json(r, plain[..]);
  seal(SIGNER, req.public_key, cast(string) plain[0..<n], t + 1, nb, m);
  return a;
}

// The client reads a response event at time t: its own p tag, the
// payload opened, the reply matched to its request.
Got :: struct { take: N46Take; reply: N46Reply; }
receive :: (c: [] N46Client, res: NostrEvent, t: int, g: [] Got) {
  g[0].take.error = n46_event(res, key_of(CLIENT));
  if g[0].take.error != N46_OK { return; }
  ck: [32] u8; sk := secret_of(CLIENT); k := n46_key(sk[..], res.public_key, ck[..]);
  opened: [8192] u8; o := n46_open(ck[..], content_of(res), opened[..]);
  g[0].reply = n46_response(cast(string) opened[0..<o.n]);
  g[0].take = n46_take(c, res.public_key, g[0].reply, t);
}
STATES :: [5] string.["idle", "connecting", "connected", "ready", "closed"];
show :: (what: string, c: [] N46Client, g: [] Got) {
  print("% -> %, client %", what, E46[g[0].take.error], STATES[c[0].state]);
  if g[0].reply.failed { print(", error \""); say(g[0].reply.message); print("\""); }
  print("\n");
}

main :: () {
  hex: [64] u8; scratch: [16384] u8;
  print("== 1. The bunker token and the three keys\n");
  u := n46_uri(BUNKER);
  print("URI: %, remote signer %\n", E46[u.error], hex32(u.peer, hex[..]));
  print("  relay "); say(u.relays[0]); print("; secret: % bytes, kept and not printed\n", u.secret.n);
  print("  client key %\n  user key   % (learned by get_public_key)\n", PUB[CLIENT], PUB[USER]);
  b: [1] N46Bunker; b[0].key = key_of(SIGNER); b[0].secret = u.secret;
  b[0].offer = n46_perms("sign_event:1,sign_event:7,nip44_encrypt");
  c := [1] N46Client.[n46_client(u, T0 - 10, 60)];

  print("\n== 2. connect, then get_public_key\n");
  q: [1] Msg; s: [1] Msg; g: [1] Got;
  // connect carries the signer's key, the token's secret and the perms asked for.
  sec := u.secret; connect := [3] string.[PUB[SIGNER], cast(string) sec.b[0..<sec.n], "sign_event:1,sign_event:4"];
  e := send(c[..], M_CONNECT, "c1", connect[..], T0 - 10, 0x41, q[..]);
  // Its plaintext holds the secret, so only its shape is printed.
  print("request c1 (% bytes): connect, params [the signer's key, the secret, \"%\"]\n", q[0].np, connect[2]);
  a := serve(b[..], q[0].event, T0 - 10, 0x42, s[..]);
  print("  the signer: %, session %, granted kinds", E46[a.error], a.session);
  gr := b[0].sessions[a.session].perms;
  for k: 0..<gr.nkinds { print(" %", gr.kinds[k]); }
  print(" (it offered 1 and 7; the client asked for 1 and 4)\n");
  print("response: %\n", cast(string) s[0].plain[0..<s[0].np]);
  receive(c[..], s[0].event, T0 - 9, g[..]); show("  client", c[..], g[..]);
  none: [1] string;
  e = send(c[..], M_PUBKEY, "k1", none[0..<0], T0 - 5, 0x43, q[..]);
  a = serve(b[..], q[0].event, T0 - 5, 0x44, s[..]);
  print("response: %\n", cast(string) s[0].plain[0..<s[0].np]);
  receive(c[..], s[0].event, T0 - 4, g[..]); show("  client", c[..], g[..]);
  print("  the user's key is the client's key: %; is the signer's key: %\n", n46_same(c[0].user, key_of(CLIENT)), n46_same(c[0].user, key_of(SIGNER)));

  print("\n== 3. sign_event, by hand and by the code\n");
  // Chapter 12's contract first: the request fixes the event's id.
  t: [1] SignerTable; user := key_of(USER);
  ask := signer_ask(t[..], SIGNER_PHONE, SCOPE_EVENT, user[..], 1, T0, "[]", "hello from a bunker", T0, 60, scratch[..]);
  ok := signer_approve(t[..], ask.id, T0);
  print("chapter 12: request %, approved %, expected id %\n", ask.id, SR[ok], hex32(t[0].slots[0].expect, hex[..]));
  tm: [256] u8; tn := n46_sign_params(1, T0, "[]", "hello from a bunker", tm[..]);
  sp := [1] string.[cast(string) tm[0..<tn]];
  e = send(c[..], M_SIGN, "s1", sp[..], T0, 0x46, q[..]);
  ck: [32] u8; sk := secret_of(CLIENT); k := n46_key(sk[..], key_of(SIGNER), ck[..]);
  print("plaintext (% bytes): %\n", q[0].np, cast(string) q[0].plain[0..<q[0].np]);
  print("conversation key: %\n", hex32(ck, hex[..]));
  print("payload (% characters): %\n", q[0].ns, cast(string) q[0].sealed[0..<q[0].ns]);
  print("request event id %, tags %\n", hex32(q[0].event.id, hex[..]), q[0].event.original[q[0].event.tags_from..<q[0].event.tags_to]);
  a = serve(b[..], q[0].event, T0, 0x47, s[..]);
  print("  the signer: %, kind %\n", E46[a.error], a.kind);
  print("response (% bytes): %\n", s[0].np, cast(string) s[0].plain[0..<s[0].np]);
  print("response event id %\n", hex32(s[0].event.id, hex[..]));
  receive(c[..], s[0].event, T0 + 1, g[..]); show("  client", c[..], g[..]);
  own := g[0].reply.result; result := cast(string) own.b[0..<own.n];
  print("  chapter 12's signer_reply on the result: %, state signed %\n", SR[signer_reply(t[..], ask.id, result, T0 + 1, scratch[..])], signer_state(t[..], ask.id) == SLOT_SIGNED);

  print("\n== 4. The same client and an independent bunker\n");
  ask2 := signer_ask(t[..], SIGNER_PHONE, SCOPE_EVENT, user[..], 1, T0 + 2, "[[\"t\",\"nip46\"]]", "hello from Python", T0 + 2, 60, scratch[..]);
  ok = signer_approve(t[..], ask2.id, T0 + 2);
  tn = n46_sign_params(1, T0 + 2, "[[\"t\",\"nip46\"]]", "hello from Python", tm[..]);
  sp[0] = cast(string) tm[0..<tn];
  e = send(c[..], M_SIGN, "s2", sp[..], T0 + 2, 0x48, q[..]);
  print("request s2 sent: %\n", E46[e]);
  check: [16384] u8;
  py := nostr_event_verify_vartime(PY_BUNKER_EVENT, check[..], false);
  print("Python's response event: verified %, id %\n", py.verified, hex32(py.id, hex[..]));
  receive(c[..], py, T0 + 3, g[..]); show("  client", c[..], g[..]);
  own = g[0].reply.result;
  print("  chapter 12's signer_reply on the result: %\n", SR[signer_reply(t[..], ask2.id, cast(string) own.b[0..<own.n], T0 + 3, scratch[..])]);

  print("\n== 5. The client's own nostrconnect token\n");
  v := n46_uri(CONNECT);
  print("URI: %, client %, name ", E46[v.error], hex32(v.peer, hex[..])); say(v.name); print(", perms: % kind(s)\n", v.perms.nkinds);
  d := [1] N46Client.[n46_client(v, T0, 120)];
  print("  the signer accepts the pasted token: %\n", E46[n46_accept(b[..], v)]);
  wrong: [1] Msg; seal(SIGNER, key_of(CLIENT), "{\"id\":\"r9\",\"result\":\"ack\"}", T0 + 5, 0x49, wrong[..]);
  receive(d[..], wrong[0].event, T0 + 6, g[..]); show("  a connect response with \"ack\"", d[..], g[..]);
  // The signer echoes the token's secret: the plaintext is built from it, never printed.
  echo: N46Reply; echo.id.b[0] = 'r'; echo.id.b[1] = '1'; echo.id.n = 2; echo.has_result = true;
  for i: 0..<v.secret.n { echo.result.b[i] = v.secret.b[i]; }
  echo.result.n = v.secret.n;
  ep: [256] u8; en := n46_response_json(echo, ep[..]);
  right: [1] Msg; seal(SIGNER, key_of(CLIENT), cast(string) ep[0..<en], T0 + 7, 0x4a, right[..]);
  receive(d[..], right[0].event, T0 + 8, g[..]); show("  a connect response echoing the secret", d[..], g[..]);
  print("  the signer is the response's author: %\n", n46_same(d[0].signer, key_of(SIGNER)));

  print("\n== 6. Refusals\n");
  bad := [3] string.[
    "bunker://c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5?relay=wss%3A%2F%2Frelay.example.com",
    "bunker://c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5?secret=s3cret",
    "nostrconnect://79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798?relay=wss%3A%2F%2Frelay.example.com&secret=x"];
  why := [3] string.["a bunker URI with no secret", "a bunker URI with no relay", "an uppercase client key"];
  for i: 0..<3 { print("%: %\n", why[i], E46[n46_uri(bad[i]).error]); }
  other := n90_t_key(0x22);
  c2 := n46_request("{\"id\":\"c9\",\"method\":\"connect\",\"params\":[\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\",\"s3cret\"]}");
  print("connect again with the used secret, from another client: %\n", E46[n46_admit(b[..], other, c2).error]);
  c3 := n46_request("{\"id\":\"c9\",\"method\":\"connect\",\"params\":[\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\",\"guess\"]}");
  print("connect with a wrong secret: %\n", E46[n46_admit(b[..], other, c3).error]);
  tn = n46_sign_params(4, T0, "[]", "a direct message", tm[..]);
  sp[0] = cast(string) tm[0..<tn];
  e = send(c[..], M_SIGN, "s4", sp[..], T0 + 10, 0x4b, q[..]);
  a = serve(b[..], q[0].event, T0 + 10, 0x4c, s[..]);
  print("sign_event for kind 4: the signer says %; response %\n", E46[a.error], cast(string) s[0].plain[0..<s[0].np]);
  receive(c[..], s[0].event, T0 + 11, g[..]); show("  client", c[..], g[..]);
  print("a sign_event request from a key with no session: %\n", E46[n46_admit(b[..], other, n46_request(cast(string) q[0].plain[0..<q[0].np])).error]);
  print("the client asks nip04_encrypt: %\n", E46[n46_ask(c[..], M_NIP04_ENC, "x", T0 + 12, 60)]);
  print("a response to an id the client never sent: %\n", E46[n46_take(c[..], key_of(SIGNER), n46_response("{\"id\":\"zz\",\"result\":\"pong\"}"), T0 + 12).error]);
  e = n46_ask(c[..], M_PING, "p1", T0 + 12, 30);
  print("ping p1 at T0+12, waiting 30 s, answered by another key: %\n", E46[n46_take(c[..], key_of(USER), n46_response("{\"id\":\"p1\",\"result\":\"pong\"}"), T0 + 13).error]);
  print("  answered at T0+43: %\n", E46[n46_take(c[..], key_of(SIGNER), n46_response("{\"id\":\"p1\",\"result\":\"pong\"}"), T0 + 43).error]);
  ask3 := signer_ask(t[..], SIGNER_PHONE, SCOPE_EVENT, user[..], 1, T0, "[]", "hello from a bunker", T0 + 20, 60, scratch[..]);
  ok = signer_approve(t[..], ask3.id, T0 + 20);
  other_body: [4096] u8; ob := sign(USER, T0, 1, "[]", "hello from a bunker!", other_body[..]);
  print("a bunker that signs another content: chapter 12 says %\n", SR[signer_reply(t[..], ask3.id, ob.original, T0 + 21, scratch[..])]);
  plain: [256] u8;
  print("a NIP-04 payload: %\n", E46[n46_open(ck[..], "zooMmtdlrlTCiIbfLqTqYrpCbTWrcVkP7RdcqCuM8TQ=?iv=obLD1OX2BxgpOktcbX6PkA==", plain[..]).error]);
  forget := [1] N46Uri.[u]; n46_forget(forget[..]);
  wiped := forget[0].secret.n == 0; for i: 0..<N46_MAX_ID { if forget[0].secret.b[i] != 0 { wiped = false; } }
  print("after n46_forget: % relays, secret all zero %\n", forget[0].nrelays, wiped);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/remote-signer.coax --run-steps 250000000
```

The output, which the book's checks compare in debug and release:

```text
== 1. The bunker token and the three keys
URI: ok, remote signer c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5
  relay wss://relay.example.com; secret: 6 bytes, kept and not printed
  client key 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
  user key   f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9 (learned by get_public_key)

== 2. connect, then get_public_key
request c1 (145 bytes): connect, params [the signer's key, the secret, "sign_event:1,sign_event:4"]
  the signer: ok, session 0, granted kinds 1 (it offered 1 and 7; the client asked for 1 and 4)
response: {"id":"c1","result":"ack"}
  client -> ok, client connected
response: {"id":"k1","result":"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"}
  client -> ok, client ready
  the user's key is the client's key: false; is the signer's key: false

== 3. sign_event, by hand and by the code
chapter 12: request 1, approved ok, expected id c12bdb20e95e5fef04d90dc20dbf353d840152939c9ba32d017bfd133caac907
plaintext (133 bytes): {"id":"s1","method":"sign_event","params":["{\"kind\":1,\"created_at\":1762000000,\"tags\":[],\"content\":\"hello from a bunker\"}"]}
conversation key: c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d
payload (304 characters): AkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGPveWFooJ5gTeFF72e2bMrC0it/WhLD89S7E98cOCdJvP9W7nbfYlslmwWRiFMqT0swE0LZD9r98tftDbuo+IZSSCohUuweK7rGMAyEu9HrH+DjQo014i82RWsCfKKjhrICSSNc4qQFptHOWCR5/afRojWelhcCIYzQkZG/Qr72k5OeXgTFUQihwsTiCT9b3U3l+OQjuP0tcO6c9KFsgs1xuvhXoBsu46TrOocUDR7WsoxmPqfiP18NCWHCK53ljCZXk=
request event id 574edf34e6bf0d063cccb4f9c12fc90f7373f09af3d8ef0052daffdf3fd66513, tags [["p","c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"]]
  the signer: ok, kind 1
response (406 bytes): {"id":"s1","result":"{\"id\":\"c12bdb20e95e5fef04d90dc20dbf353d840152939c9ba32d017bfd133caac907\",\"pubkey\":\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"created_at\":1762000000,\"kind\":1,\"tags\":[],\"content\":\"hello from a bunker\",\"sig\":\"08489dfb50ed3d512a20c9feb7b543ae7c7dd9c5f63beb52d731c9e53b899cc893a1d4fd082809efeaad22be4b75f2edcd1d71d7ceff8fe47a048a2f45b03546\"}"}
response event id 4d46758978ffcdbe707d6ee2104f0ee1d36818997b8b37c5a65bd74846439f31
  client -> ok, client ready
  chapter 12's signer_reply on the result: ok, state signed true

== 4. The same client and an independent bunker
request s2 sent: ok
Python's response event: verified true, id c24cfeb25376bab1ad4223ddc91a35c6764c40c43396d2d535c513a72ee05ea2
  client -> ok, client ready
  chapter 12's signer_reply on the result: ok

== 5. The client's own nostrconnect token
URI: ok, client 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798, name Coax, perms: 1 kind(s)
  the signer accepts the pasted token: ok
  a connect response with "ack" -> the secret is not echoed, client connecting
  a connect response echoing the secret -> ok, client connected
  the signer is the response's author: true

== 6. Refusals
a bunker URI with no secret: no secret
a bunker URI with no relay: no relay
an uppercase client key: bad key
connect again with the used secret, from another client: the secret was used
connect with a wrong secret: bad secret
sign_event for kind 4: the signer says not allowed; response {"id":"s4","error":"not allowed"}
  client -> the signer answered an error, client ready, error "not allowed"
a sign_event request from a key with no session: not allowed
the client asks nip04_encrypt: NIP-04 methods are refused
a response to an id the client never sent: no request has this id
ping p1 at T0+12, waiting 30 s, answered by another key: another author
  answered at T0+43: late
a bunker that signs another content: chapter 12 says another body
a NIP-04 payload: not NIP-44 v2
after n46_forget: 0 relays, secret all zero true
```

Section 3 prints the values worked out by hand: the 133-byte plaintext,
the conversation key, the 304-character payload, the request id, the
signed event id inside the 406-byte response, and the response id. Section
4 shows the Coax client accepting the Python bunker's response, and
chapter 12 accepting the event inside it. Section 6 shows the deliberately
broken inputs and their refusals: tokens without a secret, without a
relay, and with an uppercase key; a reused connect secret; a wrong secret;
`sign_event` for kind 4, which the session doesn't allow, answered with an
error that the client reports; a request from a key with no session;
`nip04_encrypt`; a response to an unknown id, from another key, and after
the deadline; a bunker that signs another content, which chapter 12 refuses
as `another body`; and a NIP-04 payload. The secrets never appear in the
output: the `connect` request's plaintext is described, not printed.

## How the code is checked

`coax test coax/lib/nostr/nip46.coax` runs the library's checks:

- NIP-46's `nostrconnect://` example reads: two relays, the perms
  `nip44_encrypt`, `nip44_decrypt` and kinds 13, 14 and 1059, the name `My
  Client` and the secret.
- Every URI refusal has its code, with exact boundaries: a 64-byte secret
  reads, 65 bytes don't; four relays read, five don't.
- Every method's request and response, written by Python's `json.dumps`,
  reads and writes back to the same bytes.
- Request and response refusals: repeated keys, wrong types, wrong param
  counts per method, broken JSON and values over the limit.
- NIP-44's published vector (secret keys 1 and 2, nonce 1, plaintext `a`)
  through `n46_key`, `n46_seal` and `n46_open`.
- The client: states, the fixed deadline (an answer one second late is
  refused), auth challenges, errors, a full table, expiry, and the
  `nostrconnect://` echo.
- The remote signer: the one-use secret, sessions replaced and full,
  permissions by method and kind, and the intersection of offered and asked
  perms.

[checks/remote-signer.coax](../../nostr/examples/checks/remote-signer.coax)
compares the example with a Python script written from NIP-01, NIP-44,
NIP-46, BIP-340, RFC 5869 and RFC 8439, which shares no code with Coax. The
script first reproduces NIP-44's published vector. Then it computes the
message keys, the payload, the request's id and signature, the signed
user event, and the response's id and signature. It also plays an
independent bunker: it signs the user's event for request `s2` and the
response, and seals it, and the Coax client accepts that event. NIP-46's
own examples show no event ids or ciphertexts, so the values they let you
check are the `nostrconnect://` URI and the id of the event its
`sign_event` example asks for under its user key `fa984bd7…`:
`20e27c4e…c8cb75c1`.

<!-- source: nostr/examples/checks/remote-signer.coax check -->
```jai
// From a Python script written from NIP-01, NIP-44, NIP-46, BIP-340, RFC
// 5869 and RFC 8439 (it shares no code with Coax): integer secp256k1 for
// the keys, the shared x and the signatures (32 zero bytes of aux), HMAC
// and HKDF from hashlib and hmac, ChaCha20 written out from RFC 8439, the
// plaintexts as json.dumps(..., separators=(",", ":")), and each id as the
// SHA-256 of json.dumps([0, pubkey, created_at, kind, tags, content]). The
// same script reproduces NIP-44's published vector (sec1 1, sec2 2, nonce
// 1, "a") before it computes the values below.
PY_CK :: "c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d";
PY_CHACHA_KEY :: "1dc398224d0be74c85d302e3ff47f371c809cedbe558bff993c018be325dd9db";
PY_CHACHA_NONCE :: "7557c544fed256b621631bab";
PY_HMAC_KEY :: "97ab69e765d6ba1a3a3591a6a5bcfba21a4c7affbae634424d3ebe8e6879772b";
PY_PLAIN :: "{\"id\":\"s1\",\"method\":\"sign_event\",\"params\":[\"{\\\"kind\\\":1,\\\"created_at\\\":1762000000,\\\"tags\\\":[],\\\"content\\\":\\\"hello from a bunker\\\"}\"]}";
PY_PAYLOAD :: "AkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGPveWFooJ5gTeFF72e2bMrC0it/WhLD89S7E98cOCdJvP9W7nbfYlslmwWRiFMqT0swE0LZD9r98tftDbuo+IZSSCohUuweK7rGMAyEu9HrH+DjQo014i82RWsCfKKjhrICSSNc4qQFptHOWCR5/afRojWelhcCIYzQkZG/Qr72k5OeXgTFUQihwsTiCT9b3U3l+OQjuP0tcO6c9KFsgs1xuvhXoBsu46TrOocUDR7WsoxmPqfiP18NCWHCK53ljCZXk=";
PY_REQUEST_ID :: "574edf34e6bf0d063cccb4f9c12fc90f7373f09af3d8ef0052daffdf3fd66513";
PY_REQUEST_SIG :: "19d7182bc3a67f057d0857976b5c66735473df92129085abe3b6c1989e2d286460a5f2b6f639f423ec6a4ab3ae0d633beca9bb0f4f927ff55274dde12b6098b0";
PY_SIGNED :: "{\"id\":\"c12bdb20e95e5fef04d90dc20dbf353d840152939c9ba32d017bfd133caac907\",\"pubkey\":\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"created_at\":1762000000,\"kind\":1,\"tags\":[],\"content\":\"hello from a bunker\",\"sig\":\"08489dfb50ed3d512a20c9feb7b543ae7c7dd9c5f63beb52d731c9e53b899cc893a1d4fd082809efeaad22be4b75f2edcd1d71d7ceff8fe47a048a2f45b03546\"}";
PY_RESPONSE_ID :: "4d46758978ffcdbe707d6ee2104f0ee1d36818997b8b37c5a65bd74846439f31";
PY_RESPONSE_SIG :: "7d1df34d9884d82839e27446346def8e434afe79fdfebd6462a7525b7edaf6e16ab7ec6c6ddcf7824de5de8ee1a68a6b05cbfb1a788f97d425f31b9dc996f56c";
PY_BUNKER_INNER_ID :: "d654f1a5b09bbaa97507de43f9e89a41974f581a36d405c502030f9ac2672385";
// NIP-46's example flow: the event its sign_event asks for, under its
// user key fa984bd7... The NIP shows no event ids or ciphertexts, so this
// id and its nostrconnect:// URI are the values it lets you check.
PY_NIP_EXAMPLE_ID :: "20e27c4e6634733358d9ca2ae9c023506851cdcc8672794ad271f3b4c8cb75c1";
NIP_USER :: "fa984bd7dbb282f07e16e7ae87b26a2a7b9b90b7246a44771f0cf5ae58018f52";

hex_is :: (bytes: [] u8, text: string) -> bool {
  out: [128] u8; r := hex_encode_checked(cast(string) bytes, out[..], false);
  return r.error == NC_OK && cast(string) out[0..<r.written] == text;
}
// A client and a bunker past connect and get_public_key, as the example's
// sections 1 and 2 leave them.
ready :: (c: [] N46Client, b: [] N46Bunker) {
  u := n46_uri(BUNKER); c[0] = n46_client(u, T0 - 10, 60);
  b[0].key = key_of(SIGNER); b[0].secret = u.secret; b[0].offer = n46_perms("sign_event:1,sign_event:7,nip44_encrypt");
  q: [1] Msg; s: [1] Msg; g: [1] Got; sec := u.secret;
  connect := [3] string.[PUB[SIGNER], cast(string) sec.b[0..<sec.n], "sign_event:1,sign_event:4"];
  #assert send(c, M_CONNECT, "c1", connect[..], T0 - 10, 0x41, q[..]) == N46_OK;
  #assert serve(b, q[0].event, T0 - 10, 0x42, s[..]).error == N46_OK;
  receive(c, s[0].event, T0 - 9, g[..]);
  none: [1] string;
  #assert send(c, M_PUBKEY, "k1", none[0..<0], T0 - 5, 0x43, q[..]) == N46_OK;
  #assert serve(b, q[0].event, T0 - 5, 0x44, s[..]).error == N46_OK;
  receive(c, s[0].event, T0 - 4, g[..]);
  #assert c[0].state == C_READY && n46_same(c[0].user, key_of(USER));
}

#test "by hand: the conversation key, the message keys, 133 bytes padded to 160, 304 characters" #steps 400000000 {
  ck: [32] u8; one := secret_of(CLIENT); two := secret_of(SIGNER); ck2: [32] u8;
  #assert n46_key(one[..], key_of(SIGNER), ck[..]) == N46_OK && hex_is(ck[..], PY_CK);
  #assert n46_key(two[..], key_of(CLIENT), ck2[..]) == N46_OK && n46_same(ck, ck2);
  nonce := nonce_of(0x46); keys: [76] u8;
  #assert nip44_message_keys_checked(ck[..], nonce[..], keys[..], false) == NIP44_OK;
  #assert hex_is(keys[0..<32], PY_CHACHA_KEY) && hex_is(keys[32..<44], PY_CHACHA_NONCE) && hex_is(keys[44..<76], PY_HMAC_KEY);
  // 133 bytes: the next power of two is 256, so chunks of 32; 5 chunks are 160.
  // 1 + 32 + (2 + 160) + 32 = 227 raw bytes are 304 base64 characters.
  #assert PY_PLAIN.count == 133 && nip44_calc_padded_len(133) == 160 && nip44_payload_size(NIP44_COMPAT, 133) == 304;
}
#test "sign_event through the Coax bunker: request, signed event and response are Python's, byte for byte" #steps 2000000000 {
  c: [1] N46Client; b: [1] N46Bunker; ready(c[..], b[..]);
  tm: [256] u8; tn := n46_sign_params(1, T0, "[]", "hello from a bunker", tm[..]);
  sp := [1] string.[cast(string) tm[0..<tn]]; q: [1] Msg; s: [1] Msg; g: [1] Got;
  #assert send(c[..], M_SIGN, "s1", sp[..], T0, 0x46, q[..]) == N46_OK;
  #assert cast(string) q[0].plain[0..<q[0].np] == PY_PLAIN && cast(string) q[0].sealed[0..<q[0].ns] == PY_PAYLOAD;
  #assert hex_is(q[0].event.id[..], PY_REQUEST_ID) && hex_is(q[0].event.signature[..], PY_REQUEST_SIG);
  a := serve(b[..], q[0].event, T0, 0x47, s[..]);
  #assert a.error == N46_OK && a.kind == 1 && hex_is(s[0].event.id[..], PY_RESPONSE_ID) && hex_is(s[0].event.signature[..], PY_RESPONSE_SIG);
  receive(c[..], s[0].event, T0 + 1, g[..]);
  own := g[0].reply.result;
  #assert g[0].take.error == N46_OK && g[0].take.method == M_SIGN && cast(string) own.b[0..<own.n] == PY_SIGNED;
  t: [1] SignerTable; user := key_of(USER); scratch: [16384] u8;
  ask := signer_ask(t[..], SIGNER_PHONE, SCOPE_EVENT, user[..], 1, T0, "[]", "hello from a bunker", T0, 60, scratch[..]);
  #assert signer_approve(t[..], ask.id, T0) == SR_OK && signer_reply(t[..], ask.id, PY_SIGNED, T0 + 1, scratch[..]) == SR_OK;
}
#test "sign_event through an independent bunker: Python's response event is accepted" #steps 2000000000 {
  c: [1] N46Client; b: [1] N46Bunker; ready(c[..], b[..]);
  tm: [256] u8; tn := n46_sign_params(1, T0 + 2, "[[\"t\",\"nip46\"]]", "hello from Python", tm[..]);
  sp := [1] string.[cast(string) tm[0..<tn]]; q: [1] Msg; g: [1] Got;
  #assert send(c[..], M_SIGN, "s2", sp[..], T0 + 2, 0x48, q[..]) == N46_OK;
  check: [16384] u8; py := nostr_event_verify_vartime(PY_BUNKER_EVENT, check[..], false);
  #assert py.verified;
  receive(c[..], py, T0 + 3, g[..]);
  #assert g[0].take.error == N46_OK && g[0].take.method == M_SIGN;
  own := g[0].reply.result; inner := nostr_event_verify_vartime(cast(string) own.b[0..<own.n], check[..], false);
  #assert inner.verified && hex_is(inner.id[..], PY_BUNKER_INNER_ID) && n46_same(inner.public_key, key_of(USER));
  // The same response again: its request is answered, so its id is unknown.
  #assert n46_take(c[..], py.public_key, g[0].reply, T0 + 4).error == N46_UNKNOWN;
}
#test "NIP-46's own examples: the nostrconnect URI and the example template's event id" {
  u := n46_uri(N46_T_NIP_URI);
  #assert u.error == N46_OK && u.nrelays == 2 && u.perms.nkinds == 3 && n46_is(u.secret, "0s8j2djs");
  m := n46_template(N46_T_TEMPLATE); #assert m.error == N46_OK && m.kind == 1 && m.created_at == 1714078911;
  t: [1] SignerTable; nip: [32] u8; r := hex_decode_checked(NIP_USER, nip[..], false); scratch: [16384] u8;
  ask := signer_ask(t[..], SIGNER_PHONE, SCOPE_EVENT, nip[..], 1, 1714078911, "[]", "Hello, I'm signing remotely", 0, 60, scratch[..]);
  #assert ask.status == SR_OK && hex_is(t[0].slots[0].expect[..], PY_NIP_EXAMPLE_ID);
}
```

Run on 2026-11-01: `coax test coax/lib/nostr/nip46.coax` (337 checks with
the loaded files) and the checks file (462); `coax mutate --root-only` on
`nip46.coax`, which killed 1110 of 1110 mutants, none marked equivalent
(code that gave equivalent mutants was restructured); the example in debug
and release against its transcript; and the book's structure check. Not
run: `COAX_MUTATION_FULL`, mutation of the example and the checks file, a
session over a real relay, and a session with a deployed bunker such as
nsecBunker or Amber.

## Summary

- The browser (NIP-07), Android (NIP-55) and remote (NIP-46) signers offer
  the same operations; only NIP-46 is protocol work, so only it is built in
  Coax. All three hand a signed event to the same check in chapter 12.
- A NIP-46 connection starts from a `bunker://` or `nostrconnect://` URI.
  Both carry a secret here; it is kept, never printed, and works once.
- Requests and responses are small JSON objects inside NIP-44 v2 payloads
  of kind 24133 events, matched by id against a fixed deadline.
- The client learns the user key from `get_public_key`, never from the
  remote signer key.
- The bunker grants each session what its user offers and the client asked
  for, by method and kind, and refuses the NIP-04 methods.
- Not built: the NIP-07 and NIP-55 hosts beyond chapter 12's contract,
  `switch_relays`' relay list (its result is passed through), the NIP-05
  and NIP-89 discovery of bunkers, and the relay session itself.

## Dependencies

- [12. Keys and randomness](ch012-keys-and-randomness.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [31. Publish and wait for OK](ch031-publish-and-wait-for-ok.md)

## Sources

- [NIP-07](../nips/official/07.md)
- [NIP-46](../nips/official/46.md)
- [NIP-55](../nips/official/55.md)
- [NIP-44](../nips/official/44.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
