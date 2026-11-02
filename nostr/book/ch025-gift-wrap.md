# 25. Gift wrap

Gift wrap hides who sent a message and when. A rumor (an unsigned event) is
sealed by its author and wrapped by a one-time key. This chapter builds the
three layers from chapter 21's events and chapter 20's encryption, works out
one wrap by hand, and opens it again with every check NIP-59 asks for.

Part II, The event model. Result: NIP-59 rumors, kind-13 seals and kind-1059
gift wraps from one-time keys, with randomized timestamps, checked in both
directions against an independent implementation.

## What this chapter teaches

- Rumor: an event with an ID but no signature, so it can't be published or
  proved on its own.
- Seal: kind 13 by the author, empty tags, content the NIP-44 encryption of
  the rumor to the recipient.
- Wrap: kind 1059 (or 21059) from a fresh key, a `p` tag for the recipient,
  content the NIP-44 encryption of the seal.
- Timestamps: the seal and the wrap are dated up to two days in the past,
  from host randomness. Only the rumor keeps the real time.
- Unwrapping: verify the wrap, decrypt it, verify the seal, decrypt it, and
  check that the rumor is unsigned and that its pubkey equals the seal's.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip59.coax](../../coax/lib/nostr/nip59.coax) | `nip59_wrap_checked(author, recipient_x, now, kind, tags, content, wrap_kind, fresh, work, out, overlap)` | Builds the rumor, the seal and the wrap, and writes the wrap's JSON into `out` (`#authority`) |
| | `nip59_unwrap_checked(recipient, recipient_x, wrap, work, out, overlap)` | Opens a wrap for one recipient and writes the rumor's JSON into `out`, or refuses with one status (`#authority`) |
| | `n59_rumor`, `n59_sign`, `n59_ck`, `n59_addressed`, `n59_signed_copy` | The rumor writer, the signing loop, the conversation key, the `p` tag test and the copy that lets chapter 21's reader parse an unsigned event |
| [nip44.coax](../../coax/lib/nostr/nip44.coax) | `nip44_encrypt_checked`, `nip44_decrypt_checked` | Chapter 20's payloads, in the `NIP44_COMPAT` profile |
| [event_sign.coax](../../coax/lib/nostr/event_sign.coax), [event.coax](../../coax/lib/nostr/event.coax) | `nostr_event_sign_step`, `nostr_event_read` | Chapter 21's signer and reader |

The wrapper never draws randomness itself. The caller passes one
`Nip59Fresh` record that holds everything a wrap needs from the host: the
one-time secret key, a NIP-44 nonce for each layer, a BIP-340 aux value for
each signature, and two 64-bit draws that choose how far back the seal and
the wrap are dated. A real host fills it from the platform's random source
(Bitcoin chapter 18). The example fills it with fixed bytes, so its output repeats
and can be checked byte for byte. Every admitted return zeroes the record,
the workspace (which holds the rumor and seal in plaintext) and, on a
refusal, `out`.

Nothing allocates. The caller passes a workspace of `NIP59_WORK` bytes,
seven 64 KiB regions, and an output buffer.

## Three layers

NIP-59 defines three objects. Each one hides something the layer outside it
shows:

| Layer | Kind | Signed by | Encrypted to | Tags | `created_at` | Public to a relay |
| --- | --- | --- | --- | --- | --- | --- |
| Rumor | any (14 here) | nobody | (it is the plaintext) | the message's own | the real time | nothing; it is never sent in the clear |
| Seal | 13 | the author | the recipient, with the author's conversation key | none | real time minus a random offset | nothing; it is inside the wrap |
| Wrap | 1059 or 21059 | a one-time key | the recipient, with the one-time key's conversation key | `["p", recipient]` | real time minus another random offset | the recipient, a random key, a time up to two days off |

Each layer has a reason:

- **The rumor is unsigned.** If it leaks, nobody can prove the author wrote
  it: relays and clients refuse an event without a valid signature. NIP-59
  calls this deniability.
- **The seal is signed by the author.** It proves to the recipient who wrote
  the rumor. It has no `p` tag, so a seal on its own doesn't say who it is
  for.
- **The wrap is signed by a key used once.** A relay needs a signed event and
  a `p` tag to route the message. The one-time key gives it both without
  showing the author.

The seal's content is encrypted with the conversation key of the author and
the recipient. The wrap's content is encrypted with the conversation key of
the one-time key and the recipient. The recipient computes both from their
own secret key and the public key printed on each layer. Nobody else can.

### Why the times are randomized

If the wrap carried the real time, an observer who sees Alice's client go
online at 12:00:03 and a wrap for Bob appear at 12:00:03 could link the two.
NIP-59 says every timestamp except the rumor's should be tweaked, and all of
them should be in the past, because some relays refuse events dated in the
future. `n59_back` takes a host draw `r` and computes:

```jai
n59_back :: (now:int,r:u64)->int {return now-cast(int)(r%cast(u64)NIP59_WINDOW);}
```

`NIP59_WINDOW` is 172,800 seconds, two days. The result is never later than
`now` and never two full days earlier. The seal and the wrap use separate
draws, as NIP-59 asks, so their times don't match each other either.

## Worked example: one wrap by hand

Alice sends Bob "Are you going to the party tonight?" as a kind 14 chat
message. All keys are test keys:

| Who | Secret key | x-only public key |
| --- | --- | --- |
| Alice, the author | 3 | `f9308a01…bce036f9` |
| Bob, the recipient | BIP-340 vector 1's `b7e15162…5190cfef` | `dff1d77f…502ba659` |
| The one-time wrap key | 5 | `2f8bde4d…b240efe4` |
| Carol, who should not read it | 7 | `5cbdf064…cac4f9bc` |

The fixed entropy is: seal nonce 32 bytes of `0x11`, wrap nonce 32 bytes
of `0x22`, seal aux 32 bytes of `0x33`, wrap aux 32 bytes of `0x44`,
`seal_back` = 3,600 and `wrap_back` = 200,000. `now` is 1,793,300,000.

### Step 1: the rumor

The rumor is chapter 21's event without `sig`. Its ID is the SHA-256 of the
same preimage, `[0,pubkey,created_at,kind,tags,content]`:

```text
[0,"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",1793300000,14,[["p","dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"]],"Are you going to the party tonight?"]
```

Its SHA-256 is `2f28bd6c…c6476593`. The rumor's JSON is the six fields
`id`, `pubkey`, `created_at`, `kind`, `tags` and `content`: 278 bytes of
fixed text around the 35-byte content, 313 bytes in all.

### Step 2: the seal

The times come first:

| Layer | Draw | Draw mod 172,800 | `created_at` |
| --- | ---: | ---: | ---: |
| Rumor | (none) | 0 | 1,793,300,000 |
| Seal | 3,600 | 3,600 | 1,793,296,400 |
| Wrap | 200,000 | 27,200 | 1,793,272,800 |

The 200,000 draw is more than two days, so the modulo brings it back inside
the window.

Alice's conversation key with Bob, from chapter 20, is `a7bc353f…23c5291f`.
Chapter 20's padding takes the 313-byte rumor to 320 bytes: the next power
of two above 312 is 512, so the chunk is 512 / 8 = 64, and 64 × 5 = 320. The
payload is 1 version byte, 32 nonce bytes, 2 length bytes, 320 padded
bytes and a 32-byte MAC: 387 bytes, which base64 writes as 4 × 129 = 516
characters. It starts `AhERERER`: base64 of the version byte `0x02` and
then the `0x11` nonce bytes. The nonce is public; only the keys are secret.

The seal is a kind 13 event with `tags` `[]`, that content and Alice's
signature with the `0x33` aux. Its fixed text is 343 bytes, so the seal is
343 + 516 = 859 bytes.

### Step 3: the wrap

The one-time key's conversation key with Bob is `4f2b660a…40f3b0bc`. The
859-byte seal pads to 896 bytes (next power of two 1,024, chunk 128,
128 × 7 = 896). The payload is 1 + 32 + 2 + 896 + 32 = 963 bytes, which is
4 × 321 = 1,284 base64 characters starting `AiIiIiIi` (the `0x22` nonce).

The wrap is a kind 1059 event by the one-time key with `tags`
`[["p","dff1d77f…502ba659"]]`, signed with the `0x44` aux. Its fixed text is
the seal's 343 bytes, plus 2 for `1059` instead of `13`, plus 72 for the
`p` tag instead of `[]`: 417 bytes. The wrap is 417 + 1,284 = 1,701 bytes.

### Step 4: what Bob checks

`nip59_unwrap_checked` runs these steps in order and stops at the first one
that fails. Each failure has its own status:

| Step | Check | Status on failure |
| ---: | --- | --- |
| 1 | The wrap is a NIP-01 event whose ID is its own, of kind 1059 or 21059 | `NIP59_WRAP` |
| 2 | The wrap's signature verifies | `NIP59_WRAP_SIGNATURE` |
| 3 | A `["p", recipient]` tag names this recipient, in lowercase hex | `NIP59_RECIPIENT` |
| 4 | The content decrypts with the conversation key of the recipient and the wrap's pubkey | `NIP59_WRAP_PAYLOAD` |
| 5 | The plaintext is an event whose ID is its own, of kind 13, with no tags | `NIP59_SEAL` |
| 6 | The seal's signature verifies | `NIP59_SEAL_SIGNATURE` |
| 7 | Its content decrypts with the conversation key of the recipient and the seal's pubkey | `NIP59_SEAL_PAYLOAD` |
| 8 | The plaintext is an event with its own ID and no signature | `NIP59_RUMOR` |
| 9 | The rumor's pubkey is the seal's pubkey | `NIP59_IDENTITY` |

Step 9 is what makes the author real. The seal's signature proves who
sealed the rumor, but the rumor's own `pubkey` field is just text inside
it. Without step 9, Carol could seal a rumor that says it is from Alice, and
Bob's client would show it under Alice's name.

Step 8 needs one trick. Chapter 21's reader requires a `sig` field, and a
rumor has none. `n59_signed_copy` copies the rumor with a zero `sig` added
before its last brace, and the reader parses the copy and recomputes the ID.
The reader doesn't verify signatures, so the zeros are never checked. If the
rumor already had a `sig`, the copy has two, and the reader refuses the
duplicate key: a signed rumor is `NIP59_RUMOR`, as NIP-59 requires.

## The example

The example is [nostr/examples/gift-wrap.coax](../examples/gift-wrap.coax).
It wraps Alice's message for Bob with the fixed entropy, peels the wrap and
the seal one at a time with Bob's key, and opens the wrap with
`nip59_unwrap_checked`. It then shows nine refusals:

- three edits to the wrap: its kind, one content character and one
  signature digit;
- Carol opening Bob's wrap, first with her own public key and then claiming
  Bob's;
- three wraps built by `forge`, which seals and wraps any bytes and skips
  every rule the wrapper enforces: Alice's rumor sealed by Carol, a seal
  with a `p` tag, and a rumor that Alice signed;
- a wrap whose one-time key is Alice's own key, refused by the wrapper.

Last, it opens the wrap printed in NIP-59's own example with the recipient
key the NIP publishes. The NIP's two long constants are at the end of the
file, outside the listing.

<!-- source: nostr/examples/gift-wrap.coax example -->
```jai
#load "../../coax/lib/nostr/nip59.coax";
#import "Basic";

print_hex :: (label: string, bytes: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
STATUS :: [17] string.["ok", "input", "alias", "output", "capacity", "key", "fresh",
  "wrap", "wrap signature", "recipient", "wrap payload", "seal", "seal signature",
  "seal payload", "rumor", "identity", "internal"];

// Test keys only: Alice k=3, Bob is BIP-340 vector 1's secret key, Carol
// k=7, and the one-time wrap key k=5. Never put a real key in an example.
ALICE_HEX :: "0000000000000000000000000000000000000000000000000000000000000003";
BOB_HEX :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
CAROL_HEX :: "0000000000000000000000000000000000000000000000000000000000000007";
ONE_TIME_HEX :: "0000000000000000000000000000000000000000000000000000000000000005";
NOW :: 1793300000;
BOB_TAG :: "[[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"]]";
MESSAGE :: "Are you going to the party tonight?";

test_key :: (text: string) -> [32] u8 { k: [32] u8; decode_hex(text, k[..]); return k; }
// The x-only public key: the first stage of the event signer computes it.
x_only_key :: (secret: [] u8) -> [32] u8 {
  aux: [32] u8; sw: [1] NostrSignWork; scratch: [256] u8; out: [256] u8;
  #assert nostr_event_sign_step(secret, aux[..], 0, 1, "[]", "", sw[..], scratch[..], out[..], false) == EVENT_MORE;
  return sw[0].pub;
}
// What a host's random source would supply, fixed so the output repeats:
// the one-time key, two NIP-44 nonces, two BIP-340 aux values, and the two
// draws that pick how far back the seal and the wrap are dated.
fixed_entropy :: () -> [1] Nip59Fresh {
  f: [1] Nip59Fresh;
  f[0].wrap_key = test_key(ONE_TIME_HEX);
  for i: 0..<32 {
    f[0].seal_nonce[i] = 0x11; f[0].wrap_nonce[i] = 0x22;
    f[0].seal_aux[i] = 0x33; f[0].wrap_aux[i] = 0x44;
  }
  f[0].seal_back = 3600; f[0].wrap_back = 200000;
  return f;
}

// The content field of an event, unescaped, decrypted with the conversation
// key of secret and the event's pubkey. Returns the plaintext length or -1.
// With show, prints the layer's kind, time, signer and conversation key.
open_layer :: (secret: [] u8, label: string, text: string, out: [] u8, show: bool) -> int {
  scratch: [NIP59_REGION] u8; payload: [NIP59_REGION] u8; work: [2 * NIP59_REGION] u8;
  e := nostr_event_verify_vartime(text, scratch[..], false);
  if !e.verified { return -1; }
  ck: [32] u8;
  if n59_ck(secret, e.public_key[..], ck[..]) != NIP44_OK { return -1; }
  if show {
    print("%: kind %, created_at %, verified %\n", label, e.kind, e.created_at, e.verified);
    print_hex("  signed by", e.public_key[..]);
    print_hex("  conversation key", ck[..]);
  }
  n := json_unescape(text[e.content_from..<e.content_to], payload[..]);
  d := nip44_decrypt_checked(NIP44_COMPAT, ck[..], cast(string) payload[0..<n], work[..], out, false);
  if d.error != NIP44_OK { return -1; }
  return d.written;
}

// Opens text as recipient (secret, public key) and prints the status.
check_open :: (label: string, secret: [] u8, me: [] u8, text: string) {
  work: [NIP59_WORK] u8; out: [NIP59_REGION] u8;
  o := nip59_unwrap_checked(secret, me, text, work[..], out[..], false);
  print("%: %\n", label, STATUS[o.error]);
  if o.error == NIP59_OK {
    print_hex("  author", o.author[..]);
    print_hex("  wrap key", o.wrap_key[..]);
    print_hex("  rumor id", o.rumor_id[..]);
    print("  kind %, created_at %, content %\n", o.kind, o.created_at,
          cast(string) out[o.content_from..<o.content_to]);
  }
}
// text with its signature's last hex digit changed, written into out.
flip_last_sig_digit :: (text: string, out: [] u8) -> string {
  n := json_raw(out, 0, text);
  #assert n == text.count && out[n - 1] == '}' && out[n - 2] == '"';
  out[n - 3] = ifx out[n - 3] == '0' then cast(u8) '1' else cast(u8) '0';
  return cast(string) out[0..<n];
}
// text with the first occurrence of old replaced by new.
replace :: (text: string, old: string, new: string, out: [] u8) -> string {
  at := -1;
  for i: 0..<text.count - old.count + 1 {
    if at < 0 && text[i..<i + old.count] == old { at = i; }
  }
  #assert at >= 0;
  n := json_raw(out, 0, text[0..<at]);
  n = json_raw(out, n, new);
  n = json_raw(out, n, text[at + old.count..<text.count]);
  #assert n > 0;
  return cast(string) out[0..<n];
}

// Seals any rumor bytes with seal_key and seal_tags, and wraps the seal for
// Bob with the one-time key, skipping every rule nip59_wrap_checked enforces.
// This is how a forger would build a wrap; the reader must refuse it.
forge :: (seal_key: [] u8, seal_tags: string, rumor: [] u8, out: [] u8) -> int {
  bob_key := test_key(BOB_HEX); bob := x_only_key(bob_key[..]); one := test_key(ONE_TIME_HEX); f := fixed_entropy();
  ck: [32] u8; work: [2 * NIP59_REGION] u8; pay: [NIP59_REGION] u8;
  scratch: [NIP59_REGION] u8; seal: [NIP59_REGION] u8;
  #assert n59_ck(seal_key, bob[..], ck[..]) == NIP44_OK;
  p := nip44_encrypt_checked(NIP44_COMPAT, ck[..], f[0].seal_nonce[..], rumor, work[..], pay[..], false);
  sw: [1] NostrSignWork;
  s := n59_sign(seal_key, f[0].seal_aux[..], NOW - 3600, NIP59_SEAL_KIND, seal_tags,
                cast(string) pay[0..<p.written], sw[..], scratch[..], seal[..]);
  #assert s > 0 && n59_ck(one[..], bob[..], ck[..]) == NIP44_OK;
  q := nip44_encrypt_checked(NIP44_COMPAT, ck[..], f[0].wrap_nonce[..], seal[0..<s], work[..], pay[..], false);
  ww: [1] NostrSignWork;
  return n59_sign(one[..], f[0].wrap_aux[..], NOW - 27200, NIP59_WRAP_KIND, BOB_TAG,
                  cast(string) pay[0..<q.written], ww[..], scratch[..], out);
}

main :: () {
  alice := test_key(ALICE_HEX); bob := test_key(BOB_HEX); carol := test_key(CAROL_HEX);
  bob_pub := x_only_key(bob[..]); carol_pub := x_only_key(carol[..]);
  print("keys\n");
  one := test_key(ONE_TIME_HEX); alice_pub := x_only_key(alice[..]); one_pub := x_only_key(one[..]);
  print_hex("alice", alice_pub[..]); print_hex("bob", bob_pub[..]);
  print_hex("carol", carol_pub[..]); print_hex("one-time", one_pub[..]);

  print("wrap\n");
  fresh := fixed_entropy();
  work: [NIP59_WORK] u8; wrapped: [NIP59_REGION] u8;
  w := nip59_wrap_checked(alice[..], bob_pub[..], NOW, 14, BOB_TAG, MESSAGE, NIP59_WRAP_KIND,
                          fresh[..], work[..], wrapped[..], false);
  print("status: %, % bytes\n", STATUS[w.error], w.written);
  print_hex("rumor id", w.rumor_id[..]);
  print("rumor at %, seal at % (-%), wrap at % (-%)\n", NOW, w.seal_at, NOW - w.seal_at, w.wrap_at, NOW - w.wrap_at);
  print("entropy cleared: %\n", fresh[0].wrap_key[31] == 0 && fresh[0].seal_back == 0);
  wrap := cast(string) wrapped[0..<w.written];
  print("wrap: %\n", wrap);

  print("peel it by hand as bob\n");
  seal: [NIP59_REGION] u8; rumor: [NIP59_REGION] u8;
  s := open_layer(bob[..], "wrap", wrap, seal[..], true);
  print("seal (% bytes): %\n", s, cast(string) seal[0..<s]);
  r := open_layer(bob[..], "seal", cast(string) seal[0..<s], rumor[..], true);
  print("rumor (% bytes): %\n", r, cast(string) rumor[0..<r]);

  print("unwrap\n");
  check_open("bob opens it", bob[..], bob_pub[..], wrap);

  print("refusals\n");
  edit: [NIP59_REGION] u8;
  check_open("kind 1059 -> 1060", bob[..], bob_pub[..], replace(wrap, "1059", "1060", edit[..]));
  check_open("content first A -> B", bob[..], bob_pub[..], replace(wrap, "\"content\":\"A", "\"content\":\"B", edit[..]));
  check_open("sig last digit changed", bob[..], bob_pub[..], flip_last_sig_digit(wrap, edit[..]));
  check_open("carol opens it", carol[..], carol_pub[..], wrap);
  check_open("carol claims bob's key", carol[..], bob_pub[..], wrap);

  forged: [NIP59_REGION] u8; pre: [NIP59_REGION] u8; body: [NIP59_REGION] u8; id: [32] u8;
  n := n59_rumor(alice_pub[..], NOW, 14, BOB_TAG, MESSAGE, pre[..], body[..], id[..]);
  m := forge(carol[..], "[]", body[0..<n], forged[..]);
  check_open("alice's rumor sealed by carol", bob[..], bob_pub[..], cast(string) forged[0..<m]);
  m = forge(alice[..], BOB_TAG, body[0..<n], forged[..]);
  check_open("seal with a p tag", bob[..], bob_pub[..], cast(string) forged[0..<m]);
  aux: [32] u8; sw: [1] NostrSignWork;
  n = n59_sign(alice[..], aux[..], NOW, 14, BOB_TAG, MESSAGE, sw[..], pre[..], body[..]);
  m = forge(alice[..], "[]", body[0..<n], forged[..]);
  check_open("signed rumor", bob[..], bob_pub[..], cast(string) forged[0..<m]);

  fresh = fixed_entropy(); fresh[0].wrap_key = alice;
  w = nip59_wrap_checked(alice[..], bob_pub[..], NOW, 14, BOB_TAG, MESSAGE, NIP59_WRAP_KIND,
                         fresh[..], work[..], wrapped[..], false);
  print("wrap with alice's key as the one-time key: %\n", STATUS[w.error]);

  print("NIP-59's example\n");
  recipient := test_key(NIP59_RECIPIENT_HEX); recipient_pub := x_only_key(recipient[..]);
  check_open("as published", recipient[..], recipient_pub[..], NIP59_EXAMPLE_PUBLISHED);
  check_open("trailing comma removed", recipient[..], recipient_pub[..], NIP59_EXAMPLE);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/gift-wrap.coax --run-steps 250000000
```

The secp256k1 tables are built at compile time and need more than the
default step budget, hence `--run-steps`. The program prints:

```text
keys
alice: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
bob: dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
carol: 5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc
one-time: 2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4
wrap
status: ok, 1701 bytes
rumor id: 2f28bd6c3f405f933b16e0737cfbe321afed12150c7af45093c005bdc6476593
rumor at 1793300000, seal at 1793296400 (-3600), wrap at 1793272800 (-27200)
entropy cleared: true
wrap: {"id":"dee716d487d997ac7b96dd52059fef87d018c51cab9aa0c229b92651621d2378","pubkey":"2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4","created_at":1793272800,"kind":1059,"tags":[["p","dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"]],"content":"AiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIix1GJXHcZ+mlPEiFxleaUhIdz8zu6lNYKmxg7Mk6sVXcHxmHtqaBD1ehGrUnMFL61D1d2idSkck+7/jU2ZmriAeczAh2O9ObAJy15tNI6ly4l5pZrNlkjOvMgea79P8ASrtoryFrXslbzGeiA9DuYd8w/3BG/dZGBsKx/qA9pPk6N3s//EXoMh02w7siZAPyekcx3HMn0yfC4KF5srfLKESGfgc2uw0hx38ygSntu+6Hmm9W50/JrBclh8txd/7SoRClaDfvjgDnONz5iqxaxlP7PspMLCBCJu3zz3ZHkWf2gPwArcMCmIBQ8mPiiUb3NZ8sNzoB5wbN5813NTXou6xYHwrDiRxMARvoofPQTmlpUQIx2LAiVT2+EeQBrr8gc7gXGFtT1kC0E72E5rSd4jqQBYBuoVAuUlfbl/0JQZpyF1diGQ1Qw+cweSOSg11ToNFk06rFi3PoZPUZtidD6gEyR/RhfK/M6LaidQXLLhD+5CzGQLTDoVY7bVbwe5MzXCStZ0cGzDzNyJ2i6C/c7ZzVqxwDGvzlpRD85pMZJ870xXCzkvtpALeIsOfQTESv9W1fmkavd8CfYmqvMln+hNB3427hOqitz4rIAapem1gW2qKsnbwVRmCamNyeW/JP7F17XZD2Cv6DyEHtcDr7yijl8iaQX05zzbDxpod26y2oOoCd8NnKes6m29plvV5eH3oiJaRTCXw2UkPG5dfIjugUOo2azcVUtPVWQEbGSmd1MSKZIS+tAPRGkH3pRyRLSlHnpBXgrsTALynf1F2NDnYn3peEFuWa/HY5218i+F0SnOPHal5LNKgZRDwA7RIUR8fZ1PLAwSa5oUgm9RSS5SDUSkfMXhP7lUge5GIsxCHH65F3yRrthoBSHIehkKME118hs9e+K3tFZAKb0yhtLzVrTx5TxRXC3ruimB7dnyb/ENR1ORtara3uCAN3pb6oZFUEsUlmk7LPgO4vEesOOgTF2TKsQ6nBqlpniEtlxbdlBuLQX4OZFeM6vZaQnu8k9200Cl5HU6mE+oqlyOgATCT6Y47rdsWo5vUom/n7xs+nnVvrDURloXJJjh/sBrS298LtDPT4LSiUTNFMc6LB8FCEm+4uZuQQ0LqWmPNNQIbOX9kbVAhzC5PLOBpFzgnH02U6rXPTa0ur9B/5UtaRlzq1u7GTqVwOpiqYKrHpunOyZ2ByZtHWf8wH/kJfZJGacZ1PqUCxihEMbg43tBm9FeZrz","sig":"24a9d69e5d96f36b744f63d34ea05dd2d57a31ba7a0a6cfa9e33f917c3ee71ca5058832bb054625883d308db687cbbe1cc9629ece0f9c449766eb32201f56713"}
peel it by hand as bob
wrap: kind 1059, created_at 1793272800, verified true
  signed by: 2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4
  conversation key: 4f2b660a4e72bc9036cf9171ddf4aae0025967beb20d2ee610e8adef40f3b0bc
seal (859 bytes): {"id":"10ab74e701a4a2d04303db4ef15c914386ab4a050470c4008f157221c28a0e2f","pubkey":"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","created_at":1793296400,"kind":13,"tags":[],"content":"AhERERERERERERERERERERERERERERERERERERERERERHJa6jG4w2PI7Vsjqo5olWJi8uAMy33jsuNsbuq0x62NgGBvZPNP9hjk1Q50TW3Q+8Ja46gU0vaqCTh1jtH16xZNK0hZM5y4V0Kecx382Pkl17dKDSC8RGKUMwOoVAQphtlhxG/6UlUw44bKyE09cy8WGatPKqxTNOW+pON4ikBSRRli2KvLHx+TDT0hjO7OSrRfaHweVu2jW3Css+JVNPFbeXBU/GCpI0ZN9lGK0Xd1ntjr+6vdnhD5pIAgvl+0/6JoP9x1Sc50F0Xrw75MfbjhTVmxGi5Hym3ivjYwhYBj/5nVZnA7O2M8gDhojTAagvh5l3DoqgOCFGWTa0Pt4kccK/MwUZr1LLhlMJx8+r1yCCtHibtKS7CspT2dC8ycoZXsAX+XOexOz3kIBWvynWKFAUkYqQ1o6uxfQ612eg6bcTaFz2MiHDwAaeFZVzoEBfi3hY1+8wGi8aa8NZV9+dUso","sig":"0e994d7ca8d900a50013bf5245d46c33c67985d5eba723485bf9d95e462f48ab56edf0fff9f1578617d4c7479e81c0456bcb767b4c1f00b1672bfdacc1d3035f"}
seal: kind 13, created_at 1793296400, verified true
  signed by: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
  conversation key: a7bc353f42fdfdd67d252471c994e73503f1b494108c63135bff8b0b23c5291f
rumor (313 bytes): {"id":"2f28bd6c3f405f933b16e0737cfbe321afed12150c7af45093c005bdc6476593","pubkey":"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9","created_at":1793300000,"kind":14,"tags":[["p","dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"]],"content":"Are you going to the party tonight?"}
unwrap
bob opens it: ok
  author: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
  wrap key: 2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4
  rumor id: 2f28bd6c3f405f933b16e0737cfbe321afed12150c7af45093c005bdc6476593
  kind 14, created_at 1793300000, content Are you going to the party tonight?
refusals
kind 1059 -> 1060: wrap
content first A -> B: wrap
sig last digit changed: wrap signature
carol opens it: recipient
carol claims bob's key: wrap payload
alice's rumor sealed by carol: identity
seal with a p tag: seal
signed rumor: rumor
wrap with alice's key as the one-time key: fresh
NIP-59's example
as published: wrap
trailing comma removed: ok
  author: 611df01bfcf85c26ae65453b772d8f1dfd25c264621c0277e1fc1518686faef9
  wrap key: 18b1a75918f1f2c90c23da616bce317d36e348bcf5f7ba55e75949319210c87c
  rumor id: 9dd003c6d3b73b74a85a9ab099469ce251653a7af76f523671ab828acd2a0ef9
  kind 1, created_at 1691518405, content Are you going to the party tonight?
```

What the output shows:

- The rumor ID, the three times and the 859-byte seal and 1,701-byte wrap
  are the values worked out above. The two conversation keys match too.
- Peeling by hand shows each layer's signer: the one-time key on the wrap,
  Alice on the seal. `nip59_unwrap_checked` opens the same rumor and
  reports the same two keys as the author and the wrap key. The rumor keeps
  the real time 1,793,300,000.
- `entropy cleared: true`: the wrapper zeroed the one-time key and the
  draws after use.
- Changing the kind or one content character breaks the wrap's ID
  (`wrap`); changing one signature digit keeps the ID and fails the
  signature (`wrap signature`).
- Carol's own key isn't in the `p` tag (`recipient`). When Carol claims
  Bob's public key, the tag matches, but her conversation key with the
  one-time key is not Bob's, and the MAC check fails (`wrap payload`).
- The three forged wraps pass every outer check and fail at steps 9, 5 and
  8 of the table.
- The wrapper refuses to use the author's key as the one-time key
  (`fresh`): the wrap would name its author.
- NIP-59's example wrap, as printed, has a comma after its last field. That
  is not JSON, and the reader refuses it at step 1. Without the comma it
  opens: the author is `611df01b…686faef9`, the rumor is a kind 1 event and
  the content is the NIP's sentence.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/gift-wrap.out](../fixtures/gift-wrap.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/gift-wrap.coax](../examples/checks/gift-wrap.coax),
which loads the example. The independent values come from
[nip59_reference.py](../../coax/tests/nostr/nip59_reference.py), a
pure-Python NIP-59 written for this chapter from NIP-01, NIP-44, NIP-59,
BIP-340 and RFC 8439. It uses integers, `hashlib`, `hmac` and `base64`,
and shares no code with Coax. Given the same keys, nonces, aux values and
draws, it writes the rumor, the seal and the wrap, then opens its own wrap
for Bob with all nine checks. It also opens NIP-59's example wrap with the
recipient's key, which checks the Python NIP-44 and BIP-340 code against
data its author didn't write.

The checks compare:

- the wrap from `nip59_wrap_checked` with Python's wrap, byte for byte, and
  its rumor ID and times;
- Python's wrap peeled by Coax with Python's seal, and opened by Coax with
  Python's rumor, so each implementation's wrap opens in the other;
- NIP-59's example opened by Coax with the author and rumor ID that Python
  reads from it, and the published text refused;
- the `forge` wrap of Alice's rumor sealed by Carol refused as
  `NIP59_IDENTITY`, with `out` cleared.

<!-- source: nostr/examples/checks/gift-wrap.coax check -->
```jai
// Chapter 25's rumor, seal and wrap as written by
// coax/tests/nostr/nip59_reference.py: a pure-Python NIP-59 (integers,
// hashlib, hmac and base64, with ChaCha20, NIP-44 v2 and BIP-340 written from
// their documents; it shares no code with Coax) given the same test keys,
// nonces, aux bytes and time draws.
PY_RUMOR :: "{\"id\":\"2f28bd6c3f405f933b16e0737cfbe321afed12150c7af45093c005bdc6476593\",\"pubkey\":\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"created_at\":1793300000,\"kind\":14,\"tags\":[[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"]],\"content\":\"Are you going to the party tonight?\"}";
PY_SEAL :: "{\"id\":\"10ab74e701a4a2d04303db4ef15c914386ab4a050470c4008f157221c28a0e2f\",\"pubkey\":\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\",\"created_at\":1793296400,\"kind\":13,\"tags\":[],\"content\":\"AhERERERERERERERERERERERERERERERERERERERERERHJa6jG4w2PI7Vsjqo5olWJi8uAMy33jsuNsbuq0x62NgGBvZPNP9hjk1Q50TW3Q+8Ja46gU0vaqCTh1jtH16xZNK0hZM5y4V0Kecx382Pkl17dKDSC8RGKUMwOoVAQphtlhxG/6UlUw44bKyE09cy8WGatPKqxTNOW+pON4ikBSRRli2KvLHx+TDT0hjO7OSrRfaHweVu2jW3Css+JVNPFbeXBU/GCpI0ZN9lGK0Xd1ntjr+6vdnhD5pIAgvl+0/6JoP9x1Sc50F0Xrw75MfbjhTVmxGi5Hym3ivjYwhYBj/5nVZnA7O2M8gDhojTAagvh5l3DoqgOCFGWTa0Pt4kccK/MwUZr1LLhlMJx8+r1yCCtHibtKS7CspT2dC8ycoZXsAX+XOexOz3kIBWvynWKFAUkYqQ1o6uxfQ612eg6bcTaFz2MiHDwAaeFZVzoEBfi3hY1+8wGi8aa8NZV9+dUso\",\"sig\":\"0e994d7ca8d900a50013bf5245d46c33c67985d5eba723485bf9d95e462f48ab56edf0fff9f1578617d4c7479e81c0456bcb767b4c1f00b1672bfdacc1d3035f\"}";
PY_WRAP :: "{\"id\":\"dee716d487d997ac7b96dd52059fef87d018c51cab9aa0c229b92651621d2378\",\"pubkey\":\"2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4\",\"created_at\":1793272800,\"kind\":1059,\"tags\":[[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"]],\"content\":\"AiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIix1GJXHcZ+mlPEiFxleaUhIdz8zu6lNYKmxg7Mk6sVXcHxmHtqaBD1ehGrUnMFL61D1d2idSkck+7/jU2ZmriAeczAh2O9ObAJy15tNI6ly4l5pZrNlkjOvMgea79P8ASrtoryFrXslbzGeiA9DuYd8w/3BG/dZGBsKx/qA9pPk6N3s//EXoMh02w7siZAPyekcx3HMn0yfC4KF5srfLKESGfgc2uw0hx38ygSntu+6Hmm9W50/JrBclh8txd/7SoRClaDfvjgDnONz5iqxaxlP7PspMLCBCJu3zz3ZHkWf2gPwArcMCmIBQ8mPiiUb3NZ8sNzoB5wbN5813NTXou6xYHwrDiRxMARvoofPQTmlpUQIx2LAiVT2+EeQBrr8gc7gXGFtT1kC0E72E5rSd4jqQBYBuoVAuUlfbl/0JQZpyF1diGQ1Qw+cweSOSg11ToNFk06rFi3PoZPUZtidD6gEyR/RhfK/M6LaidQXLLhD+5CzGQLTDoVY7bVbwe5MzXCStZ0cGzDzNyJ2i6C/c7ZzVqxwDGvzlpRD85pMZJ870xXCzkvtpALeIsOfQTESv9W1fmkavd8CfYmqvMln+hNB3427hOqitz4rIAapem1gW2qKsnbwVRmCamNyeW/JP7F17XZD2Cv6DyEHtcDr7yijl8iaQX05zzbDxpod26y2oOoCd8NnKes6m29plvV5eH3oiJaRTCXw2UkPG5dfIjugUOo2azcVUtPVWQEbGSmd1MSKZIS+tAPRGkH3pRyRLSlHnpBXgrsTALynf1F2NDnYn3peEFuWa/HY5218i+F0SnOPHal5LNKgZRDwA7RIUR8fZ1PLAwSa5oUgm9RSS5SDUSkfMXhP7lUge5GIsxCHH65F3yRrthoBSHIehkKME118hs9e+K3tFZAKb0yhtLzVrTx5TxRXC3ruimB7dnyb/ENR1ORtara3uCAN3pb6oZFUEsUlmk7LPgO4vEesOOgTF2TKsQ6nBqlpniEtlxbdlBuLQX4OZFeM6vZaQnu8k9200Cl5HU6mE+oqlyOgATCT6Y47rdsWo5vUom/n7xs+nnVvrDURloXJJjh/sBrS298LtDPT4LSiUTNFMc6LB8FCEm+4uZuQQ0LqWmPNNQIbOX9kbVAhzC5PLOBpFzgnH02U6rXPTa0ur9B/5UtaRlzq1u7GTqVwOpiqYKrHpunOyZ2ByZtHWf8wH/kJfZJGacZ1PqUCxihEMbg43tBm9FeZrz\",\"sig\":\"24a9d69e5d96f36b744f63d34ea05dd2d57a31ba7a0a6cfa9e33f917c3ee71ca5058832bb054625883d308db687cbbe1cc9629ece0f9c449766eb32201f56713\"}";
// What the same Python script reads from NIP-59's example with the
// recipient's published key: the seal's author and the rumor's id.
PY_NIP59_AUTHOR :: "611df01bfcf85c26ae65453b772d8f1dfd25c264621c0277e1fc1518686faef9";
PY_NIP59_RUMOR_ID :: "9dd003c6d3b73b74a85a9ab099469ce251653a7af76f523671ab828acd2a0ef9";

same_hex :: (bytes: [32] u8, text: string) -> bool {
  want: [32] u8; decode_hex(text, want[..]);
  for i: 0..<32 { if bytes[i] != want[i] { return false; } }
  return true;
}
open_as :: (secret_hex: string, text: string, out: [] u8) -> Nip59Opened {
  secret := test_key(secret_hex); me := x_only_key(secret[..]); work: [NIP59_WORK] u8;
  return nip59_unwrap_checked(secret[..], me[..], text, work[..], out, false);
}

#test "the fixed-entropy wrap is the Python reference's, byte for byte" #steps 400000000 {
  alice := test_key(ALICE_HEX); bob := test_key(BOB_HEX); bob_pub := x_only_key(bob[..]);
  fresh := fixed_entropy(); work: [NIP59_WORK] u8; out: [NIP59_REGION] u8;
  w := nip59_wrap_checked(alice[..], bob_pub[..], NOW, 14, BOB_TAG, MESSAGE, NIP59_WRAP_KIND,
                          fresh[..], work[..], out[..], false);
  #assert w.error == NIP59_OK && w.written == PY_WRAP.count && cast(string) out[0..<w.written] == PY_WRAP;
  #assert same_hex(w.rumor_id, "2f28bd6c3f405f933b16e0737cfbe321afed12150c7af45093c005bdc6476593");
  #assert w.seal_at == NOW - 3600 && w.wrap_at == NOW - 27200;
}
#test "the Python wrap peels to the Python seal and opens to the Python rumor" #steps 400000000 {
  bob := test_key(BOB_HEX); seal: [NIP59_REGION] u8; out: [NIP59_REGION] u8;
  s := open_layer(bob[..], "wrap", PY_WRAP, seal[..], false);
  #assert s == PY_SEAL.count && cast(string) seal[0..<s] == PY_SEAL;
  o := open_as(BOB_HEX, PY_WRAP, out[..]);
  #assert o.error == NIP59_OK && cast(string) out[0..<o.written] == PY_RUMOR;
  alice := test_key(ALICE_HEX); alice_pub := x_only_key(alice[..]);
  for i: 0..<32 { #assert o.author[i] == alice_pub[i]; }
}
#test "NIP-59's example opens with the recipient's key once its trailing comma is gone" #steps 400000000 {
  out: [NIP59_REGION] u8;
  o := open_as(NIP59_RECIPIENT_HEX, NIP59_EXAMPLE, out[..]);
  #assert o.error == NIP59_OK && o.kind == 1 && o.created_at == 1691518405;
  #assert same_hex(o.author, PY_NIP59_AUTHOR) && same_hex(o.rumor_id, PY_NIP59_RUMOR_ID);
  #assert cast(string) out[o.content_from..<o.content_to] == "Are you going to the party tonight?";
  #assert open_as(NIP59_RECIPIENT_HEX, NIP59_EXAMPLE_PUBLISHED, out[..]).error == NIP59_WRAP;
}
#test "a rumor by Alice sealed by Carol refuses as identity, and clears out" #steps 400000000 {
  alice := test_key(ALICE_HEX); carol := test_key(CAROL_HEX); alice_pub := x_only_key(alice[..]);
  pre: [NIP59_REGION] u8; body: [NIP59_REGION] u8; id: [32] u8; forged: [NIP59_REGION] u8; out: [NIP59_REGION] u8;
  n := n59_rumor(alice_pub[..], NOW, 14, BOB_TAG, MESSAGE, pre[..], body[..], id[..]);
  m := forge(carol[..], "[]", body[0..<n], forged[..]);
  #assert m > 0 && open_as(BOB_HEX, cast(string) forged[0..<m], out[..]).error == NIP59_IDENTITY;
  for b: out { #assert b == 0; }
}
#test "flip_last_sig_digit changes exactly the last signature digit" {
  out: [16] u8;
  #assert flip_last_sig_digit("{\"sig\":\"a0\"}", out[..]) == "{\"sig\":\"a1\"}";
  #assert flip_last_sig_digit("{\"sig\":\"a7\"}", out[..]) == "{\"sig\":\"a0\"}";
}
#test "replace changes exactly the first occurrence" {
  out: [16] u8;
  #assert replace("abab", "ab", "x", out[..]) == "xab";
  #assert replace("abab", "ba", "", out[..]) == "ab";
}
#test "same_hex notices a change in the first or last byte" {
  b: [32] u8; zero := "0000000000000000000000000000000000000000000000000000000000000000";
  #assert same_hex(b, zero);
  b[0] = 1; #assert !same_hex(b, zero);
  b[0] = 0; b[31] = 1; #assert !same_hex(b, zero);
}
```

To run them:

```shell
python3 coax/tests/nostr/nip59_reference.py
coax/bin/coax test nostr/examples/checks/gift-wrap.coax --run-steps 250000000
coax/bin/coax mutate nostr/examples/checks/gift-wrap.coax --root-only --run-steps 250000000
```

On 2026-11-01, `coax test` ran 434 checks, including the loaded files' own
checks, in about 30 seconds. The wrap uses 152 million interpreter steps,
the peel and open of Python's wrap 199 million, NIP-59's example 103 million
and the forged wrap 249 million. Root-only mutation of the checks file
killed 8 of 8 mutants. The example's own procedures and the library were not
mutated for this chapter. The example takes about 30 seconds to compile and
run in each build mode.

### The library's own checks

[nip59.coax](../../coax/lib/nostr/nip59.coax) has checks that need no curve
work: the status values and size limits, the two-day window for six
draws from 0 to 2^64 - 1, the `p` tag matcher against two right tags and
seven wrong ones, the
zero-signature copy, the rumor writer against nostr-effect's rumor byte for
byte, and every view, kind, time and size refusal before any key work.

The end-to-end checks are in
[nip59_checks.coax](../../coax/tests/nostr/nip59_checks.coax). They open
wraps made by nostr-effect, the library that the hidden-deck sealer in
`coax/web/unlock/lib.ts` uses, generated by
[nip59_fixtures.ts](../../coax/tests/nostr/nip59_fixtures.ts), and refuse
nostr-effect wraps built to break each step: no `p` tag, another `p` tag, a
wrap for Carol that names Bob, a bad seal signature, a seal of the wrong
kind, with tags or with a wrong ID, a signed rumor, a rumor with a wrong ID
and an impersonated author. They also wrap and open the largest rumor, 28,672
bytes, and refuse one byte more.
[nip59_interop.ts](../../coax/tests/nostr/nip59_interop.ts) runs the other
direction: nostr-effect opens three wraps that
[nip59_dump.coax](../../coax/tests/nostr/nip59_dump.coax) prints, and Coax
opens a fresh nostr-effect wrap. Commit `dd941f8f31` (2026-10-31) records
both directions passing. The nostr-effect run and a mutation run of
`nip59.coax` were not done for this chapter, and there is no separate
verification record for them yet.

## Summary

- A rumor is an event with an ID and no signature. A seal (kind 13) carries
  it encrypted from the author to the recipient and is signed by the author.
  A wrap (kind 1059 or 21059) carries the seal encrypted from a one-time key
  to the recipient and is signed by that key.
- A relay sees only the wrap: the recipient, a random key and a time up to
  two days in the past. The seal and the wrap take separate random offsets;
  only the rumor keeps the real time.
- All randomness comes from the host in one `Nip59Fresh` record, which the
  wrapper zeroes after use. Fixed values give a repeatable wrap for tests.
- Unwrapping runs nine checks in order. The last, that the rumor's pubkey is
  the seal's, is what stops one sender from writing as another.
- Coax's wrap and an independent Python wrap are the same bytes, each opens
  in the other, and NIP-59's own example opens once its stray comma is
  removed.

## Dependencies

- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [Bitcoin chapter 24. Sign with Schnorr](../../bitcoin/book/ch024-sign-with-schnorr.md)

## Sources

- [NIP-59](../nips/official/59.md), the three layers, the timestamp rule
  and the example the chapter opens.
- [NIP-44](../nips/official/44.md), the encryption of the seal and the
  wrap.
- [NIP-01](../nips/official/01.md), the event and its ID.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
