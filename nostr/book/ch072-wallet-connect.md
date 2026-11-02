# 72. Wallet Connect

NIP-47, Nostr Wallet Connect (NWC), lets an app ask a Lightning wallet to
pay an invoice, make one, or report its balance, over a Nostr relay. The
wallet gives the app a connection URI that holds a secret key made for that
app alone. The app signs and encrypts every request with that secret, and
the wallet service answers with an encrypted response. This chapter builds
the client's half and the rules both sides share: the URI, the info event,
the JSON of every request and response, NIP-44 encryption under the
connection secret, response matching and a spending budget per connection.

Part VI, Every NIP. Result: NIP-47 Nostr Wallet Connect:
`nostr+walletconnect://` URIs, info (13194), requests (23194) and responses
(23195) encrypted with NIP-44, and the commands `pay_invoice`,
`make_invoice`, `lookup_invoice`, `get_balance`, `get_info` and
`list_transactions`.

## What this chapter teaches

- The URI: the wallet service's public key, one or more `relay`
  parameters, the `secret`, and an optional `lud16`.
- Kind 13194: the commands a wallet supports and its `encryption` tag. No
  tag means NIP-04 only, which this stack refuses for new requests.
- Requests and responses: a JSON object with `method` and `params`, and
  one with `result_type`, `error` and `result`, each encrypted with NIP-44
  v2 under the conversation key of the connection secret and the wallet's
  key.
- The error codes, from `RATE_LIMITED` to `NOT_FOUND`.
- Matching: why a response's kind, author, `p` tag, `e` tag and
  `result_type` must all agree with the request.
- A spending budget per connection, decided in an `#authority` file: a
  payment over what is left in the current period is refused, and the
  wallet answers `QUOTA_EXCEEDED`.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip47.coax](../../coax/lib/nostr/nip47.coax) | `n47_uri(text)`, `n47_forget(u)` | Reads a connection URI; wipes it |
| [nip47.coax](../../coax/lib/nostr/nip47.coax) | `n47_info(e)`, `n47_encryption(info)` | Reads a wallet's info event; decides whether new requests may be sent |
| [nip47.coax](../../coax/lib/nostr/nip47.coax) | `n47_request(text)`, `n47_response(text)` | Read a request's or a response's plaintext JSON |
| [nip47.coax](../../coax/lib/nostr/nip47.coax) | `n47_request_json(c, out)`, `n47_response_json(r, out)` | Write them |
| [nip47.coax](../../coax/lib/nostr/nip47.coax) | `n47_request_tags`, `n47_response_tags`, `n47_request_event` | The events' tags, and the wallet's reading of a request event |
| [nip47.coax](../../coax/lib/nostr/nip47.coax) | `n47_key`, `n47_seal`, `n47_open` | The conversation key; NIP-44 v2 encryption and decryption |
| [nip47.coax](../../coax/lib/nostr/nip47.coax) | `n47_match(request, response, wallet, method, reply)` | Decides whether a response answers a request |
| [nip47.coax](../../coax/lib/nostr/nip47.coax) | `n47_roll`, `n47_spend`, `n47_code` | The budget: periods, the payment decision, the error code to answer with |
| [nip44-key.coax](../../coax/lib/nostr/nip44-key.coax) | `nip44_conversation_key_from_keys_checked` | ECDH and HKDF-extract (chapter 20) |
| [settle.coax](../../coax/lib/x402/settle.coax) | `x402_bolt11_msat(invoice)` | The amount in a mainnet BOLT11 invoice's human-readable part (chapter 86) |

`nip47.coax` is new with this chapter. It is an `#authority` file, because
`n47_spend` decides whether a payment goes ahead. Every procedure runs in a
check, and `coax mutate --root-only` leaves no survivors.
`coax/tests/nostr/nip47_fixtures.coax` holds the checks' test records, so
that mutation changes the library and not the checks. The BOLT11 amount
comes from chapter 86's `x402_bolt11_msat`; chapter 71's zap code was not
yet on `main` when this chapter was written.

Nothing in `nip47.coax` opens a socket, verifies a signature, checks an
invoice's signature or pays. The caller publishes and subscribes (chapter
31), verifies events (chapter 21), and its wallet pays.

## The connection URI

A wallet service makes one URI per app:

```jai
nostr+walletconnect://<wallet pubkey>?relay=<url>&secret=<hex>&lud16=<address>
```

- The path is the wallet service's public key, 64 hex digits. NIP-47 says
  it SHOULD be unique per connection.
- `relay` is required and may repeat: where the wallet listens. Its value
  is percent-encoded, so `wss%3A%2F%2Frelay.example.com` reads as
  `wss://relay.example.com`.
- `secret` is required: 32 random bytes in hex. It is the app's key for
  this connection. The app signs requests with it and derives the
  encryption key from it. It is not the user's Nostr key, so payments
  don't link to the user's identity, and the user can revoke one app
  without touching the others.
- `lud16` is optional: a Lightning address the app may put in the user's
  profile.

`n47_uri` reads the URI into an `N47Uri`. It takes only lowercase hex for
the wallet key and the secret, as Nostr does everywhere else. It refuses a
missing relay or secret, a relay that isn't `ws://` or `wss://` after
decoding, a broken `%XX` escape, a repeated `secret` or `lud16`, more than
four relays, and a secret that isn't a usable key (0, or not below the group
order). It derives the client's public key from the secret, because that is
the key the wallet will see on every request. Parameters it doesn't know
are skipped.

<!-- source: coax/lib/nostr/nip47.coax record -->
```jai
N47Str :: struct {b:[N47_MAX_STR]u8; n:int;}
// A connection: the wallet service's public key, its relays, the client's
// secret and the public key it signs with. The caller wipes it with
// n47_forget when the connection is removed.
N47Uri :: struct {
  error:int=N47_SCHEME;
  wallet:[32]u8;
  relays:[N47_MAX_RELAYS]N47Str; nrelays:int;
  secret:[32]u8; client:[32]u8;
  lud16:N47Str;
}
// The params of a request or the result of a response: have says which
// fields are present, one bit per N47_KEYS entry.
N47Fields :: struct {
  have:int;
  num:[N47_FIELDS]int;                // integers, enum indexes, the methods bits, the transactions count
  str:[3]N47Str;                      // invoice, description, alias
  hex:[2][32]u8;                      // payment_hash, preimage
}
N47Call :: struct {error:int=N47_JSON; method:int=-1; f:N47Fields;}
N47Reply :: struct {error:int=N47_JSON; method:int=-1; code:int=-1; message:N47Str; f:N47Fields;}
// What a wallet's info event offers.
N47Info :: struct {error:int=N47_KIND; methods:int; unknown:int; tagged:bool; nip44:bool; nip04:bool;}
N47Out :: struct {error:int; n:int;}
// A budget: limit millisats per period seconds from start (period 0: one
// budget that never renews), of which spent are used.
N47Budget :: struct {limit:int; period:int; start:int; spent:int;}
N47Spend :: struct {error:int; msats:int; budget:N47Budget;}
```

The URI holds a secret key. The record keeps it, nothing in `nip47.coax`
writes it anywhere, and the example never prints it. `n47_forget` zeroes
the record when the user removes the connection.

<!-- source: coax/lib/nostr/nip47.coax uri -->
```jai
// The value of one query parameter with its %XX escapes decoded into out:
// its length, or -1 for a broken escape, -2 when it doesn't fit.
n47_unescape :: (s:string,out:[]u8)->int {
  n:=0;i:=0;
  while i<s.count {
    c:=s[i];
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
// A connection URI: the scheme, the wallet's public key as 64 lowercase
// hex digits, then relay (one or more, ws:// or wss://), secret (once, 64
// lowercase hex digits, a usable secret key) and an optional lud16. Other
// parameters are skipped. The secret is kept, never written anywhere.
n47_uri :: (text:string)->N47Uri {
  u:N47Uri;p:=N47_PREFIX.count;
  if text.count<p+64 || text[0..<p]!=N47_PREFIX {return u;}
  u.error=N47_WALLET;
  if !ne_lower_hex(text[p..<p+64],u.wallet[..]) {return u;}
  if text.count>p+64 && text[p+64]!='?' {return u;}
  has_secret:=false;has_lud16:=false;from:=p+65;
  for i:from..<text.count+1 {
    if i<text.count && text[i]!='&' {continue;}
    part:=text[from..<i];from=i+1;
    eq:=0;
    while eq<part.count && part[eq]!='=' {eq+=1;}
    key:=part[0..<eq];value:=part[ifx eq<part.count then eq+1 else eq..<part.count];
    if key=="relay" {
      if u.nrelays==N47_MAX_RELAYS {u.error=N47_LIMIT;return u;}
      n:=n47_unescape(value,u.relays[u.nrelays].b[..]);
      u.error=ifx n== -1 then N47_ESCAPE else N47_LIMIT;
      if n<0 {return u;}
      u.relays[u.nrelays].n=n;u.error=N47_RELAY;
      rv:=u.relays[u.nrelays];
      if !nt_relay_ok(cast(string)rv.b[0..<n]) {return u;}
      u.nrelays+=1;
    } else if key=="secret" {
      u.error=N47_REPEAT;
      if has_secret {return u;}
      has_secret=true;u.error=N47_SECRET;
      if !ne_lower_hex(value,u.secret[..]) {return u;}
    } else if key=="lud16" {
      u.error=N47_REPEAT;
      if has_lud16 {return u;}
      has_lud16=true;
      n:=n47_unescape(value,u.lud16.b[..]);
      u.error=ifx n== -1 then N47_ESCAPE else N47_LIMIT;
      if n<0 {return u;}
      u.lud16.n=n;
    }
  }
  u.error=N47_NO_RELAY;
  if u.nrelays==0 {return u;}
  u.error=N47_NO_SECRET;
  if !has_secret {return u;}
  // The client signs with the secret: it must be a key (not 0, below n).
  pub:[33]u8;u.error=N47_SECRET;
  if !bip32_public(u.secret[..],pub[..]) {return u;}
  for k:0..<32 {u.client[k]=pub[k+1];}
  u.error=N47_OK;
  return u;
}
```

## The info event and the encryption choice

The wallet service publishes a replaceable kind 13194 event on its relays.
Its content lists the commands it supports, separated by spaces. Its
`encryption` tag lists the encryption schemes it accepts: `nip44_v2`,
`nip04`, or both.

NIP-47 started on NIP-04 and moved to NIP-44. Its compatibility rule says
that no `encryption` tag means the wallet supports only NIP-04. This book
reads NIP-04 only to migrate old messages (chapter 19), because NIP-04 has
no authentication and leaks the message length. So `n47_encryption`
refuses a wallet whose info event has no tag, or a tag without
`nip44_v2`, with `N47_ENCRYPTION`. The app tells the user that the wallet
must be updated, and sends nothing.

<!-- source: coax/lib/nostr/nip47.coax info -->
```jai
// The wallet's info event: the commands its content lists (space
// separated; unknown ones counted) and the encryption tag's schemes.
n47_info :: (e:NostrEvent)->N47Info {
  r:N47Info;
  if e.error!=NE_OK || e.kind!=N47_INFO {return r;}
  r.error=N47_LIMIT;
  content:[1024]u8;n:=json_unescape(e.original[e.content_from..<e.content_to],content[..]);
  if n<0 {return r;}
  text:=cast(string)content[0..<n];from:=0;
  for i:from..<n+1 {
    if i<n && content[i]!=' ' {continue;}
    w:=text[from..<i];
    if i>from {
      k:=n47_word(w,T_METHODS);
      if k<0 {r.unknown+=1;} else {r.methods|=1<<k;}
    }
    from=i+1;
  }
  for t:0..<e.tags {
    tag:=n32_tag(e,t);r.error=N47_JSON;
    if tag.error!=NT_OK {return r;}
    if n32_is(tag.v[0],"encryption") {
      r.error=N47_REPEAT;
      if r.tagged {return r;}
      r.tagged=true;v:=tag.v[1];vt:=cast(string)v.b[0..<v.n];at:=0;
      for j:at..<v.n+1 {
        if j<v.n && v.b[j]!=' ' {continue;}
        w:=vt[at..<j];at=j+1;
        if w=="nip44_v2" {r.nip44=true;}
        if w=="nip04" {r.nip04=true;}
      }
    }
  }
  r.error=N47_OK;
  return r;
}
// Whether the client may send new requests: only over NIP-44 v2. No
// encryption tag means NIP-04 only, which this stack doesn't write.
n47_encryption :: (r:N47Info)->int {
  if r.error!=N47_OK {return r.error;}
  return ifx r.nip44 then N47_OK else N47_ENCRYPTION;
}
```

## Requests and responses

The content of a request (kind 23194) is a NIP-44 payload. Inside is a JSON
object:

```jai
{"method": "pay_invoice", "params": {"invoice": "lnbc50n1..."}}
```

The content of a response (kind 23195) is likewise a payload over:

```jai
{"result_type": "pay_invoice", "error": null, "result": {"preimage": "0123..."}}
```

`result_type` must name the request's method. On success `error` is null;
on failure `error` holds a `code` and a human-readable `message`, and
`result` is null.

The commands and their fields, from the pinned NIP-47:

| Command | Params (required in bold) | Result (required in bold) |
| --- | --- | --- |
| `pay_invoice` | **`invoice`**, `amount` | **`preimage`**, `fees_paid` |
| `make_invoice` | **`amount`**, `description`, `expiry` | **`type`**, `state`, `invoice`, `description`, **`payment_hash`**, **`amount`**, `fees_paid`, **`created_at`**, `expires_at` |
| `lookup_invoice` | `payment_hash` or `invoice` (one of them) | as `make_invoice`, plus `preimage` and `settled_at` |
| `get_balance` | none | **`balance`** |
| `get_info` | none | `alias`, `network`, **`methods`** |
| `list_transactions` | `limit` | **`transactions`** |

Amounts are millisats. `list_transactions` is not in the pinned NIP-47
text: the NIP now keeps a small core and moves other commands to the NWC
repository's extensions. The earlier NIP-47 text defined it, and wallets
still offer it, so this file reads and writes its `limit` and counts the
objects in its `transactions` list. `multi_pay_invoice`, `pay_keysend` and
the notification events (kinds 23196 and 23197) are also extensions now,
and are not built here. An info event that lists them counts them as
unknown commands.

`n47_fields` reads a `params` or `result` object into one record,
`N47Fields`, with a bit per field in `have`. It reads each value by the
field's kind: a string, a non-negative integer, 64 lowercase hex digits,
one word of a fixed list, a list of command names, or a list of objects.
Keys it doesn't know (such as `metadata`) are skipped, and `null` counts as
absent. Then `n47_allowed` checks the fields against the command's two
masks: a field the command doesn't take is `N47_FIELD`, a required field
that is missing is `N47_MISSING`.

<!-- source: coax/lib/nostr/nip47.coax names -->
```jai
// The commands this file reads and writes. The pinned NIP-47 defines the
// first five; list_transactions comes from the earlier NIP-47 text, now an
// NWC extension.
N47_PAY :: 0; N47_MAKE :: 1; N47_LOOKUP :: 2; N47_BALANCE :: 3; N47_GET_INFO :: 4; N47_LIST :: 5;
N47_METHODS :: [6] string.["pay_invoice", "make_invoice", "lookup_invoice", "get_balance", "get_info", "list_transactions"];
// The error codes of a response.
N47_CODES :: [11] string.["RATE_LIMITED", "NOT_IMPLEMENTED", "INSUFFICIENT_BALANCE", "QUOTA_EXCEEDED",
  "RESTRICTED", "UNAUTHORIZED", "INTERNAL", "UNSUPPORTED_ENCRYPTION", "OTHER", "PAYMENT_FAILED", "NOT_FOUND"];
N47_C_NOT_IMPLEMENTED :: 1; N47_C_QUOTA :: 3; N47_C_UNSUPPORTED :: 7; N47_C_OTHER :: 8;
// The fields of params and result objects, in the order the writers put
// them, each with how its value reads and where it is kept.
N47_FIELDS :: 18;
N47_KEYS :: [N47_FIELDS] string.["invoice", "amount", "description", "payment_hash", "expiry", "limit",
  "type", "state", "preimage", "fees_paid", "balance", "created_at", "expires_at", "settled_at",
  "alias", "network", "methods", "transactions"];
K_STR :: 0; K_INT :: 1; K_HEX :: 2; K_TYPE :: 3; K_STATE :: 4; K_NETWORK :: 5; K_METHODS :: 6; K_LIST :: 7;
N47_KIND_OF :: [N47_FIELDS] int.[K_STR, K_INT, K_STR, K_HEX, K_INT, K_INT, K_TYPE, K_STATE, K_HEX,
  K_INT, K_INT, K_INT, K_INT, K_INT, K_STR, K_NETWORK, K_METHODS, K_LIST];
N47_SLOT :: [N47_FIELDS] int.[0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 2, 0, 0, 0];
N47_TYPES :: [2] string.["incoming", "outgoing"];
N47_STATES :: [5] string.["pending", "settled", "accepted", "expired", "failed"];
N47_NETWORKS :: [4] string.["mainnet", "testnet", "signet", "regtest"];
// Field bits: F_INVOICE is 1 << 0, and so on in N47_KEYS order.
F_INVOICE :: 1; F_AMOUNT :: 2; F_DESCRIPTION :: 4; F_HASH :: 8; F_EXPIRY :: 16; F_LIMIT :: 32;
F_TYPE :: 64; F_STATE :: 128; F_PREIMAGE :: 256; F_FEES :: 512; F_BALANCE :: 1024; F_CREATED :: 2048;
F_EXPIRES :: 4096; F_SETTLED :: 8192; F_ALIAS :: 16384; F_NETWORK :: 32768; F_LIST_METHODS :: 65536;
F_TRANSACTIONS :: 131072;
// Per command: the params it may carry and must carry, and the same for
// its result. lookup_invoice needs one of invoice and payment_hash.
N47_TX :: F_TYPE | F_STATE | F_INVOICE | F_DESCRIPTION | F_HASH | F_AMOUNT | F_FEES | F_CREATED | F_EXPIRES;
N47_TX_NEEDS :: F_TYPE | F_HASH | F_AMOUNT | F_CREATED;
N47_PARAMS :: [6] int.[F_INVOICE | F_AMOUNT, F_AMOUNT | F_DESCRIPTION | F_EXPIRY, F_INVOICE | F_HASH, 0, 0, F_LIMIT];
N47_PARAMS_NEED :: [6] int.[F_INVOICE, F_AMOUNT, 0, 0, 0, 0];
N47_RESULTS :: [6] int.[F_PREIMAGE | F_FEES, N47_TX, N47_TX | F_PREIMAGE | F_SETTLED, F_BALANCE,
  F_ALIAS | F_NETWORK | F_LIST_METHODS, F_TRANSACTIONS];
N47_RESULTS_NEED :: [6] int.[F_PREIMAGE, N47_TX_NEEDS, N47_TX_NEEDS, F_BALANCE, F_LIST_METHODS, F_TRANSACTIONS];
```

<!-- source: coax/lib/nostr/nip47.coax fields -->
```jai
// One params or result object, its { already read: every field this file
// knows, read by its kind; other keys skipped; null as absent.
n47_fields :: (j:[]Json,f:[]N47Fields)->int {
  t:=json_next(j);
  while t==JSON_STRING {
    k:=n47_word(json_text(j),T_KEYS);
    v:=json_next(j);
    if k<0 || v==JSON_NULL {
      if !json_skip(j) {return N47_JSON;}
    } else {
      bit:=1<<k;
      if (f[0].have&bit)!=0 {return N47_REPEAT;}
      f[0].have|=bit;
      kind:=N47_KIND_OF[k];slot:=N47_SLOT[k];raw:=json_text(j);
      if kind==K_STR {
        if v!=JSON_STRING {return N47_FIELD;}
        n:=json_unescape(raw,f[0].str[slot].b[..]);
        if n<0 {return N47_LIMIT;}
        f[0].str[slot].n=n;
      } else if kind==K_INT {
        x:=json_int(raw);
        if v!=JSON_NUMBER || !x.ok || x.value<0 {return N47_FIELD;}
        f[0].num[k]=x.value;
      } else if kind==K_HEX {
        if v!=JSON_STRING || !ne_lower_hex(raw,f[0].hex[slot][..]) {return N47_FIELD;}
      } else if kind==K_METHODS || kind==K_LIST {
        if v!=JSON_ARRAY {return N47_FIELD;}
        u:=json_next(j);
        while u!=JSON_ARRAY_END {
          if kind==K_METHODS {
            if u!=JSON_STRING {return N47_FIELD;}
            m:=n47_word(json_text(j),T_METHODS);
            if m>=0 {f[0].num[k]|=1<<m;}
          } else {
            if u!=JSON_OBJECT || !json_skip(j) {return N47_FIELD;}
            f[0].num[k]+=1;
          }
          u=json_next(j);
        }
      } else {
        w:=n47_word(raw,kind);
        if v!=JSON_STRING || w<0 {return N47_FIELD;}
        f[0].num[k]=w;
      }
    }
    t=json_next(j);
  }
  return ifx t==JSON_OBJECT_END then N47_OK else N47_JSON;
}
// The fields against what a command allows and needs.
n47_allowed :: (have:int,allowed:int,needed:int)->int {
  if (have&allowed)!=have {return N47_FIELD;}
  if (have&needed)!=needed {return N47_MISSING;}
  return N47_OK;
}
n47_params_ok :: (c:N47Call)->int {
  if c.method<0 {return N47_MISSING;}
  e:=n47_allowed(c.f.have,N47_PARAMS[c.method],N47_PARAMS_NEED[c.method]);
  if e==N47_OK && c.method==N47_LOOKUP && (c.f.have&(F_INVOICE|F_HASH))==0 {return N47_MISSING;}
  return e;
}
n47_reply_ok :: (r:N47Reply)->int {
  if r.method<0 {return N47_MISSING;}
  if r.code>=0 {return ifx r.f.have==0 then N47_OK else N47_FIELD;}
  return n47_allowed(r.f.have,N47_RESULTS[r.method],N47_RESULTS_NEED[r.method]);
}
```

`n47_request` and `n47_response` read the outer object in any key order.
A repeated `method`, `params`, `result_type`, `error` or `result` is
refused, not resolved. An error code that NIP-47 doesn't list reads as
`OTHER`, so a newer wallet's code still reads as a failure.

<!-- source: coax/lib/nostr/nip47.coax read -->
```jai
// A request's plaintext: {"method": ..., "params": {...}}.
n47_request :: (text:string)->N47Call {
  c:N47Call;j:[1]Json;f:[1]N47Fields;json_start(j[..],text);params:=false;
  if json_next(j[..])!=JSON_OBJECT {return c;}
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);
    if json_is(key,"method") {
      c.error=N47_REPEAT;
      if c.method>=0 {return c;}
      c.error=N47_METHOD;c.method=n47_word(json_text(j[..]),T_METHODS);
      if v!=JSON_STRING || c.method<0 {return c;}
    } else if json_is(key,"params") {
      c.error=N47_REPEAT;
      if params {return c;}
      params=true;c.error=N47_FIELD;
      if v!=JSON_OBJECT {return c;}
      c.error=n47_fields(j[..],f[..]);
      if c.error!=N47_OK {return c;}
    } else if !json_skip(j[..]) {c.error=N47_JSON;return c;}
    t=json_next(j[..]);
  }
  c.error=N47_JSON;
  if t!=JSON_OBJECT_END || json_next(j[..])!=JSON_END {return c;}
  c.f=f[0];c.error=N47_MISSING;
  if !params {return c;}
  c.error=n47_params_ok(c);
  return c;
}
// A response's plaintext: {"result_type": ..., "error": null or {"code",
// "message"}, "result": {...} or null}. Exactly one of error and result.
n47_response :: (text:string)->N47Reply {
  r:N47Reply;j:[1]Json;f:[1]N47Fields;json_start(j[..],text);result:=false;failed:=false;
  if json_next(j[..])!=JSON_OBJECT {return r;}
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);
    if json_is(key,"result_type") {
      r.error=N47_REPEAT;
      if r.method>=0 {return r;}
      r.error=N47_METHOD;r.method=n47_word(json_text(j[..]),T_METHODS);
      if v!=JSON_STRING || r.method<0 {return r;}
    } else if json_is(key,"error") && v!=JSON_NULL {
      r.error=N47_FIELD;
      if v!=JSON_OBJECT || failed {return r;}
      failed=true;message:=false;
      u:=json_next(j[..]);
      while u==JSON_STRING {
        name:=json_text(j[..]);w:=json_next(j[..]);raw:=json_text(j[..]);
        if json_is(name,"code") {
          if w!=JSON_STRING {return r;}
          r.code=n47_word(raw,T_CODES);
          if r.code<0 {r.code=N47_C_OTHER;}
        } else if json_is(name,"message") {
          if w!=JSON_STRING {return r;}
          r.message.n=json_unescape(raw,r.message.b[..]);message=true;
          if r.message.n<0 {r.message.n=0;r.error=N47_LIMIT;return r;}
        } else if !json_skip(j[..]) {return r;}
        u=json_next(j[..]);
      }
      r.error=N47_MISSING;
      if u!=JSON_OBJECT_END || r.code<0 || !message {return r;}
    } else if json_is(key,"result") && v!=JSON_NULL {
      r.error=N47_FIELD;
      if v!=JSON_OBJECT || result {return r;}
      result=true;
      r.error=n47_fields(j[..],f[..]);
      if r.error!=N47_OK {return r;}
    } else if !json_skip(j[..]) {r.error=N47_JSON;return r;}
    t=json_next(j[..]);
  }
  r.error=N47_JSON;
  if t!=JSON_OBJECT_END || json_next(j[..])!=JSON_END {return r;}
  r.f=f[0];
  if result==failed {r.error=ifx result then N47_FIELD else N47_MISSING;return r;}
  r.error=n47_reply_ok(r);
  return r;
}
```

The writers produce the fields in `N47_KEYS` order with no spaces, and
refuse a record that the readers would refuse. The same bytes come from
Python's `json.dumps(..., separators=(",", ":"))` with keys in that order,
which is how the checks compare this code with an independent client.

<!-- source: coax/lib/nostr/nip47.coax write -->
```jai
// The present fields as a JSON object, in N47_KEYS order.
n47_write_fields :: (f:N47Fields,out:[]u8,at:int)->int {
  n:=json_raw(out,at,"{");first:=true;rest:=f.have;k:=-1;
  while rest!=0 {
    k+=1;bit:=rest&1;rest=rest>>1;
    if bit!=0 {
      if !first {n=json_raw(out,n,",");}
      first=false;
      n=json_write_string(out,n,N47_KEYS[k]);n=json_raw(out,n,":");
      kind:=N47_KIND_OF[k];slot:=N47_SLOT[k];x:=f.num[k];
      sv:=f.str[slot];
      if kind==K_STR {n=json_write_string(out,n,cast(string)sv.b[0..<sv.n]);}
      else if kind==K_INT {n=json_write_int(out,n,x);}
      else if kind==K_HEX {
        hex:[64]u8;own:=f.hex[slot];h:=hex_encode_checked(cast(string)own[..],hex[..],false);
        n=json_raw(out,n,"\"");n=json_raw(out,n,cast(string)hex[..]);n=json_raw(out,n,"\"");
      }
      else if kind==K_METHODS {
        n=json_raw(out,n,"[");more:=false;
        for m:0..<N47_METHODS.count {
          if (x&(1<<m))!=0 {
            if more {n=json_raw(out,n,",");}
            more=true;n=json_write_string(out,n,N47_METHODS[m]);
          }
        }
        n=json_raw(out,n,"]");
      }
      else if kind==K_LIST {n=ifx x==0 then json_raw(out,n,"[]") else -1;}   // only an empty list is written
      else {n=json_write_string(out,n,n47_name(kind,x));}
    }
  }
  return json_raw(out,n,"}");
}
// A request's plaintext, or -1 when the call isn't one n47_request would
// accept or out is too small.
n47_request_json :: (c:N47Call,out:[]u8)->int {
  if n47_params_ok(c)!=N47_OK {return -1;}
  n:=json_raw(out,0,"{\"method\":");n=json_write_string(out,n,N47_METHODS[c.method]);
  n=json_raw(out,n,",\"params\":");n=n47_write_fields(c.f,out,n);
  return json_raw(out,n,"}");
}
// A response's plaintext, or -1 likewise.
n47_response_json :: (r:N47Reply,out:[]u8)->int {
  if n47_reply_ok(r)!=N47_OK {return -1;}
  n:=json_raw(out,0,"{\"result_type\":");n=json_write_string(out,n,N47_METHODS[r.method]);
  if r.code>=0 {
    n=json_raw(out,n,",\"error\":{\"code\":");n=json_write_string(out,n,N47_CODES[r.code]);
    m:=r.message;
    n=json_raw(out,n,",\"message\":");n=json_write_string(out,n,cast(string)m.b[0..<m.n]);
    return json_raw(out,n,"},\"result\":null}");
  }
  n=json_raw(out,n,",\"error\":null,\"result\":");n=n47_write_fields(r.f,out,n);
  return json_raw(out,n,"}");
}
```

## The events and their encryption

A request event carries an `encryption` tag naming the scheme it uses and
a `p` tag naming the wallet. It may carry an `expiration`. A response
carries a `p` tag naming the client and an `e` tag naming the request.
`n47_request_event` is the wallet service's reading of a request: kind
23194, `nip44_v2`, its own key in `p`, and an expiration not yet passed.
Without an `encryption` tag the request is NIP-04, and the service answers
`UNSUPPORTED_ENCRYPTION`.

<!-- source: coax/lib/nostr/nip47.coax events -->
```jai
// A request's tags: the encryption it uses, the wallet, and an optional
// expiration (0: none).
n47_request_tags :: (wallet:[32]u8,expiration:int,out:[]u8)->int {
  hex:[64]u8;own:=wallet;h:=hex_encode_checked(cast(string)own[..],hex[..],false);
  n:=json_raw(out,0,"[[\"encryption\",\"nip44_v2\"],[\"p\",\"");n=json_raw(out,n,cast(string)hex[..]);
  n=json_raw(out,n,"\"]");
  if expiration>0 {n=json_raw(out,n,",[\"expiration\",\"");n=json_write_int(out,n,expiration);n=json_raw(out,n,"\"]");}
  return json_raw(out,n,"]");
}
// A response's tags: the client that asked, and the request.
n47_response_tags :: (request:NostrEvent,out:[]u8)->int {
  p:[64]u8;e:[64]u8;
  a:=request.public_key;b:=request.id;
  x:=hex_encode_checked(cast(string)a[..],p[..],false);y:=hex_encode_checked(cast(string)b[..],e[..],false);
  n:=json_raw(out,0,"[[\"p\",\"");n=json_raw(out,n,cast(string)p[..]);
  n=json_raw(out,n,"\"],[\"e\",\"");n=json_raw(out,n,cast(string)e[..]);
  return json_raw(out,n,"\"]]");
}
// The wallet service's reading of a request event at time now: kind
// 23194, encryption nip44_v2 (no tag means NIP-04: refused), a p tag naming
// this wallet, and an expiration not passed.
n47_request_event :: (e:NostrEvent,wallet:[32]u8,now:int)->int {
  if e.error!=NE_OK || e.kind!=N47_REQUEST {return N47_KIND;}
  nip44:=false;mine:=false;
  for i:0..<e.tags {
    t:=n32_tag(e,i);
    if t.error!=NT_OK {return N47_JSON;}
    if n32_is(t.v[0],"encryption") {nip44=n32_is(t.v[1],"nip44_v2");}
    if n32_is(t.v[0],"p") {k:=nostr_tag(e,i);mine=k.error==NT_OK && n47_same(k.pubkey,wallet);}
    if n32_is(t.v[0],"expiration") {
      v:=t.v[1];x:=json_int(cast(string)v.b[0..<v.n]);
      if !x.ok || x.value<0 {return N47_FIELD;}
      if now>x.value {return N47_EXPIRED;}
    }
  }
  if !nip44 {return N47_ENCRYPTION;}
  return ifx mine then N47_OK else N47_P;
}
```

Both sides derive the same conversation key (chapter 20). The client uses
the URI's secret and the wallet's public key. The wallet uses its own
secret and the client's public key, which it reads from the request's
author. `n47_open` refuses a payload with NIP-04's `?iv=` suffix before
it tries anything, and NIP-44 checks the MAC before it decrypts.

<!-- source: coax/lib/nostr/nip47.coax crypt -->
```jai
// The conversation key of this side's secret and the other side's public
// key: the client uses the URI's secret and wallet, the wallet its own
// secret and the client's key.
n47_key :: (secret:[]u8,peer:[32]u8,out:[]u8)->int {
  work:[1]EcdhScratch;pub:=peer;
  r:=nip44_conversation_key_from_keys_checked(secret,pub[..],work[..],out,false);
  return ifx r==NIP44_OK then N47_OK else N47_KEY;
}
// plain encrypted as NIP-44 v2 into out with the caller's fresh nonce.
n47_seal :: (key:[]u8,nonce:[]u8,plain:string,out:[]u8)->N47Out {
  o:N47Out;o.error=N47_LIMIT;
  if plain.count>N47_MAX_PLAIN {return o;}
  work:[N47_WORK]u8;bytes:[N47_MAX_PLAIN]u8;
  for i:0..<plain.count {bytes[i]=plain[i];}
  r:=nip44_encrypt_checked(NIP44_COMPAT,key,nonce,bytes[0..<plain.count],work[..],out,false);
  o.error=ifx r.error==NIP44_OK then N47_OK else N47_KEY;o.n=r.written;
  return o;
}
// A response's or request's content opened into out. A NIP-04 payload
// (its ?iv= suffix) is refused; NIP-44's MAC is checked before decryption.
n47_open :: (key:[]u8,payload:string,out:[]u8)->N47Out {
  o:N47Out;o.error=N47_ENCRYPTION;
  for c:payload {if c=='?' {return o;}}
  work:[N47_WORK]u8;
  r:=nip44_decrypt_checked(NIP44_COMPAT,key,payload,work[..],out,false);
  o.error=ifx r.error==NIP44_OK then N47_OK else N47_CIPHER;o.n=r.written;
  return o;
}
```

## Matching a response to its request

Responses are ephemeral events on a relay that anyone can write to. A
client that has several requests in flight must not take one request's
answer for another's, and must not take an answer from anyone but the
wallet. `n47_match` checks five things:

1. The kind is 23195.
2. The author is the wallet service's key from the URI.
3. The `p` tag names the request's author, the client.
4. The `e` tag names the request's id.
5. The opened plaintext reads, and its `result_type` is the method the
   client sent.

<!-- source: coax/lib/nostr/nip47.coax match -->
```jai
// Whether a response event and its opened plaintext answer a request the
// client sent with method: kind 23195, written by the wallet, p naming the
// request's author, e naming the request, result_type the request's method.
n47_match :: (request:NostrEvent,response:NostrEvent,wallet:[32]u8,method:int,reply:N47Reply)->int {
  if response.error!=NE_OK || response.kind!=N47_RESPONSE {return N47_KIND;}
  if !n47_same(response.public_key,wallet) {return N47_AUTHOR;}
  p:=false;e:=false;
  for i:0..<response.tags {
    t:=nostr_tag(response,i);
    if t.error==NT_OK && t.type==NT_P {p=n47_same(t.pubkey,request.public_key);}
    if t.error==NT_OK && t.type==NT_E {e=n47_same(t.id,request.id);}
  }
  if !p {return N47_P;}
  if !e {return N47_E;}
  if reply.error!=N47_OK {return reply.error;}
  return ifx reply.method==method then N47_OK else N47_RESULT;
}
```

## The spending budget

NIP-47 gives each connection its own key so that a wallet can attach
limits to it, such as a budget. The NIP leaves the limits to the wallet,
and names the error a refused payment gets: `QUOTA_EXCEEDED`. This file
decides one kind of limit: at most `limit` millisats per `period` seconds,
counted from `start`. A period of 0 is a single budget that never renews.
The client can keep the same budget to refuse a payment before it sends
it, and the wallet service keeps it to refuse a request it receives.

`n47_spend` takes the budget, the time now from the caller's clock, and the
request:

1. Only `pay_invoice` spends. Every other command costs 0.
2. The amount is the invoice's own amount, read from its human-readable
   part by `x402_bolt11_msat`. That reader takes only Bitcoin mainnet
   invoices (`lnbc`). An invoice without an amount needs the request's
   `amount`. When both are given they must agree (`N47_AMOUNTS`). No
   amount, or 0, is `N47_AMOUNT`.
3. A time before the budget's start is refused (`N47_CLOCK`): the clock
   is wrong, or the budget was written for a later day.
4. `n47_roll` moves the start forward by whole periods and sets `spent` to
   0 when a period has passed.
5. If the amount is over `limit - spent`, the payment is refused
   (`N47_QUOTA`) and `spent` doesn't change. Otherwise `spent` grows by the
   amount.

The comparison `msats > limit - spent` can't overflow, because `spent`
never exceeds `limit`. A refused request is answered with the code
`n47_code` gives: `QUOTA_EXCEEDED` for the budget, `UNSUPPORTED_ENCRYPTION`
for NIP-04, and `NOT_IMPLEMENTED` for an unknown command. The budget
reserves the amount when the request is accepted. If the payment then
fails, the amount stays counted until the period ends; giving it back is
not built.

<!-- source: coax/lib/nostr/nip47.coax budget -->
```jai
// The budget at time now: whole periods since start roll it over, so
// spent starts again at 0 at each period's start.
n47_roll :: (b:N47Budget,now:int)->N47Budget {
  r:=b;
  if b.period>0 && now-b.start>=b.period {r.start=b.start+(now-b.start)/b.period*b.period;r.spent=0;}
  return r;
}
// What a call costs and the budget after it, at time now. Only pay_invoice
// spends: the invoice's own amount, or the params amount for an invoice
// without one (both given must agree). A payment over what is left in this
// period is refused (the wallet answers QUOTA_EXCEEDED); so is a time
// before the budget's start. A refused call leaves spent as it was.
n47_spend :: (b:N47Budget,now:int,c:N47Call)->N47Spend {
  s:N47Spend;s.budget=b;
  if c.error!=N47_OK || c.method!=N47_PAY {s.error=c.error;return s;}
  iv:=c.f.str[0];invoice:=x402_bolt11_msat(cast(string)iv.b[0..<iv.n]);
  s.msats=invoice;
  if (c.f.have&F_AMOUNT)!=0 {
    s.msats=c.f.num[1];
    if invoice!= -1 && invoice!=s.msats {s.error=N47_AMOUNTS;return s;}
  }
  if s.msats<=0 {s.error=N47_AMOUNT;return s;}
  if now<b.start {s.error=N47_CLOCK;return s;}
  r:=n47_roll(b,now);s.budget=r;
  if s.msats>r.limit-r.spent {s.error=N47_QUOTA;return s;}
  s.budget.spent=r.spent+s.msats;
  return s;
}
// The response code a refusal is answered with.
n47_code :: (error:int)->int {
  if error==N47_QUOTA {return N47_C_QUOTA;}
  if error==N47_ENCRYPTION {return N47_C_UNSUPPORTED;}
  if error==N47_METHOD {return N47_C_NOT_IMPLEMENTED;}
  return N47_C_OTHER;
}
```

## The worked example, by hand

The example's connection URI is:

```jai
nostr+walletconnect://c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5?relay=wss%3A%2F%2Frelay.example.com&secret=<secret key 1>&lud16=alice%40example.com
```

The wallet's key is the public key of secret key 2, and the connection
secret is secret key 1. These are the published test keys of NIP-44's
first vector. Never use them for money.

1. **Read the URI.** The path is 64 lowercase hex digits: the wallet key
   `c6047f94…709ee5`. `relay` decodes `%3A` to `:` and `%2F` to `/`, giving
   `wss://relay.example.com`. `lud16` decodes `%40` to `@`. The secret is
   1, so the client's public key is 1·G, whose x is
   `79be667e…16f81798`.
2. **Write the plaintext.** The client pays `lnbc50n1pexample`, a
   placeholder invoice. Its human-readable part says `50n`: 50 × 10⁻⁹
   bitcoin, which is 50 × 100 = 5,000 millisats. No `amount` is needed.
   The plaintext is 64 bytes:
   `{"method":"pay_invoice","params":{"invoice":"lnbc50n1pexample"}}`.
3. **Derive the conversation key.** ECDH multiplies the wallet's point by
   the secret. The secret is 1, so the shared point is the wallet's point
   itself, and the shared x is the wallet key `c6047f94…709ee5`. The
   conversation key is HKDF-extract with salt `nip44-v2`, which is
   HMAC-SHA256(key = `nip44-v2`, message = shared x):
   `c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d`.
   This is NIP-44's published conversation key for this pair of keys.
4. **Derive the message keys.** With the nonce of 32 bytes 0x47 (a fixed
   nonce for the example only; a client takes 32 fresh random bytes),
   HKDF-expand gives 76 bytes: the ChaCha20 key `8afb2405…c1a954ee`, the
   ChaCha20 nonce `727ca6a8114159322c8d921e` and the HMAC key
   `8f626fd5…6ff80d43`.
5. **Pad and encrypt.** 64 bytes pad to 64 (`calc_padded_len(64)` is 64),
   with a 2-byte length prefix: 66 bytes of ChaCha20 ciphertext. The
   payload is the version byte, the nonce, the ciphertext and the 32-byte
   MAC: 1 + 32 + 66 + 32 = 131 bytes, which is 176 base64 characters
   starting `AkdHR0dH`. `0x02` and the first 0x47 bytes read as `AkdH`.
6. **Sign the event.** Kind 23194 at T0 = 1762000000 with tags
   `[["encryption","nip44_v2"],["p","c6047f94…709ee5"]]` and the payload
   as content, signed with secret key 1: id
   `ac198124f08cbe128886dd7f53bf4a50959ab730b0a67216acc74b43359f83f5`.

On the wallet's side, the budget is 12,000 millisats a day from T0. The
trace of four payments of 5,000 millisats:

| Time | Spent before | Left | Decision | Spent after |
| --- | --- | --- | --- | --- |
| T0 | 0 | 12,000 | pay | 5,000 |
| T0 + 100 | 5,000 | 7,000 | pay | 10,000 |
| T0 + 200 | 10,000 | 2,000 | 5,000 > 2,000: `QUOTA_EXCEEDED` | 10,000 |
| T0 + 86,400 | a new day: 0 | 12,000 | pay | 5,000 |

## The example

The example, [wallet-connect.coax](../../nostr/examples/wallet-connect.coax),
plays both sides. It reads the URI, reads the wallet's info
event, builds and signs the encrypted `pay_invoice` request, lets the
wallet service open it, check the budget and answer, and lets the client
open and match the answer. Then it runs the budget trace and the
refusals.

<!-- source: nostr/examples/wallet-connect.coax example -->
```jai
#load "../../coax/lib/nostr/nip47.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

E47 :: [27] string.["ok", "not a nostr+walletconnect:// URI", "bad wallet key", "no relay", "bad relay",
  "no secret", "bad secret", "bad escape", "repeated", "limit", "kind", "not NIP-44 v2", "not JSON", "unknown method",
  "bad field", "missing field", "p names another key", "e names another request", "another author",
  "result_type differs", "expired", "bad key", "can't decrypt", "no amount", "amounts differ", "over the budget",
  "before the budget starts"];
// Secret keys 1 (the client's connection secret) and 2 (the wallet
// service's key): published test values. Never put a real secret in an
// example, and never print one.
CLIENT :: 1; WALLET :: 2;
PUB :: [3] string.["",
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"];
URI :: "nostr+walletconnect://c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5?relay=wss%3A%2F%2Frelay.example.com&secret=0000000000000000000000000000000000000000000000000000000000000001&lud16=alice%40example.com";
T0 :: 1762000000;
DAY :: 86400;
// An invoice's human-readable part with a placeholder data part: 50n is
// 50 nano-bitcoin. Not a payable invoice; nothing here pays.
INVOICE :: "lnbc50n1pexample";

key_of :: (who: int) -> [32] u8 { k: [32] u8; r := hex_decode_checked(PUB[who], k[..], false); return k; }
hex32 :: (id: [32] u8, out: [] u8) -> string {
  own := id; r := hex_encode_checked(cast(string) own[..], out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<64];
}
say :: (s: N47Str) { own := s; print("%", cast(string) own.b[0..<own.n]); }
// A nonce of 32 equal bytes, for a reproducible example only: a client
// takes 32 fresh random bytes for every message.
nonce_of :: (b: u8) -> [32] u8 { n: [32] u8; for i: 0..<32 { n[i] = b; } return n; }

// Sign an event with secret key who and read it back with its id and
// signature checked.
sign :: (who: int, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  key: [32] u8; key[31] = cast(u8) who;
  aux: [32] u8; work: [1] NostrSignWork; scratch: [16384] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [16384] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified && n47_same(e.public_key, key_of(who));
  return e;
}
content_of :: (e: NostrEvent) -> string { return e.original[e.content_from..<e.content_to]; }

// The client's pay_invoice request for INVOICE, sealed and signed at time t
// with nonce byte nb.
Sent :: struct { event: NostrEvent; plain: [512] u8; np: int; sealed: [1024] u8; ns: int; buf: [4096] u8; }
send_pay :: (u: N47Uri, t: int, nb: u8, s: [] Sent) {
  c: N47Call; c.error = N47_OK; c.method = N47_PAY; c.f.have = F_INVOICE;
  for i: 0..<INVOICE.count { c.f.str[0].b[i] = INVOICE[i]; }
  c.f.str[0].n = INVOICE.count;
  s[0].np = n47_request_json(c, s[0].plain[..]);
  ck: [32] u8; secret := u.secret;
  #assert s[0].np > 0 && n47_key(secret[..], u.wallet, ck[..]) == N47_OK;
  nonce := nonce_of(nb);
  o := n47_seal(ck[..], nonce[..], cast(string) s[0].plain[0..<s[0].np], s[0].sealed[..]);
  #assert o.error == N47_OK;
  s[0].ns = o.n;
  tags: [512] u8; tn := n47_request_tags(u.wallet, 0, tags[..]);
  s[0].event = sign(CLIENT, t, N47_REQUEST, cast(string) tags[0..<tn], cast(string) s[0].sealed[0..<o.n], s[0].buf[..]);
}

// The wallet service: read the request, open it, check the budget, and
// answer with a preimage or the refusal's error code. The budget after the
// call is in a[0].spend.budget.
Answer :: struct {
  event: NostrEvent; plain: [512] u8; np: int; buf: [4096] u8;
  read: int; opened: [512] u8; no: int; open: int; method: int; spend: N47Spend;
}
serve :: (req: NostrEvent, b: N47Budget, now: int, nb: u8, a: [] Answer) {
  wallet := key_of(WALLET);
  a[0].read = n47_request_event(req, wallet, now);
  secret: [32] u8; secret[31] = WALLET; ck: [32] u8;
  #assert n47_key(secret[..], req.public_key, ck[..]) == N47_OK;
  o := n47_open(ck[..], content_of(req), a[0].opened[..]);
  a[0].open = o.error; a[0].no = o.n;
  c := n47_request(cast(string) a[0].opened[0..<o.n]);
  a[0].method = c.method;
  s := n47_spend(b, now, c); a[0].spend = s;
  r: N47Reply; r.error = N47_OK; r.method = c.method;
  if s.error == N47_OK {
    r.f.have = F_PREIMAGE | F_FEES; r.f.num[9] = 1000;
    for i: 0..<32 { r.f.hex[1][i] = 1; }       // the wallet's payment would return this
  } else {
    r.code = n47_code(s.error); m := "over the connection's budget";
    for i: 0..<m.count { r.message.b[i] = m[i]; }
    r.message.n = m.count;
  }
  a[0].np = n47_response_json(r, a[0].plain[..]);
  nonce := nonce_of(nb); sealed: [1024] u8;
  sr := n47_seal(ck[..], nonce[..], cast(string) a[0].plain[0..<a[0].np], sealed[..]);
  tags: [512] u8; tn := n47_response_tags(req, tags[..]);
  a[0].event = sign(WALLET, now + 2, N47_RESPONSE, cast(string) tags[0..<tn], cast(string) sealed[0..<sr.n], a[0].buf[..]);
}
show_serve :: (a: [] Answer) -> N47Budget {
  s := a[0].spend;
  print("  request event: %\n", E47[a[0].read]);
  print("  opened: %, %, method %\n", E47[a[0].open], cast(string) a[0].opened[0..<a[0].no], N47_METHODS[a[0].method]);
  print("  budget: % msats, % (spent % of % since T0+%)\n", s.msats, E47[s.error], s.budget.spent, s.budget.limit, s.budget.start - T0);
  print("  answer: %\n", cast(string) a[0].plain[0..<a[0].np]);
  return s.budget;
}

// The client opens a response and matches it to its request.
receive :: (u: N47Uri, req: NostrEvent, res: NostrEvent) -> N47Reply {
  ck: [32] u8; secret := u.secret; k := n47_key(secret[..], u.wallet, ck[..]);
  opened: [1024] u8; o := n47_open(ck[..], content_of(res), opened[..]);
  r := n47_response(cast(string) opened[0..<o.n]);
  print("  client: opened %, match %", E47[o.error], E47[n47_match(req, res, u.wallet, N47_PAY, r)]);
  if r.code >= 0 { print(", error % \"", N47_CODES[r.code]); say(r.message); print("\"\n"); }
  else { hex: [64] u8; print(", preimage %, fees % msats\n", hex32(r.f.hex[1], hex[..]), r.f.num[9]); }
  return r;
}

main :: () {
  hex: [64] u8;
  print("== 1. The connection\n");
  u := n47_uri(URI);
  print("URI: %, wallet %\n", E47[u.error], hex32(u.wallet, hex[..]));
  print("  relay "); say(u.relays[0]); print(", lud16 "); say(u.lud16); print("\n");
  print("  secret: 32 bytes, kept and not printed; the client signs as %\n", hex32(u.client, hex[..]));

  print("\n== 2. The wallet's info event\n");
  ib: [4096] u8;
  info := sign(WALLET, T0 - 60, N47_INFO, "[[\"encryption\",\"nip44_v2 nip04\"]]", "pay_invoice get_balance make_invoice lookup_invoice get_info", ib[..]);
  i := n47_info(info);
  print("info: %, nip44_v2 %, nip04 %, commands:", E47[i.error], i.nip44, i.nip04);
  for m: 0..<N47_METHODS.count { if (i.methods & (1 << m)) != 0 { print(" %", N47_METHODS[m]); } }
  print("\n  new requests use NIP-44 v2: %\n", E47[n47_encryption(i)]);
  ob: [4096] u8;
  old := n47_info(sign(WALLET, T0 - 60, N47_INFO, "[]", "pay_invoice get_balance", ob[..]));
  print("  a wallet with no encryption tag (NIP-04 only): %\n", E47[n47_encryption(old)]);

  print("\n== 3. The pay_invoice request, by hand and by the code\n");
  sent: [1] Sent; send_pay(u, T0, 0x47, sent[..]);
  ck: [32] u8; secret := u.secret; k := n47_key(secret[..], u.wallet, ck[..]);
  print("plaintext (% bytes): %\n", sent[0].np, cast(string) sent[0].plain[0..<sent[0].np]);
  print("conversation key: %\n", hex32(ck, hex[..]));
  print("payload (% characters): %\n", sent[0].ns, cast(string) sent[0].sealed[0..<sent[0].ns]);
  print("request id %\n", hex32(sent[0].event.id, hex[..]));

  print("\n== 4. The wallet answers; the budget is 12000 msats a day\n");
  b := N47Budget.{limit = 12000, period = DAY, start = T0};
  ans: [1] Answer;
  serve(sent[0].event, b, T0, 0x48, ans[..]); b = show_serve(ans[..]);
  print("  response id %\n", hex32(ans[0].event.id, hex[..]));
  r := receive(u, sent[0].event, ans[0].event);

  print("\n== 5. More payments against the same budget\n");
  for t: 1..<3 {
    s2: [1] Sent; send_pay(u, T0 + 100 * t, cast(u8) (0x50 + t), s2[..]);
    print("payment % at T0+%:\n", t + 1, 100 * t);
    a2: [1] Answer; serve(s2[0].event, b, T0 + 100 * t, cast(u8) (0x60 + t), a2[..]); b = show_serve(a2[..]);
    r2 := receive(u, s2[0].event, a2[0].event);
  }
  s3: [1] Sent; send_pay(u, T0 + DAY, 0x70, s3[..]);
  print("payment 4 at T0+% (the next day):\n", DAY);
  a3: [1] Answer; serve(s3[0].event, b, T0 + DAY, 0x71, a3[..]); b = show_serve(a3[..]);

  print("\n== 6. Refusals\n");
  bad := [4] string.[
    "nostr+walletconnect://c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5?relay=wss%3A%2F%2Frelay.example.com",
    "nostr+walletconnect://c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5?secret=0000000000000000000000000000000000000000000000000000000000000001",
    "nostr+walletconnect://C6047F9441ED7D6D3045406E95C07CD85C778E4B8CEF3CA7ABAC09B95C709EE5?relay=wss%3A%2F%2Frelay.example.com",
    "nostr+walletconnect://c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5?relay=wss%3A%2F%2Frelay.example.com&secret=0000000000000000000000000000000000000000000000000000000000000000"];
  why := [4] string.["no secret", "no relay", "uppercase wallet key", "secret 0"];
  for k2: 0..<4 { print("URI with %: %\n", why[k2], E47[n47_uri(bad[k2]).error]); }
  plain: [1024] u8;
  print("a NIP-04 payload: %\n", E47[n47_open(ck[..], "zooMmtdlrlTCiIbfLqTqYrpCbTWrcVkP7RdcqCuM8TQ=?iv=obLD1OX2BxgpOktcbX6PkA==", plain[..]).error]);
  ab: [4096] u8; xb: [4096] u8;
  nip04 := sign(CLIENT, T0, N47_REQUEST, "[[\"p\",\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\"]]", content_of(sent[0].event), xb[..]);
  print("the wallet reads a request with no encryption tag: %\n", E47[n47_request_event(nip04, key_of(WALLET), T0)]);
  print("the response checked against payment 4's request: %\n", E47[n47_match(s3[0].event, ans[0].event, u.wallet, N47_PAY, r)]);
  print("the response with the client as the wallet: %\n", E47[n47_match(sent[0].event, ans[0].event, u.client, N47_PAY, r)]);
  print("the response read as a get_balance answer: %\n", E47[n47_match(sent[0].event, ans[0].event, u.wallet, N47_BALANCE, r)]);
  differ := n47_request("{\"method\":\"pay_invoice\",\"params\":{\"invoice\":\"lnbc50n1pexample\",\"amount\":6000}}");
  print("invoice 50n with amount 6000: %\n", E47[n47_spend(b, T0 + DAY, differ).error]);
  pay := n47_request(cast(string) sent[0].plain[0..<sent[0].np]);
  print("a payment at T0-1, before the budget starts: %\n", E47[n47_spend(N47Budget.{limit = 12000, period = DAY, start = T0}, T0 - 1, pay).error]);
  print("a request with \"amount\":-1: %\n", E47[n47_request("{\"method\":\"make_invoice\",\"params\":{\"amount\":-1}}").error]);
  forget := [1] N47Uri.[u]; n47_forget(forget[..]);
  wiped := true; for k3: 0..<32 { if forget[0].secret[k3] != 0 { wiped = false; } }
  print("after n47_forget: % relays, secret all zero %\n", forget[0].nrelays, wiped);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/wallet-connect.coax --run-steps 250000000
```

The output, which the book's checks compare in debug and release:

```text
== 1. The connection
URI: ok, wallet c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5
  relay wss://relay.example.com, lud16 alice@example.com
  secret: 32 bytes, kept and not printed; the client signs as 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798

== 2. The wallet's info event
info: ok, nip44_v2 true, nip04 true, commands: pay_invoice make_invoice lookup_invoice get_balance get_info
  new requests use NIP-44 v2: ok
  a wallet with no encryption tag (NIP-04 only): not NIP-44 v2

== 3. The pay_invoice request, by hand and by the code
plaintext (64 bytes): {"method":"pay_invoice","params":{"invoice":"lnbc50n1pexample"}}
conversation key: c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d
payload (176 characters): AkdHR0dHR0dHR0dHR0dHR0dHR0dHR0dHR0dHR0dHR0dHImMtJ/grvDZQgyIIOyzYxTNAnxfFQ/AQ+f1LNGiWMdEY+FRyEgZVNOMcJXahJS8NGAISWKr77CV1OW32Pn6pZKfAL5bFJVfsFKw5Do4UZVHi62dfBELy7AXgF5q/+heVhJ4=
request id ac198124f08cbe128886dd7f53bf4a50959ab730b0a67216acc74b43359f83f5

== 4. The wallet answers; the budget is 12000 msats a day
  request event: ok
  opened: ok, {"method":"pay_invoice","params":{"invoice":"lnbc50n1pexample"}}, method pay_invoice
  budget: 5000 msats, ok (spent 5000 of 12000 since T0+0)
  answer: {"result_type":"pay_invoice","error":null,"result":{"preimage":"0101010101010101010101010101010101010101010101010101010101010101","fees_paid":1000}}
  response id 5d78db41d2af141497f4320e0ca0d58c56d708d3c4038a2acbf11bf5fe752a0c
  client: opened ok, match ok, preimage 0101010101010101010101010101010101010101010101010101010101010101, fees 1000 msats

== 5. More payments against the same budget
payment 2 at T0+100:
  request event: ok
  opened: ok, {"method":"pay_invoice","params":{"invoice":"lnbc50n1pexample"}}, method pay_invoice
  budget: 5000 msats, ok (spent 10000 of 12000 since T0+0)
  answer: {"result_type":"pay_invoice","error":null,"result":{"preimage":"0101010101010101010101010101010101010101010101010101010101010101","fees_paid":1000}}
  client: opened ok, match ok, preimage 0101010101010101010101010101010101010101010101010101010101010101, fees 1000 msats
payment 3 at T0+200:
  request event: ok
  opened: ok, {"method":"pay_invoice","params":{"invoice":"lnbc50n1pexample"}}, method pay_invoice
  budget: 5000 msats, over the budget (spent 10000 of 12000 since T0+0)
  answer: {"result_type":"pay_invoice","error":{"code":"QUOTA_EXCEEDED","message":"over the connection's budget"},"result":null}
  client: opened ok, match ok, error QUOTA_EXCEEDED "over the connection's budget"
payment 4 at T0+86400 (the next day):
  request event: ok
  opened: ok, {"method":"pay_invoice","params":{"invoice":"lnbc50n1pexample"}}, method pay_invoice
  budget: 5000 msats, ok (spent 5000 of 12000 since T0+86400)
  answer: {"result_type":"pay_invoice","error":null,"result":{"preimage":"0101010101010101010101010101010101010101010101010101010101010101","fees_paid":1000}}

== 6. Refusals
URI with no secret: no secret
URI with no relay: no relay
URI with uppercase wallet key: bad wallet key
URI with secret 0: bad secret
a NIP-04 payload: not NIP-44 v2
the wallet reads a request with no encryption tag: not NIP-44 v2
the response checked against payment 4's request: e names another request
the response with the client as the wallet: another author
the response read as a get_balance answer: result_type differs
invoice 50n with amount 6000: amounts differ
a payment at T0-1, before the budget starts: before the budget starts
a request with "amount":-1: bad field
after n47_forget: 0 relays, secret all zero true
```

Section 3 prints the values worked out by hand: the 64-byte plaintext, the
conversation key, the 176-character payload and the request id. Section 6
shows the deliberately broken inputs and their refusals: a URI without a
secret, without a relay, with an uppercase wallet key and with secret 0; a
NIP-04 payload; a request event with no `encryption` tag; the first
response checked against another request (its `e` tag), against the wrong
author and as an answer to another method; an invoice for 5,000 millisats
with `amount` 6000; a payment before the budget starts; and a negative
amount. The secret never appears in the output.

## How the code is checked

`coax test coax/lib/nostr/nip47.coax` runs the library's checks:

- NIP-47's example URI reads, and its client key is the one Python
  derives from its example secret. Every URI refusal has its own case.
- NIP-47's appendix info event reads with its encryption and commands.
  Its published id is not the SHA-256 of the fields it shows (Python gives
  `ed220f16…6f636928`), so the event reader refuses it with `NE_ID`; the
  check reads it with the correct id.
- Every command's request and response, written by Python's `json.dumps`,
  reads and writes back to the same bytes. Each refusal of the readers
  and writers has a case.
- NIP-44's published vector (secret keys 1 and 2, nonce 1, plaintext `a`)
  goes through `n47_key`, `n47_seal` and `n47_open`, and a changed byte
  fails the MAC.
- The budget: exact boundaries (the last millisat passes, one more is
  refused), the second before and at a period's end, skipped periods, a
  period of 0, the clock before the start, and the amount rules.

[checks/wallet-connect.coax](../../nostr/examples/checks/wallet-connect.coax)
compares the example with a
Python script written from NIP-01, NIP-44, NIP-47, BIP-340, RFC 5869 and RFC
8439, which shares no code with Coax. The script first reproduces NIP-44's
published vector, then computes the conversation key, the message keys,
the payload, both event ids and both BIP-340 signatures.

<!-- source: nostr/examples/checks/wallet-connect.coax check -->
```jai
// From a Python script written from NIP-01, NIP-44, NIP-47, BIP-340, RFC
// 5869 and RFC 8439 (it shares no code with Coax): integer secp256k1 for
// the keys, the shared x and the signatures (32 zero bytes of aux), HMAC
// and HKDF from hashlib and hmac, ChaCha20 written out from RFC 8439, the
// plaintexts as json.dumps(..., separators=(",", ":")), and each id as the
// SHA-256 of json.dumps([0, pubkey, created_at, kind, tags, content]). The
// same script reproduces NIP-44's published vector (sec1 1, sec2 2, nonce
// 1, "a") before it computes the values below.
PY_CK :: "c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d";
PY_CHACHA_KEY :: "8afb2405058392562a3c1db5e9c517a01bf902b5abbcc80540b719f8c1a954ee";
PY_CHACHA_NONCE :: "727ca6a8114159322c8d921e";
PY_HMAC_KEY :: "8f626fd55f3de7ae1048d86d33984c658dcfc031b55a41af3c4a0e3f6ff80d43";
PY_PAYLOAD :: "AkdHR0dHR0dHR0dHR0dHR0dHR0dHR0dHR0dHR0dHR0dHImMtJ/grvDZQgyIIOyzYxTNAnxfFQ/AQ+f1LNGiWMdEY+FRyEgZVNOMcJXahJS8NGAISWKr77CV1OW32Pn6pZKfAL5bFJVfsFKw5Do4UZVHi62dfBELy7AXgF5q/+heVhJ4=";
PY_REQUEST_ID :: "ac198124f08cbe128886dd7f53bf4a50959ab730b0a67216acc74b43359f83f5";
PY_REQUEST_SIG :: "df9a19e243615bd9d7505fe23867eb582617a876328b486c5b6a4c53dec69efe9e01c9715ffd43b7328bbd932d51668f6dcdb37101c56e9ba92f094b3dea9a23";
PY_RESPONSE_ID :: "5d78db41d2af141497f4320e0ca0d58c56d708d3c4038a2acbf11bf5fe752a0c";
PY_RESPONSE_SIG :: "89d0120ad93488b48d97a9bd04ad23b09751e81d3759d58b6cb9afd14b7a8edafd658cc9ccb871d2709b923a7272445c68b924726429ef58e530d33170ac9931";
PY_PLAIN :: "{\"method\":\"pay_invoice\",\"params\":{\"invoice\":\"lnbc50n1pexample\"}}";

hex_is :: (bytes: [] u8, text: string) -> bool {
  out: [128] u8; r := hex_encode_checked(cast(string) bytes, out[..], false);
  return r.error == NC_OK && cast(string) out[0..<r.written] == text;
}

#test "by hand: secret 1 makes the shared x the wallet's key, and the conversation key is Python's" #steps 400000000 {
  u := n47_uri(URI);
  #assert u.error == N47_OK && hex_is(u.client[..], PUB[CLIENT]);
  // 1 * W = W: the shared point's x is the wallet's public key itself.
  work: [1] EcdhScratch; secret := u.secret; wallet := u.wallet;
  #assert ecdh_stage(secret[..], wallet[..], work[..]) == EC_OK && hex_is(work[0].staged[..], PUB[WALLET]);
  ck: [32] u8; #assert n47_key(secret[..], wallet, ck[..]) == N47_OK && hex_is(ck[..], PY_CK);
  // The wallet's side, its secret with the client's key, gives the same key.
  two: [32] u8; two[31] = 2; ck2: [32] u8;
  #assert n47_key(two[..], u.client, ck2[..]) == N47_OK && n47_same(ck, ck2);
  // The message keys for nonce 0x47..47.
  nonce := nonce_of(0x47); keys: [76] u8;
  #assert nip44_message_keys_checked(ck[..], nonce[..], keys[..], false) == NIP44_OK;
  #assert hex_is(keys[0..<32], PY_CHACHA_KEY) && hex_is(keys[32..<44], PY_CHACHA_NONCE) && hex_is(keys[44..<76], PY_HMAC_KEY);
  // 64 bytes pad to 64; 2 + 64 + 65 = 131 raw bytes are 176 base64 characters.
  #assert PY_PLAIN.count == 64 && nip44_calc_padded_len(64) == 64 && nip44_payload_size(NIP44_COMPAT, 64) == 176;
}
#test "the request and the response are Python's, byte for byte, and the signatures verify" #steps 900000000 {
  u := n47_uri(URI);
  sent: [1] Sent; send_pay(u, T0, 0x47, sent[..]);
  #assert cast(string) sent[0].plain[0..<sent[0].np] == PY_PLAIN && cast(string) sent[0].sealed[0..<sent[0].ns] == PY_PAYLOAD;
  e := sent[0].event;
  #assert hex_is(e.id[..], PY_REQUEST_ID) && hex_is(e.signature[..], PY_REQUEST_SIG) && e.verified;
  ans: [1] Answer;
  serve(e, N47Budget.{limit = 12000, period = DAY, start = T0}, T0, 0x48, ans[..]);
  a := ans[0].event;
  #assert ans[0].read == N47_OK && ans[0].open == N47_OK && ans[0].spend.budget.spent == 5000 && hex_is(a.id[..], PY_RESPONSE_ID) && hex_is(a.signature[..], PY_RESPONSE_SIG) && a.verified;
  #assert cast(string) ans[0].plain[0..<ans[0].np] == N47_T_RESPONSES[0];
}
#test "the budget trace by hand: 5000, 10000, refused at 15000, 5000 the next day" {
  pay := n47_request(PY_PLAIN);
  b := N47Budget.{limit = 12000, period = DAY, start = T0};
  s := n47_spend(b, T0, pay); #assert s.error == N47_OK && s.budget.spent == 5000;
  s = n47_spend(s.budget, T0 + 100, pay); #assert s.error == N47_OK && s.budget.spent == 10000;
  t := n47_spend(s.budget, T0 + 200, pay); #assert t.error == N47_QUOTA && 10000 + 5000 > 12000 && t.budget.spent == 10000;
  #assert N47_CODES[n47_code(t.error)] == "QUOTA_EXCEEDED";
  d := n47_spend(s.budget, T0 + DAY, pay); #assert d.error == N47_OK && d.budget.start == T0 + DAY && d.budget.spent == 5000;
}
#test "NIP-47's own examples: the URI, the requests and responses it shows" #steps 400000000 {
  u := n47_uri(N47_T_NIP_URI);
  #assert u.error == N47_OK && n47_t_is(u.relays[0], "wss://relay.damus.io") && hex_is(u.client[..], N47_T_NIP_CLIENT);
  // The spec's request and response shapes, with its placeholder values.
  #assert n47_request("{\"method\":\"get_balance\",\"params\":{}}").error == N47_OK;
  #assert n47_response("{\"result_type\":\"get_balance\",\"result\":{\"balance\":10000}}").f.num[10] == 10000;
  e := n47_response("{\"result_type\":\"pay_invoice\",\"error\":{\"code\":\"UNAUTHORIZED\",\"message\":\"human readable error message\"},\"result\":null}");
  #assert e.error == N47_OK && N47_CODES[e.code] == "UNAUTHORIZED";
  for k: 0..<N47_CODES.count { #assert n47_word(N47_CODES[k], T_CODES) == k; }
}
```

Run today, 2026-11-01: `coax test` on `nip47.coax` and on the checks file;
`coax mutate --root-only` on `nip47.coax`, with every mutant killed and
none marked equivalent; the example in debug and release against its
transcript; and the book's structure check. Not run: `COAX_MUTATION_FULL`,
mutation of the example and the checks file, and a session with a real
wallet service over a relay.

## Summary

- A NIP-47 connection is a URI with the wallet's key, its relays and a
  secret key made for one app. The app signs and encrypts with that
  secret, never with the user's key, and never prints it.
- The info event says which commands and encryption schemes a wallet
  supports. Without `nip44_v2`, this stack sends nothing.
- Requests and responses are small JSON objects inside NIP-44 v2
  payloads. Each command allows and requires its own fields.
- A response counts only if its kind, author, `p`, `e` and `result_type`
  all agree with the request.
- A budget per connection refuses a payment over what is left in the
  period, and the wallet answers `QUOTA_EXCEEDED`.
- Not built: `multi_pay_invoice`, `pay_keysend`, notifications, the
  contents of `list_transactions` entries, returning a failed payment's
  amount to the budget, and the relay session itself.

## Dependencies

- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [31. Publish and wait for OK](ch031-publish-and-wait-for-ok.md)

## Sources

- [NIP-47](../nips/official/47.md)
- [NIP-44](../nips/official/44.md)
- The BOLT11 amount reader: [settle.coax](../../coax/lib/x402/settle.coax), from
  [86. Lightning payments for requests](ch086-lightning-payments-for-requests.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
