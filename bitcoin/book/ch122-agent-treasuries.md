# 122. Agent treasuries

> **Status: the scripts, the policy and the PSBT flow run; the network
> half waits for the node.** This chapter builds an agent treasury's two
> output forms, the agent's signing policy and the PSBT flow between the
> agent and its owner. Chapter 36's interpreter runs every P2WSH spend,
> and Python checks every line. Broadcasting a spend on signet and
> recovering the coins with an independent wallet tool need a node, so
> the plan's "Done means" line holds only after chapters 80, 93 and 118.
> See [What needs a node](#what-needs-a-node).

An OpenAgents agent earns bitcoin, and it has to spend some of it
without asking anyone: fees for the work it buys, refunds, small
payments. If one key on the agent holds everything, then whoever takes
over the agent's host takes everything. If the owner signs every spend,
then the agent can't work while the owner sleeps. This chapter splits
the money in two. A **treasury** holds most of the coins under a 2-of-3
of the agent, the owner and a recovery key, with the recovery key alone
after a delay. A **hot wallet** holds what the agent may spend alone. The
agent refills the hot wallet from the treasury, and it adds its
signature to a treasury spend only when that spend is a refill within
the limits the owner set.

The Nostr book's
[chapter 104](../../nostr/book/ch104-agent-treasuries-and-a-network-of-keys.md)
builds the records around such a wallet: NIP-KEYS descriptors, admission
of independent key agents, and the signing requests that travel between
them. This chapter is the Bitcoin side: the scripts the coins sit in,
the rule the agent applies before it signs, and the transactions that
move the coins, run through Bitcoin's own interpreter.

The worked example uses four fixed test keys and a delay of 4,320 blocks,
about 30 days.

## What this chapter teaches

- A 2-of-3 of the agent, the owner and a recovery key, with the recovery
  key alone after a relative delay, as one P2WSH script.
- The same policy as a Taproot output: a MuSig2 key path for the agent
  and the owner, a `multi_a` leaf for any two of the three, and a
  recovery leaf.
- Why the agent's spending limit is a separate wallet and not a script.
- What the agent signs alone, what it co-signs, and what it never signs.
- The agent's check of a treasury PSBT, and the PSBT flow between the
  agent and the owner.
- Timelocked recovery with BIP-68's sequence and BIP-112's
  `OP_CHECKSEQUENCEVERIFY`, and why the owner has to keep coins young.
- What waits for the node: broadcast, confirmation ages and rescans.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [treasury.coax](../../coax/lib/bitcoin/treasury.coax) | `treasury_wsh_script_vartime(agent, owner, recovery, delay, out)` | the P2WSH witness script |
| | `treasury_wsh_spk_vartime(script, out)` | its P2WSH scriptPubKey |
| | `treasury_tr_vartime(agent, owner, recovery, delay, t)` | the Taproot form: MuSig2 internal key, two leaves, tree and output key |
| | `treasury_check_vartime(m, treasury, hot, policy, w)` | what a treasury PSBT does, and whether the agent may sign it |
| | `treasury_agent_sign_vartime(m, key, treasury, hot, policy, w)` | that check, then chapter 94's signer |
| | `treasury_finalize_vartime(m, script, w)` | the witness of a treasury input, by the primary or the recovery path |
| [psbt.coax](../../coax/lib/bitcoin/psbt.coax) | `psbt_create_vartime`, `psbt_set1_vartime`, `psbt_sign_vartime`, `psbt_extract_vartime` | chapter 94: the PSBT roles |
| [musig.coax](../../coax/lib/bitcoin/musig.coax) | `ms_key_agg_vartime`, `ms_nonce_gen`, `ms_sign`, `ms_partial_agg_vartime` | chapter 101: MuSig2 |
| [taproot.coax](../../coax/lib/bitcoin/taproot.coax) | `taproot_leaf_hash_vartime`, `taproot_tree_vartime`, `taproot_output_key_vartime`, `taproot_control_block_vartime` | chapters 41 and 42: leaves, trees and output keys |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `script_verify_vartime` | chapter 36: the interpreter that runs each spend |

Keys, scripts and PSBTs are public, so the treasury code runs in
variable time. Private keys go only to chapter 94's signer and chapter
101's MuSig2, which keep chapter 3's constant-time rules.

## Refusals

<!-- source: coax/lib/bitcoin/treasury.coax codes -->
```jai
TY_OK :: 0;
TY_KEY :: -1;              // a key that isn't a valid 33-byte compressed public key
TY_DELAY :: -2;            // a delay outside 1 to 65535 blocks: BIP-68's 16-bit block count
TY_CAPACITY :: -3;         // a buffer that is too small, or a PSBT that doesn't parse
TY_NO_UTXO :: -4;          // an input whose spent output the PSBT doesn't give
TY_FOREIGN_INPUT :: -5;    // an input that isn't the treasury's
TY_PAYMENT :: -6;          // an output to neither the hot wallet nor the treasury: a payment
TY_OVER_CEILING :: -7;     // the hot wallet would hold more than its ceiling
TY_FEE :: -8;              // outputs above the inputs, or a fee above the cap
TY_NO_REFILL :: -9;        // nothing for the hot wallet
TY_SIGN :: -10;            // chapter 94's signer refused
TY_REASONS :: [11] string.["ok", "key", "delay", "capacity", "no UTXO", "foreign input", "payment",
  "over the ceiling", "fee", "no refill", "signer refused"];
TY_WSH_MAX :: 160;         // 148 bytes for a three-byte delay
TY_LEAF_MAX :: 110;        // multi_a(2,A,O,R): 3 * 34 + 2
```

## The P2WSH form

In miniscript (chapter 98) the policy is
`or_d(multi(2,A,O,R),and_v(v:pk(R),older(4320)))`. Written out as script:

```text
2 <A> <O> <R> 3 OP_CHECKMULTISIG
OP_IFDUP OP_NOTIF
  <R> OP_CHECKSIGVERIFY <4320> OP_CHECKSEQUENCEVERIFY
OP_ENDIF
```

<!-- source: coax/lib/bitcoin/treasury.coax wsh -->
```jai
// A push of d as a script number: OP_1 to OP_16, or its minimal bytes.
ty_number :: (d: int, out: [] u8, at: int) -> int {
  if d >= 1 && d <= 16 { out[at] = cast(u8) (0x50 + d); return at + 1; }
  n := script_num_write_vartime(d, out[at + 1..<at + 10]);
  out[at] = cast(u8) n;
  return at + 1 + n;
}
ty_key :: (k: [] u8, out: [] u8, at: int) -> int {
  out[at] = 33;
  for j: 0..<33 { out[at + 1 + j] = k[j]; }
  return at + 34;
}
// or_d(multi(2,A,O,R),and_v(v:pk(R),older(delay))):
//
//   2 <A> <O> <R> 3 CHECKMULTISIG IFDUP NOTIF <R> CHECKSIGVERIFY <delay> CHECKSEQUENCEVERIFY ENDIF
//
// The primary path is any two of the three; IFDUP leaves its true result
// as the script's. When the multisig fails, NOTIF runs the recovery path:
// R's signature, and an input sequence of at least delay blocks (BIP-112).
// Returns the script's length or a refusal.
treasury_wsh_script_vartime :: (agent: [] u8, owner: [] u8, recovery: [] u8, delay: int, out: [] u8) -> int {
  if !pr_compressed(agent) || !pr_compressed(owner) || !pr_compressed(recovery) { return TY_KEY; }
  if delay < 1 || delay > 0xffff { return TY_DELAY; }
  if out.count < TY_WSH_MAX { return TY_CAPACITY; }
  out[0] = 0x52;                                            // OP_2
  at := ty_key(recovery, out, ty_key(owner, out, ty_key(agent, out, 1)));
  out[at] = 0x53; out[at + 1] = OP_CHECKMULTISIG; out[at + 2] = 0x73; out[at + 3] = 0x64;   // OP_3, IFDUP, NOTIF
  at = ty_key(recovery, out, at + 4);
  out[at] = OP_CHECKSIGVERIFY;
  at = ty_number(delay, out, at + 1);
  out[at] = OP_CHECKSEQUENCEVERIFY; out[at + 1] = 0x68;    // ENDIF
  return at + 2;
}
// The P2WSH scriptPubKey of a witness script: OP_0 and its SHA-256.
treasury_wsh_spk_vartime :: (script: [] u8, out: [] u8) -> int {
  n := p2wsh_script_vartime(script, out);
  return ifx n < 0 then TY_CAPACITY else n;
}
```

The two paths run like this:

- **Primary path.** The witness is an empty dummy, two signatures in key
  order and the script. `OP_CHECKMULTISIG` leaves true, `OP_IFDUP`
  copies it, and `OP_NOTIF` consumes the copy and skips the recovery
  branch. The remaining true is the script's result.
- **Recovery path.** The witness is R's signature, three empty items and
  the script. `OP_CHECKMULTISIG` takes the three empty items as the
  dummy and two empty signatures and leaves false. Under `NULLFAIL` an
  empty signature is the only kind allowed to fail. `OP_IFDUP` doesn't
  copy false, `OP_NOTIF` consumes it and runs the branch: R's signature
  must verify, and `OP_CHECKSEQUENCEVERIFY` requires the input's
  sequence to say at least 4,320 blocks (BIP-112). BIP-68 makes that
  sequence a consensus rule: the input can't be mined until the coin it
  spends is 4,320 blocks old.

The delay is a script number, pushed minimally: `OP_1` to `OP_16` for 1
to 16, then one to three bytes. A delay above 65,535 would need BIP-68's
other fields, so `TY_DELAY` refuses it.

## The Taproot form

<!-- source: coax/lib/bitcoin/treasury.coax tr -->
```jai
// The Taproot form of a treasury: the MuSig2 keys in KeySort order and
// their aggregate (the internal key), the two leaves and their tree, and
// the output key with its parity. tweak is the x-only tweak the MuSig2
// signers apply for the key path: TapTweak(internal || root).
TreasuryTr :: struct {
  keys: [66] u8; internal: [32] u8;
  multi: [TY_LEAF_MAX] u8; multi_n: int; late: [TY_LEAF_MAX] u8; late_n: int;
  tree: [1] TrTree; output: [32] u8; parity: int; tweak: [32] u8; spk: [34] u8;
}
// multi_a(2,A,O,R): <A> CHECKSIG <O> CHECKSIGADD <R> CHECKSIGADD 2 NUMEQUAL,
// BIP-387's leaf (chapter 100) with the keys' x coordinates in this order.
// and_v(v:pk(R),older(delay)): <R> CHECKSIGVERIFY <delay> CHECKSEQUENCEVERIFY.
treasury_tr_vartime :: (agent: [] u8, owner: [] u8, recovery: [] u8, delay: int, t: [] TreasuryTr) -> int {
  if t.count != 1 { return TY_CAPACITY; }
  if !pr_compressed(agent) || !pr_compressed(owner) || !pr_compressed(recovery) { return TY_KEY; }
  if delay < 1 || delay > 0xffff { return TY_DELAY; }
  fresh: TreasuryTr; t[0] = fresh;
  for j: 0..<33 { t[0].keys[j] = agent[j]; t[0].keys[33 + j] = owner[j]; }
  ms_key_sort_vartime(t[0].keys[..]);
  k := ms_key_agg_vartime(t[0].keys[..]);
  if k.code != MS_OK { return TY_KEY; }
  ms_xbytes_vartime(k.q, t[0].internal[..]);
  at := 0;
  for i: 0..<3 {
    key := ifx i == 0 then agent else ifx i == 1 then owner else recovery;
    t[0].multi[at] = 32;
    for j: 0..<32 { t[0].multi[at + 1 + j] = key[1 + j]; }
    t[0].multi[at + 33] = ifx i == 0 then OP_CHECKSIG else 0xba;   // OP_CHECKSIGADD
    at += 34;
  }
  t[0].multi[at] = 0x52; t[0].multi[at + 1] = 0x9c;              // 2 NUMEQUAL
  t[0].multi_n = at + 2;
  t[0].late[0] = 32;
  for j: 0..<32 { t[0].late[1 + j] = recovery[1 + j]; }
  t[0].late[33] = OP_CHECKSIGVERIFY;
  n := ty_number(delay, t[0].late[..], 34);
  t[0].late[n] = OP_CHECKSEQUENCEVERIFY; t[0].late_n = n + 1;
  leaves: [2][32] u8; depths := [2] int.[1, 1];
  #assert taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, t[0].multi[0..<t[0].multi_n], leaves[0][..]) == TR_OK;
  #assert taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, t[0].late[0..<t[0].late_n], leaves[1][..]) == TR_OK;
  #assert taproot_tree_vartime(depths[..], leaves[..], t[0].tree[..]) == TR_OK;
  t[0].parity = taproot_output_key_vartime(t[0].internal[..], t[0].tree[0].root[..], t[0].output[..]);
  #assert ms_taproot_tweak_vartime(t[0].keys[..], t[0].tree[0].root[..], t[0].tweak[..]) == MS_OK;
  t[0].spk[0] = 0x51; t[0].spk[1] = 32;
  for j: 0..<32 { t[0].spk[2 + j] = t[0].output[j]; }
  return TY_OK;
}
```

A Taproot output can spend three ways:

- **The key path** is the MuSig2 key of the agent and the owner
  (chapter 101). When both agree, which is the common case, the spend is
  one 64-byte signature and looks like any single-key Taproot spend.
  Both keys are KeySorted first, so the internal key doesn't depend on
  which of them the caller names first.
- **The `multi_a(2,A,O,R)` leaf** is BIP-387's tapscript multisig
  (chapter 100): any two of the three, when the agent or the owner is
  unavailable. Its witness has one item per key, in reverse order, with
  an empty item for a key that doesn't sign.
- **The recovery leaf** `<R> OP_CHECKSIGVERIFY <4320>
  OP_CHECKSEQUENCEVERIFY` is R alone after the delay.

The two leaves sit at depth 1 under one TapBranch (chapter 42), and the
output key is the internal key tweaked by the root (chapter 41).
`tweak` is the x-only tweak the MuSig2 signers add for the key path. The
example signs the key path with MuSig2 and verifies it as BIP-340 under
the output key, and it signs the `multi_a` leaf's BIP-342 signature hash
with the owner's and the recovery key. Running a script-path spend
through an interpreter needs tapscript, chapter 44, and the control
block checks of chapter 45, which are still planned. This chapter checks
each leaf's control block against the output key with chapter 42's
`taproot_commitment_vartime`.

## A separate wallet for the agent's limit

Bitcoin script can't limit an amount: a signature covers a whole
transaction, and no opcode compares an output's value with a number.
So the treasury's script can't say "the agent may spend up to 25,000
sat". The limit is the balance of a second wallet. The **hot wallet** is
a BIP-86 key of the agent's seed (chapter 121), which the agent spends
alone. Whatever happens to the agent, its loss is at most what the hot
wallet holds.

The owner sets three numbers, which the agent stores with the
treasury's descriptor:

- `ceiling`: the most the hot wallet may hold after a refill;
- `fee_cap`: the most fee a refill may pay;
- the hot wallet's current balance, which the agent reads from its own
  wallet (for now, from the caller; see [What needs a node](#what-needs-a-node)).

## What the agent signs

| Spend | Who signs | The agent's part |
| --- | --- | --- |
| A payment from the hot wallet | the agent alone | signs; the loss is bounded by the hot wallet's balance |
| A refill: treasury coins to the hot wallet, under the ceiling, with change back to the treasury and a fee under the cap | the agent and the owner | signs first, after `treasury_check_vartime` |
| Any other treasury spend: a payment, an overfill, a high fee, a coin that isn't the treasury's | the owner and one more key | never signs; the owner's signer and the recovery key decide |
| A treasury spend by the recovery path | the recovery key alone, after the delay | never involved |
| A key-path spend of the Taproot form | the agent and the owner with MuSig2 | the same rules as a refill, for each MuSig2 session it joins |

The agent's signature is never enough on its own: every treasury path
needs a second key, or the recovery key and the delay. Its check
protects the owner from a different mistake, signing a PSBT after the
agent did and trusting that the agent read it. A refill the agent signed
is one the owner can check against the same three numbers.

## The agent's check

<!-- source: coax/lib/bitcoin/treasury.coax check -->
```jai
// The hot wallet's state and the limits the owner set.
TreasuryPolicy :: struct { hot_balance: int; ceiling: int; fee_cap: int; }
// What a treasury PSBT does: its inputs' total, what it pays the hot
// wallet, what it returns to the treasury, its fee, and the first rule
// it breaks (TY_OK when it is a refill the agent may sign).
TreasuryCheck :: struct { code: int; total: int; refill: int; change: int; fee: int; }
// The check reads only the PSBT's unsigned transaction and the spent
// outputs it carries, and compares scripts by their bytes: every input
// must spend the treasury, and every output must pay the hot wallet or
// the treasury. A spend that pays anyone else is a payment, which the
// agent alone never approves.
treasury_check_vartime :: (m: [] PsbtMaps, treasury: [] u8, hot: [] u8, p: TreasuryPolicy, w: [] PsbtSign) -> TreasuryCheck {
  c: TreasuryCheck;
  size := psbt_unsigned_vartime(m, w[0].tx[..]);
  if size < 0 { c.code = TY_CAPACITY; return c; }
  b := w[0].tx[0..<size];
  w[0].t = tx_read_vartime(b, false, w[0].ins[..], w[0].outs[..], w[0].items[0..<0]);
  for n: 0..<m[0].nin {
    if !ps_spent(m, n, w) { c.code = TY_NO_UTXO; return c; }
    if !ps_equal(w[0].spk[0..<w[0].spk_n], treasury) { c.code = TY_FOREIGN_INPUT; return c; }
    c.total += w[0].amount;
  }
  paid := 0;
  for i: 0..<m[0].nout {
    o := w[0].outs[i];
    value := tx_le_vartime(b, o.value, 8); s := b[o.script..<o.script + o.script_n];
    if ps_equal(s, hot) { c.refill += value; }
    else if ps_equal(s, treasury) { c.change += value; }
    else { c.code = TY_PAYMENT; return c; }
    paid += value;
  }
  c.fee = c.total - paid;
  if c.refill == 0 { c.code = TY_NO_REFILL; }
  else if c.fee < 0 || c.fee > p.fee_cap { c.code = TY_FEE; }
  else if p.hot_balance + c.refill > p.ceiling { c.code = TY_OVER_CEILING; }
  return c;
}
// The agent's signer: the check, then chapter 94's signer on every input
// with the agent's key. Returns TY_OK or the rule that refused; a refusal
// adds no signature.
treasury_agent_sign_vartime :: (m: [] PsbtMaps, key: [] u8, treasury: [] u8, hot: [] u8, p: TreasuryPolicy, w: [] PsbtSign) -> int {
  c := treasury_check_vartime(m, treasury, hot, p, w);
  if c.code != TY_OK { return c.code; }
  aux: [32] u8;
  for n: 0..<m[0].nin { if psbt_sign_vartime(m, n, key, aux[..], true, w) != PSBT_SIGN_OK { return TY_SIGN; } }
  return TY_OK;
}
```

The check reads only the PSBT's unsigned transaction and the spent
outputs it carries (`PSBT_IN_WITNESS_UTXO`), and it compares scripts by
their bytes, so it trusts nothing the PSBT says about itself. Every
input must spend the treasury's scriptPubKey: a PSBT that slips in
another coin, even one of the hot wallet's, is `TY_FOREIGN_INPUT`. Every
output must pay the hot wallet or back to the treasury: anything else is
a payment, `TY_PAYMENT`. Then a refill must give the hot wallet
something, its fee must be between 0 and the cap, and the hot wallet
must stay at or under the ceiling. The order of the rules decides which
one a PSBT that breaks several reports; the example shows each one
alone.

## The PSBT flow

A refill moves through chapter 94's roles:

1. The agent is the **Creator** and **Updater**: it builds the unsigned
   transaction, and adds each input's witness UTXO and the treasury's
   witness script.
2. The agent runs `treasury_agent_sign_vartime`: the check, then
   chapter 94's **Signer** with its key, which adds an ECDSA partial
   signature.
3. The agent sends the PSBT to the owner. In OpenAgents the request
   travels as a NIP-KEYS signing request (Nostr chapter 104), and the
   owner's signer repeats the check against its own copy of the policy
   before it signs.
4. The owner's signer adds its partial signature.
5. Either side runs the **Input Finalizer** below and the **Transaction
   Extractor**, and broadcasts the transaction.

<!-- source: coax/lib/bitcoin/treasury.coax finalize -->
```jai
// Chapter 94's finalizer knows multisig scripts; this one knows the
// treasury's. For each input whose witness script is script: with two of
// the three keys' partial signatures, the primary path's witness (an
// empty dummy, the two signatures in key order, the script); with only
// R's, the recovery path's (R's signature, three empty items that make
// CHECKMULTISIG fail with NULLFAIL's empty signatures, the script). The
// recovery path's delay is the script's to check, against the input's
// sequence. True when every input is final.
treasury_finalize_vartime :: (m: [] PsbtMaps, script: [] u8, w: [] PsbtSign) -> bool {
  for n: 0..<m[0].nin {
    if psbt_final_vartime(m, n) { continue; }
    ws := psbt_find1_vartime(m, PSBT_INPUT, n, 0x05);
    if ws < 0 || !ps_equal(pm_value(m, ws), script) { return false; }
    sig: [3] int; got := 0;
    for k: 0..<3 { sig[k] = pf_sig(m, n, script[2 + 34 * k..<35 + 34 * k]); if sig[k] >= 0 { got += 1; } }
    wit: [PSBT_TX] u8; at := 0;
    if got >= 2 {
      at = tx_compact_write_vartime(wit[..], 0, 4);
      at = tx_compact_write_vartime(wit[..], at, 0);
      used := 0;
      for k: 0..<3 {
        if sig[k] < 0 || used == 2 { continue; }
        v := pm_value(m, sig[k]);
        at = tx_compact_write_vartime(wit[..], at, v.count); at = tx_put(wit[..], at, v); used += 1;
      }
    } else if sig[2] >= 0 {
      at = tx_compact_write_vartime(wit[..], 0, 5);
      v := pm_value(m, sig[2]);
      at = tx_compact_write_vartime(wit[..], at, v.count); at = tx_put(wit[..], at, v);
      for k: 0..<3 { at = tx_compact_write_vartime(wit[..], at, 0); }
    } else { return false; }
    at = tx_compact_write_vartime(wit[..], at, script.count); at = tx_put(wit[..], at, script);
    if !psbt_set1_vartime(m, PSBT_INPUT, n, 0x08, wit[0..<at]) { return false; }
  }
  return true;
}
```

Chapter 94's finalizer knows single-key and plain multisig scripts but
not this one, so the treasury has its own. With two of the three keys'
signatures it writes the primary path's witness. With only R's it writes
the recovery path's. It doesn't check the delay: the script does, against
the input's sequence, so a recovery transaction that is final but too
early fails in the interpreter and, on the network, isn't mined.

## Timelocked recovery

The recovery key is for the day the agent's host is gone and the owner
has lost its key, or the owner is gone and the agent must not be
stranded. Whoever holds R, such as a second device of the owner or a
custodian of last resort, can move the coins alone once they are 4,320
blocks old.

The clock is per coin, and it starts when the coin confirms. So a coin
that sits in the treasury longer than the delay is spendable by R alone.
The owner keeps coins young by spending them back to the treasury before
the delay runs out: any refill with change does this for the coin it
spends. A treasury that holds coins for months needs a refresh spend
every few weeks, which needs a node that knows each coin's age.

## What needs a node

These parts need a full node, and our node decides no money on its own
until chapters 80, 93 and 118 are done (the
[book's rule](../README.md)):

- **Broadcast on signet** and confirmation of a refill: our node's
  mempool and relay, chapters 80 and 93, with Bitcoin Core beside it
  until then.
- **Recovery with an independent wallet tool**, which this chapter's
  plan line asks for. The treasury's descriptor (chapter 104 of the
  Nostr book computes one for its `wsh(multi)` wallets) is what that tool
  imports; its `or_d` form needs miniscript, chapter 98.
- **Coin ages for the refresh**, the hot wallet's real balance, and
  rescans after a restore: the descriptor wallet on our node, chapter
  99.
- **Running the Taproot script paths**: tapscript, chapter 44, and
  control blocks, chapter 45.
- **The agent on Pylons and Verse OS**, with its keys on separate
  machines: chapter 124.

## The example

[bitcoin/examples/treasury.coax](../examples/treasury.coax) builds both
forms of the treasury and the hot wallet, runs a refill from creation to
the interpreter, shows the five things the agent refuses, spends by the
recovery path at the delay and one block before, and signs the Taproot
key path with MuSig2 and the `multi_a` leaf with two keys:

<!-- source: bitcoin/examples/treasury.coax example -->
```jai
#load "../../coax/lib/bitcoin/treasury.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, b: [] u8) {
  digits := "0123456789abcdef";
  print("%: ", label);
  for x: b { print("%", digits[cast(int) (x >> 4)..<cast(int) (x >> 4) + 1]); print("%", digits[cast(int) (x & 15)..<cast(int) (x & 15) + 1]); }
  print("\n");
}
// Print the address of a scriptPubKey on the test networks (signet's tb1).
address :: (spk: [] u8) {
  out: [100] u8;
  n := address_encode_vartime(SW_TESTNET, spk, out[..]);
  #assert n > 0;
  print("address: %\n", cast(string) out[0..<n]);
}
// The four private keys: the agent's, the owner's, the recovery key and
// the agent's hot wallet key. A real treasury takes each from its own seed
// (chapter 121); these are chapter 94's style of fixed test keys.
KEYS :: [4] string.["1111111111111111111111111111111111111111111111111111111111111111",
  "2222222222222222222222222222222222222222222222222222222222222222",
  "3333333333333333333333333333333333333333333333333333333333333333",
  "4444444444444444444444444444444444444444444444444444444444444444"];
NAMES :: [4] string.["agent", "owner", "recovery", "hot"];
DELAY :: 4320;                // about 30 days of blocks
FUNDING :: 100000;            // the treasury coin, in satoshis

// An unsigned transaction: version, one input (txid bytes all equal to
// prev, output 0, sequence), and outputs of values[k] to spks[k].
unsigned :: (version: int, prev: int, sequence: int, values: [] int, spks: [] [34] u8, lengths: [] int, out: [] u8) -> [] u8 {
  out[0] = cast(u8) version; out[1] = 0; out[2] = 0; out[3] = 0;
  at := tx_compact_write_vartime(out, 4, 1);
  for k: 0..<32 { out[at + k] = cast(u8) prev; }
  for k: 0..<4 { out[at + 32 + k] = 0; out[at + 37 + k] = cast(u8) ((sequence >> (8 * k)) & 255); }
  out[at + 36] = 0; at += 41;
  at = tx_compact_write_vartime(out, at, values.count);
  for i: 0..<values.count {
    for k: 0..<8 { out[at + k] = cast(u8) ((values[i] >> (8 * k)) & 255); }
    at = tx_compact_write_vartime(out, at + 8, lengths[i]);
    at = tx_put(out, at, spks[i][0..<lengths[i]]);
  }
  for k: 0..<4 { out[at + k] = 0; }
  return out[0..<at + 4];
}
// A PSBT of a transaction spending one coin of amount locked to spk, with
// the witness script when there is one.
psbt_of :: (tx: [] u8, amount: int, spk: [] u8, script: [] u8, m: [] PsbtMaps) {
  #assert psbt_create_vartime(tx, m);
  u: [64] u8;
  for k: 0..<8 { u[k] = cast(u8) ((amount >> (8 * k)) & 255); }
  u[8] = cast(u8) spk.count; for k: 0..<spk.count { u[9 + k] = spk[k]; }
  #assert psbt_set1_vartime(m, PSBT_INPUT, 0, 0x01, u[0..<9 + spk.count]);
  if script.count > 0 { #assert psbt_set1_vartime(m, PSBT_INPUT, 0, 0x05, script); }
}
// The final transaction of a PSBT, its txid, and input 0 run through
// chapter 36's interpreter with the consensus and standardness flags.
verify :: (m: [] PsbtMaps, amount: int, spk: [] u8, w: [] PsbtSign) -> int {
  net: [4096] u8; ins: [4] TxIn; outs: [4] TxOut; items: [16] TxItem; id: [32] u8; rev: [32] u8;
  size := psbt_extract_vartime(m, net[..], w);
  if size < 0 { return -1; }
  b := net[0..<size];
  t := tx_read_vartime(b, true, ins[..], outs[..], items[..]);
  #assert tx_txid_vartime(b, t, id[..]);
  for k: 0..<32 { rev[k] = id[31 - k]; }
  show("  txid", rev[..]);
  print("  witness items: %\n", ins[0].count);
  sw: [1] ScriptWork;
  flags := SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_WITNESS | SCRIPT_VERIFY_DERSIG | SCRIPT_VERIFY_LOW_S | SCRIPT_VERIFY_STRICTENC
    | SCRIPT_VERIFY_NULLDUMMY | SCRIPT_VERIFY_CLEANSTACK | SCRIPT_VERIFY_NULLFAIL | SCRIPT_VERIFY_WITNESS_PUBKEYTYPE
    | SCRIPT_VERIFY_MINIMALIF | SCRIPT_VERIFY_CHECKLOCKTIMEVERIFY | SCRIPT_VERIFY_CHECKSEQUENCEVERIFY;
  sp := ScriptSpend.{b=b, t=t, ins=ins[..], outs=outs[..], items=items[..], index=0, amount=amount, spk=spk};
  return script_verify_vartime(sp, flags, sw[..]);
}
// A BIP-340 signature of 32 bytes by a key (a script-path key signs as
// itself), with zero aux.
sign340 :: (key: [] u8, msg: [] u8, sig: [] u8) {
  w: [1] SignWork; aux: [32] u8; status := 0;
  for k: 0..<6 { status = schnorr_sign_step(key, aux[..], msg, w[..], sig, false); }
  #assert status == SIGN_DONE;
}

main :: () {
  keys: [4][32] u8; pubs: [4][33] u8;
  print("The keys\n");
  for k: 0..<4 {
    #assert psbt_hex(KEYS[k], keys[k][..]).count == 32 && bip32_public(keys[k][..], pubs[k][..]);
    show(NAMES[k], pubs[k][..]);
  }

  // The P2WSH form.
  print("The P2WSH treasury, recovery after % blocks\n", DELAY);
  script: [TY_WSH_MAX] u8; spks: [3][34] u8; lengths: [3] int;
  n := treasury_wsh_script_vartime(pubs[0][..], pubs[1][..], pubs[2][..], DELAY, script[..]);
  ws := script[0..<n];
  print("witness script: % bytes\n", n);
  show("script", ws);
  wsh: [34] u8;
  #assert treasury_wsh_spk_vartime(ws, wsh[..]) == 34;
  address(wsh[..]);

  // The Taproot form.
  print("The Taproot treasury\n");
  tr: [1] TreasuryTr;
  #assert treasury_tr_vartime(pubs[0][..], pubs[1][..], pubs[2][..], DELAY, tr[..]) == TY_OK;
  show("MuSig2 internal key (agent, owner)", tr[0].internal[..]);
  show("leaf multi_a(2,A,O,R)", tr[0].multi[0..<tr[0].multi_n]);
  show("leaf and_v(v:pk(R),older(4320))", tr[0].late[0..<tr[0].late_n]);
  show("root", tr[0].tree[0].root[..]);
  show("output key", tr[0].output[..]);
  address(tr[0].spk[..]);

  // The hot wallet: the agent's own BIP-86 key, which it spends alone.
  print("The hot wallet\n");
  hot: [34] u8;
  #assert p2tr_script_vartime(pubs[3][1..<33], hot[..]) == 34;
  address(hot[..]);

  // A refill: 20,000 sat to the hot wallet, 79,500 back to the treasury.
  print("A refill through PSBT\n");
  policy := TreasuryPolicy.{hot_balance = 3000, ceiling = 25000, fee_cap = 1000};
  print("hot balance %, ceiling %, fee cap %\n", policy.hot_balance, policy.ceiling, policy.fee_cap);
  w: [1] PsbtSign; m: [1] PsbtMaps; txb: [512] u8;
  spks[0] = hot; spks[1] = wsh; lengths[0] = 34; lengths[1] = 34;
  values := [2] int.[20000, 79500];
  tx := unsigned(2, 0xa1, 0xfffffffd, values[..], spks[0..<2], lengths[0..<2], txb[..]);
  psbt_of(tx, FUNDING, wsh[..], ws, m[..]);
  c := treasury_check_vartime(m[..], wsh[..], hot[..], policy, w[..]);
  print("inputs %, refill %, change %, fee %: %\n", c.total, c.refill, c.change, c.fee, TY_REASONS[-c.code]);
  print("the agent signs: %\n", TY_REASONS[-treasury_agent_sign_vartime(m[..], keys[0][..], wsh[..], hot[..], policy, w[..])]);
  aux: [32] u8;
  print("the owner signs: %\n", PSBT_SIGN_REASONS[psbt_sign_vartime(m[..], 0, keys[1][..], aux[..], true, w[..])]);
  print("final: %\n", treasury_finalize_vartime(m[..], ws, w[..]));
  print("  script: %\n", SCRIPT_ERR_NAMES[verify(m[..], FUNDING, wsh[..], w[..])]);

  // What the agent refuses to sign.
  print("What the agent refuses\n");
  stranger: [34] u8;
  #assert p2wpkh_script_vartime(pubs[1][..], stranger[..]) == 22;
  cases := [5] string.["refill 30,000", "a payment to another address", "a fee of 5,000", "a coin of the hot wallet", "change only"];
  for k: 0..<5 {
    v := [2] int.[20000, 79500];
    spks[0] = hot; spks[1] = wsh; lengths[0] = 34; lengths[1] = 34;
    in_spk := wsh;
    if k == 0 { v[0] = 30000; v[1] = 69500; }
    if k == 1 { spks[0] = stranger; lengths[0] = 22; }
    if k == 2 { v[1] = 75000; }
    if k == 3 { in_spk = hot; }
    if k == 4 { v[0] = 99500; spks[0] = wsh; v[1] = 0; }
    t := unsigned(2, 0xa1, 0xfffffffd, v[0..<ifx k == 4 then 1 else 2], spks[0..<2], lengths[0..<2], txb[..]);
    psbt_of(t, FUNDING, in_spk[..], ws, m[..]);
    print("%: %\n", cases[k], TY_REASONS[-treasury_agent_sign_vartime(m[..], keys[0][..], wsh[..], hot[..], policy, w[..])]);
  }

  // The recovery path: the recovery key alone, once the coin is DELAY
  // blocks old. BIP-68 reads the input's sequence as that relative age.
  print("The recovery path\n");
  rec: [34] u8;
  lengths[0] = p2wpkh_script_vartime(pubs[2][..], rec[..]);
  spks[0] = rec;
  one := [1] int.[99000];
  for k: 0..<2 {
    sequence := DELAY - k;
    t := unsigned(2, 0xa1, sequence, one[..], spks[0..<1], lengths[0..<1], txb[..]);
    psbt_of(t, FUNDING, wsh[..], ws, m[..]);
    print("sequence %: the recovery key signs: %\n", sequence, PSBT_SIGN_REASONS[psbt_sign_vartime(m[..], 0, keys[2][..], aux[..], true, w[..])]);
    print("final: %\n", treasury_finalize_vartime(m[..], ws, w[..]));
    print("  script: %\n", SCRIPT_ERR_NAMES[verify(m[..], FUNDING, wsh[..], w[..])]);
  }
  t := unsigned(2, 0xa1, DELAY, one[..], spks[0..<1], lengths[0..<1], txb[..]);
  psbt_of(t, FUNDING, wsh[..], ws, m[..]);
  #assert psbt_sign_vartime(m[..], 0, keys[0][..], aux[..], true, w[..]) == PSBT_SIGN_OK;
  print("the agent's signature alone: final %\n", treasury_finalize_vartime(m[..], ws, w[..]));

  // The Taproot key path: the agent and the owner sign one BIP-340
  // signature with MuSig2, for the output key tweaked by the tree.
  print("The Taproot key path, MuSig2\n");
  spks[0] = hot; spks[1] = tr[0].spk; lengths[0] = 34; lengths[1] = 34;
  t = unsigned(2, 0xb2, 0xfffffffd, values[..], spks[0..<2], lengths[0..<2], txb[..]);
  tp := bt_tx_read(t);
  amounts := [1] int.[FUNDING]; spent: [1] BtScript; sighash: [32] u8;
  spent[0].n = 34; for k: 0..<34 { spent[0].b[k] = tr[0].spk[k]; }
  #assert bt_sighash_taproot(t, tp, 0, amounts[..], spent[..], BT_DEFAULT, sighash[..]);
  show("sighash", sighash[..]);
  nw: [1] MsNonceWork; sw: [1] MsSignWork; sec: [194] u8; pub: [132] u8; agg: [66] u8; psigs: [64] u8; sig: [64] u8;
  sk: [64] u8; pk: [66] u8; rand: [32] u8; xonly := [1] bool.[true];
  for i: 0..<2 {
    // KeySort put the signers in this order.
    who := ifx ps_equal(tr[0].keys[33 * i..<33 * i + 33], pubs[0][..]) then 0 else 1;
    for j: 0..<32 { sk[32 * i + j] = keys[who][j]; rand[j] = cast(u8) (0xc0 + i); }
    for j: 0..<33 { pk[33 * i + j] = pubs[who][j]; }
    #assert ms_nonce_gen(rand[..], sk[32 * i..<32 * i + 32], pk[33 * i..<33 * i + 33], tr[0].output[..], sighash[..], true, rand[0..<0],
      sec[97 * i..<97 * i + 97], pub[66 * i..<66 * i + 66], nw[..]) == MS_OK;
  }
  #assert ms_nonce_agg_vartime(pub[..], agg[..]).code == MS_OK;
  for i: 0..<2 {
    #assert ms_sign(sec[97 * i..<97 * i + 97], sk[32 * i..<32 * i + 32], agg[..], tr[0].keys[..], tr[0].tweak[..], xonly[..], sighash[..],
      psigs[32 * i..<32 * i + 32], sw[..]).code == MS_OK;
  }
  #assert ms_partial_agg_vartime(psigs[..], ms_session_vartime(agg[..], tr[0].keys[..], tr[0].tweak[..], xonly[..], sighash[..]), sig[..]).code == MS_OK;
  show("signature", sig[..]);
  print("valid under the output key: %\n", taproot_key_path_verify_vartime(tr[0].output[..], sig[..], sighash[..]));

  // The script path: the owner and the recovery key sign the multi_a leaf
  // when the agent is gone.
  print("The Taproot script path, owner and recovery\n");
  leaf: [32] u8; leafsig: [32] u8; s1: [64] u8; s2: [64] u8; cb: [65] u8; xo: [32] u8; xr: [32] u8;
  #assert taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, tr[0].multi[0..<tr[0].multi_n], leaf[..]) == TR_OK;
  #assert bt_sighash_taproot_ext(t, tp, 0, amounts[..], spent[..], BT_DEFAULT, t[0..<0], leaf[..], BT_NO_CODESEP, leafsig[..]);
  show("leaf sighash", leafsig[..]);
  sign340(keys[1][..], leafsig[..], s1[..]); sign340(keys[2][..], leafsig[..], s2[..]);
  for j: 0..<32 { xo[j] = pubs[1][1 + j]; xr[j] = pubs[2][1 + j]; }
  print("owner's signature valid: %, recovery's: %\n", schnorr_verify_vartime(leafsig[..], xo[..], s1[..]), schnorr_verify_vartime(leafsig[..], xr[..], s2[..]));
  #assert taproot_control_block_vartime(tr[0].tree[..], 0, tr[0].internal[..], TR_LEAF_TAPSCRIPT, cb[..]) == 65;
  show("control block", cb[..]);
  print("the leaf is in the output: %\n", taproot_commitment_vartime(cb[..], tr[0].multi[0..<tr[0].multi_n], tr[0].output[..]) == TR_OK);
  print("witness: recovery's signature, owner's, an empty item for the agent, the leaf, the control block\n");
  #assert taproot_control_block_vartime(tr[0].tree[..], 1, tr[0].internal[..], TR_LEAF_TAPSCRIPT, cb[..]) == 65;
  print("the recovery leaf is in the output: %\n", taproot_commitment_vartime(cb[..], tr[0].late[0..<tr[0].late_n], tr[0].output[..]) == TR_OK);
}
```

Run it:

```shell
coax/bin/coax run bitcoin/examples/treasury.coax --run-steps 250000000
```

It prints:

```text
The keys
agent: 034f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa
owner: 02466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f27
recovery: 023c72addb4fdf09af94f0c94d7fe92a386a7e70cf8a1d85916386bb2535c7b1b1
hot: 032c0b7cf95324a07d05398b240174dc0c2be444d96b159aa6c7f7b1e668680991
The P2WSH treasury, recovery after 4320 blocks
witness script: 147 bytes
script: 5221034f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa2102466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f2721023c72addb4fdf09af94f0c94d7fe92a386a7e70cf8a1d85916386bb2535c7b1b153ae736421023c72addb4fdf09af94f0c94d7fe92a386a7e70cf8a1d85916386bb2535c7b1b1ad02e010b268
address: tb1qafc4p45rnqqvqv4qk5ljhqff8s0hdf32khhdrcm5nhy3fwr0zh9s85qry4
The Taproot treasury
MuSig2 internal key (agent, owner): fc23182b2ee6d25438f148c2836a5ca041b4caca3d99202294ae89c790332fb3
leaf multi_a(2,A,O,R): 204f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aaac20466d7fcae563e5cb09a0d1870bb580344804617879a14949cf22285f1bae3f27ba203c72addb4fdf09af94f0c94d7fe92a386a7e70cf8a1d85916386bb2535c7b1b1ba529c
leaf and_v(v:pk(R),older(4320)): 203c72addb4fdf09af94f0c94d7fe92a386a7e70cf8a1d85916386bb2535c7b1b1ad02e010b2
root: 2a00640980afc9ad8f96c60e669446c1bdd6c15d3ea88a22209059fb494b1a56
output key: a2b9a50d8d9ee9a1e1be2f79e1e1814666db33d6e6baa18fc78a4379e33e5d25
address: tb1p52u62rvdnm56rcd79au7rcvpgendkv7ku6a2rr783fphnce7t5jshn59j9
The hot wallet
address: tb1py6453jm8063t35dhf6yddhqnf65hel4aryj9sz673xkkqpy73u9syqkqtg
A refill through PSBT
hot balance 3000, ceiling 25000, fee cap 1000
inputs 100000, refill 20000, change 79500, fee 500: ok
the agent signs: ok
the owner signs: ok
final: true
  txid: 2d640ed3f62d34675dcb16b38579c84d0dc3ffcfc546dc4323b4858cc7ba1ad9
  witness items: 4
  script: OK
What the agent refuses
refill 30,000: over the ceiling
a payment to another address: payment
a fee of 5,000: fee
a coin of the hot wallet: foreign input
change only: no refill
The recovery path
sequence 4320: the recovery key signs: ok
final: true
  txid: 608fa64c609e651b2b6f916e7a68fb676c94f3c8cd8b26c4a86e56c6d61b259e
  witness items: 5
  script: OK
sequence 4319: the recovery key signs: ok
final: true
  txid: ce64de2065c4b61620e7911fc92cc3abbda244ab8f8856f893b187300f79a2d4
  witness items: 5
  script: UNSATISFIED_LOCKTIME
the agent's signature alone: final false
The Taproot key path, MuSig2
sighash: 7f31d10c762b3c12d5dae03ed1173f460462d4bd40c3fa28320dbf4821b32f4c
signature: 43b50bfcaa8d830d1eefcdb700b9c8a0e3f1fb4b0f16aad276cf4f498681f12dc604ad0e2a2b3f2016b0ca77e69434b427f510168b1f6f2c70ed12c71542d945
valid under the output key: true
The Taproot script path, owner and recovery
leaf sighash: ebc706ec8393a8064d4f6e7ba8a8f95a2d2eb6c4e0d47ac0518373597d9df159
owner's signature valid: true, recovery's: true
control block: c1fc23182b2ee6d25438f148c2836a5ca041b4caca3d99202294ae89c790332fb34ebaffe97ce52ce32d2f192a7d0734af6725deb1b1b75a7e905a0163e8e9e678
the leaf is in the output: true
witness: recovery's signature, owner's, an empty item for the agent, the leaf, the control block
the recovery leaf is in the output: true
```

The expected output is in
[bitcoin/fixtures/treasury.out](../fixtures/treasury.out).

The refill's witness has four items (dummy, two signatures, script) and
the recovery spend's five. At sequence 4,319 the recovery spend is
final and well formed, and the interpreter refuses it with
`UNSATISFIED_LOCKTIME`. The agent's signature alone finalizes nothing.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/treasury.coax](../examples/checks/treasury.coax),
which loads the example:

- every pair of the three keys (agent and owner, agent and recovery,
  owner and recovery) spends the treasury at once, through the
  interpreter;
- the recovery key alone spends at the delay and fails one block
  before, for delays of 1, 16, 17 and 65,535 blocks, which cover each
  size of the delay's push;
- the agent signs a refill of 20,000 to 25,000 sat exactly when the hot
  wallet stays at or under its ceiling;
- the Taproot form's internal key doesn't depend on the order of the
  agent and the owner, while the `multi_a` leaf does.

<!-- source: bitcoin/examples/checks/treasury.coax check -->
```jai
#load "../treasury.coax";

// The keys of the library's checks: agent, owner, recovery, hot.
pub_of :: (k: int, key: [] u8, pub: [] u8) {
  #assert psbt_hex(KEYS[k], key).count == 32 && bip32_public(key, pub);
}
// The P2WSH treasury with a given delay, its scriptPubKey and the hot
// wallet's.
treasury_with :: (delay: int, script: [] u8, wsh: [] u8, hot: [] u8) -> int {
  k: [32] u8; a: [33] u8; o: [33] u8; r: [33] u8; h: [33] u8;
  pub_of(0, k[..], a[..]); pub_of(1, k[..], o[..]); pub_of(2, k[..], r[..]); pub_of(3, k[..], h[..]);
  n := treasury_wsh_script_vartime(a[..], o[..], r[..], delay, script);
  #assert n > 0 && treasury_wsh_spk_vartime(script[0..<n], wsh) == 34 && p2tr_script_vartime(h[1..<33], hot) == 34;
  return n;
}

#forall "any two of the three keys spend the treasury at once" pair: 0..<3 #steps 8000000000 {
  s: [TY_WSH_MAX] u8; wsh: [34] u8; hot: [34] u8; k: [32] u8; p: [33] u8; aux: [32] u8;
  m: [1] PsbtMaps; w: [1] PsbtSign;
  n := treasury_with(DELAY, s[..], wsh[..], hot[..]);
  tyt_psbt(wsh[..], s[0..<n], 0xfffffffd, hot[..], 99000, wsh[0..<0], 0, m[..]);
  first := ifx pair == 2 then 1 else 0;
  second := ifx pair == 0 then 1 else 2;
  pub_of(first, k[..], p[..]);
  #assert psbt_sign_vartime(m[..], 0, k[..], aux[..], true, w[..]) == PSBT_SIGN_OK;
  pub_of(second, k[..], p[..]);
  #assert psbt_sign_vartime(m[..], 0, k[..], aux[..], true, w[..]) == PSBT_SIGN_OK;
  #assert treasury_finalize_vartime(m[..], s[0..<n], w[..]);
  #assert tyt_verify(m[..], wsh[..], w[..]) == SCRIPT_ERR_OK;
}
#forall "the recovery key alone spends at the delay and not a block before, for small and large delays" i: 0..<4 #steps 12000000000 {
  s: [TY_WSH_MAX] u8; wsh: [34] u8; hot: [34] u8; k: [32] u8; p: [33] u8; aux: [32] u8;
  m: [1] PsbtMaps; w: [1] PsbtSign;
  delay := ifx i == 0 then 1 else ifx i == 1 then 16 else ifx i == 2 then 17 else 0xffff;
  n := treasury_with(delay, s[..], wsh[..], hot[..]);
  pub_of(2, k[..], p[..]);
  for early: 0..<2 {
    tyt_psbt(wsh[..], s[0..<n], delay - early, hot[..], 99000, wsh[0..<0], 0, m[..]);
    #assert psbt_sign_vartime(m[..], 0, k[..], aux[..], true, w[..]) == PSBT_SIGN_OK;
    #assert treasury_finalize_vartime(m[..], s[0..<n], w[..]);
    want := ifx early == 0 then SCRIPT_ERR_OK else SCRIPT_ERR_UNSATISFIED_LOCKTIME;
    #assert tyt_verify(m[..], wsh[..], w[..]) == want;
  }
}
#forall "the agent signs a refill exactly when the hot wallet stays at or under its ceiling" refill: 0..<6 #steps 4000000000 {
  s: [TY_WSH_MAX] u8; wsh: [34] u8; hot: [34] u8; m: [1] PsbtMaps; w: [1] PsbtSign;
  n := treasury_with(DELAY, s[..], wsh[..], hot[..]);
  amount := 20000 + 1000 * refill;                    // 20,000 to 25,000
  tyt_psbt(wsh[..], s[0..<n], 0xfffffffd, hot[..], amount, wsh[..], 99500 - amount, m[..]);
  policy := TreasuryPolicy.{hot_balance = 3000, ceiling = 25000, fee_cap = 1000};
  c := treasury_check_vartime(m[..], wsh[..], hot[..], policy, w[..]);
  #assert c.fee == 500 && c.refill == amount;
  #assert (c.code == TY_OK) == (3000 + amount <= 25000);
  #assert c.code == TY_OK || c.code == TY_OVER_CEILING;
}
#test "the Taproot form doesn't depend on which of agent and owner comes first, except in the leaf" #steps 2000000000 {
  k: [32] u8; a: [33] u8; o: [33] u8; r: [33] u8; t1: [1] TreasuryTr; t2: [1] TreasuryTr;
  pub_of(0, k[..], a[..]); pub_of(1, k[..], o[..]); pub_of(2, k[..], r[..]);
  #assert treasury_tr_vartime(a[..], o[..], r[..], DELAY, t1[..]) == TY_OK;
  #assert treasury_tr_vartime(o[..], a[..], r[..], DELAY, t2[..]) == TY_OK;
  for j: 0..<32 { #assert t1[0].internal[j] == t2[0].internal[j]; }
  for j: 0..<66 { #assert t1[0].keys[j] == t2[0].keys[j]; }
  #assert t1[0].multi[1] != t2[0].multi[1];          // multi_a lists its keys in order
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/treasury.coax --run-steps 250000000
```

`coax test` runs 512 checks, including the loaded files' own checks,
in about 290 CPU seconds on 2026-11-03.

### The library's own checks

`treasury.coax` is an `#authority` file. Its checks cover the script's
bytes and each size of the delay's push, the Taproot leaves and output
key, every refusal of the script builders, a refill from creation to the
interpreter, each of the agent's refusals, the recovery path at the delay
and one block before, and one signature that isn't R's finalizing
nothing.

```shell
coax/bin/coax test coax/lib/bitcoin/treasury.coax --run-steps 250000000
```

`coax test` runs 508 checks, including the loaded files' own checks, in
about 265 CPU seconds on 2026-11-03. Mutation testing: not run
(owner speed order).

### A Python implementation

[treasury.py](../../coax/tests/bitcoin/treasury.py) writes the example's
transcript with code that shares nothing with Coax:

- the script, the leaves, the tree, the control blocks and the output
  keys, written there from BIP-141, BIP-342 and BIP-341;
- the agent's rules, written again from the table above;
- the PSBTs and their partial signatures from chapter 94's
  `psbt_reference.py`, with the witnesses assembled there;
- every spend through chapter 36's `script_reference.py` interpreter,
  with its BIP-68 and BIP-112 checks;
- the key-path signature from BIP-327's pinned `reference.py`, and the
  leaf signatures from the secp256k1lab copy pinned with chapter 102;
- the addresses from the pinned `segwit_addr.py`.

```shell
python3 -I coax/tests/bitcoin/treasury.py --coax coax/bin/coax
```

On 2026-11-03 its transcript and the fixture agree on all 56 lines, and
the example prints them in debug and release builds.

## What this chapter leaves out

- Broadcast, confirmation and coin ages, which need the node: see
  [What needs a node](#what-needs-a-node).
- The NIP-KEYS records, admission of independent key agents and the
  signing requests between them: Nostr chapter 104.
- Lightning as the hot wallet. A channel's balance is a hot wallet too,
  and Nostr chapter 104's `ky_topup` applies the same ceiling to it.
- Miniscript compilation and satisfaction of the `or_d` policy:
  chapter 98.
- FROST (chapter 102) for a threshold key path across all three keys.

## Summary

- A treasury holds most of an agent's coins under a 2-of-3 of the agent,
  the owner and a recovery key, and the recovery key alone after a
  BIP-68 delay.
- The P2WSH form is one script with two paths; the Taproot form has a
  MuSig2 key path and two leaves.
- Script can't limit amounts, so the agent's limit is a separate hot
  wallet's balance, refilled from the treasury.
- The agent signs a treasury PSBT only for a refill under the ceiling and
  the fee cap; every payment needs the owner.
- Coins older than the delay are spendable by the recovery key alone,
  so the owner keeps them young.

## Dependencies

- [94. PSBT](ch094-psbt.md)
- [100. Multisig](ch100-multisig.md)
- [101. MuSig2](ch101-musig2.md)
- [121. One seed for Nostr and Bitcoin](ch121-one-seed-for-nostr-and-bitcoin.md)
- [36. The script interpreter](ch036-the-script-interpreter.md)
- [42. Script trees: tapleaves and tapbranches](ch042-script-trees-tapleaves-and-tapbranches.md)
- Nostr book: [104. Agent treasuries and a network of keys](../../nostr/book/ch104-agent-treasuries-and-a-network-of-keys.md)

## Sources

- [BIP-68](https://github.com/bitcoin/bips/blob/master/bip-0068.mediawiki)
  (relative lock-time using consensus-enforced sequence numbers) and
  [BIP-112](https://github.com/bitcoin/bips/blob/master/bip-0112.mediawiki)
  (`OP_CHECKSEQUENCEVERIFY`).
- [BIP-141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
  (segregated witness) and
  [BIP-147](https://github.com/bitcoin/bips/blob/master/bip-0147.mediawiki)
  (the `NULLDUMMY` rule).
- [BIP-174](https://github.com/bitcoin/bips/blob/master/bip-0174.mediawiki)
  (PSBT).
- [BIP-327](https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki)
  (MuSig2), pinned with its reference in
  [coax/tests/vectors/upstream/bip327/](../../coax/tests/vectors/upstream/bip327/reference.py).
- [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki)
  and [BIP-342](https://github.com/bitcoin/bips/blob/master/bip-0342.mediawiki)
  (Taproot and tapscript).
- [BIP-379](https://github.com/bitcoin/bips/blob/master/bip-0379.mediawiki)
  (miniscript) and
  [BIP-387](https://github.com/bitcoin/bips/blob/master/bip-0387.mediawiki)
  (`multi_a`).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
