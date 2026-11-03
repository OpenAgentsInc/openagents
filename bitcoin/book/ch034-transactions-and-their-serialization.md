# 34. Transactions and their serialization

A transaction is a run of bytes: a version, the coins it spends, the
outputs it creates, and a locktime. Since segwit, it can also carry a
witness for each input, in a form that old software doesn't read. This
chapter reads those bytes as Bitcoin Core does, with and without the
witness form, writes them back, and computes the two IDs and the weight
that every later chapter uses. It reads every transaction in Bitcoin
Core's `tx_valid.json` and `tx_invalid.json` and six mainnet
transactions pinned by their txids, and checks the reader against a
Python transliteration of Core's code, natively and in Wasm.

The chapter stays at the byte layer. What a script means, what a
signature commits to, and whether a transaction is valid are for
chapters 35 to 40 and 52.

## What this chapter teaches

- The layout of a transaction: version, inputs, outputs, locktime, and
  where BIP-144 puts the marker, the flag and the witness stacks.
- CompactSize, the variable-length integer that prefixes every count and
  length, why each value has exactly one encoding, and the largest value
  Core accepts.
- How Core reads the witness form, and the bytes that read both ways: a
  transaction with no inputs looks like the start of a segwit one.
- The txid and the wtxid: what each one hashes, and why explorers show
  them byte-reversed.
- Stripped size, total size, weight and virtual size.
- Seven refusals, each with its own code and the byte where reading
  stopped: truncation, a non-canonical or oversized count, an unknown
  flag, a witness flag with no witnesses, trailing bytes and a caller's
  arrays that are too small.
- Writing a read transaction back, byte for byte, in either form.

## Where Bitcoin uses this serialization

Every place a transaction moves or is named uses these bytes:

| Where | What is serialized | Chapter |
| --- | --- | --- |
| The P2P network | the full form, witnesses included, in `tx` and `block` messages | 70 |
| A block | a CompactSize count of transactions, then each one in full | 47 |
| The Merkle root of a block | the txids | 47 |
| The witness commitment in the coinbase | the wtxids | 48 |
| A signature hash | fields of the transaction, in the same encodings | 37, 43 |
| An outpoint (the coin an input spends) | the txid of the transaction that made it, and an output index | 34, 49 |
| Fees and block limits | the weight and the virtual size | 40, 52 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_compact_read_vartime(b, at)` | one CompactSize, or a refusal |
| | `tx_compact_size_vartime(value)`, `tx_compact_write_vartime(out, at, value)` | the shortest encoding of a value |
| | `tx_parse_vartime(b, witness, ins, outs, items)` | one transaction at the start of `b`, with or without the witness form |
| | `tx_read_vartime(b, witness, ins, outs, items)` | the same, and the transaction must use every byte |
| | `tx_decode_vartime(b, ins, outs, items)` | Core's `DecodeTx`: both readings, and which one to keep |
| | `tx_write_vartime(b, t, ins, outs, items, witness, out)` | the transaction back, in the extended or the legacy form |
| | `tx_txid_vartime`, `tx_wtxid_vartime` | the two IDs, in hash order |
| | `tx_weight_vartime`, `tx_vsize_vartime` | BIP-141's weight and virtual size |
| | `tx_scripts_sane_vartime`, `tx_ops_valid_vartime`, `tx_coinbase_vartime` | Core's `CheckTxScriptsSanity` and what it uses |
| [hash.coax](../../coax/lib/bitcoin/hash.coax) | `sha256d_checked`, `bitcoin_reverse32` | SHA-256d and the display order (chapter 5) |

Transactions are public, so every procedure here runs in variable time
and its name ends in `_vartime` (chapter 3).

The reader copies nothing. It returns a `Tx` and fills the caller's
arrays of `TxIn`, `TxOut` and `TxItem` with positions in the bytes it
read. Every result is `TX_OK` or a refusal, with the byte where reading
stopped:

<!-- source: coax/lib/bitcoin/tx.coax codes -->
```jai
// Refusal codes of the reader. Core throws std::ios_base::failure with the
// message in quotes; the trailing-bytes check is DecodeTx's.
TX_OK :: 0;
TX_TRUNCATED :: 1;       // the bytes end inside a field: "end of data"
TX_NONCANONICAL :: 2;    // a CompactSize longer than it needs: "non-canonical ReadCompactSize()"
TX_SIZE_TOO_LARGE :: 3;  // a count or length above MAX_SIZE: "ReadCompactSize(): size too large"
TX_UNKNOWN_FLAG :: 4;    // a flag other than 0x01: "Unknown transaction optional data"
TX_NO_WITNESS :: 5;      // the witness flag with every stack empty: "Superfluous witness record"
TX_TRAILING :: 6;        // bytes after the locktime
TX_CAPACITY :: 7;        // more inputs, outputs or witness items than the caller's arrays hold

TX_MAX_SIZE :: 0x02000000;   // serialize.h MAX_SIZE: no count or length is larger
TX_SCALE :: 4;               // consensus.h WITNESS_SCALE_FACTOR
```

## The layout

A legacy transaction is four fields, and the middle two are lists:

| Field | Bytes | Encoding |
| --- | --- | --- |
| version | 4 | little-endian |
| input count | 1, 3, 5 or 9 | CompactSize |
| each input: outpoint | 36 | a txid in hash order, then a 4-byte little-endian output index |
| each input: scriptSig | its length as a CompactSize, then the bytes | |
| each input: sequence | 4 | little-endian |
| output count | 1, 3, 5 or 9 | CompactSize |
| each output: value | 8 | little-endian satoshis |
| each output: scriptPubKey | its length as a CompactSize, then the bytes | |
| locktime | 4 | little-endian |

BIP-144's extended form adds three things. After the version come a
marker byte `00` and a flag byte `01`. After the outputs come the witness
stacks, one per input: a count of items, then each item with its length.
The locktime is still last.

The reader's types hold the positions of these fields:

<!-- source: coax/lib/bitcoin/tx.coax layout -->
```jai
// One input: the outpoint (a txid in hash order, then a 4-byte output
// index), the scriptSig and the sequence, as positions in the bytes; and
// its witness, items[first..<first+count] of the item array.
TxIn :: struct { outpoint: int; script: int; script_n: int; sequence: int; first: int; count: int; }
// One output: an 8-byte signed value in satoshis, then the scriptPubKey.
TxOut :: struct { value: int; script: int; script_n: int; }
// One witness item: n bytes at at.
TxItem :: struct { at: int; n: int; }
// A read transaction. code is TX_OK or a refusal found at byte at. The
// version is at byte 0; the witness stacks, when segwit, run from witness
// to locktime. total is the full size, stripped the size without the
// marker, the flag and the witnesses.
Tx :: struct {
  code: int; at: int; segwit: bool;
  nin: int; nout: int; nitems: int;
  witness: int; locktime: int; total: int; stripped: int;
}
```

### Worked example: a legacy transaction, byte by byte

`tx_valid.json` names transaction `23b397ed…` in a comment. It has one
input and one output, 158 bytes:

| Bytes | Field | Value |
| --- | --- | --- |
| 0–3 | version | `01000000`: 1 |
| 4 | input count | `01` |
| 5–36 | the spent txid, hash order | `b14bdc…0ba260`, shown as `60a20bd9…dc4bb1` |
| 37–40 | the output index | `00000000`: 0 |
| 41 | scriptSig length | `49`: 73 |
| 42–114 | scriptSig | a 73-byte push |
| 115–118 | sequence | `ffffffff` |
| 119 | output count | `01` |
| 120–127 | value | `40420f0000000000`: 1,000,000 satoshis |
| 128 | scriptPubKey length | `19`: 25 |
| 129–153 | scriptPubKey | `76a914…88ac`, a P2PKH script |
| 154–157 | locktime | `00000000`: 0 |

The example prints these fields from the positions the reader returns.

## CompactSize

Every count and every length is a CompactSize. A value below `0xfd`
is one byte. Above that, a prefix byte says how many little-endian bytes
follow:

| First byte | Then | Values |
| --- | --- | --- |
| `00`–`fc` | nothing | 0–252 |
| `fd` | 2 bytes | 253–65,535 |
| `fe` | 4 bytes | 65,536–4,294,967,295 |
| `ff` | 8 bytes | 4,294,967,296 and up |

The format can write a small value in a longer form: `fd 05 00` would
mean 5. Core refuses that: its `ReadCompactSize` throws "non-canonical ReadCompactSize()" for any value
written in more bytes than it needs. Without the rule, one transaction
would have many serializations, and so many txids, while meaning the same
thing; anyone relaying it could change its ID.

Core also refuses any count or length above `MAX_SIZE`, 32 MiB
(`0x02000000`), with "ReadCompactSize(): size too large". No transaction
or block is that big, so no valid 9-byte form exists: every value that
needs 8 bytes is above the limit. The reader refuses a 9-byte form as
non-canonical when its value fits in 4 bytes, and as too large otherwise,
the same order Core checks in.

<!-- source: coax/lib/bitcoin/tx.coax compact -->
```jai
// ReadCompactSize at b[at]: one byte below 0xfd; 0xfd, 0xfe or 0xff and
// then 2, 4 or 8 bytes, little-endian. Each value has one encoding, the
// shortest, and none is above MAX_SIZE. A refusal leaves next at at.
tx_compact_read_vartime :: (b: [] u8, at: int) -> TxCompact #export_c {
  if at >= b.count { return TxCompact.{next=at, code=TX_TRUNCATED}; }
  first := cast(int) b[at];
  if first < 0xfd { return TxCompact.{value=first, next=at+1}; }
  width := ifx first == 0xfd then 2 else ifx first == 0xfe then 4 else 8;
  if width > b.count - at - 1 { return TxCompact.{next=at, code=TX_TRUNCATED}; }
  if width == 8 {
    // Canonical from 2^32, which is above MAX_SIZE: refused either way,
    // as non-canonical when the high four bytes are zero.
    high := 0;
    for i: 4..<8 { high |= cast(int) b[at+1+i]; }
    return TxCompact.{next=at, code=ifx high == 0 then TX_NONCANONICAL else TX_SIZE_TOO_LARGE};
  }
  value := 0;
  for i: 0..<width { value |= cast(int) b[at+1+i] << (8 * i); }
  if value < (ifx width == 2 then 0xfd else 0x10000) { return TxCompact.{next=at, code=TX_NONCANONICAL}; }
  if value > TX_MAX_SIZE { return TxCompact.{next=at, code=TX_SIZE_TOO_LARGE}; }
  return TxCompact.{value=value, next=at+1+width};
}
// The bytes WriteCompactSize uses for value (at least 0).
tx_compact_size_vartime :: (value: int) -> int #export_c {
  if value < 0xfd { return 1; }
  if value <= 0xffff { return 3; }
  if value <= 0xffffffff { return 5; }
  return 9;
}
// WriteCompactSize into out at at, which has room; where writing goes on.
tx_compact_write_vartime :: (out: [] u8, at: int, value: int) -> int #export_c {
  size := tx_compact_size_vartime(value);
  if size == 1 { out[at] = cast(u8) value; return at + 1; }
  out[at] = cast(u8) (ifx size == 3 then 0xfd else ifx size == 5 then 0xfe else 0xff);
  for i: 0..<size-1 { out[at+1+i] = cast(u8) ((value >> (8 * i)) & 255); }
  return at + size;
}
```

## Reading the lists

Inputs, outputs and witness stacks are read the same way: a count, then
that many entries, each with its own lengths. Each step checks that the
bytes it needs are there before it reads them, and reports a short read
at the start of the field that ran out. A count can be as large as
`MAX_SIZE`, and Core reads until the count is done or the bytes run out.
So does this reader; a count larger than the bytes can hold ends as
`TX_TRUNCATED`, after at most one step per entry the bytes could hold.

The caller's arrays have a fixed size. When a transaction has more inputs,
outputs or witness items than they hold, the reader returns
`TX_CAPACITY` at the entry that didn't fit. That limit is the caller's,
not Core's.

<!-- source: coax/lib/bitcoin/tx.coax inputs -->
```jai
// A count of inputs at b[start], then each input: 36 bytes of outpoint,
// the scriptSig with its length, 4 bytes of sequence. Like Core, it reads
// until the count is done or the bytes refuse, so a count larger than the
// bytes can hold ends as TX_TRUNCATED, after at most one step per 41 bytes.
tx_inputs_vartime :: (b: [] u8, start: int, ins: [] TxIn) -> TxStep {
  n := tx_compact_read_vartime(b, start);
  if n.code != TX_OK { return TxStep.{at=start, code=n.code}; }
  at := n.next;
  for k: 0..<n.value {
    if k == ins.count { return TxStep.{at=at, code=TX_CAPACITY}; }
    if b.count - at < 36 { return TxStep.{at=at, code=TX_TRUNCATED}; }
    script := tx_span_vartime(b, at + 36);
    if script.code != TX_OK { return TxStep.{at=script.fail, code=script.code}; }
    if b.count - script.next < 4 { return TxStep.{at=script.next, code=TX_TRUNCATED}; }
    ins[k] = TxIn.{outpoint=at, script=script.at, script_n=script.n, sequence=script.next};
    at = script.next + 4;
  }
  return TxStep.{at=at, count=n.value};
}
// A count of outputs, then each output: 8 bytes of value, the
// scriptPubKey with its length.
tx_outputs_vartime :: (b: [] u8, start: int, outs: [] TxOut) -> TxStep {
  n := tx_compact_read_vartime(b, start);
  if n.code != TX_OK { return TxStep.{at=start, code=n.code}; }
  at := n.next;
  for k: 0..<n.value {
    if k == outs.count { return TxStep.{at=at, code=TX_CAPACITY}; }
    if b.count - at < 8 { return TxStep.{at=at, code=TX_TRUNCATED}; }
    script := tx_span_vartime(b, at + 8);
    if script.code != TX_OK { return TxStep.{at=script.fail, code=script.code}; }
    outs[k] = TxOut.{value=at, script=script.at, script_n=script.n};
    at = script.next;
  }
  return TxStep.{at=at, count=n.value};
}
// One witness stack per input: a count of items, then each item with its
// length. count is the number of items in all the stacks.
tx_witness_vartime :: (b: [] u8, start: int, ins: [] TxIn, items: [] TxItem) -> TxStep {
  at := start; total := 0;
  for k: 0..<ins.count {
    n := tx_compact_read_vartime(b, at);
    if n.code != TX_OK { return TxStep.{at=at, code=n.code}; }
    at = n.next; ins[k].first = total; ins[k].count = n.value;
    for j: 0..<n.value {
      if total == items.count { return TxStep.{at=at, code=TX_CAPACITY}; }
      item := tx_span_vartime(b, at);
      if item.code != TX_OK { return TxStep.{at=item.fail, code=item.code}; }
      items[total] = TxItem.{at=item.at, n=item.n};
      total += 1; at = item.next;
    }
  }
  return TxStep.{at=at, count=total};
}
```

## Marker, flag and witness

An old node reads the marker `00` as an input count of zero. A
transaction with no inputs is invalid, so BIP-144 could take that value:
when the input count is zero and the reader allows witnesses, the next
byte is a flag. Core's `UnserializeTransaction` then does this:

1. If the flag is not zero, read the inputs and the outputs.
2. If bit 0 of the flag is set, read one witness stack per input. If
   every stack is empty, refuse: "Superfluous witness record". A
   transaction with no witness data must use the legacy form, so it has
   one serialization.
3. If any other bit of the flag is set, refuse: "Unknown transaction
   optional data". The bits are kept for future extensions.
4. Read the locktime.

A flag of zero reads nothing in step 1. That is what Core does for a
legacy transaction with no inputs and no outputs, whose output count
sits where the flag would be: it reads the same either way.

<!-- source: coax/lib/bitcoin/tx.coax read -->
```jai
// UnserializeTransaction. With witness false it reads the legacy form:
// version, inputs, outputs, locktime. With witness true, an input count of
// zero is BIP-144's marker: the next byte is the flag, and a flag other
// than zero is followed by the inputs, the outputs and, for flag 1, one
// witness stack per input. A flag of zero is also what a legacy
// transaction with no inputs and no outputs has there, its output count,
// and reads the same. Bytes after the locktime are left for the caller.
tx_parse_vartime :: (b: [] u8, witness: bool, ins: [] TxIn, outs: [] TxOut, items: [] TxItem) -> Tx #export_c {
  if b.count < 4 { return tx_refused(TX_TRUNCATED, 0); }
  vin := tx_inputs_vartime(b, 4, ins);
  if vin.code != TX_OK { return tx_refused(vin.code, vin.at); }
  vout: TxStep; at := vin.at; flags := 0;
  if vin.count == 0 && witness {
    if at == b.count { return tx_refused(TX_TRUNCATED, at); }
    flags = cast(int) b[at]; at += 1;
    if flags != 0 {
      vin = tx_inputs_vartime(b, at, ins);
      if vin.code != TX_OK { return tx_refused(vin.code, vin.at); }
      vout = tx_outputs_vartime(b, vin.at, outs);
      if vout.code != TX_OK { return tx_refused(vout.code, vout.at); }
      at = vout.at;
    }
  } else {
    vout = tx_outputs_vartime(b, at, outs);
    if vout.code != TX_OK { return tx_refused(vout.code, vout.at); }
    at = vout.at;
  }
  t := Tx.{nin=vin.count, nout=vout.count, witness=at};
  if (flags & 1) != 0 {
    flags ^= 1;
    w := tx_witness_vartime(b, at, ins[0..<vin.count], items);
    if w.code != TX_OK { return tx_refused(w.code, w.at); }
    if w.count == 0 { return tx_refused(TX_NO_WITNESS, at); }     // every stack empty
    t.segwit = true; t.nitems = w.count; at = w.at;
  }
  if flags != 0 { return tx_refused(TX_UNKNOWN_FLAG, 5); }
  if b.count - at < 4 { return tx_refused(TX_TRUNCATED, at); }
  t.locktime = at; t.total = at + 4;
  t.stripped = ifx t.segwit then t.total - 2 - (t.locktime - t.witness) else t.total;
  return t;
}
// One whole transaction: the bytes after the locktime refuse, as in
// Core's DecodeTx. Inside a block, tx_parse_vartime reads one and the next
// starts at its total.
tx_read_vartime :: (b: [] u8, witness: bool, ins: [] TxIn, outs: [] TxOut, items: [] TxItem) -> Tx #export_c {
  t := tx_parse_vartime(b, witness, ins, outs, items);
  if t.code == TX_OK && t.total != b.count { return tx_refused(TX_TRAILING, t.total); }
  return t;
}
```

`tx_parse_vartime` stops after the locktime, as Core's reader does in a
block, where the next transaction starts there. `tx_read_vartime` reads
one transaction on its own and refuses bytes left over, as Core's
`DecodeTx` does.

### Worked example: a P2WPKH spend

Transaction `f5864806…` is the one rust-bitcoin's tests use. It spends one
P2WPKH output, 193 bytes:

| Bytes | Field |
| --- | --- |
| 0–3 | version 2 |
| 4, 5 | marker `00`, flag `01` |
| 6 | input count 1 |
| 7–42 | outpoint |
| 43 | scriptSig length 0: a native segwit spend has an empty scriptSig |
| 44–47 | sequence |
| 48 | output count 1 |
| 49–56 | value 506,078 satoshis |
| 57–80 | scriptPubKey length 23 and a P2SH script |
| 81 | witness items for input 0: 2 |
| 82–154 | a signature: length 72, then 72 bytes |
| 155–188 | a public key: length 33, then 33 bytes |
| 189–192 | locktime |

The witness runs from byte 81 to byte 189, 108 bytes. Without it, and
without the marker and the flag, the transaction is 193 − 2 − 108 = 83
bytes. That is the stripped form.

## Bytes that read two ways

A transaction with no inputs and some outputs is invalid on the chain,
but it is a fine template: a wallet creates one, and later adds inputs to
pay for it. Its bytes start with the version and a zero, the same as a
segwit transaction. For example:

```text
01000000 00 01 00e1f50500000000 19 76a914…88ac 00000000
version  no inputs, one output of 1 BTC, a P2PKH script, locktime
```

Read with witnesses, the `00` is the marker and the `01` is the flag. The
reader then takes the value's first byte, `00`, as the input count, `e1`
as the output count, and runs out of bytes. Read without witnesses, it is
a valid template.

Core's RPCs that take raw transactions, such as `decoderawtransaction`,
use `DecodeTx` in `core_io.cpp`. It tries both readings, keeps each one
only if it uses every byte, and then decides:

1. If the extended reading worked and every script in it parses
   (`CheckTxScriptsSanity`), keep it.
2. Else if the legacy reading worked and its scripts parse, keep that.
3. Else keep whichever reading worked, the extended one first.

A script "parses" when each push fits in the script and is at most 520
bytes, each opcode is at most `OP_NOP10`, and the script is at most 10,000
bytes. A coinbase's scriptSig isn't checked. This is a guess about
intent, not a validity check: a reading that produced nonsense scripts is
probably the wrong reading. Chapter 36 parses scripts fully.

<!-- source: coax/lib/bitcoin/tx.coax sanity -->
```jai
// CScript::HasValidOps: every op parses, a push's length fits in the
// script, no push is longer than 520 bytes, and no opcode is above
// OP_NOP10 (0xb9).
tx_ops_valid_vartime :: (s: [] u8) -> bool #export_c {
  pc := 0;
  while pc < s.count {
    op := cast(int) s[pc]; pc += 1;
    if op <= 0x4e {
      width := ifx op < 0x4c then 0 else ifx op == 0x4c then 1 else ifx op == 0x4d then 2 else 4;
      if width > s.count - pc { return false; }
      size := ifx width == 0 then op else tx_le_vartime(s, pc, width);
      pc += width;
      if size > s.count - pc || size > TX_MAX_ELEMENT { return false; }
      pc += size;
    }
    if op > TX_MAX_OPCODE { return false; }
  }
  return true;
}
// A coinbase has one input, whose outpoint is null: 32 zero bytes and
// index 0xffffffff.
tx_coinbase_vartime :: (b: [] u8, t: Tx, ins: [] TxIn) -> bool #export_c {
  if t.nin != 1 { return false; }
  at := ins[0].outpoint;
  for i: 0..<32 { if b[at+i] != 0 { return false; } }
  return tx_le_vartime(b, at + 32, 4) == 0xffffffff;
}
// CheckTxScriptsSanity: every scriptSig (not a coinbase's) and every
// scriptPubKey has valid ops and at most 10,000 bytes.
tx_scripts_sane_vartime :: (b: [] u8, t: Tx, ins: [] TxIn, outs: [] TxOut) -> bool #export_c {
  if !tx_coinbase_vartime(b, t, ins) {
    for k: 0..<t.nin {
      if ins[k].script_n > TX_MAX_SCRIPT || !tx_ops_valid_vartime(b[ins[k].script..<ins[k].script+ins[k].script_n]) { return false; }
    }
  }
  for k: 0..<t.nout {
    if outs[k].script_n > TX_MAX_SCRIPT || !tx_ops_valid_vartime(b[outs[k].script..<outs[k].script+outs[k].script_n]) { return false; }
  }
  return true;
}
```

<!-- source: coax/lib/bitcoin/tx.coax decode -->
```jai
// Core's DecodeTx, as decoderawtransaction runs it: read with witnesses
// and without, keep only readings that use every byte, and when both do,
// prefer the one whose scripts parse, then the extended one. When neither
// reads, the extended reading's refusal is returned (Core gives no reason).
// The arrays hold the reading returned.
tx_decode_vartime :: (b: [] u8, ins: [] TxIn, outs: [] TxOut, items: [] TxItem) -> Tx #export_c {
  extended := tx_read_vartime(b, true, ins, outs, items);
  if extended.code == TX_OK && tx_scripts_sane_vartime(b, extended, ins, outs) { return extended; }
  legacy := tx_read_vartime(b, false, ins, outs, items);
  if legacy.code == TX_OK && tx_scripts_sane_vartime(b, legacy, ins, outs) { return legacy; }
  if extended.code == TX_OK { return tx_read_vartime(b, true, ins, outs, items); }
  if legacy.code == TX_OK { return legacy; }
  return extended;
}
```

Core gives no reason when both readings fail. `tx_decode_vartime` returns
the extended reading's refusal, so the caller still sees a code and a
byte. The example builds bytes that read both ways in each of the three
cases and shows which reading Core keeps.

## txid and wtxid

The txid is SHA-256d (chapter 5) of the stripped serialization. The
wtxid, from BIP-141, is SHA-256d of the full one. For a transaction
without witnesses the two forms are the same bytes, so the two IDs are
equal.

Witnesses carry the signatures, and a signature can change without
changing what it signs: chapter 21 shows how a DER encoding could be
changed before BIP-66, and an ECDSA signature `(r, s)` stays valid as
`(r, n − s)`. Before segwit, a relaying node could change the bytes of
a signature and so the txid of a transaction that spent the coin, and
any transaction that referred to it by txid broke. Segwit moved the
signatures out of what the txid hashes. A segwit spend's txid is fixed
once it is signed, which is what the Lightning Network needs to sign
transactions that spend outputs of transactions not yet broadcast.

The reader doesn't build the stripped form to hash it. It hashes three
ranges of the original bytes: the version (bytes 0 to 4), everything from
the input count to the end of the outputs (6 to `witness`), and the
locktime. A large transaction is hashed in pieces of at most one
SHA-256 update (chapter 4's `CB_MAX_BYTES`, 1 MiB).

<!-- source: coax/lib/bitcoin/tx.coax ids -->
```jai
// The txid: SHA-256d of the stripped serialization. For a segwit
// transaction that is the bytes without the marker and flag (4 and 5) and
// without the stacks (witness to locktime). The wtxid is SHA-256d of all
// the bytes; without witnesses the two are equal. Both are written in hash
// order; bitcoin_reverse32 gives the order explorers and RPCs show.
tx_txid_vartime :: (b: [] u8, t: Tx, out: [] u8) -> bool #export_c {
  if t.code != TX_OK || out.count != 32 { return false; }
  if t.segwit { tx_hash(b, [6] int.[0, 4, 6, t.witness, t.locktime, t.total], out); }
  else { tx_hash(b, [6] int.[0, t.total, 0, 0, 0, 0], out); }
  return true;
}
tx_wtxid_vartime :: (b: [] u8, t: Tx, out: [] u8) -> bool #export_c {
  if t.code != TX_OK || out.count != 32 { return false; }
  tx_hash(b, [6] int.[0, t.total, 0, 0, 0, 0], out);
  return true;
}
```

### Byte order

SHA-256d returns 32 bytes, and those bytes are the txid in hash order:
the order an outpoint stores it in, and the order a Merkle tree hashes.
Bitcoin's software has always shown it reversed, as if the 32 bytes were
a little-endian number: block explorers, the RPCs and `tx_valid.json`'s
comments all use that order. The legacy example above hashes to
`63dd949a…97b323`, which is shown as `23b397ed…94dd63`.
`bitcoin_reverse32` (chapter 5) converts between the two. When you copy a
txid from an explorer into an outpoint, reverse it.

## Size, weight and virtual size

BIP-141 replaced the 1 MB block size limit with a weight limit of
4,000,000. A byte outside the witness weighs 4 and a witness byte weighs
1:

```text
weight = stripped size × 4 + (total size − stripped size)
       = stripped size × 3 + total size
virtual size (vsize) = weight / 4, rounded up
```

Old nodes see only the stripped form, and 4,000,000 weight units hold at
most 1,000,000 stripped bytes, so the old limit still holds for them. For
a legacy transaction the weight is four times the size and the vsize is
the size. Fees are quoted in satoshis per virtual byte.

The P2WPKH spend above has stripped size 83 and total size 193:
83 × 3 + 193 = 442, and 442 / 4 = 110.5, rounded up to 111. rust-bitcoin
publishes the same weight and vsize, and mempool.space the same weight.

<!-- source: coax/lib/bitcoin/tx.coax weight -->
```jai
// BIP-141: weight = stripped size × 3 + total size, so a byte outside the
// witness weighs 4 and a witness byte 1. The virtual size is weight / 4,
// rounded up.
tx_weight_vartime :: (t: Tx) -> int #export_c { return t.stripped * (TX_SCALE - 1) + t.total; }
tx_vsize_vartime :: (t: Tx) -> int #export_c { return (tx_weight_vartime(t) + TX_SCALE - 1) / TX_SCALE; }
```

## Writing a transaction back

Core's `SerializeTransaction` writes the extended form only when the
caller allows witnesses and some input has a witness; otherwise it writes
the legacy form. Counts and lengths are written in their shortest form,
which is the only form the reader accepts, so a transaction read and
written back is the same bytes. Written without witnesses, it is the
stripped form, which reads back as a legacy transaction with the same
txid.

<!-- source: coax/lib/bitcoin/tx.coax write -->
```jai
// SerializeTransaction of a read transaction into out: the extended form
// when witness is true and some input has a witness, else the legacy form.
// Counts and lengths are written in their shortest form. Returns the bytes
// written, or -1 when t is a refusal or out is too short (nothing written).
tx_write_vartime :: (b: [] u8, t: Tx, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, witness: bool, out: [] u8) -> int #export_c {
  extended := witness && t.segwit;
  if t.code != TX_OK || out.count < (ifx extended then t.total else t.stripped) { return -1; }
  at := tx_put(out, 0, b[0..<4]);
  if extended { out[at] = 0; out[at+1] = 1; at += 2; }
  at = tx_compact_write_vartime(out, at, t.nin);
  for k: 0..<t.nin {
    i := ins[k];
    at = tx_put(out, at, b[i.outpoint..<i.outpoint+36]);
    at = tx_compact_write_vartime(out, at, i.script_n);
    at = tx_put(out, at, b[i.script..<i.script+i.script_n]);
    at = tx_put(out, at, b[i.sequence..<i.sequence+4]);
  }
  at = tx_compact_write_vartime(out, at, t.nout);
  for k: 0..<t.nout {
    o := outs[k];
    at = tx_put(out, at, b[o.value..<o.value+8]);
    at = tx_compact_write_vartime(out, at, o.script_n);
    at = tx_put(out, at, b[o.script..<o.script+o.script_n]);
  }
  if extended {
    for k: 0..<t.nin {
      at = tx_compact_write_vartime(out, at, ins[k].count);
      for j: ins[k].first..<ins[k].first+ins[k].count {
        at = tx_compact_write_vartime(out, at, items[j].n);
        at = tx_put(out, at, b[items[j].at..<items[j].at+items[j].n]);
      }
    }
  }
  return tx_put(out, at, b[t.locktime..<t.total]);
}
```

## The refusals

| Code | Core's message | Reported at |
| --- | --- | --- |
| `TX_TRUNCATED` | "end of data" | the start of the field that ran out; for a script or item, the first byte after its length |
| `TX_NONCANONICAL` | "non-canonical ReadCompactSize()" | the CompactSize's first byte |
| `TX_SIZE_TOO_LARGE` | "ReadCompactSize(): size too large" | the CompactSize's first byte |
| `TX_UNKNOWN_FLAG` | "Unknown transaction optional data" | byte 5, the flag |
| `TX_NO_WITNESS` | "Superfluous witness record" | the first witness stack |
| `TX_TRAILING` | `DecodeTx` returns false | the first byte after the locktime |
| `TX_CAPACITY` | none: the caller's arrays | the input, output or item that didn't fit |

Reading in order and stopping at the first problem means a transaction
has one refusal, and the Python transliteration finds the same one at the
same byte.

## The audit

Two readers and two writers of CompactSize already existed, written for
Nostr chapter 104's agent treasuries.

- `sighash.coax` had its own unsigned-transaction reader, `bt_tx_read`,
  over its own CompactSize reader, `bt_var`. It read only the 1-, 3- and
  5-byte forms and refused the 9-byte form, which is the right answer for
  every 9-byte form as long as `MAX_SIZE` holds. It found the field
  positions first and checked that the last one ended at the end of the
  bytes before copying anything; a malformed length made that check fail.
  Its answers were right for the transactions it reads, but it was a
  second reader to keep in step with Core.
- `bt_var_put` wrote only the 1- and 3-byte forms, so a value of 65,536 or
  more would have been written wrong. The signature hashes it writes for
  have short scripts and few inputs, so no published answer was affected.

`bt_tx_read` now calls `tx_read_vartime` in the legacy form, then copies
what it needs as before; `bt_var_put` calls `tx_compact_write_vartime`;
`bt_var` is gone. Its own checks still pass, and Nostr chapter 104's
listings of `sighash.coax` (`bip143` and `tweak`) didn't change.

`nipkeys.coax` reads PSBTs with `ky_var`, which takes only the 1- and
3-byte forms. PSBTs are chapter 94's subject, so it is left as it is and
noted for that chapter.

## The example

The example is
[bitcoin/examples/transactions.coax](../examples/transactions.coax). It
reads the legacy transaction field by field, writes and reads each width
of CompactSize and the forms Core refuses, reads the three segwit mainnet
transactions with their witness stacks and both IDs, hashes the stripped
form by hand, shows one refusal of each kind with the byte it is at,
decodes the bytes that read two ways, and reads and writes back every
transaction in Core's two test files.

<!-- source: bitcoin/examples/transactions.coax example -->
```jai
#load "../../coax/lib/bitcoin/tx.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/tx_core_vectors.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [400] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode hex into out and return the filled part. Every literal here and
// in the vector tables is valid lowercase hex, so an error stops the program.
decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
equal_bytes :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}
// The name of each refusal code.
CODES :: [8] string.["ok", "truncated", "non-canonical CompactSize", "size too large", "unknown flag",
  "witness flag, no witnesses", "trailing bytes", "capacity"];
// A txid or wtxid as explorers show it: the digest reversed.
show_id :: (label: string, digest: [] u8) {
  shown: [32] u8;
  #assert bitcoin_reverse32(digest, shown[..]) == SHA_OK;
  show(label, shown[..]);
}
// Is the displayed hex the reverse of digest?
id_is :: (digest: [] u8, hex: string) -> bool {
  want: [32] u8; shown: [32] u8;
  #assert bitcoin_reverse32(digest, shown[..]) == SHA_OK;
  return equal_bytes(shown[..], decode(hex, want[..]));
}
// Read hex both ways, as Core's DecodeTx does, and say what each gives.
both_ways :: (label: string, hex: string) {
  b: [256] u8; ins: [4] TxIn; outs: [4] TxOut; items: [4] TxItem;
  s := decode(hex, b[..]);
  e := tx_read_vartime(s, true, ins[..], outs[..], items[..]);
  sane_e := e.code == TX_OK && tx_scripts_sane_vartime(s, e, ins[..], outs[..]);
  l := tx_read_vartime(s, false, ins[..], outs[..], items[..]);
  sane_l := l.code == TX_OK && tx_scripts_sane_vartime(s, l, ins[..], outs[..]);
  d := tx_decode_vartime(s, ins[..], outs[..], items[..]);
  print("%\n  with witnesses: %, scripts parse %\n  without: %, scripts parse %\n", label, CODES[e.code], sane_e, CODES[l.code], sane_l);
  if d.code != TX_OK { print("  Core keeps neither: %\n", CODES[d.code]); }
  else { print("  Core keeps: inputs %, outputs %, segwit %\n", d.nin, d.nout, d.segwit); }
}

main :: () {
  b: [4096] u8; o: [4096] u8; d: [32] u8; e: [32] u8;
  ins: [16] TxIn; outs: [16] TxOut; items: [32] TxItem;

  // A legacy mainnet transaction, field by field.
  m := TX_MAINNET[0];
  print("a legacy transaction, %\n", m.txid);
  s := decode(m.tx, b[..]);
  t := tx_read_vartime(s, true, ins[..], outs[..], items[..]);
  print("version %, inputs %, outputs %, locktime %\n", tx_le_vartime(s, 0, 4), t.nin, t.nout, tx_le_vartime(s, t.locktime, 4));
  show_id("input 0 spends txid", s[ins[0].outpoint..<ins[0].outpoint+32]);
  print("  output index %, scriptSig % bytes at byte %\n", tx_le_vartime(s, ins[0].outpoint + 32, 4), ins[0].script_n, ins[0].script);
  show("  sequence", s[ins[0].sequence..<ins[0].sequence+4]);
  print("output 0: % satoshis\n", tx_le_vartime(s, outs[0].value, 8));
  show("  scriptPubKey", s[outs[0].script..<outs[0].script+outs[0].script_n]);
  print("% bytes, weight %, vsize %\n", t.total, tx_weight_vartime(t), tx_vsize_vartime(t));
  #assert tx_txid_vartime(s, t, d[..]);
  show("txid in hash order", d[..]);
  show_id("txid as shown", d[..]);
  print("matches tx_valid.json: %\n", id_is(d[..], m.txid));

  // CompactSize: the shortest form of each value, and what Core refuses.
  print("CompactSize\n");
  values := [7] int.[0, 252, 253, 0xffff, 0x10000, TX_MAX_SIZE, 0x100000000];
  for v: values {
    n := tx_compact_write_vartime(o[..], 0, v);
    c := tx_compact_read_vartime(o[0..<n], 0);
    print("  %: % bytes, ", v, n);
    show(CODES[c.code], o[0..<n]);
  }
  bad := [6] string.["fdfc00", "feffff0000", "ffffffffff00000000", "fe01000002", "fdfd", "ff0000000001000000"];
  for x: bad {
    c := tx_compact_read_vartime(decode(x, b[..]), 0);
    print("  %: %\n", x, CODES[c.code]);
  }

  // The pinned segwit transactions: marker, flag and witness, two IDs.
  for i: 3..<6 {
    m = TX_MAINNET[i];
    s = decode(m.tx, b[..]);
    t = tx_read_vartime(s, true, ins[..], outs[..], items[..]);
    print("a segwit transaction, %\n", m.txid);
    print("marker %, flag %, inputs %, outputs %, witness stacks from byte % to %\n", s[4], s[5], t.nin, t.nout, t.witness, t.locktime);
    for k: 0..<t.nin {
      print("  input % stack, item sizes:", k);
      for j: ins[k].first..<ins[k].first+ins[k].count { print(" %", items[j].n); }
      print("\n");
    }
    print("stripped %, total %, weight % (published %), vsize %\n", t.stripped, t.total, tx_weight_vartime(t), m.weight, tx_vsize_vartime(t));
    #assert tx_txid_vartime(s, t, d[..]) && tx_wtxid_vartime(s, t, e[..]);
    show_id("txid ", d[..]);
    show_id("wtxid", e[..]);
    print("txid matches: %, wtxid matches: %\n", id_is(d[..], m.txid), id_is(e[..], m.wtxid));
  }
  // The stripped form of the P2WPKH spend: what the txid hashes.
  s = decode(TX_MAINNET[3].tx, b[..]);
  t = tx_read_vartime(s, true, ins[..], outs[..], items[..]);
  n := tx_write_vartime(s, t, ins[..], outs[..], items[..], false, o[..]);
  show("stripped", o[0..<n]);
  #assert sha256d_checked(o[0..<n], d[..], false) == SHA_OK;
  print("SHA-256d of it is the txid: %\n", id_is(d[..], TX_MAINNET[3].txid));

  // One change to each part of a transaction, and what the reader says.
  print("refusals\n");
  legacy := TX_LEGACY_OK;
  s = decode(legacy, b[..]);
  for cut: [5] int.[3, 20, 45, 52, 60] {
    r := tx_read_vartime(s[0..<cut], true, ins[..], outs[..], items[..]);
    print("  the first % bytes: % at byte %\n", cut, CODES[r.code], r.at);
  }
  b[62] = 0xab;
  r := tx_read_vartime(b[0..<63], true, ins[..], outs[..], items[..]);
  print("  a byte after the locktime: % at byte %\n", CODES[r.code], r.at);
  cases := [5] string.[
    "01000000fd0100",
    "01000000fe00000003",
    "010000000002000000000000000000",
    "010000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff0151ffffffff01000000000000000001510000000000",
    "010000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff0151ffffffff0100000000000000000151fd0000"];
  labels := [5] string.["input count 1 in three bytes", "input count 0x03000000", "flag 2",
    "flag 1, every stack empty", "item count 0 in three bytes"];
  for i: 0..<5 {
    r = tx_read_vartime(decode(cases[i], b[..]), true, ins[..], outs[..], items[..]);
    print("  %: % at byte %\n", labels[i], CODES[r.code], r.at);
  }
  s = decode(TX_MAINNET[5].tx, b[..]);
  r = tx_read_vartime(s, true, ins[..], outs[..], items[0..<3]);
  print("  % witness items into room for 3: % at byte %\n", 4, CODES[r.code], r.at);

  // Bytes that read two ways.
  print("zero inputs\n");
  both_ways("no inputs, no outputs", "01000000000000000000");
  both_ways("legacy, no inputs, one output", TX_ZERO_INPUTS);
  both_ways("extended, no inputs, no witness", TX_SEGWIT_NO_INPUTS);
  both_ways("both read, both sane", TX_BOTH_SANE);
  both_ways("both read, legacy sane", TX_LEGACY_SANE);
  both_ways("both read, neither sane", TX_NEITHER_SANE);

  // Every transaction in Core's tx_valid.json and tx_invalid.json.
  print("Bitcoin Core tx_valid.json and tx_invalid.json\n");
  read := 0; segwit := 0; same := 0; stripped := 0; valid := 0;
  for row: TX_CORE {
    s = decode(row.tx, b[..]);
    t = tx_read_vartime(s, true, ins[..], outs[..], items[..]);
    if t.code != TX_OK { continue; }
    read += 1;
    if row.valid { valid += 1; }
    if t.segwit { segwit += 1; }
    // Written back with witnesses: the same bytes.
    if tx_write_vartime(s, t, ins[..], outs[..], items[..], true, o[..]) == s.count && equal_bytes(o[0..<s.count], s) { same += 1; }
    // Written without: a legacy transaction with the same txid.
    n = tx_write_vartime(s, t, ins[..], outs[..], items[..], false, o[..]);
    u := tx_read_vartime(o[0..<n], false, ins[..], outs[..], items[..]);
    #assert tx_txid_vartime(s, t, d[..]) && tx_txid_vartime(o[0..<n], u, e[..]);
    if u.code == TX_OK && !u.segwit && u.total == t.stripped && equal_bytes(d[..], e[..]) { stripped += 1; }
  }
  print("% of % read (% of tx_valid.json), % with witnesses\n", read, TX_CORE.count, valid, segwit);
  print("% write back byte for byte, % stripped and read back with the same txid\n", same, stripped);
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/transactions.coax
```

The output:

```text
a legacy transaction, 23b397edccd3740a74adb603c9756370fafcde9bcc4483eb271ecad09a94dd63
version 1, inputs 1, outputs 1, locktime 0
input 0 spends txid: 60a20bd93aa49ab4b28d514ec10b06e1829ce6818ec06cd3aabd013ebcdc4bb1
  output index 0, scriptSig 73 bytes at byte 42
  sequence: ffffffff
output 0: 1000000 satoshis
  scriptPubKey: 76a914660d4ef3a743e3e696ad990364e555c271ad504b88ac
158 bytes, weight 632, vsize 158
txid in hash order: 63dd949ad0ca1e27eb8344cc9bdefcfa706375c903b6ad740a74d3cced97b323
txid as shown: 23b397edccd3740a74adb603c9756370fafcde9bcc4483eb271ecad09a94dd63
matches tx_valid.json: true
CompactSize
  0: 1 bytes, ok: 00
  252: 1 bytes, ok: fc
  253: 3 bytes, ok: fdfd00
  65535: 3 bytes, ok: fdffff
  65536: 5 bytes, ok: fe00000100
  33554432: 5 bytes, ok: fe00000002
  4294967296: 9 bytes, size too large: ff0000000001000000
  fdfc00: non-canonical CompactSize
  feffff0000: non-canonical CompactSize
  ffffffffff00000000: non-canonical CompactSize
  fe01000002: size too large
  fdfd: truncated
  ff0000000001000000: size too large
a segwit transaction, f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206
marker 0, flag 1, inputs 1, outputs 1, witness stacks from byte 81 to 189
  input 0 stack, item sizes: 72 33
stripped 83, total 193, weight 442 (published 442), vsize 111
txid : f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206
wtxid: 80b7d8a82d5d5bf92905b06f2014dd699e03837ca172e3a59d51426ebbe3e7f5
txid matches: true, wtxid matches: true
a segwit transaction, 33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036
marker 0, flag 1, inputs 1, outputs 2, witness stacks from byte 150 to 216
  input 0 stack, item sizes: 64
stripped 152, total 220, weight 676 (published 676), vsize 169
txid : 33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036
wtxid: af2fdc4c54270adfb2a65987a79ed2f0e771a779ea48bb0ef06095b48395f74d
txid matches: true, wtxid matches: true
a segwit transaction, 37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8
marker 0, flag 1, inputs 2, outputs 1, witness stacks from byte 121 to 354
  input 0 stack, item sizes: 64
  input 1 stack, item sizes: 64 34 65
stripped 123, total 358, weight 727 (published 727), vsize 182
txid : 37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8
wtxid: 2fb9e9ff2c166ffbd48406f7b9362125b665da2a760540065250188417f048c2
txid matches: true, wtxid matches: true
stripped: 0200000001595895ea20179de87052b4046dfe6fd515860505d6511a9004cf12a1f93cac7c0100000000ffffffff01deb807000000000017a9140f3444e271620c736808aa7b33e370bd87cb5a078700000000
SHA-256d of it is the txid: true
refusals
  the first 3 bytes: truncated at byte 0
  the first 20 bytes: truncated at byte 5
  the first 45 bytes: truncated at byte 43
  the first 52 bytes: truncated at byte 48
  the first 60 bytes: truncated at byte 58
  a byte after the locktime: trailing bytes at byte 62
  input count 1 in three bytes: non-canonical CompactSize at byte 4
  input count 0x03000000: size too large at byte 4
  flag 2: unknown flag at byte 5
  flag 1, every stack empty: witness flag, no witnesses at byte 60
  item count 0 in three bytes: non-canonical CompactSize at byte 60
  4 witness items into room for 3: capacity at byte 288
zero inputs
no inputs, no outputs
  with witnesses: ok, scripts parse true
  without: ok, scripts parse true
  Core keeps: inputs 0, outputs 0, segwit false
legacy, no inputs, one output
  with witnesses: truncated, scripts parse false
  without: ok, scripts parse true
  Core keeps: inputs 0, outputs 1, segwit false
extended, no inputs, no witness
  with witnesses: witness flag, no witnesses, scripts parse false
  without: trailing bytes, scripts parse false
  Core keeps neither: witness flag, no witnesses
both read, both sane
  with witnesses: ok, scripts parse true
  without: ok, scripts parse true
  Core keeps: inputs 1, outputs 1, segwit true
both read, legacy sane
  with witnesses: ok, scripts parse false
  without: ok, scripts parse true
  Core keeps: inputs 0, outputs 1, segwit false
both read, neither sane
  with witnesses: ok, scripts parse false
  without: ok, scripts parse false
  Core keeps: inputs 1, outputs 1, segwit true
Bitcoin Core tx_valid.json and tx_invalid.json
215 of 215 read (122 of tx_valid.json), 41 with witnesses
215 write back byte for byte, 215 stripped and read back with the same txid
```

What the output shows:

- The legacy transaction's fields, its 158 bytes, weight 632 (4 × 158)
  and vsize 158, and its txid in both orders.
- CompactSize: 252 is the last 1-byte value, 253 needs 3 bytes, and
  `MAX_SIZE` fits in 5. Each longer form of a smaller value refuses, and
  so do `MAX_SIZE` + 1 and every 9-byte form.
- The three segwit transactions: the marker and the flag, the witness
  stacks' item sizes (a 72-byte signature and a 33-byte key for P2WPKH; a
  64-byte Schnorr signature for the Taproot key path; a signature, a
  script and a control block for the script path), and weights equal to
  mempool.space's.
- The stripped form of the P2WPKH spend, whose SHA-256d is its txid.
- The refusals: a 62-byte legacy transaction cut inside its version, its
  outpoint, its sequence, its output's value and its locktime, then with
  a byte after the locktime; a count written in a longer form and one above
  `MAX_SIZE`; flag 2; flag 1 with every stack empty; and four witness
  items into room for three.
- The six cases of bytes that read two ways. A transaction with no inputs
  and no outputs reads the same both ways. The template with one output
  reads only as legacy. rust-bitcoin's extended form with no inputs and no
  witness refuses both ways. The three built cases show each of Core's
  three rules.
- All 215 transactions of `tx_valid.json` (122) and `tx_invalid.json`
  (93) read, 41 of them with witnesses. Each writes back byte for byte,
  and its stripped form reads back with the same txid.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/transactions.out](../fixtures/transactions.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/transactions.coax](../examples/checks/transactions.coax),
which loads the example:

- each of Core's 215 transactions: read, written back in full and
  stripped, the stripped form read back with the same txid and equal to
  its own wtxid, and the weight and vsize formulas;
- the six mainnet transactions through `tx_decode_vartime`: the published
  txid, wtxid, size and weight;
- a changed witness byte changes the wtxid and not the txid, and a
  changed value changes both;
- the example's refusals, with their codes and bytes;
- what Core keeps for each case of bytes that read two ways.

<!-- source: bitcoin/examples/checks/transactions.coax check -->
```jai
// Every transaction in Bitcoin Core's tx_valid.json and tx_invalid.json
// reads with witnesses allowed and uses every byte. Written back with
// witnesses it is the same bytes; written without, it reads back as a
// legacy transaction of the stripped size with the same txid, and writes
// the same bytes again.
#forall "Core's transactions read and round-trip" i: 0..<215 #steps 200000000 {
  row := TX_CORE[i];
  b: [4096] u8; o: [4096] u8; p: [4096] u8; d: [32] u8; e: [32] u8;
  ins: [16] TxIn; outs: [16] TxOut; items: [32] TxItem;
  s := decode(row.tx, b[..]);
  t := tx_read_vartime(s, true, ins[..], outs[..], items[..]);
  #assert t.code == TX_OK && t.total == s.count;
  #assert tx_write_vartime(s, t, ins[..], outs[..], items[..], true, o[..]) == s.count && equal_bytes(o[0..<s.count], s);
  n := tx_write_vartime(s, t, ins[..], outs[..], items[..], false, o[..]);
  #assert n == t.stripped;
  u := tx_read_vartime(o[0..<n], false, ins[..], outs[..], items[..]);
  #assert u.code == TX_OK && !u.segwit && u.total == n && u.nin == t.nin && u.nout == t.nout;
  #assert tx_write_vartime(o[0..<n], u, ins[..], outs[..], items[..], true, p[..]) == n && equal_bytes(p[0..<n], o[0..<n]);
  #assert tx_txid_vartime(s, t, d[..]) && tx_txid_vartime(o[0..<n], u, e[..]) && equal_bytes(d[..], e[..]);
  // Without witnesses, the txid is the wtxid and the weight is 4 bytes a byte.
  #assert tx_wtxid_vartime(o[0..<n], u, e[..]) && equal_bytes(d[..], e[..]) && tx_weight_vartime(u) == 4 * n;
  #assert tx_weight_vartime(t) == 3 * t.stripped + t.total && (tx_vsize_vartime(t) - 1) * 4 < tx_weight_vartime(t) && tx_vsize_vartime(t) * 4 >= tx_weight_vartime(t);
}
// The pinned mainnet transactions: published txids, wtxids and weights.
#forall "mainnet transactions by txid" i: 0..<6 #steps 200000000 {
  m := TX_MAINNET[i];
  b: [1024] u8; d: [32] u8; ins: [4] TxIn; outs: [4] TxOut; items: [8] TxItem;
  s := decode(m.tx, b[..]);
  t := tx_decode_vartime(s, ins[..], outs[..], items[..]);
  #assert t.code == TX_OK && t.segwit == m.segwit && tx_weight_vartime(t) == m.weight && t.total == m.size;
  #assert tx_txid_vartime(s, t, d[..]) && id_is(d[..], m.txid) && tx_wtxid_vartime(s, t, d[..]) && id_is(d[..], m.wtxid);
}
// Changing a witness changes the wtxid and leaves the txid.
#test "a witness byte moves the wtxid only" #steps 100000000 {
  b: [1024] u8; d: [32] u8; e: [32] u8; ins: [4] TxIn; outs: [4] TxOut; items: [8] TxItem;
  s := decode(TX_MAINNET[3].tx, b[..]);
  t := tx_read_vartime(s, true, ins[..], outs[..], items[..]);
  s[items[0].at + 10] ^= 1;
  #assert tx_txid_vartime(s, t, d[..]) && id_is(d[..], TX_MAINNET[3].txid);
  #assert tx_wtxid_vartime(s, t, e[..]) && !id_is(e[..], TX_MAINNET[3].wtxid);
  s[outs[0].value] ^= 1;
  #assert tx_txid_vartime(s, t, d[..]) && !id_is(d[..], TX_MAINNET[3].txid);
}
// The refusals the example prints, each with its code and byte.
#test "the example's refusals" {
  b: [256] u8; ins: [4] TxIn; outs: [4] TxOut; items: [4] TxItem;
  s := decode(TX_LEGACY_OK, b[..]);
  cuts := [5] int.[3, 20, 45, 52, 60]; at := [5] int.[0, 5, 43, 48, 58];
  for k: 0..<5 {
    r := tx_read_vartime(s[0..<cuts[k]], true, ins[..], outs[..], items[..]);
    #assert r.code == TX_TRUNCATED && r.at == at[k];
  }
  r := tx_read_vartime(decode("010000000002000000000000000000", b[..]), true, ins[..], outs[..], items[..]);
  #assert r.code == TX_UNKNOWN_FLAG && r.at == 5 && CODES[r.code] == "unknown flag";
  c := tx_compact_read_vartime(decode("fe01000002", b[..]), 0);
  #assert c.code == TX_SIZE_TOO_LARGE && CODES[c.code] == "size too large";
}
// Each ambiguous case decodes as the example prints.
#test "zero inputs: what Core keeps" {
  b: [256] u8; ins: [4] TxIn; outs: [4] TxOut; items: [4] TxItem;
  t := tx_decode_vartime(decode(TX_BOTH_SANE, b[..]), ins[..], outs[..], items[..]);
  #assert t.code == TX_OK && t.segwit && t.nin == 1;
  t = tx_decode_vartime(decode(TX_LEGACY_SANE, b[..]), ins[..], outs[..], items[..]);
  #assert t.code == TX_OK && !t.segwit && t.nin == 0 && t.nout == 1;
  t = tx_decode_vartime(decode(TX_NEITHER_SANE, b[..]), ins[..], outs[..], items[..]);
  #assert t.code == TX_OK && t.segwit;
  t = tx_decode_vartime(decode(TX_ZERO_INPUTS, b[..]), ins[..], outs[..], items[..]);
  #assert t.code == TX_OK && !t.segwit && t.nout == 1;
  t = tx_decode_vartime(decode(TX_SEGWIT_NO_INPUTS, b[..]), ins[..], outs[..], items[..]);
  #assert t.code == TX_NO_WITNESS;
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/transactions.coax
coax/bin/coax mutate bitcoin/examples/checks/transactions.coax --root-only
```

`coax test` runs 74 checks, including the loaded files' own checks, in about 10 seconds on 2026-11-02. The checks file has no procedures of its own, so root-only mutation finds no site in it; the example's helpers are checked by the output comparison.

### The library's own checks

`tx.coax` is an `#authority` file. Its own checks cover:

- CompactSize at each width's edges, each non-canonical and oversized
  form, the 9-byte forms, short reads, and the writer at each width;
- the legacy transaction field by field, every position the reader
  returns, against positions computed by `tx_reference.py`;
- the P2WPKH spend's witness items and sizes, and stacks with empty items
  and empty stacks beside full ones;
- each refusal with its byte, including truncation at every field, a
  count larger than the bytes, and capacity at each array;
- the serializer, with an output buffer one byte short;
- hashing in pieces: the same digest for every split, and the piece size
  equal to SHA-256's update limit;
- the six cases that read two ways, and `HasValidOps` on each push form,
  each limit and each opcode edge;
- the coinbase rule and the 10,000-byte script limit.

```shell
coax/bin/coax test coax/lib/bitcoin/tx.coax
coax/bin/coax mutate coax/lib/bitcoin/tx.coax --root-only
```

`coax test` runs 66 checks, including those of the hash files it loads, in about 2 seconds on 2026-11-02. Root-only mutation on 2026-11-02 makes 525 mutants: it kills 524, and one is marked equivalent in the source with its reason (a piece size of `remaining` or `limit` when the two are equal). None survives.

### Core in Python, natively and in Wasm

[tx_reference.py](../../coax/tests/bitcoin/tx_reference.py) is a
transliteration of the pinned Core sources: `ReadCompactSize`,
`UnserializeTransaction`, `SerializeTransaction`, `DecodeTx`,
`CheckTxScriptsSanity` and `HasValidOps`. It shares no code with Coax.
[tx.py](../../coax/tests/bitcoin/tx.py) compiles `tx.coax` to C in checked
and release builds, through a flat test entry point
([tx_report.coax](../../coax/tests/bitcoin/tx_report.coax)), and reads each
case with witnesses, without them and as `DecodeTx`. Its cases are:

- Core's 215 transactions, the six mainnet ones and the ambiguous ones;
- 400 seeded random transactions, with and without witness stacks, some
  stacks and items empty;
- four seeded edits of each: a bit flipped, a byte inserted or removed,
  the bytes cut, a CompactSize rewritten in a longer form, the flag
  changed, a byte appended, a count set to `MAX_SIZE`, one more, or larger;
- each readable transaction again with room for one input, output or
  witness item fewer than it has, and with exactly enough;
- a transaction of about 1.65 MB whose stripped form is over 1 MiB, so
  both IDs are hashed in several pieces.

For each case the report holds the code and the byte, and for a
transaction that reads, its counts, both sizes, its weight and vsize, its
version and locktime, both IDs and both serializations. The report must
equal the one built from the Python reader, and the output buffer past it
must be untouched. Then the harness builds the same C for wasm32 and runs
every case in Node.

```shell
python3 coax/tests/bitcoin/tx.py
```

On 2026-11-02, 6,976 cases agree with Python in native checked and release builds and in Wasm checked and release builds, and every result code appears among them. The run takes about 15 seconds.

The vectors come from
[make_tx_vectors.py](../../coax/tests/bitcoin/make_tx_vectors.py), which
checks each pinned file's SHA-256, checks that each mainnet transaction's
bytes hash to its txid and that its weight is mempool.space's, and checks
the P2WPKH spend against rust-bitcoin's published txid, wtxid, weight 442
and vsize 111.

## What this chapter leaves out

- Malleability in depth, the ways a txid could be changed before segwit
  and which rules closed each one: chapter 35.
- What scripts mean, and what the witness items are: chapter 36.
- Fees, and the weight and vsize Core's `decoderawtransaction` reports:
  chapter 40 builds on the formulas here.
- Signature hashes: chapters 37 and 43. `sighash.coax` already computes
  the BIP-143 and BIP-341 key-path ones for Nostr chapter 104.
- Validity: a transaction with no inputs or outputs, a value above 21
  million bitcoin, a duplicate input, or a coinbase scriptSig of the wrong
  length all read here. Core's `CheckTransaction` refuses them; that is
  chapter 52. Core's `tx_invalid.json` transactions are invalid for their
  scripts or those rules, not their bytes, so all of them read.
- The prevouts and flags in Core's test files; chapters 36 to 40 run them.
- Blocks, which are a header and a CompactSize count of transactions:
  chapter 47.
- PSBTs, which wrap a transaction with key-value maps: chapter 94.

## Summary

- A transaction is a version, inputs, outputs and a locktime. BIP-144's
  extended form adds a marker `00`, a flag `01` and a witness stack per
  input.
- Every count and length is a CompactSize in its shortest form, at most
  `MAX_SIZE`; any other encoding refuses.
- An input count of zero is the marker only when the reader allows
  witnesses. Bytes can read both ways, and Core's `DecodeTx` keeps the
  reading whose scripts parse, the extended one first.
- The txid hashes the stripped form and the wtxid the full one; both are
  shown byte-reversed.
- Weight is stripped size × 3 + total size, and vsize is weight / 4,
  rounded up.
- The reader has seven refusals, each with its own code and byte, and a
  transaction read and written back is the same bytes.

## Dependencies

- [1. Bytes, hex and Base64](ch001-bytes-hex-and-base64.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)
- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)

## Sources

- [BIP-144](https://github.com/bitcoin/bips/blob/master/bip-0144.mediawiki)
  (the witness serialization) and
  [BIP-141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
  (the wtxid and weight).
- Bitcoin Core at revision `ddea9196`: `tx_valid.json`, `tx_invalid.json`,
  `primitives/transaction.h`, `serialize.h`, `core_io.cpp`,
  `script/script.cpp`, `script/script.h` and `consensus/consensus.h`,
  pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/transaction.h).
- rust-bitcoin 0.32.5's `transaction.rs`, pinned in
  [coax/tests/vectors/upstream/rust-bitcoin/](../../coax/tests/vectors/upstream/rust-bitcoin/transaction.rs).
- Three mainnet transactions from mempool.space, pinned in
  [coax/tests/vectors/upstream/mainnet/](../../coax/tests/vectors/upstream/mainnet/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206.hex).
- All are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
