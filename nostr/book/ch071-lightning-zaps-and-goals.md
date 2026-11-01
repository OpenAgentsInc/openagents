# 71. Lightning zaps and goals

> **Status: partly implemented.** The code, its checks, the example and the
> chapter text exist and pass. Mutation testing of the two `#authority`
> files, `nip57.coax` and `nip75.coax`, has not finished: see "How the code
> is checked".

Zaps are Lightning payments tied to Nostr events. A client asks the
recipient's Lightning server for an invoice and hands it a signed event,
the zap request, that says who pays whom and for what. When the invoice is
paid, the server publishes a zap receipt. This chapter writes the request,
checks the receipt the way a client must, and adds up receipts toward a
fundraising goal, under the Bitcoin-only rule.

Part X, Every NIP. Result: NIP-57 zap requests (9734) sent to an LNURL server
and zap receipts (9735) whose bolt11 description hash commits to the request,
and NIP-75 zap goals (9041).

## What this chapter teaches

- How a profile's `lud16` lightning address becomes an LNURL-pay URL, and
  what the LNURL-pay response must say before a client zaps:
  `allowsNostr` and `nostrPubkey`.
- Kind 9734: `relays`, `amount` in millisats, `lnurl`, `p`, optional `e`
  or `a`. It goes to the server's callback URL as a query parameter, and is
  never published.
- How a bolt11 invoice is laid out: an amount in its human-readable part,
  then 5-bit words: a timestamp, tagged fields, a signature. The `h` field
  is the SHA-256 of the invoice's description.
- Kind 9735, the receipt, from `nostrPubkey`: `bolt11`, `description`
  (the request's JSON) and an optional `preimage`; and Appendix F's checks:
  the receipt's signer is `nostrPubkey`, the invoice's description hash is
  SHA-256 of `description`, and the amounts agree.
- NIP-75: kind 9041 with `amount`, `relays` and an optional `closed_at`,
  and progress as the sum of valid receipts, each counted once, none after
  `closed_at`.
- A receipt is a claim by the recipient's server, not proof of payment.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [settle.coax](../../coax/lib/x402/settle.coax) | `x402_bolt11_msat`, `x402_preimage_matches` | An invoice's amount; a preimage against a payment hash (chapter 86) |
| [bech32-core.coax](../../coax/lib/nostr/bech32-core.coax) | `bech32_decode_checked`, `bech32_encode_checked` | Bech32 (chapter 15) |
| [tags.coax](../../coax/lib/nostr/tags.coax), [nip32.coax](../../coax/lib/nostr/nip32.coax) | `nostr_tag`, `n32_tag` | Read `p`, `e`, `a` and other tags (chapters 22 and 55) |
| [event.coax](../../coax/lib/nostr/event.coax) | `nostr_event_verify_vartime` | Read an event and check its id and signature (chapter 21) |
| [nip57.coax](../../coax/lib/nostr/nip57.coax) | `n57_lnurlp`, `n57_lnurl` | A lightning address's LNURL-pay URL, and that URL as an `lnurl` |
| [nip57.coax](../../coax/lib/nostr/nip57.coax) | `n57_pay` | The LNURL-pay response, read for a zap |
| [nip57.coax](../../coax/lib/nostr/nip57.coax) | `n57_request_tags`, `n57_callback` | A zap request's tags, and the GET URL that sends it |
| [nip57.coax](../../coax/lib/nostr/nip57.coax) | `n57_request` | A zap request read as Appendix D says |
| [nip57.coax](../../coax/lib/nostr/nip57.coax) | `n57_invoice` | A bolt11 invoice: checksum, amount, timestamp, `p` and `h` |
| [nip57.coax](../../coax/lib/nostr/nip57.coax) | `n57_receipt` | A zap receipt checked as Appendix F says |
| [nip75.coax](../../coax/lib/nostr/nip75.coax) | `n75_goal`, `n75_tally` | A goal, and its progress from receipts |

`nip57.coax` and `nip75.coax` are new with this chapter, and both are
`#authority` files: they decide whether a payment counts. Their fixtures
and test helpers are in
[coax/tests/nostr/nip57_fixtures.coax](../../coax/tests/nostr/nip57_fixtures.coax),
outside the authority files. The invoice amount and the preimage rule are
chapter 86's, reused, not copied. Nothing here fetches a URL, verifies an
invoice's signature or pays anything. The caller verifies the events it
passes in (chapter 21).

## The records

<!-- source: coax/lib/nostr/nip57.coax record -->
```jai
N57Pay :: struct {
  status:int=N57P_JSON;
  callback:[N57_MAX_URL]u8; nc:int;   // where the zap request goes
  min:int=-1; max:int=-1;             // msat
  nostr_pubkey:[32]u8;                // who signs the receipts
}
N57Request :: struct {
  status:int=N57R_EVENT; at:int=-1;   // the refusing tag
  msat:int=-1;                        // the amount tag, or -1
  recipient:[32]u8;                   // the p tag
  has_event:bool; event:[32]u8;       // the e tag
  has_address:bool;                   // an a tag
  relays:int;                         // relays in the relays tag
  has_sender:bool; sender:[32]u8;     // a P tag
  lnurl:[N57_MAX_URL]u8; nl:int;      // the lnurl tag, or nl 0
}
N57Invoice :: struct {
  status:int=N57I_BECH32;
  msat:int=-1; timestamp:int;
  has_payment_hash:bool; payment_hash:[32]u8;           // field p
  has_description_hash:bool; description_hash:[32]u8;   // field h
}
N57Zap :: struct {
  status:int=N57_EVENT;
  request:N57Request;                 // the zap request in the description
  sender:[32]u8;                      // its signer
  msat:int;                           // the invoice's amount
  payment_hash:[32]u8;
  created_at:int;                     // the receipt's
}
```

<!-- source: coax/lib/nostr/nip75.coax record -->
```jai
N75Goal :: struct {
  status:int=N75_EVENT; at:int=-1;   // the refusing tag
  msat:int=-1;                       // the target
  relays:int;                        // relays in the relays tag
  closed_at:int=-1;                  // or -1: never closes
  id:[32]u8; author:[32]u8;          // the goal event's
}
N75Progress :: struct {
  status:int=N75_EVENT;              // the goal's
  msat:int;                          // the sum of the counted receipts
  counted:int;                       // receipts counted
  refused:int;                       // receipts n57_receipt refuses
  other:int;                         // valid receipts for another event or person
  late:int;                          // valid receipts after closed_at
  future:int;                        // valid receipts after now
  duplicates:int;                    // a payment hash already counted
  full:int;                          // not counted: N75_MAX_COUNTED already are
  reached:bool;                      // msat >= the target
  closed:bool;                       // now is after closed_at
  hashes:[N75_MAX_COUNTED][32]u8;    // the counted payment hashes
}
```

## From a lightning address to a zap endpoint

A profile (chapter 45) names a lightning address in its `lud16` field, for
example `bob@zaps.example.com`. LUD-16 turns it into an HTTPS URL on the
domain: `https://zaps.example.com/.well-known/lnurlp/bob`, or `http://`
for a Tor `.onion` domain. NIP-57 also carries that URL in bech32 with the
prefix `lnurl`, its bytes as 5-bit words, in the request's `lnurl` tag.

<!-- source: coax/lib/nostr/nip57.coax address -->
```jai
// The LNURL-pay URL of a lightning address (LUD-16): name@domain becomes
// https://domain/.well-known/lnurlp/name, or http:// for an .onion domain.
// The name is a-z 0-9 - _ . and the domain two or more dot-separated
// labels of a-z 0-9 -, all lower case. Its length in out, or -1.
n57_lnurlp :: (address:string,out:[]u8)->int {
  at:=0;
  while at<address.count && n57_name_ok(address[at]) {at+=1;}
  if at==0 || at>=address.count || address[at]!='@' {return -1;}
  domain:=address[at+1..<address.count];dotted:=false;empty:=true;
  for c:domain {
    if c=='.' {if empty {return -1;}dotted=true;empty=true;continue;}
    if !n57_label_ok(c) {return -1;}
    empty=false;
  }
  if empty || !dotted {return -1;}
  onion:=domain.count>=7 && domain[domain.count-6..<domain.count]==".onion";
  n:=json_raw(out,0,ifx onion then "http://" else "https://");
  n=json_raw(out,n,domain);n=json_raw(out,n,"/.well-known/lnurlp/");
  return json_raw(out,n,address[0..<at]);
}
// The URL as NIP-57's lnurl tag carries it: bech32, prefix lnurl, the URL's
// bytes as 5-bit words. Its length in out, or -1.
n57_lnurl :: (url:string,out:[]u8)->int {
  r:=bech32_encode_checked("lnurl",url,true,out,true,false);
  if r.error!=NC_OK {return -1;}
  return r.written;
}
// Whether s is an lnurl: bech32 with prefix lnurl.
n57_is_lnurl :: (s:string)->bool {
  hrp:[90]u8;w:[N57_MAX_URL*2]u8;
  r:=bech32_decode_checked(s,hrp[..],w[..],true,false);
  return r.error==NC_OK && cast(string)hrp[0..<r.hrp_written]=="lnurl";
}
```

The name allows only `a-z`, `0-9`, `-`, `_` and `.`, so `Bob@...` is
refused rather than lower-cased. NIP-57's own `lnurl` tag decodes to
`https://stacker.news/.well-known/lnurlp/odell`, and `n57_lnurl` of that
URL gives back the NIP's string.

A GET of that URL returns the LNURL-pay response. Before zapping, a client
needs four things from it: `tag` is `payRequest`, `allowsNostr` is `true`,
`nostrPubkey` is a 64-digit hex key, and `callback` is where the zap
request goes, with `minSendable` and `maxSendable` around the amount.

<!-- source: coax/lib/nostr/nip57.coax pay -->
```jai
N57_PAY_KEYS :: [6] string.["tag", "callback", "minSendable", "maxSendable", "allowsNostr", "nostrPubkey"];
// The LNURL-pay response, read for a zap: tag payRequest, allowsNostr true,
// nostrPubkey a key, an https callback, and 1 <= minSendable <= maxSendable.
// Other keys are skipped; a key given twice refuses.
n57_pay :: (text:string)->N57Pay {
  p:N57Pay;j:[1]Json;json_start(j[..],text);
  if json_next(j[..])!=JSON_OBJECT {return p;}
  seen:=0;is_pay:=false;allows:=false;has_key:=false;
  while json_next(j[..])!=JSON_OBJECT_END {
    f:= -1;key:=json_text(j[..]);
    for k:0..<6 {if json_is(key,N57_PAY_KEYS[k]) {f=k;}}
    v:=json_next(j[..]);raw:=json_text(j[..]);
    if !json_skip(j[..]) {return p;}
    if f<0 {continue;}
    if ((seen>>f)&1)==1 {return p;}
    seen|=1<<f;
    if f==0 {is_pay=json_is(raw,"payRequest");}
    if f==1 {p.nc=json_unescape(raw,p.callback[..]);if p.nc<0 {return p;}}
    if f==2 && v==JSON_NUMBER {p.min=json_int(raw).value;}
    if f==3 && v==JSON_NUMBER {p.max=json_int(raw).value;}
    if f==4 {allows=v==JSON_TRUE;}
    if f==5 {has_key=v==JSON_STRING && ne_lower_hex(raw,p.nostr_pubkey[..]);}
  }
  if json_next(j[..])!=JSON_END {return p;}
  p.status=N57P_TAG;if !is_pay {return p;}
  p.status=N57P_NOSTR;if !allows {return p;}
  p.status=N57P_KEY;if !has_key {return p;}
  p.status=N57P_CALLBACK;
  if p.nc<9 || cast(string)p.callback[0..<8]!="https://" {return p;}
  p.status=N57P_RANGE;if p.min<1 || p.max<p.min {return p;}
  p.status=N57P_OK;return p;
}
```

The key in `nostrPubkey` is the one that signs receipts. It belongs to the
recipient's server, not to the recipient. A client stores it with the
recipient and checks every receipt against it.

## The zap request

The request is a kind 9734 event signed by the payer. Its tags name the
relays the receipt goes to, the amount in millisats, the recipient's
`lnurl`, the recipient in `p`, and, when zapping an event, the event in
`e` and its kind in `k`. NIP-57 says the request is not published: the
client sends it to the callback in the `nostr` query parameter,
percent-encoded, next to `amount` and `lnurl`.

<!-- source: coax/lib/nostr/nip57.coax write -->
```jai
// A zap request's tags in NIP-57's order: relays, amount, lnurl (when not
// ""), p, then e and k when event is not "". Its length in out, or -1 for a
// relay that is not ws:// or wss://, no relays, an amount out of range, a
// key or id that is not 64 lowercase hex digits, or an lnurl that is not one.
n57_request_tags :: (relays:[]string,msat:int,lnurl:string,recipient:string,event:string,kind:int,out:[]u8)->int {
  k:[32]u8;
  if relays.count==0 || msat<1 || msat>X402_MAX_MSAT || !ne_lower_hex(recipient,k[..]) {return -1;}
  if event!="" && !ne_lower_hex(event,k[..]) {return -1;}
  if lnurl!="" && !n57_is_lnurl(lnurl) {return -1;}
  n:=json_raw(out,0,"[[\"relays\"");
  for s:relays {if !nt_relay_ok(s) {return -1;}n=json_raw(out,n,",");n=json_write_string(out,n,s);}
  n=json_raw(out,n,"],[\"amount\",\"");n=json_write_int(out,n,msat);n=json_raw(out,n,"\"]");
  if lnurl!="" {n=json_raw(out,n,",[\"lnurl\",");n=json_write_string(out,n,lnurl);n=json_raw(out,n,"]");}
  n=json_raw(out,n,",[\"p\",");n=json_write_string(out,n,recipient);n=json_raw(out,n,"]");
  if event!="" {
    n=json_raw(out,n,",[\"e\",");n=json_write_string(out,n,event);
    n=json_raw(out,n,"],[\"k\",\"");n=json_write_int(out,n,kind);n=json_raw(out,n,"\"]");
  }
  return json_raw(out,n,"]");
}
// s percent-encoded for a URL query: A-Z a-z 0-9 - _ . ~ as they are,
// every other byte as %XX.
n57_query :: (out:[]u8,at:int,s:string)->int {
  digits:="0123456789ABCDEF";n:=at;
  for c:s {
    plain:=(c>='A' && c<='Z') || (c>='a' && c<='z') || (c>='0' && c<='9') || c=='-' || c=='_' || c=='.' || c=='~';
    need:=ifx plain then 1 else 3;
    if n<0 || n+need>out.count {return -1;}
    if plain {out[n]=c;n+=1;continue;}
    out[n]='%';out[n+1]=digits[c>>4];out[n+2]=digits[c&15];n+=3;
  }
  return n;
}
// The GET URL that sends the signed zap request to the server's callback
// (Appendix B): amount, nostr (the request JSON, percent-encoded), lnurl.
n57_callback :: (callback:string,msat:int,request:string,lnurl:string,out:[]u8)->int {
  q:=false;for c:callback {if c=='?' {q=true;}}
  n:=json_raw(out,0,callback);n=json_raw(out,n,ifx q then "&amount=" else "?amount=");
  n=json_write_int(out,n,msat);n=json_raw(out,n,"&nostr=");n=n57_query(out,n,request);
  if lnurl!="" {n=json_raw(out,n,"&lnurl=");n=json_raw(out,n,lnurl);}
  return n;
}
```

The server reads the request before it makes an invoice. Appendix D lists
what it must hold: a valid signature, exactly one `p`, at most one `e`, a
`relays` tag, an `amount` equal to the query's when present, a valid `a`
coordinate, and at most one `P`.

<!-- source: coax/lib/nostr/nip57.coax request -->
```jai
// A zap request read as Appendix D says the server must: kind 9734, exactly
// one p, at most one e, one relays tag with a ws:// or wss:// relay first,
// an amount that is a positive msat number, a tags that are addresses, at
// most one P, at most one lnurl, and it an lnurl. The signature is the
// caller's to verify (n57_receipt does).
n57_request :: (e:NostrEvent)->N57Request {
  r:N57Request;if e.error!=NE_OK {return r;}
  if e.kind!=N57_KIND_REQUEST {return n57_rrefuse(r,N57R_KIND,-1);}
  ps:=0;es:=0;i:=0;
  while true {
    t:=nostr_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_EVENT {return n57_rrefuse(r,N57R_EVENT,i);}
    n:=n32_tag(e,i);
    name:=ifx n.error==NT_OK then cast(string)n.v[0].b[0..<n.v[0].n] else "";
    value:=cast(string)n.v[1].b[0..<n.v[1].n];
    if t.type==NT_P {
      ps+=1;if ps>1 || t.error!=NT_OK {return n57_rrefuse(r,N57R_P,i);}
      r.recipient=t.pubkey;
    } else if t.type==NT_E {
      es+=1;if es>1 || t.error!=NT_OK {return n57_rrefuse(r,N57R_E,i);}
      r.has_event=true;r.event=t.id;
    } else if t.type==NT_A {
      if t.error!=NT_OK {return n57_rrefuse(r,N57R_ADDRESS,i);}
      r.has_address=true;
    } else if name=="relays" {
      if r.relays>0 || !nt_relay_ok(value) {return n57_rrefuse(r,N57R_RELAYS,i);}
      r.relays=n.values-1;
    } else if name=="amount" {
      if r.msat!= -1 {return n57_rrefuse(r,N57R_AMOUNT,i);}
      r.msat=n57_msat(value);
      if r.msat== -1 {return n57_rrefuse(r,N57R_AMOUNT,i);}
    } else if name=="P" {
      if r.has_sender || !ne_lower_hex(value,r.sender[..]) {return n57_rrefuse(r,N57R_SENDER,i);}
      r.has_sender=true;
    }
    i+=1;
  }
  if ps!=1 {return n57_rrefuse(r,N57R_P,-1);}
  if r.relays==0 {return n57_rrefuse(r,N57R_RELAYS,-1);}
  l:=n57_long(e,"lnurl",r.lnurl[..]);
  if l.tags>1 || (l.tags==1 && !n57_is_lnurl(cast(string)r.lnurl[0..<l.n])) {return n57_rrefuse(r,N57R_LNURL,-1);}
  r.nl=l.n;r.status=N57R_OK;return r;
}
```

## The invoice and its description hash

The server makes a description-hash invoice: one whose `h` field is the
SHA-256 of the request's JSON, and nothing else. A bolt11 invoice is
bech32 with a human-readable part such as `lnbc210n`: `lnbc` for Bitcoin
mainnet and the amount, here 210 nano-bitcoin. The data part is 5-bit
words: 7 words (35 bits) of timestamp, then tagged fields, then 104 words
(65 bytes) of signature. Each field is one word of type, two words (10
bits) of length, and that many words of data. The payment hash `p` is type
1 and `h` is type 23; both are 52 words, 260 bits, of which 256 are the
hash and 4 are padding.

<!-- source: coax/lib/nostr/nip57.coax invoice -->
```jai
// 52 five-bit words from at as 32 bytes; the last 4 bits are padding.
n57_bytes32 :: (w:[]u8,at:int)->[32]u8 {
  k:[32]u8;
  for b:0..<256 {k[b/8]|=cast(u8)(((cast(int)w[at+b/5]>>(4-b%5))&1)<<(7-b%8));}
  return k;
}
// A bolt11 invoice: the bech32 checksum, the amount (chapter 86's
// x402_bolt11_msat: mainnet lnbc with an amount), the 35-bit timestamp, then
// the tagged fields, each a type word, a 10-bit length and that many words,
// up to the 104-word signature. The first p and the first h of 52 words are
// kept; other fields are skipped, as BOLT #11 says a reader must. The
// signature is not verified.
n57_invoice :: (text:string)->N57Invoice {
  v:N57Invoice;hrp:[90]u8;w:[N57_MAX_INVOICE]u8;
  r:=bech32_decode_checked(text,hrp[..],w[..],true,false);
  if r.error!=NC_OK {return v;}
  v.status=N57I_AMOUNT;v.msat=x402_bolt11_msat(text);
  if v.msat<0 {return v;}
  v.status=N57I_SHAPE;
  end:=r.written-104;
  if end<7 {return v;}
  for i:0..<7 {v.timestamp=v.timestamp*32+cast(int)w[i];}
  i:=7;
  while i<end {
    if i+3>end {return v;}
    type:=w[i];size:=cast(int)w[i+1]*32+cast(int)w[i+2];i+=3;
    if i+size>end {return v;}
    if size==52 && type==1 && !v.has_payment_hash {v.has_payment_hash=true;v.payment_hash=n57_bytes32(w[..],i);}
    if size==52 && type==23 && !v.has_description_hash {v.has_description_hash=true;v.description_hash=n57_bytes32(w[..],i);}
    i+=size;
  }
  v.status=N57I_OK;return v;
}
```

`n57_invoice` checks the bech32 checksum, so one changed character
refuses. It does not check the signature: that would recover the node's
key from it, and the server that signs the receipt also made the
invoice. The amount comes from chapter 86's `x402_bolt11_msat`, which reads
only mainnet `lnbc` invoices with an amount. A testnet `lntb` invoice
refuses.

## Checking a receipt

When the invoice is paid, the server publishes a kind 9735 receipt, signed
with the `nostrPubkey` key. It carries the recipient's `p`, the request's
`e` and the payer's `P`, the invoice in `bolt11`, the request's JSON in
`description`, and optionally the `preimage`. These values are longer
than the 256 bytes `n32_tag` decodes, so a reader of long values comes
first:

<!-- source: coax/lib/nostr/nip57.coax long -->
```jai
// How many tags are named name, and the first one's value unescaped into
// out: n is its length, or 0 when it has none or it does not fit. Unlike
// n32_tag it reads values longer than 256 bytes: an invoice, a zap request.
// Tags that do not read as arrays of strings read as no tags.
N57Value :: struct {tags:int; n:int;}
n57_long :: (e:NostrEvent,name:string,out:[]u8)->N57Value {
  r:N57Value;nothing:N57Value;j:[1]Json;
  if !nt_start(j[..],e) {return r;}
  tok:=json_next(j[..]);
  while tok==JSON_ARRAY {
    v:=json_next(j[..]);
    if v==JSON_STRING && json_is(json_text(j[..]),name) {
      r.tags+=1;v=json_next(j[..]);
      if r.tags==1 && v==JSON_STRING {k:=json_unescape(json_text(j[..]),out);if k!= -1 {r.n=k;}}
    }
    while v==JSON_STRING {v=json_next(j[..]);}
    if v!=JSON_ARRAY_END {return nothing;}
    tok=json_next(j[..]);
  }
  if tok!=JSON_ARRAY_END {return nothing;}
  return r;
}
```

Appendix F asks for three checks: the receipt's signer is `nostrPubkey`,
the invoice's amount equals the request's `amount`, and the request's
`lnurl` matches the recipient's. Appendix E adds that SHA-256 of
`description` matches the invoice's description hash. `n57_receipt` does
all of them, in this order, and the ones that follow from them:

<!-- source: coax/lib/nostr/nip57.coax receipt -->
```jai
// A zap receipt checked as Appendix F says a client must, against the
// nostrPubkey of the recipient's LNURL server and, when not "", the
// recipient's lnurl. In order: kind 9735; signed by nostrPubkey; one bolt11
// and one description tag; the invoice reads; its description hash is
// SHA-256 of the description; the description is a signed zap request that
// keeps Appendix D; the invoice's amount is the request's amount when it
// names one; every p and e of the receipt is the request's, and its P the
// request's signer; the lnurl matches; a preimage hashes to the payment hash.
n57_receipt :: (e:NostrEvent,nostr_pubkey:[32]u8,lnurl:string)->N57Zap {
  z:N57Zap;if e.error!=NE_OK {return z;}
  if e.kind!=N57_KIND_RECEIPT {return n57_zrefuse(z,N57_KIND);}
  if !ns_equal32(e.public_key,nostr_pubkey) {return n57_zrefuse(z,N57_SIGNER);}
  bolt:[N57_MAX_INVOICE]u8;b:=n57_long(e,"bolt11",bolt[..]);
  if b.tags!=1 {return n57_zrefuse(z,N57_BOLT11);}
  desc:[N57_MAX_DESCRIPTION]u8;d:=n57_long(e,"description",desc[..]);
  if d.tags!=1 {return n57_zrefuse(z,N57_DESCRIPTION);}
  inv:=n57_invoice(cast(string)bolt[0..<b.n]);
  if inv.status!=N57I_OK {return n57_zrefuse(z,N57_INVOICE);}
  if !inv.has_description_hash {return n57_zrefuse(z,N57_NO_HASH);}
  h:[32]u8;hashed:=sha256_hash_checked(desc[0..<d.n],h[..],false);
  if !ns_equal32(h,inv.description_hash) {return n57_zrefuse(z,N57_HASH);}
  check:[N57_MAX_DESCRIPTION]u8;
  q:=nostr_event_verify_vartime(cast(string)desc[0..<d.n],check[..],false);
  if q.error!=NE_OK {return n57_zrefuse(z,N57_REQUEST);}
  z.request=n57_request(q);
  if z.request.status!=N57R_OK {return n57_zrefuse(z,N57_REQUEST_TAGS);}
  z.sender=q.public_key;z.msat=inv.msat;z.payment_hash=inv.payment_hash;z.created_at=e.created_at;
  if z.request.msat!= -1 && z.request.msat!=inv.msat {return n57_zrefuse(z,N57_AMOUNT);}
  has_p:=false;has_e:=false;i:=0;
  while true {
    t:=nostr_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.type==NT_P {has_p=true;if t.error!=NT_OK || !ns_equal32(t.pubkey,z.request.recipient) {return n57_zrefuse(z,N57_RECIPIENT);}}
    if t.type==NT_E {has_e=true;if t.error!=NT_OK || !ns_equal32(t.id,z.request.event) {return n57_zrefuse(z,N57_TARGET);}}
    i+=1;
  }
  if !has_p {return n57_zrefuse(z,N57_RECIPIENT);}
  if has_e!=z.request.has_event {return n57_zrefuse(z,N57_TARGET);}
  sender:[64]u8;s:=n57_long(e,"P",sender[..]);k:[32]u8;
  if s.tags>0 && (s.tags>1 || !ne_lower_hex(cast(string)sender[0..<s.n],k[..]) || !ns_equal32(k,z.sender)) {return n57_zrefuse(z,N57_SENDER);}
  if lnurl!="" && z.request.nl!=0 && cast(string)z.request.lnurl[0..<z.request.nl]!=lnurl {return n57_zrefuse(z,N57_LNURL);}
  pre:[64]u8;p:=n57_long(e,"preimage",pre[..]);hex:[64]u8;
  if p.tags>0 && (p.tags>1 || !x402_preimage_matches(cast(string)pre[0..<p.n],n57_hex(inv.payment_hash,hex[..]))) {return n57_zrefuse(z,N57_PREIMAGE);}
  z.status=N57_OK;return z;
}
```

Every `p` and `e` in the receipt must be the request's, because a goal
tally reads the target from the request, which the payer signed and the
invoice commits to. A preimage proves less than it seems: the server knows
it either way. NIP-57 says so, and a receipt stays a claim by the server.
What the checks rule out is a receipt that contradicts itself: signed by
someone else, for an invoice that commits to another request, or for
another amount.

## Worked example: one receipt by hand

Alice zaps Bob's goal 21000 msat. Her signed request is 678 bytes of JSON,
`{"id":"bb0f28e7...` to `...4f47c8e"}`. Bob's server makes this invoice
(the zero signature is a fixture; no wallet would pay it):

```jai
lnbc210n1p5szvaapp54cskcth4y3ar0qkpxhh6y7drunxuvyy5yu8462l9333qfdaxztyshp5dkd9rwjk905634vuevxp9lrc57m9yqz97uyg0lgf626zyf2kjzzqqqqqqqqq...qqucmd5p
```

1. The amount. The human-readable part ends at the last `1`: `lnbc210n`.
   `n` is 10^-9 BTC and one bitcoin is 10^11 msat, so 210n is 210 × 100 =
   21000 msat. The request's `amount` tag is `"21000"`. They agree.
2. Find `h`. After the separator come 7 timestamp characters, `p5szvaa`,
   then the `p` field: `pp5` and 52 characters. Then `hp5`: `h` is word 23
   in the bech32 alphabet `qpzry9x8gf2tvdw0s3jn54khce6mua7l`, and `p5` is
   the length, 1 × 32 + 20 = 52 words.
3. Read the first bytes of `h`. Its first four characters `dkd9` are the
   words 13, 22, 13, 5: the bits `01101 10110 01101 00101`. Regrouped in
   fours they are `0110 1101 1001 1010 0101`, hex `6d9a5`.
4. Hash the description. SHA-256 of the 678 bytes in the receipt's
   `description` tag starts `6d9a51ba`, and is
   `6d9a51ba562be9a8d59ccb0c12fc78a7b6520045f70887fd09d2b42225569084`, the
   same 32 bytes as all of `h`. (SHA-256 is too long to do by hand; the
   program and an independent Python script agree on it.)
5. The receipt is signed by Dave's key, `79be667e...`, the `nostrPubkey`
   of Bob's server. Its `p` is Bob, its `e` is the goal, its `P` is Alice,
   and its preimage hashes to the invoice's `p` field.

So the receipt is valid: 21000 msat from Alice to Bob's goal. If Alice's
request had said "For the microphone!" instead, its SHA-256 would differ in
every byte, and the same invoice would no longer commit to it.

## Zap goals

A goal is a kind 9041 event: a target `amount` in millisats, the `relays`
its zaps go to and are counted from, and an optional `closed_at`. NIP-75
says receipts published after `closed_at` should not count.

<!-- source: coax/lib/nostr/nip75.coax goal -->
```jai
n75_refuse :: (g:N75Goal,status:int,at:int)->N75Goal {o:=g;o.status=status;o.at=at;return o;}
// A goal: kind 9041, one amount tag with a positive msat number, one relays
// tag whose first relay is ws:// or wss://, at most one closed_at, a
// positive number of seconds.
n75_goal :: (e:NostrEvent)->N75Goal {
  g:N75Goal;if e.error!=NE_OK {return g;}
  if e.kind!=N75_KIND_GOAL {return n75_refuse(g,N75_KIND,-1);}
  g.id=e.id;g.author=e.public_key;
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_EVENT {return n75_refuse(g,N75_EVENT,i);}
    name:=cast(string)t.v[0].b[0..<t.v[0].n];value:=cast(string)t.v[1].b[0..<t.v[1].n];
    if name=="amount" {
      if g.msat!= -1 {return n75_refuse(g,N75_AMOUNT,i);}
      g.msat=n57_msat(value);
      if g.msat== -1 {return n75_refuse(g,N75_AMOUNT,i);}
    } else if name=="relays" {
      if g.relays>0 || !nt_relay_ok(value) {return n75_refuse(g,N75_RELAYS,i);}
      g.relays=t.values-1;
    } else if name=="closed_at" {
      if g.closed_at!= -1 {return n75_refuse(g,N75_CLOSED,i);}
      g.closed_at=n57_msat(value);
      if g.closed_at== -1 {return n75_refuse(g,N75_CLOSED,i);}
    }
    i+=1;
  }
  if g.msat== -1 {return n75_refuse(g,N75_AMOUNT,-1);}
  if g.relays==0 {return n75_refuse(g,N75_RELAYS,-1);}
  g.status=N75_OK;return g;
}
```

The progress is the sum of valid receipts whose request names the goal in
`e` and its author in `p`. The caller passes the time `now`, so the same
events give the same answer on every run. A receipt dated after
`closed_at` is late; one dated after `now` is not counted yet. A receipt's
`created_at` is the invoice's `paid_at`, as Appendix E says. Each payment
counts once: the same payment hash published in a second receipt is a
duplicate.

<!-- source: coax/lib/nostr/nip75.coax tally -->
```jai
// One valid receipt z added to the progress p of goal g at time now.
n75_add :: (p:N75Progress,g:N75Goal,z:N57Zap,now:int)->N75Progress {
  r:=p;
  if !z.request.has_event || !ns_equal32(z.request.event,g.id) || !ns_equal32(z.request.recipient,g.author) {r.other+=1;return r;}
  if g.closed_at!= -1 && z.created_at>g.closed_at {r.late+=1;return r;}
  if z.created_at>now {r.future+=1;return r;}
  for k:0..<r.counted {if ns_equal32(r.hashes[k],z.payment_hash) {r.duplicates+=1;return r;}}
  if r.counted==N75_MAX_COUNTED {r.full+=1;return r;}
  r.hashes[r.counted]=z.payment_hash;r.counted+=1;r.msat+=z.msat;
  return r;
}
// The goal's progress at time now from the receipts in set, checked
// against the nostrPubkey of the goal author's LNURL server and, when not
// "", the author's lnurl. Events of other kinds are passed over.
n75_tally :: (goal:NostrEvent,set:[]NostrEvent,server:[32]u8,lnurl:string,now:int)->N75Progress {
  p:N75Progress;g:=n75_goal(goal);p.status=g.status;
  if g.status!=N75_OK {return p;}
  for e:set {
    if e.kind!=N57_KIND_RECEIPT {continue;}
    z:=n57_receipt(e,server,lnurl);
    if z.status!=N57_OK {p.refused+=1;continue;}
    p=n75_add(p,g,z,now);
  }
  p.reached=p.msat>=g.msat;
  p.closed=g.closed_at!= -1 && now>g.closed_at;
  return p;
}
```

## The example

The example is [nostr/examples/lightning-zaps.coax](../examples/lightning-zaps.coax).
It turns Bob's lightning address into his LNURL-pay URL and `lnurl`, reads
his server's LNURL-pay response, and signs Bob's goal and the zap requests
of Alice, Carol and Erin with the published test keys. It plays Bob's
server: it makes each description-hash invoice and signs each receipt with
Dave's key. It checks Alice's receipt with the hand values above, tallies
the goal, and shows the refusals: a receipt signed by Mallory, an invoice
for 20000 msat for a 21000 msat request, a description edited after the
invoice was made, and a preimage of another payment. Last, it checks
NIP-57's own example receipt.

<!-- source: nostr/examples/lightning-zaps.coax example -->
```jai
#load "../../coax/lib/nostr/nip75.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// BIP-340 test vectors 0 to 3's public keys, then the public keys of the
// secret keys 1 and 2. Dave runs Bob's LNURL server: his key signs the
// receipts, and the server says so in nostrPubkey.
PEOPLE :: [6] string.["Alice", "Bob", "Carol", "Mallory", "Dave", "Erin"];
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
ALICE :: 0; BOB :: 1; CAROL :: 2; MALLORY :: 3; DAVE :: 4; ERIN :: 5;
RELAY :: "wss://relay.example.com";
T0 :: 1761686400;
// Fixture values only: each payment's preimage (Alice's is the bytes 01 02
// ... 20), and every invoice here carries a zero signature, so no wallet
// would pay it.
PREIMAGE :: "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
PREIMAGE_CAROL :: "2222222222222222222222222222222222222222222222222222222222222222";
PREIMAGE_ERIN :: "3333333333333333333333333333333333333333333333333333333333333333";
// The LNURL-pay response Bob's server sends for his lightning address.
PAY :: "{\"tag\":\"payRequest\",\"callback\":\"https://zaps.example.com/lnurlp/bob/callback\",\"minSendable\":1000,\"maxSendable\":100000000,\"metadata\":\"[[\\\"text/plain\\\",\\\"Bob\\\"]]\",\"allowsNostr\":true,\"nostrPubkey\":\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\"}";

decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
key_of :: (who: int) -> [32] u8 { k: [32] u8; decode_hex(PUB[who], k[..]); return k; }
hex32 :: (k: [32] u8, out: [] u8) -> string { return n57_hex(k, out); }
sha_of :: (text: string) -> [32] u8 {
  b: [8192] u8; h: [32] u8;
  for i: 0..<text.count { b[i] = text[i]; }
  s := sha256_hash_checked(b[0..<text.count], h[..], false);
  #assert s == SHA_OK;
  return h;
}

// Sign an event into out and read it back with its ID and signature checked.
sign :: (who: int, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  key: [32] u8; decode_hex(SECRET[who], key[..]);
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
text_of :: (e: NostrEvent) -> string { return e.original; }

// What Bob's server does with a zap request: a bolt11 invoice for msat
// (in units of n, 100 msat), dated at, whose p field is the payment hash,
// SHA-256 of the preimage, and whose h field is SHA-256 of the request's
// JSON, the description.
make_invoice :: (msat: int, at: int, preimage: string, description: string, out: [] u8) -> string {
  w: [400] u8; n := 0;
  for i: 0..<7 { w[n] = cast(u8) ((at >> (5 * (6 - i))) & 31); n += 1; }
  n = n57_field(w[..], n, 1, sha_of_hex(preimage));      // p
  n = n57_field(w[..], n, 23, sha_of(description));     // h
  for i: 0..<104 { w[n] = 0; n += 1; }                    // a zero signature
  hrp: [20] u8; k := json_raw(hrp[..], 0, "lnbc"); k = json_write_int(hrp[..], k, msat / 100); k = json_raw(hrp[..], k, "n");
  r := bech32_encode_checked(cast(string) hrp[0..<k], cast(string) w[0..<n], false, out, true, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<r.written];
}
// A zap request: who zaps the goal (or Bob when goal is "") for msat.
zap_request :: (who: int, at: int, msat: int, goal: string, lnurl: string, content: string, out: [] u8) -> NostrEvent {
  tags: [1000] u8; relays := [1] string.[RELAY];
  n := n57_request_tags(relays[..], msat, lnurl, PUB[BOB], goal, ifx goal.count > 0 then 9041 else 0, tags[..]);
  #assert n > 0;
  return sign(who, at, 9734, cast(string) tags[0..<n], content, out);
}
// The receipt the server publishes when the invoice is paid: p, e, P, the
// invoice, the request as description, the preimage; signed by signer.
zap_receipt :: (signer: int, at: int, request: NostrEvent, goal: string, invoice: string, preimage: string, out: [] u8) -> NostrEvent {
  tags: [3000] u8; pk: [64] u8;
  t := n57_receipt_tags(PUB[BOB], goal, hex32(request.public_key, pk[..]), invoice, text_of(request), preimage, tags[..]);
  return sign(signer, at, 9735, t, "", out);
}
show_zap :: (label: string, z: N57Zap) {
  if z.status == N57_OK { print("%: valid, % msat from %\n", label, z.msat, name_of(z.sender)); }
  else { print("%: refused, %\n", label, N57_WHY[z.status]); }
}
name_of :: (k: [32] u8) -> string {
  for i: 0..<PEOPLE.count { if ns_equal32(key_of(i), k) { return PEOPLE[i]; } }
  return "someone";
}

main :: () {
  print("Bob's lightning address\n");
  url: [256] u8; ln: [400] u8;
  nu := n57_lnurlp("bob@zaps.example.com", url[..]);
  print("bob@zaps.example.com -> %\n", cast(string) url[0..<nu]);
  nl := n57_lnurl(cast(string) url[0..<nu], ln[..]);
  lnurl := cast(string) ln[0..<nl];
  print("lnurl: %\n", lnurl);
  print("Bob@zaps.example.com -> % (upper case is not LUD-16's)\n", n57_lnurlp("Bob@zaps.example.com", url[..]));

  print("the LNURL-pay response\n");
  pay := n57_pay(PAY);
  hx: [64] u8;
  print("read: %, callback %, % to % msat\n", N57P_WHY[pay.status], cast(string) pay.callback[0..<pay.nc], pay.min, pay.max);
  print("nostrPubkey: % (%)\n", hex32(pay.nostr_pubkey, hx[..]), name_of(pay.nostr_pubkey));
  print("without allowsNostr: %\n", N57P_WHY[n57_pay("{\"tag\":\"payRequest\",\"callback\":\"https://zaps.example.com/cb\",\"minSendable\":1000,\"maxSendable\":5000,\"nostrPubkey\":\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\"}").status]);

  print("Bob's goal\n");
  gb: [1000] u8; closes := T0 + 3600;
  goal := sign(BOB, T0, 9041, "[[\"relays\",\"wss://relay.example.com\"],[\"amount\",\"50000\"],[\"closed_at\",\"1761690000\"]]", "A new microphone", gb[..]);
  g := n75_goal(goal);
  gid := hex32(goal.id, hx[..]); gidb: [64] u8; for i: 0..<64 { gidb[i] = gid[i]; } goal_id := cast(string) gidb[..];
  print("kind 9041 %: target % msat, closes at %\n", goal_id, g.msat, g.closed_at);

  print("Alice zaps the goal 21000 msat\n");
  rb: [6][1500] u8;
  alice := zap_request(ALICE, T0 + 60, 21000, goal_id, lnurl, "For the microphone", rb[0][..]);
  print("zap request %: % bytes, not published\n", hex32(alice.id, hx[..]), alice.original.count);
  cb: [4000] u8;
  nc := n57_callback(cast(string) pay.callback[0..<pay.nc], 21000, text_of(alice), lnurl, cb[..]);
  print("GET %... (% bytes)\n", cast(string) cb[0..<72], nc);
  ib: [6][600] u8;
  inv := make_invoice(21000, T0 + 61, PREIMAGE, text_of(alice), ib[0][..]);
  print("invoice: %\n", inv);

  print("the receipt, checked by hand\n");
  eb: [8][3000] u8;
  receipt := zap_receipt(DAVE, T0 + 70, alice, goal_id, inv, PREIMAGE, eb[0][..]);
  v := n57_invoice(inv);
  print("invoice amount: % msat (210n is 210 * 100 msat)\n", v.msat);
  print("request amount tag: %\n", n57_request(alice).msat);
  print("h field:            %\n", hex32(v.description_hash, hx[..]));
  print("sha256(description): %\n", hex32(sha_of(text_of(alice)), hx[..]));
  show_zap("Alice's receipt", n57_receipt(receipt, pay.nostr_pubkey, lnurl));

  print("more zaps\n");
  carol := zap_request(CAROL, T0 + 180, 30000, goal_id, lnurl, "", rb[1][..]);
  inv2 := make_invoice(30000, T0 + 181, PREIMAGE_CAROL, text_of(carol), ib[1][..]);
  erin := zap_request(ERIN, T0 + 3700, 5000, goal_id, lnurl, "Too late?", rb[2][..]);
  inv3 := make_invoice(5000, T0 + 3701, PREIMAGE_ERIN, text_of(erin), ib[2][..]);
  set: [6] NostrEvent;
  set[0] = receipt;
  set[1] = zap_receipt(DAVE, T0 + 190, carol, goal_id, inv2, PREIMAGE_CAROL, eb[1][..]);
  set[2] = zap_receipt(DAVE, T0 + 200, alice, goal_id, inv, PREIMAGE, eb[2][..]);      // Alice's payment, published again
  set[3] = zap_receipt(DAVE, T0 + 3710, erin, goal_id, inv3, PREIMAGE_ERIN, eb[3][..]);  // after closed_at
  set[4] = zap_receipt(MALLORY, T0 + 300, alice, goal_id, inv, PREIMAGE, eb[4][..]);   // not Bob's server
  show_zap("Carol's receipt", n57_receipt(set[1], pay.nostr_pubkey, lnurl));
  show_zap("Alice's again", n57_receipt(set[2], pay.nostr_pubkey, lnurl));
  show_zap("Erin's receipt", n57_receipt(set[3], pay.nostr_pubkey, lnurl));

  print("the goal's progress at closed_at + 1 hour\n");
  t := n75_tally(goal, set[0..<5], pay.nostr_pubkey, lnurl, closes + 3600);
  print("% of % msat: counted %, duplicates %, late %, refused %\n", t.msat, g.msat, t.counted, t.duplicates, t.late, t.refused);
  print("reached: %, closed: %\n", t.reached, t.closed);
  t = n75_tally(goal, set[0..<5], pay.nostr_pubkey, lnurl, T0 + 185);
  print("at T0 + 185 s: % msat, % receipts dated later not counted yet\n", t.msat, t.future);

  print("refused receipts\n");
  show_zap("signed by Mallory", n57_receipt(set[4], pay.nostr_pubkey, lnurl));
  short := make_invoice(20000, T0 + 61, PREIMAGE, text_of(alice), ib[3][..]);
  show_zap("invoice for 20000 msat", n57_receipt(zap_receipt(DAVE, T0 + 70, alice, goal_id, short, PREIMAGE, eb[5][..]), pay.nostr_pubkey, lnurl));
  // The description edited after the invoice was made: Alice's request
  // with its content changed and signed again.
  edited := zap_request(ALICE, T0 + 60, 21000, goal_id, lnurl, "For the microphone!", rb[3][..]);
  show_zap("description edited", n57_receipt(zap_receipt(DAVE, T0 + 70, edited, goal_id, inv, PREIMAGE, eb[6][..]), pay.nostr_pubkey, lnurl));
  show_zap("preimage of another payment", n57_receipt(zap_receipt(DAVE, T0 + 70, alice, goal_id, inv, PREIMAGE_CAROL, eb[7][..]), pay.nostr_pubkey, lnurl));

  print("NIP-57's example receipt\n");
  nb: [3000] u8; tags: [3000] u8;
  ex := n57_event_in(9735, n57_receipt_tags(NIP57_P, NIP57_E, NIP57_SENDER, NIP57_BOLT11, NIP57_DESCRIPTION, NIP57_PREIMAGE, tags[..]), NIP57_SIGNER, nb[..]);
  ev := n57_invoice(NIP57_BOLT11);
  print("invoice amount: % msat (10u)\n", ev.msat);
  print("h field:            %\n", hex32(ev.description_hash, hx[..]));
  print("sha256(description): %\n", hex32(sha_of(NIP57_DESCRIPTION), hx[..]));
  print("preimage hashes to p: %\n", x402_preimage_matches(NIP57_PREIMAGE, hex32(ev.payment_hash, hx[..])));
  show_zap("NIP-57's receipt", n57_receipt(ex, n57_key(NIP57_SIGNER), ""));
}

sha_of_hex :: (hex: string) -> [32] u8 { b: [32] u8; decode_hex(hex, b[..]); return sha_of(cast(string) b[..]); }

// NIP-57's example zap receipt (Appendix E), its tags as the pinned text
// gives them. It has no sig, so it is read here unsigned.
NIP57_SIGNER :: "9630f464cca6a5147aa8a35f0bcdd3ce485324e732fd39e09233b1d848238f31";
NIP57_P :: "32e1827635450ebb3c5a7d12c1f8e7b2b514439ac10a67eef3d9fd9c5c68e245";
NIP57_SENDER :: "97c70a44366a6535c145b333f973ea86dfdc2d7a99da618c40c64705ad98e322";
NIP57_E :: "3624762a1274dd9636e0c552b53086d70bc88c165bc4dc0f9e836a1eaf86c3b8";
NIP57_BOLT11 :: "lnbc10u1p3unwfusp5t9r3yymhpfqculx78u027lxspgxcr2n2987mx2j55nnfs95nxnzqpp5jmrh92pfld78spqs78v9euf2385t83uvpwk9ldrlvf6ch7tpascqhp5zvkrmemgth3tufcvflmzjzfvjt023nazlhljz2n9hattj4f8jq8qxqyjw5qcqpjrzjqtc4fc44feggv7065fqe5m4ytjarg3repr5j9el35xhmtfexc42yczarjuqqfzqqqqqqqqlgqqqqqqgq9q9qxpqysgq079nkq507a5tw7xgttmj4u990j7wfggtrasah5gd4ywfr2pjcn29383tphp4t48gquelz9z78p4cq7ml3nrrphw5w6eckhjwmhezhnqpy6gyf0";
NIP57_DESCRIPTION :: "{\"pubkey\":\"97c70a44366a6535c145b333f973ea86dfdc2d7a99da618c40c64705ad98e322\",\"content\":\"\",\"id\":\"d9cc14d50fcb8c27539aacf776882942c1a11ea4472f8cdec1dea82fab66279d\",\"created_at\":1674164539,\"sig\":\"77127f636577e9029276be060332ea565deaf89ff215a494ccff16ae3f757065e2bc59b2e8c113dd407917a010b3abd36c8d7ad84c0e3ab7dab3a0b0caa9835d\",\"kind\":9734,\"tags\":[[\"e\",\"3624762a1274dd9636e0c552b53086d70bc88c165bc4dc0f9e836a1eaf86c3b8\"],[\"p\",\"32e1827635450ebb3c5a7d12c1f8e7b2b514439ac10a67eef3d9fd9c5c68e245\"],[\"relays\",\"wss://relay.damus.io\",\"wss://nostr-relay.wlvs.space\",\"wss://nostr.fmt.wiz.biz\",\"wss://relay.nostr.bg\",\"wss://nostr.oxtr.dev\",\"wss://nostr.v0l.io\",\"wss://brb.io\",\"wss://nostr.bitcoiner.social\",\"ws://monad.jb55.com:8080\",\"wss://relay.snort.social\"]]}";
NIP57_PREIMAGE :: "5d006d2cf1e73c7148e7519a4c68adc81642ce0e25a432b2434c99f97344c15f";
```

The output, which the book's checks compare in debug and release
([fixture](../fixtures/lightning-zaps.out)):

```text
Bob's lightning address
bob@zaps.example.com -> https://zaps.example.com/.well-known/lnurlp/bob
lnurl: lnurl1dp68gurn8ghj77npwpejuetcv9khqmr99e3k7mf09emk2mrv944kummhdchkcmn4wfk8qtmzda3qcm6szy
Bob@zaps.example.com -> -1 (upper case is not LUD-16's)
the LNURL-pay response
read: ok, callback https://zaps.example.com/lnurlp/bob/callback, 1000 to 100000000 msat
nostrPubkey: 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798 (Dave)
without allowsNostr: allowsNostr is not true
Bob's goal
kind 9041 0d139a55394917832d2a910655312ec8362a01ef3a194c0243ecf679b96beb72: target 50000 msat, closes at 1761690000
Alice zaps the goal 21000 msat
zap request bb0f28e7cb5d7a648b1944ae80602e8961a7d675f1113f46bb4d01b992653b15: 678 bytes, not published
GET https://zaps.example.com/lnurlp/bob/callback?amount=21000&nostr=%7B%22id... (1019 bytes)
invoice: lnbc210n1p5szvaapp54cskcth4y3ar0qkpxhh6y7drunxuvyy5yu8462l9333qfdaxztyshp5dkd9rwjk905634vuevxp9lrc57m9yqz97uyg0lgf626zyf2kjzzqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqucmd5p
the receipt, checked by hand
invoice amount: 21000 msat (210n is 210 * 100 msat)
request amount tag: 21000
h field:            6d9a51ba562be9a8d59ccb0c12fc78a7b6520045f70887fd09d2b42225569084
sha256(description): 6d9a51ba562be9a8d59ccb0c12fc78a7b6520045f70887fd09d2b42225569084
Alice's receipt: valid, 21000 msat from Alice
more zaps
Carol's receipt: valid, 30000 msat from Carol
Alice's again: valid, 21000 msat from Alice
Erin's receipt: valid, 5000 msat from Erin
the goal's progress at closed_at + 1 hour
51000 of 50000 msat: counted 2, duplicates 1, late 1, refused 1
reached: true, closed: true
at T0 + 185 s: 21000 msat, 2 receipts dated later not counted yet
refused receipts
signed by Mallory: refused, not signed by the server's nostrPubkey
invoice for 20000 msat: refused, the invoice's amount is not the request's amount
description edited: refused, the description hash is not SHA-256 of the description
preimage of another payment: refused, the preimage does not hash to the invoice's payment hash
NIP-57's example receipt
invoice amount: 1000000 msat (10u)
h field:            132c3de7685de2be270c4ff629092c92dea8cfa2fdff212a65bf56b95527900e
sha256(description): 92968c435ae8cc21825307b119c5ac0231023795e520f9aec7108ad733da91c5
preimage hashes to p: true
NIP-57's receipt: refused, the description hash is not SHA-256 of the description
```

The goal reaches 51000 of 50000 msat: Alice's 21000 and Carol's 30000.
Alice's receipt published a second time is a duplicate, Erin's arrives 110
seconds after `closed_at`, and Mallory's is refused. At `T0 + 185`
seconds, Carol's receipt (dated `T0 + 190`) and Alice's second one
(`T0 + 200`) are not counted yet; Erin's is already late.

NIP-57's example receipt does not pass: its invoice's `h` is `132c3de7...`,
but SHA-256 of its `description` is `92968c43...`. The description's own
id does not recompute either: it states `d9cc14d5...`, and NIP-01's
serialization of it hashes to `034e151d...`. The text was edited after the
invoice was made. Its amount (10u, 1,000,000 msat) and its preimage, which
does hash to the invoice's payment hash, still hold.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/lightning-zaps.coax](../examples/checks/lightning-zaps.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- Bob's goal id, Alice's zap request (its id, its JSON byte for byte and
  its SHA-256), the invoice and the `lnurl`, from a Python script written
  from BIP-340, NIP-01, bech32 and BOLT #11 with `hashlib` only.
- BOLT #11's "Now send $24 for an entire list of things (hashed)" vector,
  whose `h` field is SHA-256 of its description of a picnic.
- NIP-57's example: its `lnurl` decodes to odell's URL, its invoice is 10u
  and its preimage matches, while its description hash and its request id
  do not, as Python computes them too.
- The hand values of the worked example.
- Done means: a receipt whose description hash does not match refuses
  (`N57_HASH`), and the goal's progress sums only the valid receipt.

<!-- source: nostr/examples/checks/lightning-zaps.coax check -->
```jai
// Values from a Python script written from BIP-340, NIP-01, bech32 and
// BOLT #11 with hashlib only, sharing no code with Coax: Bob's goal id,
// Alice's zap request (its id and the SHA-256 of its JSON), the invoice Bob's
// server makes for it, and the lnurl of Bob's address.
PY_GOAL_ID :: "0d139a55394917832d2a910655312ec8362a01ef3a194c0243ecf679b96beb72";
PY_REQUEST_ID :: "bb0f28e7cb5d7a648b1944ae80602e8961a7d675f1113f46bb4d01b992653b15";
PY_REQUEST_SHA :: "6d9a51ba562be9a8d59ccb0c12fc78a7b6520045f70887fd09d2b42225569084";
PY_INVOICE :: "lnbc210n1p5szvaapp54cskcth4y3ar0qkpxhh6y7drunxuvyy5yu8462l9333qfdaxztyshp5dkd9rwjk905634vuevxp9lrc57m9yqz97uyg0lgf626zyf2kjzzqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqucmd5p";
PY_LNURL :: "lnurl1dp68gurn8ghj77npwpejuetcv9khqmr99e3k7mf09emk2mrv944kummhdchkcmn4wfk8qtmzda3qcm6szy";
// NIP-57's example receipt, recomputed in Python: SHA-256 of its
// description, and the NIP-01 id of that description (the zap request),
// which is not the id the request states.
PY_NIP57_DESCRIPTION_SHA :: "92968c435ae8cc21825307b119c5ac0231023795e520f9aec7108ad733da91c5";
PY_NIP57_REQUEST_ID :: "034e151dc827fd9feee508bdda46d21f601b0be443f4d80a03080f88152b8d11";
// BOLT #11's "Now send $24 for an entire list of things (hashed)": h is the
// SHA-256 of this description.
BOLT11_PICNIC :: "One piece of chocolate cake, one icecream cone, one pickle, one slice of swiss cheese, one slice of salami, one lollypop, one piece of cherry pie, one sausage, one cupcake, and one slice of watermelon";

#test "Alice's request and Bob's invoice are Python's, byte for byte" #steps 2000000000 {
  hx: [64] u8; gb: [1000] u8; rb: [1500] u8; ib: [600] u8; url: [256] u8; ln: [400] u8;
  nu := n57_lnurlp("bob@zaps.example.com", url[..]);
  nl := n57_lnurl(cast(string) url[0..<nu], ln[..]);
  #assert cast(string) ln[0..<nl] == PY_LNURL;
  goal := sign(BOB, T0, 9041, "[[\"relays\",\"wss://relay.example.com\"],[\"amount\",\"50000\"],[\"closed_at\",\"1761690000\"]]", "A new microphone", gb[..]);
  #assert hex32(goal.id, hx[..]) == PY_GOAL_ID;
  alice := zap_request(ALICE, T0 + 60, 21000, PY_GOAL_ID, PY_LNURL, "For the microphone", rb[..]);
  #assert hex32(alice.id, hx[..]) == PY_REQUEST_ID && text_of(alice) == N57_T_REQUEST;
  #assert hex32(sha_of(text_of(alice)), hx[..]) == PY_REQUEST_SHA;
  #assert make_invoice(21000, T0 + 61, PREIMAGE, text_of(alice), ib[..]) == PY_INVOICE;
}

#test "BOLT #11's description-hash vector" {
  v := n57_invoice(N57_T_BOLT11);
  hx: [64] u8;
  #assert v.status == N57I_OK && hex32(v.description_hash, hx[..]) == hex32(sha_of(BOLT11_PICNIC), hx[0..<64]);
  #assert hex32(sha_of(BOLT11_PICNIC), hx[..]) == "3925b6f67e2c340036ed12093dd44e0368df1b6ea26c53dbe4811f58fd5db8c1";
}

#test "NIP-57's example: its lnurl, amount and preimage hold; its description hash does not" #steps 400000000 {
  ln: [400] u8; hx: [64] u8;
  n := n57_lnurl("https://stacker.news/.well-known/lnurlp/odell", ln[..]);
  #assert cast(string) ln[0..<n] == "lnurl1dp68gurn8ghj7um5v93kketj9ehx2amn9uh8wetvdskkkmn0wahz7mrww4excup0dajx2mrv92x9xp";
  v := n57_invoice(NIP57_BOLT11);
  #assert v.status == N57I_OK && v.msat == 1_000_000 && v.timestamp == 1674164540;
  #assert hex32(v.description_hash, hx[..]) == "132c3de7685de2be270c4ff629092c92dea8cfa2fdff212a65bf56b95527900e";
  #assert x402_preimage_matches(NIP57_PREIMAGE, hex32(v.payment_hash, hx[..]));
  #assert hex32(sha_of(NIP57_DESCRIPTION), hx[..]) == PY_NIP57_DESCRIPTION_SHA;
  // The description's own id does not recompute: it reads as NE_ID.
  out: [4000] u8; e := nostr_event_read(NIP57_DESCRIPTION, out[..], false);
  #assert e.error == NE_ID && hex32(e.computed_id, hx[..]) == PY_NIP57_REQUEST_ID;
  nb: [3000] u8; tags: [3000] u8;
  ex := n57_event_in(9735, n57_receipt_tags(NIP57_P, NIP57_E, NIP57_SENDER, NIP57_BOLT11, NIP57_DESCRIPTION, NIP57_PREIMAGE, tags[..]), NIP57_SIGNER, nb[..]);
  #assert n57_receipt(ex, n57_key(NIP57_SIGNER), "").status == N57_HASH;
}

// The hand values of the chapter's worked example.
#test "the worked example's numbers" {
  #assert x402_bolt11_msat(PY_INVOICE) == 210 * 100;          // 210n: 210 * 10^-9 BTC * 10^11 msat/BTC
  // "hp5" after the p field: h is word 23, then the length 1 * 32 + 20.
  at := x402_bolt11_sep(PY_INVOICE) + 1 + 7 + 55;
  #assert cast(string) PY_INVOICE[at..<at + 3] == "hp5";
  #assert b32_digit('h') == 23 && b32_digit('p') * 32 + b32_digit('5') == 52;
  // its first four words, d k d 9, are 13 22 13 5: 01101 10110 01101 00101,
  // so the hash starts 0110 1101 1001 1010 0101 = 6 d 9 a 5.
  #assert cast(string) PY_INVOICE[at + 3..<at + 7] == "dkd9";
  #assert b32_digit('d') == 13 && b32_digit('k') == 22 && b32_digit('9') == 5;
  w := (13 << 15) | (22 << 10) | (13 << 5) | 5;
  #assert w == 0x6d9a5 && PY_REQUEST_SHA[0..<5] == "6d9a5";
}

// Done means: a receipt whose description hash does not match refuses, and
// goal progress sums only valid receipts.
#test "Done means" #steps 4000000000 {
  hx: [64] u8; gb: [1000] u8; rb: [3][1500] u8; ib: [2][600] u8; eb: [4][3000] u8;
  goal := sign(BOB, T0, 9041, "[[\"relays\",\"wss://relay.example.com\"],[\"amount\",\"50000\"],[\"closed_at\",\"1761690000\"]]", "A new microphone", gb[..]);
  alice := zap_request(ALICE, T0 + 60, 21000, PY_GOAL_ID, PY_LNURL, "For the microphone", rb[0][..]);
  edited := zap_request(ALICE, T0 + 60, 21000, PY_GOAL_ID, PY_LNURL, "For the microphone!", rb[1][..]);
  server := key_of(DAVE);
  set: [3] NostrEvent;
  set[0] = zap_receipt(DAVE, T0 + 70, alice, PY_GOAL_ID, PY_INVOICE, PREIMAGE, eb[0][..]);
  set[1] = zap_receipt(DAVE, T0 + 80, edited, PY_GOAL_ID, PY_INVOICE, PREIMAGE, eb[1][..]);
  set[2] = zap_receipt(MALLORY, T0 + 90, alice, PY_GOAL_ID, PY_INVOICE, PREIMAGE, eb[2][..]);
  #assert n57_receipt(set[0], server, PY_LNURL).status == N57_OK;
  #assert n57_receipt(set[1], server, PY_LNURL).status == N57_HASH;
  #assert n57_receipt(set[2], server, PY_LNURL).status == N57_SIGNER;
  t := n75_tally(goal, set[..], server, PY_LNURL, T0 + 7200);
  #assert t.msat == 21000 && t.counted == 1 && t.refused == 2 && !t.reached;
}
```

### Checks in the library

Each library file has `#test` checks written before its code: lightning
addresses and their refusals, the `lnurl` of NIP-57's example, every
LNURL-pay refusal, the request writer and the callback URL at exact buffer
sizes, every Appendix D refusal with its tag, BOLT #11's vector and invoice
shapes (fields that overrun, fields not 52 words long, two `p` and two `h`
fields, no amount, testnet, a changed character), every receipt refusal,
the goal's refusals, and the tally's bounds: `closed_at` and `now`
inclusive, order independence, duplicates, and the 64-receipt bound.

```shell
coax/bin/coax test coax/lib/nostr/nip57.coax --library
coax/bin/coax test coax/lib/nostr/nip75.coax --library
coax/bin/coax test nostr/examples/checks/lightning-zaps.coax
coax/bin/coax mutate coax/lib/nostr/nip57.coax --root-only --shard I/4
coax/bin/coax mutate coax/lib/nostr/nip75.coax --root-only
```

On 2026-11-01, `coax test` passed all 233 checks of `nip57.coax` and all
238 of `nip75.coax` (with the loaded files'), and all 392 of the example's
checks. The example ran in debug and release with the same output.

Mutation testing is not finished, so the chapter is partly implemented.
`coax mutate --root-only` ran on earlier versions of `nip57.coax`: one
quarter of an 848-mutant version left 18 survivors, and two quarters of a
690-mutant version left 3 and 5. Each of those survivors has a check or a
code change since (a boundary test, or a check rewritten so it has no
equivalent mutant). Root-only mutation of the files as committed, and of
`nip75.coax`, was stopped at the task's time limit and remains to run, with
zero survivors required, before the status becomes implemented.

### Not built

- Fetching the LNURL-pay response and the invoice, and paying it.
- Verifying the invoice's signature and expiry. The receipt's server made
  the invoice, so its signature adds nothing a client can check here.
- Appendix D's rule that a `P` tag in the request equals the receipt's
  pubkey; the receipt's `P` is checked against the request's signer
  instead, as Appendix E describes it.
- Zap splits (`zap` tags, Appendix G) and goals with several
  beneficiaries, whose receipts come from several servers; the `goal` tag
  on other events.
- Private zaps, which NIP-57 leaves to future work.

## Summary

- `n57_lnurlp` turns a lightning address into its LNURL-pay URL, and
  `n57_lnurl` writes it as NIP-57's `lnurl`.
- `n57_pay` reads the LNURL-pay response and refuses a server without
  `allowsNostr` or a `nostrPubkey`.
- `n57_request_tags` and `n57_callback` write a zap request and the URL
  that sends it; `n57_request` reads one as Appendix D says.
- `n57_invoice` decodes a bolt11 invoice to its amount, timestamp, payment
  hash and description hash, with the checksum checked.
- `n57_receipt` refuses a receipt not signed by `nostrPubkey`, whose
  description hash is not SHA-256 of its description, whose amount is not
  the request's, or whose tags disagree with the request.
- `n75_tally` sums the valid receipts to a goal, once per payment, none
  after `closed_at` or `now`.

## Dependencies

- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [45. Profiles, follows and names](ch045-profiles-follows-and-names.md)

## Sources

- [NIP-57](../nips/official/57.md)
- [NIP-75](../nips/official/75.md)
- BOLT #11, the invoice format and its test vectors:
  <https://github.com/lightning/bolts/blob/master/11-payment-encoding.md>.
- LUD-06 and LUD-16, LNURL-pay and lightning addresses:
  <https://github.com/lnurl/luds>.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
