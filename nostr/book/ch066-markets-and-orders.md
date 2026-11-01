# 66. Markets and orders

Listings and orders are signed offers. A listing says what a seller offers
and at what price. An order says what a buyer wants. This chapter reads
both under the repository's Bitcoin-only rule: any price is shown as it is
written, but only a price in BTC or sats becomes an amount you pay, and
only Bitcoin rails are ever offered or chosen for payment.

Part X, Every NIP. Result: NIP-99 classified listings (30402 and 30403
drafts) with `price` in a currency and frequency, NIP-69 peer-to-peer order
events (38383), and the unrecommended NIP-15 marketplace: stalls (30017),
products (30018), its three checkout messages and an order's total in sats.

## What this chapter teaches

- NIP-99: `title`, `summary`, `published_at`, `location`, `price` `[amount,
  currency, frequency]` and `status`, and how a decimal price in BTC or sats
  becomes an exact number of satoshis.
- NIP-69: kind 38383 with `d`, `k` `buy`/`sell`, `f` fiat code, `s` status,
  `amt`, `fa`, `pm`, `premium`, `network` and `layer`, and the rule that
  decides whether this stack may settle one.
- NIP-15 (stalls, products, checkout) is `unrecommended`, and this chapter
  builds only what a Bitcoin buyer needs from it: stalls and products, the
  new order (type 0), the payment request (type 1) and the status update
  (type 2), and an order's total.
- Why nothing here converts currencies: there is no exchange rate in this
  stack, so a price in USD stays a price in USD.
- Settlement in this stack is Lightning or on-chain Bitcoin (chapters 71
  and 86).

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_tag(e, i)` | Reads a tag of any name: the name and its first two values (chapter 55) |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag(e, i)`, `nostr_tag_d(e)` | Reads and checks an `a`, `e`, `p` or `d` tag (chapter 22) |
| [nip99.coax](../../coax/lib/nostr/nip99.coax) | `n99_decimal(s)`, `n99_unit(t)`, `n99_sats(d, unit)` | A decimal string, a currency code, and the amount in sats when the code is BTC or sats |
| [nip99.coax](../../coax/lib/nostr/nip99.coax) | `n99_price(e, i)` | A `price` tag: amount, currency, frequency and sats |
| [nip99.coax](../../coax/lib/nostr/nip99.coax) | `n99_listing(e)`, `n99_due(l)` | A classified listing, and what a buyer could be asked to pay for it |
| [nip69.coax](../../coax/lib/nostr/nip69.coax) | `n69_order(e)`, `n69_settle(o)` | A peer-to-peer order, and whether this stack may settle it |
| [nip15.coax](../../coax/lib/nostr/nip15.coax) | `n15_stall(e, buf)`, `n15_product(e, buf)` | A stall or product event: its JSON content, with the `d` tag equal to the id |
| [nip15.coax](../../coax/lib/nostr/nip15.coax) | `n15_type`, `n15_order_parse`, `n15_request_parse`, `n15_update_parse` | The three checkout messages read |
| [nip15.coax](../../coax/lib/nostr/nip15.coax) | `n15_write_order`, `n15_write_request`, `n15_write_update` | The three checkout messages written |
| [nip15.coax](../../coax/lib/nostr/nip15.coax) | `n15_choice(r)`, `n15_total(s, products, o)` | The first Bitcoin payment option, and an order's total in sats |

The three library files are new with this chapter. Each is an `#authority`
file, because each decides money: what a listing costs in sats, whether an
order may be settled, which payment option is used and what an order
totals. Every procedure runs in a check, and `coax mutate` leaves no
survivors. No file fetches an event or verifies a signature. The caller
verifies the events (chapter 21) and passes them in.

## The records

Each reader returns a record with a status and what it read. `N32Text` is
chapter 55's decoded string of at most 256 bytes.

<!-- source: coax/lib/nostr/nip99.coax record -->
```jai
// A decimal string: whole.frac, with frac_digits digits after the point.
N99Decimal :: struct {ok:bool; whole:int; frac:int; frac_digits:int;}
N99Price :: struct {
  status:int=N99_PRICE;
  amount:N32Text; currency:N32Text;   // as written
  frequency:N32Text;                  // empty: a one-time price
  decimal:N99Decimal; unit:int;       // N99_UNIT_BTC, _SAT or _OTHER
  sats:int=-1;                        // BTC or sats only, else -1
}
N99Listing :: struct {
  status:int=N99_EVENT; at:int=-1;    // the refusing tag
  draft:bool;                         // kind 30403
  d:N32Text; title:N32Text; summary:N32Text; location:N32Text;
  published_at:int=-1;
  prices:int; price:N99Price;         // the first price tag
  state:int;                          // N99_STATE_*
  topics:int; images:int;             // t and image tags
}
N99Due :: struct {status:int=N99_DUE_LISTING; sats:int=-1;}
```

<!-- source: coax/lib/nostr/nip69.coax record -->
```jai
N69Order :: struct {
  status:int=N69_EVENT; at:int=-1;    // the refusing tag
  missing:int=-1;                     // the first required tag absent, in N69_REQUIRED
  d:N32Text; side:int; fiat:N32Text; state:int;
  sats:int;                           // amt; 0: from an exchange rate
  fa:[2]N32Text; range:bool;          // the fiat amount, or a minimum and maximum
  methods:int; method:N32Text;        // pm: how many, and the first
  premium:N32Text; network:N32Text;
  layer:int; layer_name:N32Text;
  expires_at:int=-1; expiration:int=-1;
}
```

<!-- source: coax/lib/nostr/nip15.coax record -->
```jai
N15Zone :: struct {id:N32Text; cost:N32Text; decimal:N99Decimal;}   // cost as written
N15Stall :: struct {
  status:int=N15_EVENT;
  id:N32Text; name:N32Text; currency:N32Text; unit:int;
  zones:[N15_MAX_ZONES]N15Zone; zones_n:int;          // shipping: base cost per zone
}
N15Product :: struct {
  status:int=N15_EVENT;
  id:N32Text; stall_id:N32Text; name:N32Text; currency:N32Text; unit:int;
  price:N32Text; decimal:N99Decimal;
  quantity:int=-1;                                    // -1: null, unlimited
  extras:[N15_MAX_ZONES]N15Zone; extras_n:int;        // extra cost per unit, per zone
}
N15Item :: struct {product_id:N32Text; quantity:int;}
N15Order :: struct {
  status:int=N15_EVENT;
  id:N32Text; nostr:N32Text; address:N32Text;         // nostr: contact key, hex
  items:[N15_MAX_ITEMS]N15Item; items_n:int;
  shipping_id:N32Text;
}
N15Option :: struct {type:N32Text; rail:int; from:int; to:int;}   // link: raw bytes in the text
N15Request :: struct {
  status:int=N15_EVENT; id:N32Text;
  options:[N15_MAX_OPTIONS]N15Option; options_n:int;
  bitcoin:int;                                        // options with a Bitcoin rail
}
N15Update :: struct {status:int=N15_EVENT; id:N32Text; paid:bool; shipped:bool;}
N15Total :: struct {
  status:int=N15_TOTAL_INPUT; at:int=-1;              // the refusing item
  goods:int; shipping:int; sats:int=-1;
}
```

## Prices

NIP-99 writes a price as a tag:
`["price", "<number>", "<currency>", "<frequency>"]`. The amount is a
number in a string. The currency is a three-letter ISO 4217 code or a code
like it (`btc`, `eth`). The frequency is optional and names a period for a
recurring payment (`month`, `year`). The NIP gives three examples:

- $50 one time: `["price", "50", "USD"]`
- 15 EUR per month: `["price", "15", "EUR", "month"]`
- 50,000 GBP per year: `["price", "50000", "GBP", "year"]`

The code reads every one of them, and these rules decide the rest:

- The amount is a decimal string: one to 16 digits, then optionally a
  point and one to 16 more. `50,000`, `-5`, `1e3`, `.5`, `5.` and a space
  are refused. A comma could mean a thousands separator or a decimal
  point, so it is never guessed.
- The currency is three or four ASCII letters, in any case. `BTC` is
  bitcoin; `SAT` and `SATS` are satoshis. Every other code (`USD`, `EUR`,
  `eth`, `USDT`) is data: it is shown and never converted.
- A BTC amount becomes sats by moving the point eight places: at most
  eight decimals, so `0.000000001 BTC` (a tenth of a sat) is refused. A
  sats amount must be whole. No amount is over 21 million bitcoin
  (2,100,000,000,000,000 sats).
- A frequency, when present, is not empty.

<!-- source: coax/lib/nostr/nip99.coax price -->
```jai
// A decimal string: one to 16 digits, then optionally a point and one to
// 16 more. Anything else is refused.
n99_decimal :: (s:string)->N99Decimal {
  d:N99Decimal;i:=0;
  while i<s.count && n99_digit(s[i]) {
    if i==N99_MAX_DIGITS {return d;}
    d.whole=d.whole*10+cast(int)(s[i]-'0');i+=1;
  }
  if i==0 {return d;}
  if i<s.count {
    if s[i]!='.' {return d;}
    i+=1;
    while i<s.count && n99_digit(s[i]) {
      if d.frac_digits==N99_MAX_DIGITS {return d;}
      d.frac=d.frac*10+cast(int)(s[i]-'0');d.frac_digits+=1;i+=1;
    }
    if d.frac_digits==0 || i<s.count {return d;}
  }
  d.ok=true;return d;
}
// A currency code: three or four ASCII letters, any case. BTC and SAT or
// SATS are bitcoin; any other code is shown as data.
n99_unit :: (t:N32Text)->int {
  own:=t;lower:[4]u8;
  if own.n<3 || own.n>4 {return N99_UNIT_BAD;}
  for i:0..<own.n {
    c:=own.b[i]|32;   // a letter in lower case
    if c<'a' || c>'z' {return N99_UNIT_BAD;}
    lower[i]=c;
  }
  s:=cast(string)lower[0..<own.n];
  if s=="btc" {return N99_UNIT_BTC;}
  if s=="sat" || s=="sats" {return N99_UNIT_SAT;}
  return N99_UNIT_OTHER;
}
// The amount in satoshis, or -1: not bitcoin, not whole sats, or over 21
// million bitcoin.
n99_sats :: (d:N99Decimal,unit:int)->int {
  if !d.ok {return -1;}
  if unit==N99_UNIT_SAT {
    if d.frac!=0 || d.whole>N99_MAX_SATS {return -1;}
    return d.whole;
  }
  if unit!=N99_UNIT_BTC || d.frac_digits>8 || d.whole>N99_MAX_BTC {return -1;}
  f:=d.frac;for k:d.frac_digits..<8 {f*=10;}
  if d.whole==N99_MAX_BTC && f>0 {return -1;}
  return d.whole*N99_SATS_PER_BTC+f;
}
// The k'th value of the index'th tag (0 is its name).
n99_value :: (e:NostrEvent,index:int,k:int)->N99Value {
  r:N99Value;j:[1]Json;if !nt_start(j[..],e) {return r;}
  i:=0;tok:=json_next(j[..]);
  while tok==JSON_ARRAY && i<index {skipped:=json_skip(j[..]);i+=1;tok=json_next(j[..]);}
  if index<0 || tok==JSON_ARRAY_END {r.error=NT_INDEX;return r;}
  if tok!=JSON_ARRAY {return r;}
  at:=0;v:=json_next(j[..]);
  while v==JSON_STRING {
    if at==k {
      n:=json_unescape(json_text(j[..]),r.v.b[..]);
      if n<0 {r.error=NT_LIMIT;return r;}
      r.v.n=n;r.error=NT_OK;return r;
    }
    at+=1;v=json_next(j[..]);
  }
  if v==JSON_ARRAY_END {r.error=NT_INDEX;}
  return r;
}
// The index'th tag as a price: ["price", amount, currency] or with a
// fourth value, the frequency. A bitcoin price must be whole sats.
n99_price :: (e:NostrEvent,index:int)->N99Price {
  p:N99Price;t:=n32_tag(e,index);
  if t.error!=NT_OK || !n32_is(t.v[0],"price") || t.values<3 || t.values>4 {return p;}
  p.amount=t.v[1];p.currency=t.v[2];
  if t.values==4 {
    f:=n99_value(e,index,3);
    if f.error!=NT_OK || f.v.n==0 {return p;}
    p.frequency=f.v;
  }
  a:=p.amount;p.decimal=n99_decimal(cast(string)a.b[0..<a.n]);p.unit=n99_unit(p.currency);
  if !p.decimal.ok || p.unit==N99_UNIT_BAD {return p;}
  p.sats=n99_sats(p.decimal,p.unit);
  if p.unit!=N99_UNIT_OTHER && p.sats<0 {return p;}
  p.status=N99_OK;return p;
}
```

`n32_tag` reads a tag's name and its first two values, so `n99_value`
reads the fourth, the frequency.

### Worked example: three prices in sats

1. `["price", "0.0001", "BTC"]`. The whole part is 0 and the fraction is 1
   with 4 digits. Padded to 8 digits, the fraction is 1 followed by 4
   zeros, 10000. Sats: 0 x 100000000 + 10000 = 10000.
2. `["price", "250000", "sat"]`. A whole number of sats: 250000.
3. `["price", "15", "EUR", "month"]`. EUR is not bitcoin, so sats stay -1.
   The price reads and is shown as 15 EUR per month.

## Listings

A listing is kind 30402, or 30403 for a draft or an inactive listing. It
is addressable by its `d` tag, which must be present and not empty. The
first `title`, `summary`, `location`, `status` and `published_at` tags are
read; later ones are ignored. Every `price` tag must read, and the first is
the listing's price. `published_at` is whole Unix seconds. `status` is
`active`, `sold` or another word. `e`, `p` and `a` tags must be valid
references.

<!-- source: coax/lib/nostr/nip99.coax listing -->
```jai
// A classified listing. The first d, title, summary, location, status,
// published_at and price tags are read; later ones are counted or ignored.
n99_listing :: (e:NostrEvent)->N99Listing {
  l:N99Listing;if e.error!=NE_OK {return l;}
  if e.kind!=N99_KIND_LISTING && e.kind!=N99_KIND_DRAFT {return n99_refuse(l,N99_KIND,-1);}
  l.draft=e.kind==N99_KIND_DRAFT;
  seen:[6]bool;   // d, title, summary, location, status, published_at
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n99_refuse(l,N99_LIMIT,i);}
    if t.error!=NT_OK {return n99_refuse(l,N99_EVENT,i);}
    name:=t.v[0];v:=t.v[1];
    if n32_is(name,"d") && !seen[0] {seen[0]=true;l.d=v;}
    if n32_is(name,"title") && !seen[1] {seen[1]=true;l.title=v;}
    if n32_is(name,"summary") && !seen[2] {seen[2]=true;l.summary=v;}
    if n32_is(name,"location") && !seen[3] {seen[3]=true;l.location=v;}
    if n32_is(name,"status") && !seen[4] {
      seen[4]=true;
      l.state=ifx n32_is(v,"active") then N99_STATE_ACTIVE else ifx n32_is(v,"sold") then N99_STATE_SOLD else N99_STATE_OTHER;
    }
    if n32_is(name,"published_at") && !seen[5] {
      seen[5]=true;d:=n99_decimal(cast(string)v.b[0..<v.n]);
      if !d.ok || d.frac_digits>0 {return n99_refuse(l,N99_TIME,i);}
      l.published_at=d.whole;
    }
    if n32_is(name,"price") {
      p:=n99_price(e,i);if p.status!=N99_OK {return n99_refuse(l,N99_PRICE,i);}
      if l.prices==0 {l.price=p;}
      l.prices+=1;
    }
    if n32_is(name,"t") {l.topics+=1;}
    if n32_is(name,"image") {l.images+=1;}
    if n32_is(name,"e") || n32_is(name,"p") || n32_is(name,"a") {
      if nostr_tag(e,i).error!=NT_OK {return n99_refuse(l,N99_TAG,i);}
    }
    i+=1;
  }
  if l.d.n==0 {return n99_refuse(l,N99_MISSING,-1);}
  l.status=N99_OK;return l;
}
```

What a buyer could be asked to pay is a separate decision. A draft is not
for sale and a sold listing is not either. A listing without a price has
nothing to pay. A price in another currency is shown and never converted.
A recurring price is shown, but recurring payments are not built here, so
it is not settled.

<!-- source: coax/lib/nostr/nip99.coax due -->
```jai
// What a buyer could be asked to pay for a listing, in sats. A price in
// another currency is shown as written and never converted; a recurring
// price is shown and not settled here.
n99_due :: (l:N99Listing)->N99Due {
  d:N99Due;
  if l.status!=N99_OK {return d;}
  d.status=N99_DUE_DRAFT;if l.draft {return d;}
  d.status=N99_DUE_SOLD;if l.state==N99_STATE_SOLD {return d;}
  d.status=N99_DUE_NO_PRICE;if l.prices==0 {return d;}
  d.status=N99_DUE_RATE;if l.price.unit==N99_UNIT_OTHER {return d;}
  d.status=N99_DUE_RECURRING;if l.price.frequency.n>0 {return d;}
  d.status=N99_DUE_OK;d.sats=l.price.sats;return d;
}
```

## Peer-to-peer orders

A NIP-69 order is kind 38383: an offer to buy or sell bitcoin for a fiat
amount, published by a peer-to-peer platform. Its tags say which side
(`k`), which fiat currency (`f`), its status (`s`), the amount in sats
(`amt`), the fiat amount or a range (`fa`), the payment methods (`pm`), the
premium, the network and the layer the bitcoin moves on.

The reader requires `d`, `k`, `f`, `s`, `amt`, `fa`, `pm`, `premium`,
`network` and `layer`. The NIP also marks `expires_at`, `expiration`, `y`
and `z` as mandatory. They are read when present, not required, so an
order from a platform that leaves one out is still shown. Each required
value must be non-empty, `k` is `buy` or `sell`, `s` is one of the five
statuses, `amt` is whole sats, `fa` is one or two decimals, and `premium`
is a decimal with an optional minus sign.

<!-- source: coax/lib/nostr/nip69.coax order -->
```jai
// A peer-to-peer order. The first of each tag is read; later ones are
// ignored.
n69_order :: (e:NostrEvent)->N69Order {
  o:N69Order;if e.error!=NE_OK {return o;}
  if e.kind!=N69_KIND_ORDER {return n69_refuse(o,N69_KIND,-1);}
  seen:[12]bool;   // the required tags, then expires_at and expiration
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n69_refuse(o,N69_LIMIT,i);}
    if t.error!=NT_OK {return n69_refuse(o,N69_EVENT,i);}
    name:=t.v[0];v:=t.v[1];which:=-1;
    for k:0..<10 {if n32_is(name,N69_REQUIRED[k]) {which=k;}}
    if n32_is(name,"expires_at") {which=10;}
    if n32_is(name,"expiration") {which=11;}
    if which>=0 && !seen[which] {
      seen[which]=true;
      if v.n==0 {return n69_refuse(o,N69_VALUE,i);}
      ok:=true;
      if which==0 {o.d=v;}
      if which==1 {ok=n32_is(v,"buy") || n32_is(v,"sell");o.side=ifx n32_is(v,"sell") then N69_SELL else N69_BUY;}
      if which==2 {ok=n99_unit(v)!=N99_UNIT_BAD;o.fiat=v;}
      if which==3 {
        ok=false;
        for k:0..<5 {if n32_is(v,N69_STATES[k]) {ok=true;o.state=k;}}
      }
      if which==4 {o.sats=n69_whole(v);ok=o.sats>=0 && o.sats<=N99_MAX_SATS;}
      if which==5 {
        o.range=t.values==3;o.fa[0]=v;o.fa[1]=t.v[2];
        a:=n99_decimal(cast(string)v.b[0..<v.n]);b:=n99_decimal(cast(string)o.fa[1].b[0..<o.fa[1].n]);
        ok=t.values<4 && a.ok && (!o.range || b.ok);
      }
      if which==6 {o.methods=t.values-1;o.method=v;}
      if which==7 {
        o.premium=v;s:=1;if v.b[0]=='-' {s=0;}
        ok=n99_decimal(cast(string)v.b[1-s..<v.n]).ok;
      }
      if which==8 {o.network=v;}
      if which==9 {
        o.layer_name=v;
        o.layer=ifx n32_is(v,"onchain") then N69_LAYER_ONCHAIN else ifx n32_is(v,"lightning") then N69_LAYER_LIGHTNING else ifx n32_is(v,"spark") then N69_LAYER_SPARK else N69_LAYER_OTHER;
      }
      if which==10 {o.expires_at=n69_whole(v);ok=o.expires_at>=0;}
      if which==11 {o.expiration=n69_whole(v);ok=o.expiration>=0;}
      if !ok {return n69_refuse(o,N69_VALUE,i);}
    }
    i+=1;
  }
  for k:0..<10 {if !seen[k] {o.missing=k;return n69_refuse(o,N69_MISSING,-1);}}
  o.status=N69_OK;return o;
}
```

### Settling

An order reads whatever its layer. Settling is a different question, and
the layer is asked first. Only `onchain` (Bitcoin's own chain),
`lightning` and `spark` are Bitcoin layers here. An order on `liquid` or
any other layer is shown and never settled. After that, the network must
be `mainnet`, the order must be `pending`, and `amt` must not be 0. NIP-69
says an `amt` of 0 means the sats come from a public exchange rate when
the taker accepts. This stack has no exchange rate, so it does not settle
such an order.

<!-- source: coax/lib/nostr/nip69.coax settle -->
```jai
// Whether this stack may settle an order: on Bitcoin's chain, Lightning or
// Spark, on mainnet, still pending, with its amount in sats. The layer is
// checked first: an order on any other layer is shown and never settled.
n69_settle :: (o:N69Order)->int {
  if o.status!=N69_OK {return N69_SETTLE_ORDER;}
  if o.layer==N69_LAYER_OTHER {return N69_SETTLE_LAYER;}
  if !n32_is(o.network,"mainnet") {return N69_SETTLE_NETWORK;}
  if o.state!=N69_PENDING {return N69_SETTLE_STATUS;}
  if o.sats==0 {return N69_SETTLE_RATE;}
  return N69_SETTLE_OK;
}
```

NIP-69's own example sells for 100 VES on Lightning with `amt` 0. It
reads, and `n69_settle` returns `N69_SETTLE_RATE`.

## The NIP-15 marketplace

NIP-15 is marked `unrecommended`: "too complicated, try NIP-99 instead".
Its stall and product events and its checkout messages are still in use,
so this chapter reads them and builds only the Bitcoin part.

### Stalls and products

A stall (kind 30017) and a product (kind 30018) carry a JSON object in
their content. A stall has an `id`, a `name`, a `currency` and shipping
zones, each with an `id` and a base `cost`. A product has an `id`, the
`stall_id`, a `name`, a `currency`, a `price`, a `quantity` (`null` for
unlimited) and optional extra shipping costs per zone. The event's `d` tag
must equal the content's `id`.

The content is admitted by the shared JSON policy first (chapter 17), which
refuses a duplicate key anywhere. A product with two `price` keys is
refused rather than read either way. Prices and costs are JSON numbers;
they must be plain decimals, so a sign or an exponent is refused.

<!-- source: coax/lib/nostr/nip15.coax stall -->
```jai
// A stall's content: id, name and currency, and its shipping zones.
n15_stall_parse :: (text:string)->N15Stall {
  s:N15Stall;j:[1]Json;st:=n15_start(j[..],text);if st!=N15_OK {s.status=st;return s;}
  has:[3]bool;
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);
    w:=ifx json_is(key,"id") then 0 else ifx json_is(key,"name") then 1 else ifx json_is(key,"currency") then 2 else -1;
    if w>=0 {
      r:=n15_string(j[..],v);if r.error!=NT_OK {s.status=n15_err(r);return s;}
      has[w]=true;   // the id is checked by its length
      if w==0 {s.id=r.v;} else if w==1 {s.name=r.v;} else {s.currency=r.v;}
    } else if json_is(key,"shipping") {
      c:=n15_zones(j[..],v,s.zones[..]);if c.status!=N15_OK {s.status=c.status;return s;}
      s.zones_n=c.n;
    } else {skipped:=json_skip(j[..]);}
    t=json_next(j[..]);
  }
  if !has[1] || !has[2] || s.id.n==0 {s.status=N15_MISSING;return s;}
  s.unit=n99_unit(s.currency);if s.unit==N99_UNIT_BAD {s.status=N15_FIELD;return s;}
  s.status=N15_OK;return s;
}
// A kind 30017 event: its content, with the d tag the same as the id.
n15_stall :: (e:NostrEvent,buf:[]u8)->N15Stall {
  s:N15Stall;n:=n15_content(e,buf);if n== -1 {return s;}
  if e.kind!=N15_KIND_STALL {s.status=N15_KIND;return s;}
  if n<0 {s.status=N15_LIMIT;return s;}
  s=n15_stall_parse(cast(string)buf[0..<n]);
  if s.status==N15_OK && !n15_d_is(e,s.id) {s.status=N15_ID;}
  return s;
}
```

<!-- source: coax/lib/nostr/nip15.coax product -->
```jai
// A product's content: id, stall, name, currency, price, quantity (null:
// unlimited) and extra shipping costs per zone.
n15_product_parse :: (text:string)->N15Product {
  p:N15Product;j:[1]Json;st:=n15_start(j[..],text);if st!=N15_OK {p.status=st;return p;}
  has:[5]bool;
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);
    w:=ifx json_is(key,"id") then 0 else ifx json_is(key,"stall_id") then 1 else ifx json_is(key,"name") then 2 else ifx json_is(key,"currency") then 3 else -1;
    if w>=0 {
      r:=n15_string(j[..],v);if r.error!=NT_OK {p.status=n15_err(r);return p;}
      has[w]=true;
      if w==0 {p.id=r.v;} else if w==1 {p.stall_id=r.v;} else if w==2 {p.name=r.v;} else {p.currency=r.v;}
    } else if json_is(key,"price") {
      p.decimal=n15_number(j[..],v);if !p.decimal.ok {p.status=N15_FIELD;return p;}
      p.price=n99_text(json_text(j[..]));has[4]=true;
    } else if json_is(key,"quantity") {
      if v!=JSON_NULL {p.quantity=n15_int(j[..],v);if p.quantity<0 {p.status=N15_FIELD;return p;}}
    } else if json_is(key,"shipping") {
      c:=n15_zones(j[..],v,p.extras[..]);if c.status!=N15_OK {p.status=c.status;return p;}
      p.extras_n=c.n;
    } else {skipped:=json_skip(j[..]);}
    t=json_next(j[..]);
  }
  // has[0] is never read: an absent id and an empty one are both length 0.
  for k:1..<5 {if !has[k] {p.status=N15_MISSING;return p;}}
  if p.id.n==0 {p.status=N15_MISSING;return p;}
  p.unit=n99_unit(p.currency);if p.unit==N99_UNIT_BAD {p.status=N15_FIELD;return p;}
  p.status=N15_OK;return p;
}
// A kind 30018 event: its content, with the d tag the same as the id.
n15_product :: (e:NostrEvent,buf:[]u8)->N15Product {
  p:N15Product;n:=n15_content(e,buf);if n== -1 {return p;}
  if e.kind!=N15_KIND_PRODUCT {p.status=N15_KIND;return p;}
  if n<0 {p.status=N15_LIMIT;return p;}
  p=n15_product_parse(cast(string)buf[0..<n]);
  if p.status==N15_OK && !n15_d_is(e,p.id) {p.status=N15_ID;}
  return p;
}
```

### Checkout messages

The customer and the merchant exchange JSON messages in encrypted direct
messages. NIP-15 says NIP-04 (chapter 19), which is retired; gift wrap
(chapter 25) is the better envelope.
The code reads and writes the plaintext. Each message has a `type`:

| Type | Sent by | Message |
| --- | --- | --- |
| 0 | Customer | New order: `id`, `contact.nostr`, `address`, `items` (each `product_id` and `quantity`), `shipping_id` |
| 1 | Merchant | Payment request: `id`, `message`, `payment_options` (each `type` and `link`) |
| 2 | Merchant | Status update: `id`, `message`, `paid`, `shipped` |

A new order needs at least one item, every quantity at least 1, and each
product named once. A payment request needs at least one option. NIP-15
lists four option types: `url` (a payment page: Stripe, PayPal or
BTCPay Server), `btc` (an on-chain address), `ln` (a Lightning invoice)
and `lnurl` (LNURL-pay). Only `btc`, `ln` and `lnurl` are Bitcoin rails.
A `url` page could take any rail, so it is refused, and so is any other
type, such as `cashu`. The reader keeps every option and marks its rail.
`n15_choice` returns the first option on a Bitcoin rail, and the writer
refuses to write any other.

<!-- source: coax/lib/nostr/nip15.coax checkout -->
```jai
// A checkout message's type: 0, 1 or 2, or -1.
n15_type :: (text:string)->int {
  j:[1]Json;if n15_start(j[..],text)!=N15_OK {return -1;}
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);
    if json_is(key,"type") {
      k:=n15_int(j[..],v);
      return ifx k<=N15_STATUS_UPDATE then k else -1;
    }
    skipped:=json_skip(j[..]);t=json_next(j[..]);
  }
  return -1;
}
// A customer's new order (type 0): id, contact key, address, items and the
// shipping zone. A product named twice is refused.
n15_order_parse :: (text:string)->N15Order {
  o:N15Order;j:[1]Json;st:=n15_start(j[..],text);if st!=N15_OK {o.status=st;return o;}
  typed:=false;
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);
    w:=ifx json_is(key,"id") then 0 else ifx json_is(key,"address") then 1 else ifx json_is(key,"shipping_id") then 2 else -1;
    if w>=0 {
      r:=n15_string(j[..],v);if r.error!=NT_OK {o.status=n15_err(r);return o;}
      if w==0 {o.id=r.v;} else if w==1 {o.address=r.v;} else if w==2 {o.shipping_id=r.v;}
    } else if json_is(key,"type") {
      typed=n15_int(j[..],v)==N15_NEW_ORDER;
    } else if json_is(key,"contact") {
      if v!=JSON_OBJECT {o.status=N15_FIELD;return o;}
      c:=json_next(j[..]);
      while c==JSON_STRING {
        ckey:=json_text(j[..]);cv:=json_next(j[..]);
        if json_is(ckey,"nostr") {
          r:=n15_string(j[..],cv);key32:[32]u8;
          if r.error!=NT_OK || !ne_lower_hex(cast(string)r.v.b[0..<r.v.n],key32[..]) {o.status=N15_FIELD;return o;}
          o.nostr=r.v;
        } else {skipped:=json_skip(j[..]);}
        c=json_next(j[..]);
      }
    } else if json_is(key,"items") {
      if v!=JSON_ARRAY {o.status=N15_FIELD;return o;}
      el:=json_next(j[..]);
      while el!=JSON_ARRAY_END {
        if el!=JSON_OBJECT {o.status=N15_FIELD;return o;}
        if o.items_n==N15_MAX_ITEMS {o.status=N15_LIMIT;return o;}
        item:N15Item;
        f:=json_next(j[..]);
        while f==JSON_STRING {
          ikey:=json_text(j[..]);iv:=json_next(j[..]);
          if json_is(ikey,"product_id") {
            r:=n15_string(j[..],iv);if r.error!=NT_OK {o.status=n15_err(r);return o;}
            item.product_id=r.v;
          } else if json_is(ikey,"quantity") {
            item.quantity=n15_int(j[..],iv);if item.quantity<1 {o.status=N15_FIELD;return o;}
          } else {skipped:=json_skip(j[..]);}
          f=json_next(j[..]);
        }
        if item.product_id.n==0 || item.quantity==0 {o.status=N15_MISSING;return o;}
        for k:0..<o.items_n {if n15_same(o.items[k].product_id,item.product_id) {o.status=N15_DUPLICATE;return o;}}
        o.items[o.items_n]=item;o.items_n+=1;el=json_next(j[..]);
      }
    } else {skipped:=json_skip(j[..]);}
    t=json_next(j[..]);
  }
  if !typed {o.status=N15_TYPE;return o;}
  if o.id.n==0 || o.items_n==0 || o.shipping_id.n==0 {o.status=N15_MISSING;return o;}
  o.status=N15_OK;return o;
}
// A merchant's payment request (type 1): the order id and its payment
// options, each marked with its rail. Options on another rail are kept
// and never chosen.
n15_rail :: (type:string)->int {
  if type=="btc" {return N15_RAIL_ONCHAIN;}
  if type=="ln" {return N15_RAIL_LIGHTNING;}
  if type=="lnurl" {return N15_RAIL_LNURL;}
  return N15_RAIL_NONE;
}
n15_request_parse :: (text:string)->N15Request {
  r:N15Request;j:[1]Json;st:=n15_start(j[..],text);if st!=N15_OK {r.status=st;return r;}
  typed:=false;
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);
    if json_is(key,"id") {
      s:=n15_string(j[..],v);if s.error!=NT_OK {r.status=n15_err(s);return r;}
      r.id=s.v;
    } else if json_is(key,"type") {
      typed=n15_int(j[..],v)==N15_PAYMENT_REQUEST;
    } else if json_is(key,"payment_options") {
      if v!=JSON_ARRAY {r.status=N15_FIELD;return r;}
      el:=json_next(j[..]);
      while el!=JSON_ARRAY_END {
        if el!=JSON_OBJECT {r.status=N15_FIELD;return r;}
        if r.options_n==N15_MAX_OPTIONS {r.status=N15_LIMIT;return r;}
        op:N15Option;has_link:=false;
        f:=json_next(j[..]);
        while f==JSON_STRING {
          okey:=json_text(j[..]);ov:=json_next(j[..]);
          if json_is(okey,"type") {
            s:=n15_string(j[..],ov);if s.error!=NT_OK {r.status=n15_err(s);return r;}
            op.type=s.v;
          } else if json_is(okey,"link") {
            if ov!=JSON_STRING {r.status=N15_FIELD;return r;}
            op.from=j[0].from;op.to=j[0].to;has_link=true;
          } else {skipped:=json_skip(j[..]);}
          f=json_next(j[..]);
        }
        if op.type.n==0 || !has_link {r.status=N15_MISSING;return r;}
        op.rail=n15_rail(cast(string)op.type.b[0..<op.type.n]);
        if op.rail!=N15_RAIL_NONE {r.bitcoin+=1;}
        r.options[r.options_n]=op;r.options_n+=1;el=json_next(j[..]);
      }
    } else {skipped:=json_skip(j[..]);}
    t=json_next(j[..]);
  }
  if !typed {r.status=N15_TYPE;return r;}
  if r.id.n==0 || r.options_n==0 {r.status=N15_MISSING;return r;}
  r.status=N15_OK;return r;
}
// The first option on a Bitcoin rail, or -1.
n15_choice :: (r:N15Request)->int {
  if r.status!=N15_OK {return -1;}
  for k:0..<r.options_n {if r.options[k].rail!=N15_RAIL_NONE {return k;}}
  return -1;
}
// A merchant's status update (type 2): paid and shipped.
n15_update_parse :: (text:string)->N15Update {
  u:N15Update;j:[1]Json;st:=n15_start(j[..],text);if st!=N15_OK {u.status=st;return u;}
  typed:=false;has:[2]bool;
  t:=json_next(j[..]);
  while t==JSON_STRING {
    key:=json_text(j[..]);v:=json_next(j[..]);
    w:=ifx json_is(key,"paid") then 0 else ifx json_is(key,"shipped") then 1 else -1;
    if json_is(key,"id") {
      s:=n15_string(j[..],v);if s.error!=NT_OK {u.status=n15_err(s);return u;}
      u.id=s.v;
    } else if json_is(key,"type") {
      typed=n15_int(j[..],v)==N15_STATUS_UPDATE;
    } else if w>=0 {
      if v!=JSON_TRUE && v!=JSON_FALSE {u.status=N15_FIELD;return u;}
      has[w]=true;
      if w==0 {u.paid=v==JSON_TRUE;} else {u.shipped=v==JSON_TRUE;}
    } else {skipped:=json_skip(j[..]);}
    t=json_next(j[..]);
  }
  if !typed {u.status=N15_TYPE;return u;}
  if u.id.n==0 || !has[0] || !has[1] {u.status=N15_MISSING;return u;}
  u.status=N15_OK;return u;
}
```

<!-- source: coax/lib/nostr/nip15.coax write -->
```jai
n15_put :: (out:[]u8,at:int,t:N32Text)->int {own:=t;return json_write_string(out,at,cast(string)own.b[0..<own.n]);}
// A new order from a record that reads: N15_W_SHAPE when it doesn't,
// N15_W_FULL when out is too small.
n15_write_order :: (out:[]u8,o:N15Order)->int {
  if o.status!=N15_OK {return N15_W_SHAPE;}
  n:=json_raw(out,0,"{\"id\":");n=n15_put(out,n,o.id);n=json_raw(out,n,",\"type\":0");
  if o.address.n>0 {n=json_raw(out,n,",\"address\":");n=n15_put(out,n,o.address);}
  if o.nostr.n>0 {n=json_raw(out,n,",\"contact\":{\"nostr\":");n=n15_put(out,n,o.nostr);n=json_raw(out,n,"}");}
  n=json_raw(out,n,",\"items\":[");
  for k:0..<o.items_n {
    if k>0 {n=json_raw(out,n,",");}
    n=json_raw(out,n,"{\"product_id\":");n=n15_put(out,n,o.items[k].product_id);
    n=json_raw(out,n,",\"quantity\":");n=json_write_int(out,n,o.items[k].quantity);n=json_raw(out,n,"}");
  }
  n=json_raw(out,n,"],\"shipping_id\":");n=n15_put(out,n,o.shipping_id);
  return json_raw(out,n,"}");
}
// A payment request offering only Bitcoin rails: N15_W_RAIL when any type
// is not btc, ln or lnurl.
n15_write_request :: (out:[]u8,id:string,message:string,types:[]string,links:[]string)->int {
  if id.count==0 || types.count==0 || types.count!=links.count || types.count>N15_MAX_OPTIONS {return N15_W_SHAPE;}
  for k:0..<types.count {if n15_rail(types[k])==N15_RAIL_NONE {return N15_W_RAIL;}}
  n:=json_raw(out,0,"{\"id\":");n=json_write_string(out,n,id);n=json_raw(out,n,",\"type\":1");
  if message.count>0 {n=json_raw(out,n,",\"message\":");n=json_write_string(out,n,message);}
  n=json_raw(out,n,",\"payment_options\":[");
  for k:0..<types.count {
    if k>0 {n=json_raw(out,n,",");}
    n=json_raw(out,n,"{\"type\":");n=json_write_string(out,n,types[k]);
    n=json_raw(out,n,",\"link\":");n=json_write_string(out,n,links[k]);n=json_raw(out,n,"}");
  }
  return json_raw(out,n,"]}");
}
n15_write_update :: (out:[]u8,id:string,message:string,paid:bool,shipped:bool)->int {
  if id.count==0 {return N15_W_SHAPE;}
  n:=json_raw(out,0,"{\"id\":");n=json_write_string(out,n,id);
  n=json_raw(out,n,",\"type\":2,\"message\":");n=json_write_string(out,n,message);
  n=json_raw(out,n,ifx paid then ",\"paid\":true" else ",\"paid\":false");
  n=json_raw(out,n,ifx shipped then ",\"shipped\":true}" else ",\"shipped\":false}");
  return n;
}
```

### The total

NIP-15 says how a client computes the shipping: the stall's base cost for
the chosen zone, plus each product's extra cost for that zone times its
quantity. The goods are each product's price times its quantity. The
code computes the total only when the stall and every product are priced
in BTC or sats. BTC and sats can be mixed, because both convert to sats
exactly. Shipping costs are in the stall's currency, as NIP-15 says. Each
product must be in the stall and in stock. Each step is checked against 21
million bitcoin before it is added, so no multiplication overflows.

<!-- source: coax/lib/nostr/nip15.coax total -->
```jai
// An order's total in sats: each item's price times its quantity, the
// zone's base shipping cost, and each item's extra cost for that zone
// times its quantity. Only when the stall and every product are priced in
// BTC or sats; no exchange rate is ever used.
n15_total :: (s:N15Stall,products:[]N15Product,o:N15Order)->N15Total {
  r:N15Total;
  if s.status!=N15_OK || o.status!=N15_OK {return r;}
  r.status=N15_TOTAL_RATE;if s.unit==N99_UNIT_OTHER {return r;}
  zone:=-1;
  for z:0..<s.zones_n {if n15_same(s.zones[z].id,o.shipping_id) {zone=z;}}
  r.status=N15_TOTAL_ZONE;if zone<0 {return r;}
  base:=n99_sats(s.zones[zone].decimal,s.unit);
  r.status=N15_TOTAL_AMOUNT;if base<0 {return r;}
  r.shipping=base;
  for k:0..<o.items_n {
    item:=o.items[k];r.at=k;p:=-1;
    for i:0..<products.count {
      c:=products[i];
      if c.status==N15_OK && n15_same(c.id,item.product_id) && n15_same(c.stall_id,s.id) {p=i;}
    }
    r.status=N15_TOTAL_PRODUCT;if p<0 {return r;}
    c:=products[p];
    r.status=N15_TOTAL_RATE;if c.unit==N99_UNIT_OTHER {return r;}
    price:=n99_sats(c.decimal,c.unit);extra:=0;
    for z:0..<c.extras_n {if n15_same(c.extras[z].id,o.shipping_id) {extra=n99_sats(c.extras[z].decimal,s.unit);}}
    r.status=N15_TOTAL_AMOUNT;if price<0 || extra<0 {return r;}
    r.status=N15_TOTAL_STOCK;if c.quantity>=0 && item.quantity>c.quantity {return r;}
    // Each step stays within 21 million bitcoin, so nothing overflows.
    each:=price+extra;
    r.status=N15_TOTAL_LIMIT;if each>0 && item.quantity>(N99_MAX_SATS-r.goods-r.shipping)/each {return r;}
    r.goods+=item.quantity*price;r.shipping+=item.quantity*extra;
  }
  r.at= -1;r.sats=r.goods+r.shipping;r.status=N15_TOTAL_OK;return r;
}
```

### Worked example: Bob's order

Alice's stall `s1` is priced in `sat` and has two zones: `eu` with base
cost 2000 and `us` with base cost 5000.0. Her products:

- `p1`, a mug: price 15000 sat, quantity 5, extra shipping 500 per unit to
  `eu`.
- `p2`, an ebook: price 0.0001 BTC, quantity `null`.

Bob orders 2 mugs and 1 ebook, shipped to `eu`. By hand:

1. The mug: 15000 sats x 2 = 30000.
2. The ebook: 0.0001 BTC = 0.0001 x 100000000 = 10000 sats; x 1 = 10000.
3. Goods: 30000 + 10000 = 40000.
4. Shipping: the base for `eu` is 2000. The mug's extra for `eu` is 500 x 2
   = 1000. The ebook has no extra. Shipping: 2000 + 1000 = 3000.
5. Total: 40000 + 3000 = 43000 sats.

Shipped to `us` instead, the base is 5000 and the mug has no extra there:
40000 + 5000 = 45000 sats. If the stall were priced in USD, there would be
no total at all.

## The example

The example is
[nostr/examples/markets-orders.coax](../examples/markets-orders.coax). It
signs 10 events with the published test keys: Alice's stall and two
products, Alice's bike listing in sats, Carol's three listings in USD, EUR
and GBP (the room is a draft), and Bob's three orders on Lightning, on
Liquid, and NIP-69's own example. It reads each one back with its id and
signature checked. Then it runs the checkout: Bob writes his order, Alice
reads it and computes the total, Alice writes a payment request, Bob reads
a request with three options and picks the Lightning one, and Alice writes
a status update. The refusals use unsigned events and plain messages,
because no signer here would write them.

<!-- source: nostr/examples/markets-orders.coax example -->
```jai
#load "../../coax/lib/nostr/nip15.coax";
#load "../../coax/lib/nostr/nip69.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

STATUS15 :: [10] string.["ok", "event", "kind", "not JSON", "bad field", "missing field", "d tag is not the id", "limit", "wrong type", "duplicate"];
TOTAL :: [8] string.["ok", "stall or order does not read", "not priced in bitcoin: no exchange rate", "no such shipping zone", "no such product in this stall", "not enough in stock", "not whole sats", "over 21 million bitcoin"];
RAIL :: [4] string.["refused: not a Bitcoin rail", "on-chain", "Lightning", "LNURL"];
STATUS99 :: [8] string.["ok", "event", "kind", "bad tag", "missing d", "bad price", "bad published_at", "limit"];
DUE :: [7] string.["", "listing does not read", "draft: not for sale", "sold", "no price", "not in bitcoin: shown, never converted", "recurring: shown, not settled here"];
STATE99 :: [4] string.["none", "active", "sold", "other"];
STATUS69 :: [6] string.["ok", "event", "kind", "missing tag", "bad value", "limit"];
SETTLE :: [6] string.["may settle", "order does not read", "layer is not Bitcoin, Lightning or Spark: shown, never settled", "not mainnet", "not pending", "sats come from an exchange rate, which this stack does not use"];
// BIP-340 test vectors 0 to 2's public keys.
PEOPLE :: [3] string.["Alice", "Bob", "Carol"];
PUB :: [3] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
  "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"];
// Their secret keys: published test values. Never sign with a real key in
// an example.
SECRET :: [3] string.[
  "0000000000000000000000000000000000000000000000000000000000000003",
  "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef",
  "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9"];
ALICE :: 0; BOB :: 1; CAROL :: 2;
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
say :: (t: N32Text) { own := t; print("%", cast(string) own.b[0..<own.n]); }

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

// The events: who signs, when (after T0), the kind, the tags and the content.
EVENTS :: 10;
NAME :: [EVENTS] string.["ST", "P1", "P2", "L1", "L2", "L3", "L4", "O1", "O2", "O3"];
WHO :: [EVENTS] int.[ALICE, ALICE, ALICE, ALICE, CAROL, CAROL, CAROL, BOB, BOB, BOB];
AT :: [EVENTS] int.[0, 60, 120, 180, 240, 300, 360, 420, 480, 540];
KIND :: [EVENTS] int.[30017, 30018, 30018, 30402, 30402, 30403, 30402, 38383, 38383, 38383];
TAGS :: [EVENTS] string.[
  "[[\"d\",\"s1\"]]",
  "[[\"d\",\"p1\"],[\"t\",\"mugs\"]]",
  "[[\"d\",\"p2\"],[\"t\",\"books\"]]",
  "[[\"d\",\"bike\"],[\"title\",\"Road bike\"],[\"summary\",\"Steel frame, 56 cm\"],[\"published_at\",\"1761686580\"],[\"location\",\"Berlin\"],[\"price\",\"250000\",\"sat\"],[\"status\",\"active\"],[\"t\",\"bikes\"]]",
  "[[\"d\",\"desk\"],[\"title\",\"Desk\"],[\"price\",\"50\",\"USD\"]]",
  "[[\"d\",\"room\"],[\"title\",\"Room\"],[\"price\",\"15\",\"EUR\",\"month\"]]",
  "[[\"d\",\"job\"],[\"title\",\"Rust engineer\"],[\"price\",\"50000\",\"GBP\",\"year\"]]",
  "[[\"d\",\"o-ln\"],[\"k\",\"sell\"],[\"f\",\"EUR\"],[\"s\",\"pending\"],[\"amt\",\"50000\"],[\"fa\",\"30\"],[\"pm\",\"SEPA\"],[\"premium\",\"0\"],[\"network\",\"mainnet\"],[\"layer\",\"lightning\"],[\"y\",\"example\"],[\"z\",\"order\"]]",
  "[[\"d\",\"o-lq\"],[\"k\",\"buy\"],[\"f\",\"USD\"],[\"s\",\"pending\"],[\"amt\",\"20000\"],[\"fa\",\"10\",\"20\"],[\"pm\",\"cash\",\"face to face\"],[\"premium\",\"-1.5\"],[\"network\",\"mainnet\"],[\"layer\",\"liquid\"],[\"y\",\"example\"],[\"z\",\"order\"]]",
  N69_EXAMPLE];
CONTENT :: [EVENTS] string.[N15_STALL, N15_P1, N15_P2, "A good bike.", "Oak.", "Near the park.", "Remote.", "", "", ""];

show_zones :: (label: string, z: [] N15Zone) {
  for k: 0..<z.count { print("%", ifx k == 0 then label else ", "); say(z[k].id); print(" "); say(z[k].cost); }
}
show_stall :: (label: string, s: N15Stall) {
  print("%: %", label, STATUS15[s.status]);
  if s.status == N15_OK { print(", "); say(s.id); print(" \""); say(s.name); print("\" in "); say(s.currency); own := s; show_zones(", zones ", own.zones[0..<own.zones_n]); }
  print("\n");
}
show_product :: (label: string, p: N15Product) {
  print("%: %", label, STATUS15[p.status]);
  if p.status == N15_OK {
    print(", "); say(p.id); print(" \""); say(p.name); print("\" "); say(p.price); print(" "); say(p.currency);
    if p.quantity < 0 { print(", quantity unlimited"); } else { print(", quantity %", p.quantity); }
    own := p; show_zones(", extra per unit ", own.extras[0..<own.extras_n]);
  }
  print("\n");
}
show_listing :: (label: string, e: NostrEvent) {
  l := n99_listing(e);
  print("%: %", label, STATUS99[l.status]);
  if l.at >= 0 { print(" at tag %", l.at); }
  if l.status != N99_OK { print("\n"); return; }
  if l.draft { print(" (draft)"); }
  print(", d "); say(l.d); print(", \""); say(l.title); print("\"");
  p := l.price; print(", price "); say(p.amount); print(" "); say(p.currency);
  if p.frequency.n > 0 { print(" per "); say(p.frequency); }
  if p.sats >= 0 { print(" = % sats", p.sats); }
  if l.state != N99_STATE_NONE { print(", status %", STATE99[l.state]); }
  d := n99_due(l);
  if d.status == N99_DUE_OK { print("; due % sats\n", d.sats); } else { print("; %\n", DUE[d.status]); }
}
show_order :: (label: string, e: NostrEvent) {
  o := n69_order(e);
  print("%: %", label, STATUS69[o.status]);
  if o.at >= 0 { print(" at tag %", o.at); }
  if o.missing >= 0 { print(" (%)", N69_REQUIRED[o.missing]); }
  if o.status == N69_OK {
    print(", % % sats for ", ifx o.side == N69_SELL then "sell" else "buy", o.sats); say(o.fa[0]);
    if o.range { print(" to "); say(o.fa[1]); }
    print(" "); say(o.fiat); print(" by "); say(o.method); print(" on "); say(o.layer_name); print(" ("); say(o.network); print(")");
  }
  print(": %\n", SETTLE[n69_settle(o)]);
}
show_total :: (label: string, s: N15Stall, products: [] N15Product, o: N15Order) {
  t := n15_total(s, products, o);
  print("%: %", label, TOTAL[t.status]);
  if t.at >= 0 { print(" at item %", t.at); }
  if t.status == N15_TOTAL_OK { print(", goods % + shipping % = % sats", t.goods, t.shipping, t.sats); }
  print("\n");
}
// An unsigned event by Carol with these tags, parsed but not verified: its
// id and sig are zeros. Used for the refusals, which no signer here would
// write.
unsigned :: (kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"");
  n = json_raw(out, n, PUB[CAROL]); n = json_raw(out, n, "\",\"created_at\":1761686400,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":\"x\",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}
// Bob's order: two mugs and the ebook, shipped to the eu zone.
bob_order :: () -> N15Order {
  o: N15Order; o.status = N15_OK; o.id = n99_text("o1"); o.nostr = n99_text(PUB[BOB]);
  o.address = n99_text("1 Main St, Berlin"); o.shipping_id = n99_text("eu");
  o.items[0] = N15Item.{product_id = n99_text("p1"), quantity = 2};
  o.items[1] = N15Item.{product_id = n99_text("p2"), quantity = 1};
  o.items_n = 2;
  return o;
}

main :: () {
  all: [EVENTS] NostrEvent; buf: [EVENTS][4096] u8;
  for k: 0..<EVENTS { all[k] = sign(WHO[k], T0 + AT[k], KIND[k], TAGS[k], CONTENT[k], buf[k][..]); }
  print("each event's id\n");
  for k: 0..<EVENTS { hex: [64] u8; print("%: %, verified %\n", NAME[k], hex32(all[k].id, hex[..]), all[k].verified); }

  print("Alice's stall and products (NIP-15)\n");
  content: [3][2048] u8;
  stall := n15_stall(all[0], content[0][..]); show_stall("ST", stall);
  products: [2] N15Product;
  products[0] = n15_product(all[1], content[1][..]); show_product("P1", products[0]);
  products[1] = n15_product(all[2], content[2][..]); show_product("P2", products[1]);

  print("classified listings (NIP-99)\n");
  for k: 3..<7 { show_listing(NAME[k], all[k]); }

  print("peer-to-peer orders (NIP-69)\n");
  for k: 7..<10 { show_order(NAME[k], all[k]); }

  print("checkout (NIP-15 messages, the plaintext of Bob's and Alice's DMs)\n");
  msg: [1024] u8;
  n := n15_write_order(msg[..], bob_order());
  text := cast(string) msg[0..<n];
  print("Bob's new order, type %: %\n", n15_type(text), text);
  o := n15_order_parse(text);
  print("Alice reads it: %, ", STATUS15[o.status]); say(o.id); print(", % items, ship to ", o.items_n); say(o.shipping_id); print("\n");
  show_total("total", stall, products[..], o);
  types := [2] string.["ln", "btc"];
  links := [2] string.["lnbc430u1p5example", "bc1qexampleaddress"];
  req: [1024] u8;
  n = n15_write_request(req[..], "o1", "43000 sats, please", types[..], links[..]);
  print("Alice's payment request, type %: %\n", n15_type(cast(string) req[0..<n]), cast(string) req[0..<n]);
  mixed := "{\"id\":\"o1\",\"type\":1,\"payment_options\":[{\"type\":\"url\",\"link\":\"https://pay.example/o1\"},{\"type\":\"cashu\",\"link\":\"cashuAexample\"},{\"type\":\"ln\",\"link\":\"lnbc430u1p5example\"}]}";
  r := n15_request_parse(mixed);
  print("a request offering three options: %, % on a Bitcoin rail\n", STATUS15[r.status], r.bitcoin);
  for k: 0..<r.options_n { print("  option %: ", k); say(r.options[k].type); print(", %\n", RAIL[r.options[k].rail]); }
  c := n15_choice(r);
  print("  Bob pays with option %: %\n", c, n15_raw(mixed, r.options[c]));
  up: [256] u8;
  n = n15_write_update(up[..], "o1", "Paid. Shipped today.", true, true);
  u := n15_update_parse(cast(string) up[0..<n]);
  print("Alice's status update, type %: %\n", n15_type(cast(string) up[0..<n]), cast(string) up[0..<n]);
  print("Bob reads it: %, paid %, shipped %\n", STATUS15[u.status], u.paid, u.shipped);

  print("refusals\n");
  bad: [4096] u8;
  show_listing("listing priced \"50,000\"", unsigned(30402, "[[\"d\",\"x\"],[\"price\",\"50,000\",\"USD\"]]", bad[..]));
  show_listing("listing priced in 1/10 sat", unsigned(30402, "[[\"d\",\"x\"],[\"price\",\"0.000000001\",\"BTC\"]]", bad[..]));
  show_listing("listing published \"soon\"", unsigned(30402, "[[\"d\",\"x\"],[\"published_at\",\"soon\"]]", bad[..]));
  show_order("order without a layer", unsigned(38383, "[[\"d\",\"x\"],[\"k\",\"buy\"],[\"f\",\"EUR\"],[\"s\",\"pending\"],[\"amt\",\"1\"],[\"fa\",\"1\"],[\"pm\",\"SEPA\"],[\"premium\",\"0\"],[\"network\",\"mainnet\"]]", bad[..]));
  show_order("order to swap", unsigned(38383, "[[\"d\",\"x\"],[\"k\",\"swap\"]]", bad[..]));
  usd := n15_stall_parse("{\"id\":\"s1\",\"name\":\"Alice's stall\",\"currency\":\"USD\",\"shipping\":[{\"id\":\"eu\",\"cost\":20}]}");
  show_total("the same order from a stall in USD", usd, products[..], o);
  six := bob_order(); six.items[0].quantity = 6;
  show_total("six mugs, five in stock", stall, products[..], six);
  far := bob_order(); far.shipping_id = n99_text("mars");
  show_total("shipping to mars", stall, products[..], far);
  bad_types := [2] string.["ln", "cashu"];
  print("writing a request with a cashu option: %\n", ifx n15_write_request(req[..], "o1", "", bad_types[..], links[..]) == N15_W_RAIL then "refused, not a Bitcoin rail" else "written");
  print("an order naming the mug twice: %\n", STATUS15[n15_order_parse("{\"id\":\"o\",\"type\":0,\"items\":[{\"product_id\":\"p1\",\"quantity\":1},{\"product_id\":\"p1\",\"quantity\":1}],\"shipping_id\":\"eu\"}").status]);
  print("a product with two prices: %\n", STATUS15[n15_product_parse("{\"id\":\"p\",\"stall_id\":\"s1\",\"name\":\"n\",\"currency\":\"sat\",\"price\":1,\"price\":0}").status]);
  print("a stall event whose d is not its id: %\n", STATUS15[n15_stall(unsigned_content(N15_KIND_STALL, "[[\"d\",\"s2\"]]", N15_STALL, bad[..]), content[0][..]).status]);
}
// An unsigned event by Carol with these tags and this content.
unsigned_content :: (kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"");
  n = json_raw(out, n, PUB[CAROL]); n = json_raw(out, n, "\",\"created_at\":1761686400,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, content);
  n = json_raw(out, n, ",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/markets-orders.coax --run-steps 250000000
```

It prints this transcript, which is kept in
[nostr/fixtures/markets-orders.out](../fixtures/markets-orders.out) and
which the book's checks compare in debug and release. It exits with
status 0.

```text
each event's id
ST: 0138c11ffbf8659e778310639bd557f30e67dbb45629300933b4e9e617d08215, verified true
P1: 80ddaa2ec11c5370a5d69871b9b1a86c810dae31f9ba80fbd9964334f77264a3, verified true
P2: 2f97d3698fa40530e1a5741184036bd810a8b3526875574f0acdc9ede83360de, verified true
L1: 0ffd847bcb617478cc823606e4113816cb85d2c382e283420f7c266cb8372a44, verified true
L2: e12f228eeca119e2d9f6e132b6c0cce99e1742cb2a5dede4111cb524226fe6af, verified true
L3: d82f6068921cedfd5ba1bee50c2616b5ce36be1c35140682b796d8037db85892, verified true
L4: 4c97ab70b368c95933dc35e3ac8bdc98f317011b7589e5b9519b2bff8a947888, verified true
O1: 65fc586b2d4d6000c263543aa5efe8f532c6f5f302e4a44dbb6ca8fd399d7355, verified true
O2: 4b7e3fcfd9298e8fdd2f1586e371163bd9fe046a81f924eca04fbe8003f25704, verified true
O3: 2a94f91f53946a64469f7b0ca81827fa9ad5240d76a98d3ebd3d063de6aaa89f, verified true
Alice's stall and products (NIP-15)
ST: ok, s1 "Alice's stall" in sat, zones eu 2000, us 5000.0
P1: ok, p1 "Mug" 15000 sat, quantity 5, extra per unit eu 500
P2: ok, p2 "Ebook" 0.0001 BTC, quantity unlimited
classified listings (NIP-99)
L1: ok, d bike, "Road bike", price 250000 sat = 250000 sats, status active; due 250000 sats
L2: ok, d desk, "Desk", price 50 USD; not in bitcoin: shown, never converted
L3: ok (draft), d room, "Room", price 15 EUR per month; draft: not for sale
L4: ok, d job, "Rust engineer", price 50000 GBP per year; not in bitcoin: shown, never converted
peer-to-peer orders (NIP-69)
O1: ok, sell 50000 sats for 30 EUR by SEPA on lightning (mainnet): may settle
O2: ok, buy 20000 sats for 10 to 20 USD by cash on liquid (mainnet): layer is not Bitcoin, Lightning or Spark: shown, never settled
O3: ok, sell 0 sats for 100 VES by face to face on lightning (mainnet): sats come from an exchange rate, which this stack does not use
checkout (NIP-15 messages, the plaintext of Bob's and Alice's DMs)
Bob's new order, type 0: {"id":"o1","type":0,"address":"1 Main St, Berlin","contact":{"nostr":"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"},"items":[{"product_id":"p1","quantity":2},{"product_id":"p2","quantity":1}],"shipping_id":"eu"}
Alice reads it: ok, o1, 2 items, ship to eu
total: ok, goods 40000 + shipping 3000 = 43000 sats
Alice's payment request, type 1: {"id":"o1","type":1,"message":"43000 sats, please","payment_options":[{"type":"ln","link":"lnbc430u1p5example"},{"type":"btc","link":"bc1qexampleaddress"}]}
a request offering three options: ok, 1 on a Bitcoin rail
  option 0: url, refused: not a Bitcoin rail
  option 1: cashu, refused: not a Bitcoin rail
  option 2: ln, Lightning
  Bob pays with option 2: lnbc430u1p5example
Alice's status update, type 2: {"id":"o1","type":2,"message":"Paid. Shipped today.","paid":true,"shipped":true}
Bob reads it: ok, paid true, shipped true
refusals
listing priced "50,000": bad price at tag 1
listing priced in 1/10 sat: bad price at tag 1
listing published "soon": bad published_at at tag 1
order without a layer: missing tag (layer): order does not read
order to swap: bad value at tag 1: order does not read
the same order from a stall in USD: not priced in bitcoin: no exchange rate
six mugs, five in stock: not enough in stock at item 0
shipping to mars: no such shipping zone
writing a request with a cashu option: refused, not a Bitcoin rail
an order naming the mug twice: duplicate
a product with two prices: duplicate
a stall event whose d is not its id: d tag is not the id
```

The prices and the total match the worked examples. The order on Liquid
reads and is shown, and is never settled. The refusals show each broken
input and its exact diagnostic:

- `listing priced "50,000"`: `["price","50,000","USD"]`, at tag 1.
- `listing priced in 1/10 sat`: `["price","0.000000001","BTC"]`, at tag 1.
- `listing published "soon"`: `["published_at","soon"]`, at tag 1.
- `order without a layer`: every required tag but `layer`.
- `order to swap`: `["k","swap"]`, at tag 1.
- `the same order from a stall in USD`: no total, because no exchange rate
  is used.
- `six mugs, five in stock`: item 0 asks for 6 of a product with quantity
  5.
- `shipping to mars`: the stall has no zone `mars`.
- `writing a request with a cashu option`: `n15_write_request` returns
  `N15_W_RAIL` and writes nothing.
- `an order naming the mug twice`: two items with `product_id` `p1`.
- `a product with two prices`: two `price` keys, refused by the JSON
  policy.
- `a stall event whose d is not its id`: `["d","s2"]` on content with
  `id` `s1`.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/markets-orders.coax](../examples/checks/markets-orders.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The ids of all 10 events, from a Python script written from NIP-01,
  NIP-15, NIP-99 and NIP-69 (`hashlib.sha256` over NIP-01's
  serialization). The checks read each event with Python's id and require
  that the recomputed id matches.
- The total by hand: 43000 sats to `eu` and 45000 to `us`, and the same
  total after Bob's order is written and read back.
- NIP-99's three price examples and its example event, and NIP-69's
  example order, which reads and is not settled.
- Done means: orders on `liquid`, `rsk`, `tron`, `ethereum`, `Lightning`
  (the case matters) and `on-chain` read and are never settled; orders on
  `onchain`, `lightning` and `spark` may be settled.

<!-- source: nostr/examples/checks/markets-orders.coax check -->
```jai
// The ids from a Python script written from NIP-01, NIP-15, NIP-99 and
// NIP-69 (it shares no code with Coax): hashlib.sha256 of
// json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False), over the same events.
PY_ID :: [EVENTS] string.[
  "0138c11ffbf8659e778310639bd557f30e67dbb45629300933b4e9e617d08215",
  "80ddaa2ec11c5370a5d69871b9b1a86c810dae31f9ba80fbd9964334f77264a3",
  "2f97d3698fa40530e1a5741184036bd810a8b3526875574f0acdc9ede83360de",
  "0ffd847bcb617478cc823606e4113816cb85d2c382e283420f7c266cb8372a44",
  "e12f228eeca119e2d9f6e132b6c0cce99e1742cb2a5dede4111cb524226fe6af",
  "d82f6068921cedfd5ba1bee50c2616b5ce36be1c35140682b796d8037db85892",
  "4c97ab70b368c95933dc35e3ac8bdc98f317011b7589e5b9519b2bff8a947888",
  "65fc586b2d4d6000c263543aa5efe8f532c6f5f302e4a44dbb6ca8fd399d7355",
  "4b7e3fcfd9298e8fdd2f1586e371163bd9fe046a81f924eca04fbe8003f25704",
  "2a94f91f53946a64469f7b0ca81827fa9ad5240d76a98d3ebd3d063de6aaa89f"];
SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

// Event k with Python's id and a zero signature, read with its id
// recomputed; the signature is not checked.
py :: (k: int, out: [] u8, scratch: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, PY_ID[k]);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB[WHO[k]]);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, T0 + AT[k]);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, KIND[k]);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, TAGS[k]);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, CONTENT[k]);
  n = json_raw(out, n, ",\"sig\":\""); n = json_raw(out, n, SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}

#test "the ids are Python's" {
  buf: [2048] u8; scratch: [4096] u8;
  for k: 0..<EVENTS { e := py(k, buf[..], scratch[..]); #assert e.error == NE_OK && ns_equal32(e.computed_id, e.id); }
}

// The worked total by hand. The mug: 15000 sat, two of them, 30000. The
// ebook: 0.0001 BTC = 0.0001 x 100000000 = 10000 sat, one of them. Goods
// 40000. Shipping to eu: the stall's base 2000, plus the mug's extra 500
// per unit for eu, twice: 3000. Total 43000 sats. To us: base 5000 and
// no extra for the mug there: 45000.
#test "the total matches the hand values" {
  buf: [2048] u8; scratch: [4096] u8; content: [3][2048] u8;
  stall := n15_stall(py(0, buf[..], scratch[..]), content[0][..]);
  ps: [2] N15Product;
  ps[0] = n15_product(py(1, buf[..], scratch[..]), content[1][..]);
  ps[1] = n15_product(py(2, buf[..], scratch[..]), content[2][..]);
  #assert stall.status == N15_OK && ps[0].status == N15_OK && ps[1].status == N15_OK;
  #assert n99_sats(ps[1].decimal, ps[1].unit) == 10000;
  t := n15_total(stall, ps[..], bob_order());
  #assert t.status == N15_TOTAL_OK && t.goods == 2 * 15000 + 10000 && t.shipping == 2000 + 2 * 500 && t.sats == 43000;
  us := bob_order(); us.shipping_id = n99_text("us");
  t = n15_total(stall, ps[..], us); #assert t.status == N15_TOTAL_OK && t.sats == 45000;
  // The order Bob writes reads back the same.
  out: [1024] u8; n := n15_write_order(out[..], bob_order());
  o := n15_order_parse(cast(string) out[0..<n]);
  t = n15_total(stall, ps[..], o); #assert t.sats == 43000;
}

// NIP-99's three price examples and its example event; NIP-69's example
// order. The listing's d, title and price are the NIP's; the order reads
// with VES, amt 0 and lightning, and is not settled: its sats would come
// from an exchange rate.
#test "the NIPs' examples read as the NIPs say" {
  b: [2048] u8;
  e := unsigned(30402, "[[\"price\",\"50\",\"USD\"],[\"price\",\"15\",\"EUR\",\"month\"],[\"price\",\"50000\",\"GBP\",\"year\"]]", b[..]);
  p := n99_price(e, 0); #assert p.status == N99_OK && n32_is(p.amount, "50") && n32_is(p.currency, "USD") && p.frequency.n == 0 && p.sats == -1;
  p = n99_price(e, 1); #assert p.status == N99_OK && n32_is(p.currency, "EUR") && n32_is(p.frequency, "month");
  p = n99_price(e, 2); #assert p.status == N99_OK && p.decimal.whole == 50000 && n32_is(p.frequency, "year");
  e = unsigned(30402, "[[\"d\",\"lorem-ipsum\"],[\"title\",\"Lorem Ipsum\"],[\"published_at\",\"1296962229\"],[\"t\",\"electronics\"],[\"image\",\"https://url.to.img\",\"256x256\"],[\"summary\",\"More lorem ipsum that is a little more than the title\"],[\"location\",\"NYC\"],[\"price\",\"100\",\"USD\"],[\"e\",\"b3e392b11f5d4f28321cedd09303a748acfd0487aea5a7450b3481c60b6e4f87\",\"wss://relay.example.com\"],[\"a\",\"30023:a695f6b60119d9521934a691347d9f78e8770b56da16bb255ee286ddf9fda919:ipsum\",\"wss://relay.nostr.org\"]]", b[..]);
  l := n99_listing(e);
  #assert l.status == N99_OK && n32_is(l.d, "lorem-ipsum") && n32_is(l.title, "Lorem Ipsum") && l.published_at == 1296962229 && n32_is(l.location, "NYC") && l.topics == 1 && l.images == 1;
  #assert n99_due(l).status == N99_DUE_RATE;
  o := n69_order(unsigned(38383, N69_EXAMPLE, b[..]));
  #assert o.status == N69_OK && o.side == N69_SELL && n32_is(o.fiat, "VES") && o.sats == 0 && o.methods == 2 && o.layer == N69_LAYER_LIGHTNING;
  #assert n69_settle(o) == N69_SETTLE_RATE;
}

// Done means: an order on any layer but onchain, lightning or spark is
// shown (it reads) and never settled, whatever its other fields say.
#test "an order on another layer is shown and never settled" {
  b: [4096] u8;
  layers := [6] string.["liquid", "rsk", "tron", "ethereum", "Lightning", "on-chain"];
  for k: 0..<layers.count {
    t: [1024] u8; n := json_raw(t[..], 0, "[[\"d\",\"x\"],[\"k\",\"sell\"],[\"f\",\"EUR\"],[\"s\",\"pending\"],[\"amt\",\"5000\"],[\"fa\",\"3\"],[\"pm\",\"SEPA\"],[\"premium\",\"0\"],[\"network\",\"mainnet\"],[\"layer\",\"");
    n = json_raw(t[..], n, layers[k]); n = json_raw(t[..], n, "\"]]");
    o := n69_order(unsigned(38383, cast(string) t[0..<n], b[..]));
    #assert o.status == N69_OK && o.layer == N69_LAYER_OTHER && n69_settle(o) == N69_SETTLE_LAYER;
  }
  good := [3] string.["onchain", "lightning", "spark"];
  for k: 0..<good.count {
    t: [1024] u8; n := json_raw(t[..], 0, "[[\"d\",\"x\"],[\"k\",\"sell\"],[\"f\",\"EUR\"],[\"s\",\"pending\"],[\"amt\",\"5000\"],[\"fa\",\"3\"],[\"pm\",\"SEPA\"],[\"premium\",\"0\"],[\"network\",\"mainnet\"],[\"layer\",\"");
    n = json_raw(t[..], n, good[k]); n = json_raw(t[..], n, "\"]]");
    o := n69_order(unsigned(38383, cast(string) t[0..<n], b[..]));
    #assert o.status == N69_OK && o.layer == k + 1 && n69_settle(o) == N69_SETTLE_OK;
  }
}
```

```shell
coax/bin/coax test nostr/examples/checks/markets-orders.coax --run-steps 250000000
```

On 2026-11-01, `coax test` ran 382 checks, including the loaded files'
own checks; all passed.

### The library's own checks

`nip99.coax`, `nip69.coax` and `nip15.coax` have their own `#test` and
`#forall` blocks: every price form in NIP-99, every byte as a digit or a
letter, the BTC and sats bounds at their edges (21,000,000 BTC and
21,000,000.00000001 BTC), NIP-99's and NIP-69's examples, each missing
required tag of an order, each refusal, the first-of-each-tag rules, the
settle rule in every order, stalls and products with every missing and
mistyped field, forged event spans, duplicate keys, zones and items, each
message type read and written back byte for byte, payment requests with
mixed rails, and the total at the 21-million-bitcoin bound with one sat
per unit.

```shell
coax/bin/coax test coax/lib/nostr/nip99.coax
coax/bin/coax test coax/lib/nostr/nip69.coax
coax/bin/coax test coax/lib/nostr/nip15.coax
coax/bin/coax mutate coax/lib/nostr/nip99.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip69.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip15.coax --root-only
```

On 2026-11-01, `coax test` passed all 213, 217 and 225 checks (with the
loaded files'). `coax mutate --root-only` killed all 356 mutants of
`nip99.coax`, all 207 of `nip69.coax` and all 793 of `nip15.coax`, with
none surviving and none marked equivalent. The full mutation run
(`COAX_MUTATION_FULL=1`) and a mutation run of the example were not run.

### Not built

- Recurring payments for a listing with a frequency. The price is shown.
- Exchange rates of any kind. A price in another currency is shown, and an
  NIP-69 order with `amt` 0 is not settled.
- Checking that a payment request's invoice or address pays the total.
  Chapter 86 reads the amount in a bolt11 invoice; chapter 71 covers zaps.
- The NIP-04 or gift-wrap encryption around checkout messages (chapters
  19 and 25).
- NIP-15's marketplace UI events (30019) and auctions (30020, bids 1021,
  confirmations 1022). Auction bids are in sats, but they need the
  merchant's confirmation flow; they are left for later.
- The `rating` and `bond` tags of NIP-69 orders, and the GammaMarkets
  extension that NIP-99 points to.

## Summary

- `n99_price` reads every NIP-99 price form. A price in BTC or sats
  becomes exact sats; every other currency is shown and never converted.
- `n99_listing` reads a kind 30402 or 30403 listing, and `n99_due` says
  what a buyer could be asked to pay: only a one-time price in bitcoin on
  a listing that is neither a draft nor sold.
- `n69_order` reads a kind 38383 order whatever its layer, and
  `n69_settle` settles only `onchain`, `lightning` or `spark` orders on
  mainnet that are pending and have their sats stated.
- `n15_stall`, `n15_product` and the checkout readers read NIP-15's JSON
  without duplicate keys. Only `btc`, `ln` and `lnurl` payment options are
  chosen or written.
- `n15_total` adds goods and shipping in sats for a stall and products
  priced in BTC or sats, and refuses any other currency.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Sources

- [NIP-99](../nips/official/99.md)
- [NIP-69](../nips/official/69.md)
- [NIP-15](../nips/official/15.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
