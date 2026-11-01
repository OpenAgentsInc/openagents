# 86. Lightning payments for requests

Some OpenAgents routes charge per request. NIP-X402 binds the payment to the
request with x402 v2 over HTTP and Lightning only. The server answers a
request with 402 and a bolt11 invoice. The client pays the invoice, then
sends the same request again with proof of payment. The server settles the
payment against its own wallet's record and answers. This chapter builds the
client's codec and the settlement rules in Coax, and follows one top-up
through the whole exchange with fixture values.

Part X, Every NIP. Result: NIP-X402: x402 v2 over HTTP with the three base64
JSON headers, Bitcoin mainnet Lightning with a bolt11 invoice in millisats,
and the `openagents:spark-receiver:1` settlement profile.

## What this chapter teaches

- The three headers, each base64 of JSON: `PAYMENT-REQUIRED` carries the
  terms, `PAYMENT-SIGNATURE` carries the retry's proof, and
  `PAYMENT-RESPONSE` carries the settlement.
- The only terms a client accepts: version 2, scheme `exact`, network
  `lnbtc:000000000019d6689c085ae165831e93`, asset `BTC`, method `bolt11`,
  flow `upfront`, receiver profile `openagents:spark-receiver:1`, and an
  amount as a positive decimal string of millisatoshis (msat).
- The amount in a bolt11 invoice's human-readable part, and the rule that it
  equals the terms' amount.
- Settlement: the receiver's wallet record of the invoice paid, an amount at
  least the invoice's, and a preimage whose SHA-256 is the payment hash.
- Refusals for every other scheme, network or asset, including stablecoins:
  money here is Bitcoin only.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [x402.coax](../../coax/lib/x402/x402.coax) | `x402_read_required(header, o, scratch)` | Reads `PAYMENT-REQUIRED` (or the same JSON in a 402 body) into an `X402Offer`: the msat amount, `maxTimeoutSeconds`, the invoice, the payment hash and the accepted entry byte for byte. Any other term refuses, with its reason |
| | `x402_signature(o, preimage, out)` | Writes `PAYMENT-SIGNATURE`: the accepted entry as sent, the payment hash and, when the wallet gave one, the preimage |
| | `x402_read_response(header, o, scratch)` | Whether `PAYMENT-RESPONSE` says success for this payment hash |
| [settle.coax](../../coax/lib/x402/settle.coax) | `x402_bolt11_msat(invoice)` | The amount in a mainnet invoice's human-readable part, in msat (`#authority`) |
| | `x402_check_invoice(o)` | The invoice's amount equals the terms' amount |
| | `x402_preimage_matches(preimage, hash)` | SHA-256 of the 32-byte preimage is the payment hash |
| | `x402_settle(o, rec, payer, issued_s, now_s)` | The settlement decision, in the receiver profile's order |
| [inference.c](../../coax/web/api/inference.c), [0066_inference_credit.sql](../../coax/web/api/migrations/0066_inference_credit.sql) | `inference.topup_issue`, `topup_proof`, `topup_settle` | The deployed server role: `POST /api/v1/inference/credit` buys inference credit, and SQL makes every decision |

`x402.coax` is the client's half. The Lab desktop app uses it. It reads the
challenge strictly, but before this chapter nothing in Coax checked the
invoice's amount or the preimage. `settle.coax` adds those rules for this
chapter. It is an `#authority` file because it decides money. Its
`x402_settle` follows `inference.topup_settle` in the deployed route, with
the same refusal words, and adds two checks of the NIP's shared rules in
front: the invoice's amount and its expiry with the 60-second grace. The
deployed route still decides in SQL and does not call `settle.coax`; the
SQL does not check expiry at settlement.

What is not built in Coax: the BOLT11 signature, the tagged fields other
than the example's reading of `p`, and the bech32 checksum of an invoice.
The MCP role and the native Nostr role of NIP-X402 are not built at all.

## The exchange

1. The client sends the request, for example
   `POST /api/v1/inference/credit` with `{"pubkey": hex, "msat": "50000"}`.
2. The server asks its Spark wallet for an exact-amount invoice, stores the
   invoice, its payment hash, the request digest and the expiry in its own
   database, and answers **402** with `PAYMENT-REQUIRED`.
3. The client reads the terms. It refuses anything it does not take.
4. The client's wallet pays the invoice. Most wallets return the preimage,
   the 32 bytes whose SHA-256 is the payment hash.
5. The client sends the same request again with `PAYMENT-SIGNATURE`.
6. The server matches the retry to its stored row, asks its wallet for the
   record of that invoice, and settles: **200** with `PAYMENT-RESPONSE`,
   **402** `payment_pending` while the wallet has not seen the payment, or
   **409** `duplicate_settlement` for a second settlement of the same hash.

### PAYMENT-REQUIRED

The header is base64 of a `PaymentRequired` object with one `accepts`
entry:

```jai
{"x402Version":2,"error":"payment required","resource":{"url":"..."},
 "accepts":[{"scheme":"exact","network":"lnbtc:000000000019d6689c085ae165831e93",
             "amount":"50000","asset":"BTC","payTo":"02ab","maxTimeoutSeconds":600,
             "extra":{"method":"bolt11","paymentFlow":"upfront",
                      "receiverProfile":"openagents:spark-receiver:1",
                      "invoice":"lnbc500n1...","paymentHash":"ae216c2e...","requestDigest":"..."}}]}
```

`x402_entry` reads the entry and refuses, in this order, with these words:

| Term | Required | Refusal |
| --- | --- | --- |
| `scheme` | `exact` | `not the exact scheme` |
| `network` | `lnbtc:000000000019d6689c085ae165831e93` | `not Bitcoin mainnet Lightning` |
| `asset`, `extra.method` | `BTC`, `bolt11` | `not a bolt11 payment in BTC` |
| `extra.paymentFlow` | `upfront` | `not an upfront payment` |
| `extra.receiverProfile` | `openagents:spark-receiver:1` | `not a receiver profile this client takes` |
| `amount` | a positive decimal msat string | `the amount is not a msat string` |
| `extra.invoice` | starts `lnbc` | `no mainnet invoice` |
| `extra.paymentHash` | 64 lowercase hex digits | `no payment hash` |

Then `x402Version` must be 2 (`not x402 version 2`). The x402 standard also
defines EVM and other networks and token assets such as USDC. This client
refuses all of them: an EVM network fails the network check, and asset
`USDC` fails the asset check. The upstream `lnbtc` testnet network
(`lnbtc:000000000933ea01ad0ee984209779ba`) refuses too, because this route
sells on mainnet only.

### The bolt11 amount

A bolt11 invoice starts with a human-readable part: `ln`, the currency
(`bc` for mainnet), and an optional amount. The amount is digits followed
by an optional multiplier. The separator is the last `1` in the invoice;
the data part after it never contains `1`.

| Multiplier | BTC | msat per unit |
| --- | --- | ---: |
| none | 1 | 100,000,000,000 |
| `m` | 10^-3 | 100,000,000 |
| `u` | 10^-6 | 100,000 |
| `n` | 10^-9 | 100 |
| `p` | 10^-12 | 0.1 |

One BTC is 10^8 sat and one sat is 1,000 msat, so one BTC is 10^11 msat. A
`p` amount must end in `0` to be a whole number of msat. `x402_bolt11_msat`
refuses an invoice with no amount, another currency, a leading zero, a
non-digit, or more than 21 million BTC. In the `p` unit it also refuses more
than 2.1 × 10^17 msat, which keeps every step inside 64-bit integers.

NIP-X402 says that the invoice's face amount is the amount settled. So
`x402_check_invoice` refuses an offer whose invoice amount is not the
terms' `amount`: `the invoice's amount is not the terms' amount`.

<!-- source: coax/lib/x402/settle.coax amount -->
```jai
// Where a bolt11 invoice's human-readable part ends: the index of its last
// '1', the separator, when that is past "lnbc" and at least one more
// character; s.count when there is none.
x402_bolt11_sep :: (s: string) -> int {
  i := s.count;
  while i > 5 { i -= 1; if s[i] == '1' { return i; } }
  return s.count;
}

// The amount in a mainnet bolt11 invoice's human-readable part, in msat:
// "lnbc", digits, then m (10^-3 BTC), u (10^-6), n (10^-9), p (10^-12) or
// nothing (whole BTC), then the separator. -1 for no amount, another prefix,
// a leading zero, a p amount that is not whole msat (its last digit must be
// 0), more than 21 million BTC, or, in the p unit, more than 2.1 * 10^17 msat
// (2.1 million BTC), which keeps the arithmetic inside 64 bits.
x402_bolt11_msat :: (invoice: string) -> int {
  sep := x402_bolt11_sep(invoice);
  if sep == invoice.count || invoice[0..<4] != "lnbc" { return -1; }
  hrp := invoice[4..<sep];
  unit := X402_MSAT_PER_BTC; cap := 21_000_000; end := hrp.count;
  last := hrp[end - 1];
  if last == 'm' { unit = 100_000_000; cap = 21_000_000_000; end -= 1; }
  else if last == 'u' { unit = 100_000; cap = 21_000_000_000_000; end -= 1; }
  else if last == 'n' { unit = 100; cap = 21_000_000_000_000_000; end -= 1; }
  else if last == 'p' && invoice[sep - 2] == '0' { unit = 1; cap = 210_000_000_000_000_000; end -= 2; }   // 10p is 1 msat
  if end == 0 || hrp[0] == '0' { return -1; }
  v := 0;
  for i: 0..<end {
    c := hrp[i];
    if c < '0' || c > '9' { return -1; }
    v = v * 10 + cast(int) (c - '0');
    if v > cap { return -1; }
  }
  return v * unit;
}
```

### PAYMENT-SIGNATURE

The retry's header is base64 of

```jai
{"x402Version":2,"accepted":<the entry, byte for byte>,
 "payload":{"paymentHash":"<hex>","preimage":"<hex>"}}
```

The `accepted` entry is copied from the challenge as the server sent it,
not rebuilt, so the server can compare it with its own row exactly. The
preimage is left out when the payer's wallet returned none. Under the Spark
receiver profile that still settles, because the proof is the receiver's
own record, not the payer's preimage.

### Settlement

`x402_settle` returns `""` when the payment settles, or why not:

1. The invoice's amount is the terms' amount (`the invoice's amount is not
   the terms' amount`, or `the invoice states no amount`).
2. `now ≤ issued + maxTimeoutSeconds + 60`. The 60 seconds are the paid-retry
   grace of the NIP, and the equality counts (`the invoice has expired`).
3. The wallet's record shows a completed receive (`payment_pending`).
4. The record's amount is at least the terms' amount. Overpayment buys
   nothing extra (`invalid_payment: less than the invoice`).
5. The record's preimage hashes to the payment hash (`invalid_payment: the
   preimage does not hash to the payment hash`).
6. When the payer sent a preimage, it is the same one (`invalid_payment: the
   payer's preimage is not the invoice's`).

The deployed server then inserts the replay key `network:payment_hash` once
and adds the credit in the same transaction. A second settlement of the same
hash is `duplicate_settlement` and adds nothing. That step is a database
row, so it is not in `settle.coax`.

<!-- source: coax/lib/x402/settle.coax settle -->
```jai
// The offer's invoice states exactly the terms' amount: "" or why not.
x402_check_invoice :: (o: [] X402Offer) -> string {
  m := x402_bolt11_msat(cast(string) o[0].invoice[0..<o[0].ni]);
  if m == -1 { return "the invoice states no amount"; }
  if m != o[0].msat { return "the invoice's amount is not the terms' amount"; }
  return "";
}

// One lowercase hex digit already checked by x402_is_hex64.
x402_nibble :: (c: u8) -> int { return ifx c >= 'a' then cast(int) (c - 'a') + 10 else cast(int) (c - '0'); }

// Whether the 64-digit hex preimage hashes, by SHA-256 of its 32 bytes, to the
// 64-digit hex payment hash.
x402_preimage_matches :: (preimage: string, hash: string) -> bool {
  if !x402_is_hex64(preimage) || !x402_is_hex64(hash) { return false; }
  b: [32] u8; h: [32] u8;
  for i: 0..<32 { b[i] = cast(u8) (x402_nibble(preimage[2 * i]) * 16 + x402_nibble(preimage[2 * i + 1])); }
  same := sha256_hash_checked(b[..], h[..], false) == SHA_OK;
  for i: 0..<32 {
    if cast(int) h[i] != x402_nibble(hash[2 * i]) * 16 + x402_nibble(hash[2 * i + 1]) { same = false; }
  }
  return same;
}

// The receiver's wallet's record of the offer's invoice.
X402Record :: struct {
  paid: bool;              // a completed receive
  msat: int;               // what it received
  preimage: [64] u8; np: int;   // the invoice's preimage, hex
}

// Whether the paid retry for o settles: "" or why not, in the receiver
// profile's order. issued_s is when the invoice was made; it ends
// maxTimeoutSeconds later and a paid retry has X402_SKEW_S more, inclusive.
// rec is one record; payer is the preimage the payer sent, or "".
x402_settle :: (o: [] X402Offer, rec: [] X402Record, payer: string, issued_s: int, now_s: int) -> string {
  why := x402_check_invoice(o);
  if why != "" { return why; }
  if now_s > issued_s + o[0].timeout_s + X402_SKEW_S { return "the invoice has expired"; }
  if !rec[0].paid { return "payment_pending"; }
  if rec[0].msat < o[0].msat { return "invalid_payment: less than the invoice"; }
  pre := cast(string) rec[0].preimage[0..<rec[0].np];
  if !x402_preimage_matches(pre, cast(string) o[0].hash[0..<o[0].nh]) { return "invalid_payment: the preimage does not hash to the payment hash"; }
  if payer != "" && payer != pre { return "invalid_payment: the payer's preimage is not the invoice's"; }
  return "";
}
```

### PAYMENT-RESPONSE

The server answers with base64 of
`{"success":true,"transaction":"<payment hash>","network":"lnbtc:..."}`. It
never contains the preimage. `x402_read_response` accepts it only when
`success` is `true` and `transaction` is this offer's payment hash.

## Worked example: one top-up by hand

The fixture buys 50,000 msat (50 sat) of credit. None of these values is a
real invoice or payment.

- Preimage: the 32 bytes `01 02 ... 20`, written as
  `0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20`.
- Payment hash: SHA-256 of those bytes,
  `ae216c2ef5247a3782c135efa279a3e4cdc61094270f5d2be58c6204b7a612c9`.
- Invoice:
  `lnbc500n1p4wfve5pp54cskcth4y3ar0qkpxhh6y7drunxuvyy5yu8462l9333qfdaxztys`.
  It holds the human-readable part, a timestamp and the `p` field, and
  nothing else: no signature and no checksum.

### Step 1: the amount

The last `1` is at index 8, so the human-readable part is `lnbc500n`. The
currency is `bc`, mainnet. The amount is `500n`:

```jai
500 × 10^-9 BTC = 5 × 10^-7 BTC = 50 sat = 50,000 msat      (500 × 100 msat)
```

That equals the terms' `amount` of `"50000"`, so the invoice check passes.

### Step 2: the payment hash in the invoice

The data part starts after the separator. Each bech32 character is 5 bits,
its index in `qpzry9x8gf2tvdw0s3jn54khce6mua7l`.

- `p4wfve5` is the 35-bit timestamp: 1, 21, 14, 9, 12, 25, 20 in base 32 is
  1,793,372,980, which is 2026-10-30 15:09:40 UTC.
- `p` is field type 1, the payment hash, and `p5` is its length: 1 × 32 + 20
  = 52 characters, or 260 bits for 256 bits of hash and 4 bits of padding.
- The first four characters `4csk` are 21, 24, 16, 22:

```jai
10101 11000 10000 10110   →   10101110 00100001 0110...
                              ae       21       6...
```

The hash starts `ae21 6`, as the fixture's payment hash does. The example
decodes all 52 characters.

### Step 3: the challenge's size

The accepts entry is 425 bytes. Around it, `{"x402Version":2,"error":"payment
required","resource":{"url":"https://openagents.com/api/v1/inference/credit"},"accepts":[`
is 123 bytes, and `]}` is 2, so the JSON is 550 bytes. Base64 writes each 3
bytes as 4 characters: `550 = 3 × 183 + 1`, so it writes 183 full groups and
one group of 1 byte with `==`, `184 × 4 = 736` characters.

### Step 4: the retry's size

`{"x402Version":2,"accepted":` is 28 bytes, the entry 425, and
`,"payload":{"paymentHash":"<64>","preimage":"<64>"}}` is 172 bytes:
`28 + 425 + 172 = 625`. Then `625 = 3 × 208 + 1`, so the header is
`209 × 4 = 836` characters.

### Step 5: what the server decides

The server made the invoice at `issued = 1790000000` with
`maxTimeoutSeconds` 600. Its wallet's record says paid, 50,000 msat, with
the preimage above.

- At `now = issued + 30`: the amount matches, `1790000030 ≤ 1790000660`,
  paid, `50000 ≥ 50000`, SHA-256 of the preimage is the payment hash: 200.
- At `now = issued + 660`: still `≤ 1790000660`, so 200.
- At `now = issued + 661`: `1790000661 > 1790000660`, so `the invoice has
  expired`.
- A record whose preimage is 32 zero bytes: SHA-256 of 32 zero bytes is
  `66687aad...0d5f2925`, not `ae216c2e...`, so it refuses.

## The example

The example is
[nostr/examples/lightning-requests.coax](../examples/lightning-requests.coax).
It builds the worked example's challenge and reads it, checks the invoice's
amount and `p` field, checks the preimage, writes the retry, and settles it
three ways. Then it settles five records that refuse, and reads challenges
with one changed term each.

<!-- source: nostr/examples/lightning-requests.coax example -->
```jai
#load "../../coax/lib/x402/x402.coax";
#load "../../coax/lib/x402/settle.coax";
#import "Basic";

// Fixture values only: no real invoice, payment or wallet. The preimage is
// the bytes 01 02 ... 20; the payment hash is its SHA-256. The invoice is the
// human-readable part, a timestamp and the p field (the payment hash in
// bech32); it has no signature and no checksum, so no wallet would pay it.
PREIMAGE :: "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
HASH     :: "ae216c2ef5247a3782c135efa279a3e4cdc61094270f5d2be58c6204b7a612c9";
INVOICE  :: "lnbc500n1p4wfve5pp54cskcth4y3ar0qkpxhh6y7drunxuvyy5yu8462l9333qfdaxztys";
ZEROS    :: "0000000000000000000000000000000000000000000000000000000000000000";
ISSUED   :: 1790000000;     // when the server made the invoice
URL      :: "https://openagents.com/api/v1/inference/credit";

// The one accepts entry the server offers, in the Spark receiver profile.
entry_json :: (amount: string, scheme: string, network: string, asset: string, out: [] u8) -> string {
  n := json_raw(out, 0, "{\"scheme\":\""); n = json_raw(out, n, scheme);
  n = json_raw(out, n, "\",\"network\":\""); n = json_raw(out, n, network);
  n = json_raw(out, n, "\",\"amount\":\""); n = json_raw(out, n, amount);
  n = json_raw(out, n, "\",\"asset\":\""); n = json_raw(out, n, asset);
  n = json_raw(out, n, "\",\"payTo\":\"02ab\",\"maxTimeoutSeconds\":600,\"extra\":{\"method\":\"bolt11\",\"paymentFlow\":\"upfront\",\"receiverProfile\":\"openagents:spark-receiver:1\",\"invoice\":\"");
  n = json_raw(out, n, INVOICE); n = json_raw(out, n, "\",\"paymentHash\":\""); n = json_raw(out, n, HASH);
  n = json_raw(out, n, "\",\"requestDigest\":\"00\"}}");
  return cast(string) out[0..<n];
}
// The server's 402: PAYMENT-REQUIRED, base64 of the PaymentRequired JSON.
challenge :: (entry: string, version: string, json: [] u8, out: [] u8) -> string {
  n := json_raw(json, 0, "{\"x402Version\":"); n = json_raw(json, n, version);
  n = json_raw(json, n, ",\"error\":\"payment required\",\"resource\":{\"url\":\""); n = json_raw(json, n, URL);
  n = json_raw(json, n, "\"},\"accepts\":["); n = json_raw(json, n, entry); n = json_raw(json, n, "]}");
  m := x402_b64_encode(cast(string) json[0..<n], out);
  return cast(string) out[0..<m];
}
// The client reads a challenge with one changed term, and prints why it refuses.
refuse_terms :: (label: string, entry: string, version: string) {
  j: [3000] u8; h: [4000] u8; s: [3000] u8; o: [1] X402Offer;
  why := x402_read_required(challenge(entry, version, j[..], h[..]), o[..], s[..]);
  print("%: %\n", label, ifx why == "" then "accepted" else why);
}
// The payment hash in the invoice's p field, as hex: the field starts 10
// characters after the separator (7 of timestamp, then "pp5": type p, length
// 52). Each bech32 character is 5 bits; 52 of them are 260 bits, 256 used.
p_field :: (invoice: string, out: [] u8) -> string {
  set := "qpzry9x8gf2tvdw0s3jn54khce6mua7l"; digits := "0123456789abcdef";
  at := x402_bolt11_sep(invoice) + 11;
  if cast(string) invoice[at - 3..<at] != "pp5" { return ""; }
  acc := 0; bits := 0; n := 0;
  for k: 0..<52 {
    v := 0;
    for c: 0..<32 { if set[c] == invoice[at + k] { v = c; } }
    acc = ((acc << 5) | v) & 4095; bits += 5;
    if bits >= 8 && n < 64 { bits -= 8; b := (acc >> bits) & 255; out[n] = digits[b >> 4]; out[n + 1] = digits[b & 15]; n += 2; }
  }
  return cast(string) out[0..<n];
}
// The receiver's wallet's record of the invoice.
record :: (r: [] X402Record, paid: bool, msat: int, preimage: string) -> [] X402Record {
  r[0].paid = paid; r[0].msat = msat;
  for i: 0..<preimage.count { r[0].preimage[i] = preimage[i]; }
  r[0].np = preimage.count;
  return r;
}
// The server's settlement decision, as the HTTP answer.
settle :: (label: string, o: [] X402Offer, r: [] X402Record, payer: string, now: int) {
  why := x402_settle(o, r, payer, ISSUED, now);
  if why == "" { print("%: 200, settled\n", label); } else { print("%: 402, %\n", label, why); }
}

main :: () {
  print("the server's 402\n");
  tb: [1200] u8; jb: [3000] u8; hb: [4000] u8;
  entry := entry_json("50000", "exact", X402_MAINNET, "BTC", tb[..]);
  header := challenge(entry, "2", jb[..], hb[..]);
  print("accepts entry: % bytes\n", entry.count);
  print("PAYMENT-REQUIRED: % bytes, %...\n", header.count, header[0..<24]);

  print("the client reads it\n");
  o: [1] X402Offer; s: [3000] u8;
  why := x402_read_required(header, o[..], s[..]);
  print("read: %\n", ifx why == "" then "ok" else why);
  print("amount: % msat, maxTimeoutSeconds %\n", o[0].msat, o[0].timeout_s);
  print("invoice: %\n", cast(string) o[0].invoice[0..<o[0].ni]);
  print("invoice amount (500n): % msat\n", x402_bolt11_msat(cast(string) o[0].invoice[0..<o[0].ni]));
  print("invoice check: %\n", ifx x402_check_invoice(o[..]) == "" then "the terms' amount" else x402_check_invoice(o[..]));
  pb: [64] u8;
  p := p_field(INVOICE, pb[..]);
  print("p field: %\n", p);
  print("p field is paymentHash: %\n", p == cast(string) o[0].hash[0..<o[0].nh]);

  print("the wallet pays and returns the preimage\n");
  print("preimage: %\n", PREIMAGE);
  print("sha256(preimage) is the payment hash: %\n", x402_preimage_matches(PREIMAGE, HASH));
  print("sha256(zeros) is the payment hash: %\n", x402_preimage_matches(ZEROS, HASH));

  print("the retry\n");
  sig: [6000] u8; back: [4000] u8;
  n := x402_signature(o[..], PREIMAGE, sig[..]);
  m := x402_b64_decode(cast(string) sig[0..<n], back[..]);
  payload := cast(string) back[0..<m];
  print("PAYMENT-SIGNATURE: % bytes, decodes to % bytes\n", n, m);
  print("accepted entry unchanged: %\n", payload[28..<28 + entry.count] == entry);
  print("then: %\n", payload[29 + entry.count..<m - 1]);
  print("a preimage that is not 64 hex digits: %\n", x402_signature(o[..], "abc", sig[..]));

  print("the server settles\n");
  r: [1] X402Record;
  settle("paid, preimage sent", o[..], record(r[..], true, 50000, PREIMAGE), PREIMAGE, ISSUED + 30);
  settle("paid, no preimage sent", o[..], record(r[..], true, 50000, PREIMAGE), "", ISSUED + 30);
  settle("paid, at expiry + 60 s", o[..], record(r[..], true, 50000, PREIMAGE), "", ISSUED + 660);
  rb: [600] u8;
  k := x402_b64_encode("{\"success\":true,\"transaction\":\"ae216c2ef5247a3782c135efa279a3e4cdc61094270f5d2be58c6204b7a612c9\",\"network\":\"lnbtc:000000000019d6689c085ae165831e93\"}", rb[..]);
  print("PAYMENT-RESPONSE for this hash: %\n", x402_read_response(cast(string) rb[0..<k], o[..], s[..]));
  k = x402_b64_encode("{\"success\":true,\"transaction\":\"0e216c2ef5247a3782c135efa279a3e4cdc61094270f5d2be58c6204b7a612c9\"}", rb[..]);
  print("PAYMENT-RESPONSE for another hash: %\n", x402_read_response(cast(string) rb[0..<k], o[..], s[..]));

  print("refused settlements\n");
  settle("at expiry + 61 s", o[..], record(r[..], true, 50000, PREIMAGE), "", ISSUED + 661);
  settle("not paid yet", o[..], record(r[..], false, 0, ""), "", ISSUED + 30);
  settle("paid 49999 msat", o[..], record(r[..], true, 49999, PREIMAGE), "", ISSUED + 30);
  settle("wallet preimage zeros", o[..], record(r[..], true, 50000, ZEROS), "", ISSUED + 30);
  settle("payer preimage zeros", o[..], record(r[..], true, 50000, PREIMAGE), ZEROS, ISSUED + 30);

  print("refused terms\n");
  t2: [1200] u8;
  refuse_terms("x402 version 1", entry, "1");
  refuse_terms("scheme upto", entry_json("50000", "upto", X402_MAINNET, "BTC", t2[..]), "2");
  refuse_terms("testnet", entry_json("50000", "exact", "lnbtc:000000000933ea01ad0ee984209779ba", "BTC", t2[..]), "2");
  refuse_terms("an EVM network", entry_json("50000", "exact", "eip155:8453", "BTC", t2[..]), "2");
  refuse_terms("asset USDC", entry_json("50000", "exact", X402_MAINNET, "USDC", t2[..]), "2");
  refuse_terms("amount 50.5", entry_json("50.5", "exact", X402_MAINNET, "BTC", t2[..]), "2");
  o2: [1] X402Offer;
  read := x402_read_required(challenge(entry_json("60000", "exact", X402_MAINNET, "BTC", t2[..]), "2", jb[..], hb[..]), o2[..], s[..]);
  print("amount 60000, invoice 500n: read %, then %\n", ifx read == "" then "ok" else read, x402_check_invoice(o2[..]));
  settle("amount 60000, invoice 500n, paid", o2[..], record(r[..], true, 60000, PREIMAGE), "", ISSUED + 30);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/lightning-requests.coax --run-steps 250000000
```

The output matches the worked values: a 425-byte entry, a 736-byte
challenge, 50,000 msat from `500n`, a `p` field equal to the payment hash, an
836-byte retry, and a refusal at `issued + 661` but not at `issued + 660`.

```text
the server's 402
accepts entry: 425 bytes
PAYMENT-REQUIRED: 736 bytes, eyJ4NDAyVmVyc2lvbiI6Miwi...
the client reads it
read: ok
amount: 50000 msat, maxTimeoutSeconds 600
invoice: lnbc500n1p4wfve5pp54cskcth4y3ar0qkpxhh6y7drunxuvyy5yu8462l9333qfdaxztys
invoice amount (500n): 50000 msat
invoice check: the terms' amount
p field: ae216c2ef5247a3782c135efa279a3e4cdc61094270f5d2be58c6204b7a612c9
p field is paymentHash: true
the wallet pays and returns the preimage
preimage: 0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20
sha256(preimage) is the payment hash: true
sha256(zeros) is the payment hash: false
the retry
PAYMENT-SIGNATURE: 836 bytes, decodes to 625 bytes
accepted entry unchanged: true
then: "payload":{"paymentHash":"ae216c2ef5247a3782c135efa279a3e4cdc61094270f5d2be58c6204b7a612c9","preimage":"0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"}
a preimage that is not 64 hex digits: -1
the server settles
paid, preimage sent: 200, settled
paid, no preimage sent: 200, settled
paid, at expiry + 60 s: 200, settled
PAYMENT-RESPONSE for this hash: true
PAYMENT-RESPONSE for another hash: false
refused settlements
at expiry + 61 s: 402, the invoice has expired
not paid yet: 402, payment_pending
paid 49999 msat: 402, invalid_payment: less than the invoice
wallet preimage zeros: 402, invalid_payment: the preimage does not hash to the payment hash
payer preimage zeros: 402, invalid_payment: the payer's preimage is not the invoice's
refused terms
x402 version 1: not x402 version 2
scheme upto: not the exact scheme
testnet: not Bitcoin mainnet Lightning
an EVM network: not Bitcoin mainnet Lightning
asset USDC: not a bolt11 payment in BTC
amount 50.5: the amount is not a msat string
amount 60000, invoice 500n: read ok, then the invoice's amount is not the terms' amount
amount 60000, invoice 500n, paid: 402, the invoice's amount is not the terms' amount
```

The deliberately broken inputs are under `refused settlements` and
`refused terms`, plus a preimage that is not 64 hex digits (`-1`) and a
`PAYMENT-RESPONSE` for another hash (`false`). A wallet record
with the wrong preimage refuses with `invalid_payment: the preimage does not
hash to the payment hash`. Asset `USDC` refuses with `not a bolt11 payment
in BTC`, and an EVM network with `not Bitcoin mainnet Lightning`. Terms of
60,000 msat with a `500n` invoice read, because `x402.coax` does not decode
the invoice, but `x402_check_invoice` and `x402_settle` refuse them.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/lightning-requests.coax](../examples/checks/lightning-requests.coax),
which loads the example, so running the example doesn't run them. The
independent values come from Python's `hashlib`, `base64` and `json`
modules, and from BOLT #11's example invoices.

- The payment hash is Python's SHA-256 of the preimage, and SHA-256 of 32
  zero bytes is `66687aad...`.
- The invoice's `p` field decodes to that hash.
- Python built the same challenge and retry JSON and encoded it with
  `base64.b64encode`. The Coax headers have Python's lengths and SHA-256.
- The human-readable parts of BOLT #11's examples: `2500u` is 250,000 sat,
  `20m` is 0.02 BTC, `9678785340p` is 0.00967878534 BTC, and the donation
  invoice `lnbc1...` has no amount.
- The worked example's arithmetic: `500 × 100`, `(550 + 2) / 3 × 4`,
  `(625 + 2) / 3 × 4`, and the expiry boundary.

<!-- source: nostr/examples/checks/lightning-requests.coax check -->
```jai
// Values from Python's hashlib, base64 and json modules, which share no code
// with Coax. The challenge and the retry are the example's JSON, built in
// Python, base64-encoded by base64.b64encode; the constants are the length
// and the SHA-256 of that base64 text.
PY_SHA256_PREIMAGE :: "ae216c2ef5247a3782c135efa279a3e4cdc61094270f5d2be58c6204b7a612c9";   // sha256(bytes(range(1, 33)))
PY_SHA256_ZEROS    :: "66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925";   // sha256(bytes(32))
PY_REQUIRED_LEN :: 736;
PY_REQUIRED_SHA :: "36f7863c0ad1a5d05a049e04ac0db60198c682be444139e48f5cdda10cc2b678";
PY_SIGNATURE_LEN :: 836;
PY_SIGNATURE_SHA :: "ad83676b2033a7cd4d5146c2a96970c77ee38fec978b6e77fc48c18ae5b39e4f";

// SHA-256 of text as lowercase hex.
sha_hex :: (text: string, out: [] u8) -> string {
  b: [1000] u8; h: [32] u8; digits := "0123456789abcdef";
  for i: 0..<text.count { b[i] = text[i]; }
  #assert sha256_hash_checked(b[0..<text.count], h[..], false) == SHA_OK;
  for i: 0..<32 { out[2 * i] = digits[h[i] >> 4]; out[2 * i + 1] = digits[h[i] & 15]; }
  return cast(string) out[0..<64];
}

#test "the fixture's payment hash is Python's SHA-256 of the preimage" {
  #assert HASH == PY_SHA256_PREIMAGE;
  #assert x402_preimage_matches(PREIMAGE, PY_SHA256_PREIMAGE);
  #assert x402_preimage_matches(ZEROS, PY_SHA256_ZEROS) && !x402_preimage_matches(ZEROS, PY_SHA256_PREIMAGE);
}

#test "the invoice's p field decodes to the payment hash" {
  pb: [64] u8;
  #assert p_field(INVOICE, pb[..]) == PY_SHA256_PREIMAGE;
}

#test "PAYMENT-REQUIRED and PAYMENT-SIGNATURE are Python's base64 of the same JSON" {
  tb: [1200] u8; jb: [3000] u8; hb: [4000] u8; w: [64] u8;
  entry := entry_json("50000", "exact", X402_MAINNET, "BTC", tb[..]);
  header := challenge(entry, "2", jb[..], hb[..]);
  #assert header.count == PY_REQUIRED_LEN && sha_hex(header, w[..]) == PY_REQUIRED_SHA;
  o: [1] X402Offer; s: [3000] u8; sig: [6000] u8;
  #assert x402_read_required(header, o[..], s[..]) == "";
  n := x402_signature(o[..], PREIMAGE, sig[..]);
  #assert n == PY_SIGNATURE_LEN && sha_hex(cast(string) sig[0..<n], w[..]) == PY_SIGNATURE_SHA;
}

// BOLT #11's example invoices, their human-readable parts: 2500u is
// 250,000 sat, 20m is 0.02 BTC, 9678785340p is 0.00967878534 BTC, and
// lnbc1... (the donation example) has no amount.
#test "BOLT #11 example amounts" {
  #assert x402_bolt11_msat("lnbc2500u1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypq") == 250_000_000;
  #assert x402_bolt11_msat("lnbc20m1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypq") == 2_000_000_000;
  #assert x402_bolt11_msat("lnbc9678785340p1pwmna7lpp5gc3xfm08u9qy06djf8dfflhugl6p7lgza6dsjxq454gxhj9t7a0s") == 967_878_534;
  #assert x402_bolt11_msat("lnbc1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypq") == -1;
}

// The hand values of the chapter's worked example.
#test "the worked example's numbers" {
  #assert x402_bolt11_msat(INVOICE) == 500 * 100;            // 500n: 500 * 10^-9 BTC * 10^11 msat/BTC
  #assert (550 + 2) / 3 * 4 == PY_REQUIRED_LEN;              // 550 JSON bytes
  #assert (625 + 2) / 3 * 4 == PY_SIGNATURE_LEN;             // 625 JSON bytes
  o: [1] X402Offer; r: [1] X402Record; tb: [1200] u8; jb: [3000] u8; hb: [4000] u8; s: [3000] u8;
  #assert x402_read_required(challenge(entry_json("50000", "exact", X402_MAINNET, "BTC", tb[..]), "2", jb[..], hb[..]), o[..], s[..]) == "";
  #assert x402_settle(o[..], record(r[..], true, 50000, PREIMAGE), "", ISSUED, ISSUED + 600 + 60) == "";
  #assert x402_settle(o[..], record(r[..], true, 50000, PREIMAGE), "", ISSUED, ISSUED + 600 + 61) == "the invoice has expired";
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/lightning-requests.coax --run-steps 250000000
coax/bin/coax mutate nostr/examples/checks/lightning-requests.coax --root-only --run-steps 250000000
```

`coax test` runs 74 checks, including the loaded files' own checks. On
2026-11-01, root-only mutation of the checks file killed 21 of 21 mutants.

### The library's own checks

```shell
coax/bin/coax test coax/lib/x402/settle.coax --library
coax/bin/coax mutate coax/lib/x402/settle.coax --root-only --library
```

`settle.coax` carries four checks: every multiplier and each unit's
ceiling and one more, the invoice amount against the terms, preimages
against hashes, and every settlement refusal in order with the inclusive
grace boundary. On 2026-11-01, root-only mutation of `settle.coax` killed
189 of 189 mutants, none marked equivalent. Its run also runs `x402.coax`'s
six checks: RFC 4648 base64 vectors, msat strings, a challenge read byte for
byte, every refused term, the retry's exact JSON, and a response for this
hash only. Mutation of `x402.coax` itself and of the loaded SHA-256 code was
not run today.

The deployed route's SQL has its own tests in
[test_inference.py](../../coax/web/api/test_inference.py) against Postgres;
they were not run for this chapter.

## Summary

- x402 v2 over HTTP uses three base64 JSON headers: the terms in
  `PAYMENT-REQUIRED`, the proof in `PAYMENT-SIGNATURE`, and the settlement in
  `PAYMENT-RESPONSE`.
- The client takes only version 2, `exact`, Bitcoin mainnet Lightning, `BTC`,
  `bolt11`, `upfront` and the Spark receiver profile. Stablecoins, EVM
  networks and testnet refuse.
- The invoice's amount must equal the terms': `500n` is 50,000 msat.
- Settlement needs the wallet's record of the invoice paid, at least the
  amount, and a preimage whose SHA-256 is the payment hash, before the
  invoice's end plus 60 seconds.

## Dependencies

- [49. HTTP auth](ch049-http-auth.md): the same base64-in-a-header pattern,
  and the signed request that NIP-X402 can bind.
- [71. Lightning zaps and goals](ch071-lightning-zaps-and-goals.md), a later
  chapter, still a design draft: bolt11 invoices and preimages in Nostr zaps.
  This chapter uses none of its code.

## Sources

- [OpenAgents NIP-X402](../nips/openagents/NIP-X402.md)
- [x402 `exact` scheme on Lightning](https://github.com/x402-foundation/x402/blob/4fcf836cc393174130e1358577ce5d37356da1c3/specs/schemes/exact/scheme_exact_lnbtc.md),
  at the commit the NIP pins.
- [BOLT #11: Invoice Protocol for Lightning Payments](https://github.com/lightning/bolts/blob/master/11-payment-encoding.md)
- [The Breez Spark wallet plan](../../docs/lightning/README.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
