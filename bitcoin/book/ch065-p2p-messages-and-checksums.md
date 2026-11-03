# 65. P2P messages and checksums

Nodes talk to each other in messages. On the original transport, which
BIP-324 calls v1, each message is a 24-byte header and a payload: four
bytes that name the network, a command such as `version` or `inv`, the
payload's length, and a 4-byte checksum, the first four bytes of the
payload's SHA-256d (chapter 5). This chapter reads and writes that header
with every refusal Bitcoin Core makes, and the payloads of the messages a
node sends first: `version` and `verack`, `ping` and `pong`, `inv` and
`getdata`, `getheaders` and `headers` with their block locator, `addr` and
`addrv2`, `feefilter`, `sendcmpct`, `wtxidrelay` and `sendaddrv2`.

The code only encodes and decodes. It opens no socket and keeps no peer
state. Chapter 67 encrypts the same messages, and chapter 68 uses them to
connect.

## What this chapter teaches

- The 24-byte header: the message start ("magic") of each network, the
  12-byte command and its padding rule, the length and its limit of
  4,000,000 bytes, and the checksum.
- The order of Core's header checks, and why a reader returns "not yet"
  instead of "no" while bytes are still arriving.
- The `version` message as Core reads it, with its four optional groups.
- Inventory entries and their types, including the witness flag.
- The block locator: which heights `getheaders` names and why.
- `addr` and BIP-155's `addrv2`, with its network IDs.
- Where each count limit sits: before the entries are read, or after.

## Where Bitcoin uses P2P messages

| Where | What it uses | Chapter |
| --- | --- | --- |
| The v2 transport | the same payloads and commands, inside encrypted packets | 67 |
| The handshake | `version`, `verack` and the feature messages | 68 |
| The address manager | `addr` and `addrv2` | 69 |
| Denial-of-service limits | the size and count limits | 71 |
| Headers sync and block download | `getheaders`, `headers`, `inv`, `getdata` | 72 |
| Fuzzing | every reader here | 119 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [p2p.coax](../../coax/lib/bitcoin/p2p.coax) | `p2p_frame_read_vartime(b, magic)` | the header's checks, in Core's order |
| | `p2p_frame_write_vartime(magic, command, payload, out)` | a whole message |
| | `p2p_command_valid_vartime`, `p2p_command_is` | the command's padding rule |
| | `p2p_version_read_vartime`, `p2p_version_write_vartime` | `version` |
| | `p2p_u64_read_vartime`, `p2p_feefilter_read_vartime`, `p2p_sendcmpct_read_vartime` | `ping`, `pong`, `feefilter`, `sendcmpct` |
| | `p2p_inv_read_vartime`, `p2p_inv_write_vartime`, `p2p_inv_type_name` | `inv`, `getdata`, `notfound` |
| | `p2p_locator_read_vartime`, `p2p_locator_write_vartime`, `p2p_locator_heights_vartime` | `getblocks`, `getheaders` |
| | `p2p_headers_read_vartime` | `headers` |
| | `p2p_addr_read_vartime`, `p2p_addr_write_vartime` | `addr` and `addrv2` |
| [hash.coax](../../coax/lib/bitcoin/hash.coax) | `bitcoin_checksum_checked`, `bitcoin_checksum_matches` | the checksum (chapter 5) |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_compact_read_vartime`, `tx_compact_write_vartime` | CompactSize (chapter 34) |
| [pow.coax](../../coax/lib/bitcoin/pow.coax) | `pow_header_read_vartime`, `pow_header_check_vartime` | the headers in `headers` (chapter 47) |

Messages are public, so every procedure runs in variable time and its
name ends in `_vartime` (chapter 3). Readers copy nothing: like chapter
34's transactions, a read message is positions in the caller's bytes.

Each refusal has a code. The comments quote the message Core logs:

<!-- source: coax/lib/bitcoin/p2p.coax codes -->
```jai
// Refusal codes. The quoted words are Core's log messages.
P2P_OK :: 0;
P2P_TRUNCATED :: 1;     // fewer bytes than the header or its payload: a reader waits for more
P2P_BAD_MAGIC :: 2;     // "Header error: Wrong MessageStart"
P2P_OVERSIZE :: 3;      // "Header error: Size too large"
P2P_BAD_CHECKSUM :: 4;  // "Header error: Wrong checksum"
P2P_BAD_COMMAND :: 5;   // "Header error: Invalid message type"
P2P_BAD_PAYLOAD :: 6;   // the payload ends early or holds a value its message refuses
P2P_TOO_MANY :: 7;      // a count above the message's limit: Core disconnects or penalizes
P2P_CAPACITY :: 8;      // more entries than the caller's array holds, or no room to write

P2P_HEADER :: 24;             // magic 4, command 12, length 4, checksum 4
P2P_COMMAND :: 12;            // protocol.h MESSAGE_TYPE_SIZE
P2P_MAX_MESSAGE :: 4000000;   // net.h MAX_PROTOCOL_MESSAGE_LENGTH
P2P_MAX_INV :: 50000;         // net_processing.cpp MAX_INV_SZ
P2P_MAX_LOCATOR :: 101;       // net_processing.cpp MAX_LOCATOR_SZ
P2P_MAX_HEADERS :: 2000;      // net_processing.h MAX_HEADERS_RESULTS
P2P_MAX_ADDR :: 1000;         // net_processing.cpp MAX_ADDR_TO_SEND
P2P_MAX_AGENT :: 256;         // net.h MAX_SUBVERSION_LENGTH
P2P_MAX_ADDRV2 :: 512;        // netaddress.h MAX_ADDRV2_SIZE
```

## The message start

The first four bytes say which network a message belongs to. They are
chosen to be rare in ordinary data, so a reader that loses its place can
find the next message. Core keeps them in `kernel/chainparams.cpp`, next to
each network's default port:

<!-- source: coax/lib/bitcoin/p2p.coax networks -->
```jai
// Each network's message start ("magic") and default port, from Bitcoin
// Core's kernel/chainparams.cpp. Signet's is the default signet's, which
// Core derives from its challenge (chapter 50); messages.py pins it.
P2pNetwork :: struct { name: string; magic: [4] u8; port: int; }
P2P_MAINNET :: 0; P2P_TESTNET3 :: 1; P2P_TESTNET4 :: 2; P2P_SIGNET :: 3; P2P_REGTEST :: 4;
P2P_NETWORKS :: [5] P2pNetwork.[
  P2pNetwork.{name="mainnet", magic=[4] u8.[0xf9, 0xbe, 0xb4, 0xd9], port=8333},
  P2pNetwork.{name="testnet3", magic=[4] u8.[0x0b, 0x11, 0x09, 0x07], port=18333},
  P2pNetwork.{name="testnet4", magic=[4] u8.[0x1c, 0x16, 0x3f, 0x28], port=48333},
  P2pNetwork.{name="signet", magic=[4] u8.[0x0a, 0x03, 0xcf, 0x40], port=38333},
  P2pNetwork.{name="regtest", magic=[4] u8.[0xfa, 0xbf, 0xb5, 0xda], port=18444},
];
```

Signet's message start is not a constant in Core. It is the first four
bytes of SHA-256d of the network's challenge script, so a custom signet
gets its own. The value here is the default signet's, as Core's
`messages.py` pins it. Chapter 50 derives it.

## The header

A header is 24 bytes:

```text
offset  size  field
0       4     message start, f9beb4d9 on mainnet
4       12    command, ASCII, padded with zero bytes
16      4     payload length, little-endian
20      4     checksum: SHA-256d(payload), first 4 bytes
24      n     payload
```

The developer reference's `verack` header has no payload, so its checksum
is that of nothing, `5df6e0e2`:

```text
f9beb4d9 76657261636b000000000000 00000000 5df6e0e2
mainnet  v e r a c k              0 bytes  checksum
```

Core's `V1Transport` checks the header in two steps. As soon as the 24
bytes are there, `readHeader` checks the message start and the length.
Once the payload has arrived, `GetReceivedMessage` checks the checksum and
then the command. The reader follows the same order:

<!-- source: coax/lib/bitcoin/p2p.coax frame -->
```jai
// The header of the message at the start of b, then its payload. Checks
// follow V1Transport: the magic and the length as soon as the 24 header
// bytes are there, then the checksum and the command once the payload is.
// TRUNCATED means "not yet": a stream reader waits for more bytes.
P2pFrame :: struct { code: int; command_n: int; length: int; payload: int; next: int; }
p2p_frame_read_vartime :: (b: [] u8, magic: [4] u8) -> P2pFrame #export_c {

  if b.count < P2P_HEADER { return P2pFrame.{code=P2P_TRUNCATED}; }
  for i: 0..<4 { if b[i] != magic[i] { return P2pFrame.{code=P2P_BAD_MAGIC}; } }
  length := tx_le_vartime(b, 16, 4);
  if length > P2P_MAX_MESSAGE { return P2pFrame.{code=P2P_OVERSIZE}; }
  if b.count - P2P_HEADER < length { return P2pFrame.{code=P2P_TRUNCATED}; }
  payload := b[P2P_HEADER..<P2P_HEADER+length];
  if !bitcoin_checksum_matches(payload, b[20..<24]) { return P2pFrame.{code=P2P_BAD_CHECKSUM}; }
  if !p2p_command_valid_vartime(b[4..<16]) { return P2pFrame.{code=P2P_BAD_COMMAND}; }
  n := 0;
  while n < P2P_COMMAND && b[4+n] != 0 { n += 1; }
  return P2pFrame.{command_n=n, length=length, payload=P2P_HEADER, next=P2P_HEADER+length};
}
// CMessageHeader::IsMessageTypeValid: printable ASCII (0x20 to 0x7e), and
// after the first zero byte nothing but zeros.
p2p_command_valid_vartime :: (command: [] u8) -> bool #export_c {
  if command.count != P2P_COMMAND { return false; }
  ended := false;
  for c: command {
    if c == 0 { ended = true; }
    else if ended || c < 0x20 || c > 0x7e { return false; }
  }
  return true;
}
// True when the read frame's command is name.
p2p_command_is :: (b: [] u8, f: P2pFrame, name: string) -> bool {
  if f.code != P2P_OK || f.command_n != name.count { return false; }
  for i: 0..<name.count { if b[4+i] != name[i] { return false; } }
  return true;
}
// A whole message into out: magic, command padded with zeros, length,
// checksum, payload. Returns its size, or -1 for a command Core would
// refuse, a payload above the limit or an out too small.
p2p_frame_write_vartime :: (magic: [4] u8, command: string, payload: [] u8, out: [] u8) -> int #export_c {
  if command.count > P2P_COMMAND || payload.count > P2P_MAX_MESSAGE { return -1; }
  if out.count < P2P_HEADER + payload.count { return -1; }
  padded: [12] u8;
  for i: 0..<command.count { padded[i] = command[i]; }
  if !p2p_command_valid_vartime(padded[..]) { return -1; }
  for i: 0..<4 { out[i] = magic[i]; }
  for i: 0..<12 { out[4+i] = padded[i]; }
  stored := cb_store_le32(out, 16, cast(u32) payload.count);
  #assert bitcoin_checksum_checked(payload, out[20..<24], false) == SHA_OK;
  for i: 0..<payload.count { out[P2P_HEADER+i] = payload[i]; }
  return P2P_HEADER + payload.count;
}
```

Three details matter:

- **The length is checked before the payload arrives.** When the header
  was checked only against `MAX_SIZE`, 32 MiB, a peer could make a node
  allocate that much per connection; the comment in `readHeader` links
  Core's 2024 disclosure. The limit is now
  `MAX_PROTOCOL_MESSAGE_LENGTH`, 4,000,000 bytes, checked in the header.
  A length of 4,000,000 is allowed and the reader waits for it; 4,000,001
  refuses at once.
- **`TRUNCATED` is not a refusal.** A stream delivers bytes in pieces. A
  reader that has 23 bytes, or a header and half a payload, must wait for
  more. Every other code is final.
- **The command's padding.** `IsMessageTypeValid` accepts printable ASCII
  (0x20 to 0x7e) up to the first zero byte, and then only zeros. So
  `ver\0sion` refuses, a control character refuses, and a command of
  twelve zero bytes is valid: it is the empty command, which Core then
  ignores as unknown. A space is printable, so `verack ` (with a trailing
  space) is valid too, and is a different command.

The checksum is not security. Anyone can compute it. It catches
corruption, and chapter 67's v2 transport drops it: its authenticated
encryption already does more.

## version

The first message on a connection is `version`. The developer reference's
example is 101 bytes:

```text
72110100                          protocol version 70002
0100000000000000                  services: NODE_NETWORK
bc8f5e5400000000                  time 1415483324
0100000000000000                  the receiver's services, as the sender sees them
00000000000000000000ffffc61b6409  the receiver's address: ::ffff:198.27.100.9
208d                              port 8333, big-endian
0100000000000000                  the sender's services
00000000000000000000ffffcb0071c0  the sender's address: ::ffff:203.0.113.192
208d                              port 8333
128035cbc97953f8                  nonce
0f 2f5361746f7368693a302e392e332f user agent: /Satoshi:0.9.3/
cf050500                          start height 329167
01                                relay transactions: yes
```

An address in this form is 16 bytes. An IPv4 address is "mapped": ten
zero bytes, two `ff` bytes, then the four bytes of the address. The port
is the only big-endian number in the protocol.

Core reads the first four fields and then four optional groups, each only
if bytes remain: the sender's address and nonce, the user agent, the start
height and the relay flag. A group that starts but is cut short is an
error. A missing start height defaults to -1 and a missing relay flag to
true. The user agent may be at most 256 bytes. Bytes after the relay flag
are left unread.

<!-- source: coax/lib/bitcoin/p2p.coax version -->
```jai
// One address as positions in the bytes: its time (addr and addrv2 only),
// services, BIP-155 network, the address bytes at at..at+n, and the port,
// which is big-endian. In the 16-byte v1 form an IPv4 address is mapped
// (::ffff:a.b.c.d), so at points past the 12-byte prefix and n is 4.
// known is false for an addrv2 network Core skips.
P2pAddress :: struct { time: u32; services: u64; net: int; at: int; n: int; port: int; known: bool; }
P2P_V4_PREFIX :: [12] u8.[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff];
// The 26-byte v1 form without time: services, 16 address bytes, port.
p2p_addr_v1_read_vartime :: (b: [] u8, at: int) -> P2pAddress {
  a := P2pAddress.{services=p2p_u64(b, at), net=P2P_NET_IPV4, at=at+20, n=4, known=true};
  for i: 0..<12 { if b[at+8+i] != P2P_V4_PREFIX[i] { a.net = P2P_NET_IPV6; a.at = at + 8; a.n = 16; } }
  a.port = (cast(int) b[at+24] << 8) | cast(int) b[at+25];
  return a;
}
p2p_addr_v1_write_vartime :: (src: [] u8, a: P2pAddress, out: [] u8, at: int) -> int {
  w := p2p_put_u64(out, at, a.services);
  prefix := P2P_V4_PREFIX;
  if a.n == 4 { w = p2p_put(out, w, prefix[..]); }
  w = p2p_put(out, w, src[a.at..<a.at+a.n]);
  out[w] = cast,trunc(u8) (a.port >> 8); out[w+1] = cast,trunc(u8) a.port;
  return w + 2;
}
// The version message as Core's ProcessMessage reads it: version,
// services, time and the receiver's address are required; then four
// optional groups, each read only if bytes remain: the sender's address
// and the nonce, the user agent (at most 256 bytes), the start height,
// and the relay flag. parts counts the groups present. A group cut short
// refuses. Bytes after the relay flag are left unread, as Core leaves them.
P2pVersion :: struct {
  code: int; version: int; services: u64; time: int; recv: P2pAddress;
  parts: int; from: P2pAddress; nonce: u64; agent: int; agent_n: int;
  height: int = -1; relay: bool = true; next: int;
}
p2p_version_read_vartime :: (b: [] u8) -> P2pVersion #export_c {
  bad := P2pVersion.{code=P2P_BAD_PAYLOAD};
  if b.count < 46 { return bad; }
  v := P2pVersion.{version=p2p_signed32(b, 0), services=p2p_u64(b, 4), time=tx_le_vartime(b, 12, 8)};
  v.recv = p2p_addr_v1_read_vartime(b, 20);
  at := 46;
  if at < b.count {
    if b.count - at < 34 { return bad; }
    v.from = p2p_addr_v1_read_vartime(b, at); v.nonce = p2p_u64(b, at + 26);
    at += 34; v.parts = 1;
  }
  if at < b.count {
    s := tx_span_vartime(b, at);
    if s.code != TX_OK || s.n > P2P_MAX_AGENT { return bad; }
    v.agent = s.at; v.agent_n = s.n; at = s.next; v.parts = 2;
  }
  if at < b.count {
    if b.count - at < 4 { return bad; }
    v.height = p2p_signed32(b, at); at += 4; v.parts = 3;
  }
  if at < b.count { v.relay = b[at] != 0; at += 1; v.parts = 4; }
  v.next = at;
  return v;
}
// The message back, with v.parts optional groups; the address and user
// agent bytes come from src. Returns the size, or -1 if out is too small.
p2p_version_write_vartime :: (src: [] u8, v: P2pVersion, out: [] u8) -> int #export_c {
  need := 46;
  if v.parts >= 1 { need += 34; }
  if v.parts >= 2 { need += tx_compact_size_vartime(v.agent_n) + v.agent_n; }
  if v.parts >= 3 { need += 4; }
  if v.parts >= 4 { need += 1; }
  if out.count < need { return -1; }
  at := p2p_put_u32(out, 0, cast,trunc(u32) v.version);
  at = p2p_put_u64(out, at, v.services);
  at = p2p_put_u64(out, at, cast,trunc(u64) v.time);
  at = p2p_addr_v1_write_vartime(src, v.recv, out, at);
  if v.parts >= 1 { at = p2p_addr_v1_write_vartime(src, v.from, out, at); at = p2p_put_u64(out, at, v.nonce); }
  if v.parts >= 2 { at = tx_compact_write_vartime(out, at, v.agent_n); at = p2p_put(out, at, src[v.agent..<v.agent+v.agent_n]); }
  if v.parts >= 3 { at = p2p_put_u32(out, at, cast,trunc(u32) v.height); }
  if v.parts >= 4 { out[at] = ifx v.relay then 1 else 0; at += 1; }
  return at;
}
```

The nonce lets a node detect a connection to itself: if a `version`
arrives with a nonce it sent, it disconnects. A node answers `version`
with `verack`, which has no payload. Chapter 68 runs that exchange and
negotiates the features.

## ping, pong and the small messages

`ping` carries an 8-byte nonce and `pong` echoes it, which measures
latency and keeps a connection alive. `feefilter` (BIP-133) asks a peer
not to announce transactions below a feerate, in satoshis per 1,000
virtual bytes. Core reads it as a signed amount and ignores a rate outside
0 to 21 million bitcoin. `sendcmpct` (BIP-152) carries a flag and a
version; chapter 74 acts on it. `wtxidrelay` (BIP-339), `sendaddrv2`
(BIP-155), `verack`, `sendheaders` and `getaddr` have empty payloads.

<!-- source: coax/lib/bitcoin/p2p.coax small -->
```jai
// ping, pong and the 8-byte nonces they carry; feefilter's rate in
// sat/kvB, which Core applies only inside MoneyRange.
P2pNumber :: struct { code: int; value: u64; }
p2p_u64_read_vartime :: (b: [] u8) -> P2pNumber #export_c {
  if b.count < 8 { return P2pNumber.{code=P2P_BAD_PAYLOAD}; }
  return P2pNumber.{value=p2p_u64(b, 0)};
}
p2p_u64_write_vartime :: (v: u64, out: [] u8) -> int #export_c {
  if out.count < 8 { return -1; }
  return p2p_put_u64(out, 0, v);
}
P2pFeeFilter :: struct { code: int; rate: int; in_range: bool; }
p2p_feefilter_read_vartime :: (b: [] u8) -> P2pFeeFilter #export_c {
  if b.count < 8 { return P2pFeeFilter.{code=P2P_BAD_PAYLOAD}; }
  rate := tx_le_vartime(b, 0, 8);        // a signed CAmount
  return P2pFeeFilter.{rate=rate, in_range=rate >= 0 && rate <= 2100000000000000};
}
// sendcmpct (BIP-152): announce with cmpctblock or not, and a version.
// Read only; chapter 74 acts on it.
P2pSendCmpct :: struct { code: int; announce: bool; version: u64; }
p2p_sendcmpct_read_vartime :: (b: [] u8) -> P2pSendCmpct #export_c {
  if b.count < 9 { return P2pSendCmpct.{code=P2P_BAD_PAYLOAD}; }
  return P2pSendCmpct.{announce=b[0] != 0, version=p2p_u64(b, 1)};
}
```

## inv and getdata

A node announces transactions and blocks with `inv` and asks for them
with `getdata`. Both carry a list of entries, each a 4-byte type and a
32-byte hash:

| Type | Value | Name in Core's logs |
| --- | --- | --- |
| `MSG_TX` | 1 | `tx` |
| `MSG_BLOCK` | 2 | `block` |
| `MSG_FILTERED_BLOCK` | 3 | `merkleblock` |
| `MSG_CMPCT_BLOCK` | 4 | `cmpctblock` |
| `MSG_WTX` | 5 | `wtx` |
| any of these with bit 30 set | `MSG_WITNESS_FLAG` | `witness-` and the name |

A `getdata` for `MSG_WITNESS_TX` (0x40000001) asks for the transaction
with its witness (chapter 35). `MSG_WTX` (BIP-339) names a transaction by
its wtxid instead of its txid.

<!-- source: coax/lib/bitcoin/p2p.coax inv -->
```jai
// inv, getdata and notfound: a count of at most 50,000, then 36-byte
// entries of a 4-byte type and a 32-byte hash (hash is its position).
// Core reads all the entries before it checks the count, so a count
// longer than the bytes is a failed read, not too many.
P2pInv :: struct { type: u32; hash: int; }
P2pList :: struct { code: int; count: int; next: int; }
p2p_inv_read_vartime :: (b: [] u8, out: [] P2pInv) -> P2pList #export_c {
  n := tx_compact_read_vartime(b, 0);
  if n.code != TX_OK { return P2pList.{code=P2P_BAD_PAYLOAD}; }
  if b.count - n.next < 36 * n.value { return P2pList.{code=P2P_BAD_PAYLOAD}; }
  if n.value > P2P_MAX_INV { return P2pList.{code=P2P_TOO_MANY}; }
  if n.value > out.count { return P2pList.{code=P2P_CAPACITY}; }
  for k: 0..<n.value {
    at := n.next + 36 * k;
    out[k] = P2pInv.{type=cast(u32) tx_le_vartime(b, at, 4), hash=at+4};
  }
  return P2pList.{count=n.value, next=n.next+36*n.value};
}
p2p_inv_write_vartime :: (src: [] u8, entries: [] P2pInv, out: [] u8) -> int #export_c {
  if entries.count > P2P_MAX_INV || out.count < tx_compact_size_vartime(entries.count) + 36 * entries.count { return -1; }
  at := tx_compact_write_vartime(out, 0, entries.count);
  for e: entries { at = p2p_put_u32(out, at, e.type); at = p2p_put(out, at, src[e.hash..<e.hash+32]); }
  return at;
}
// CInv::GetMessageType's names; "" for a type Core doesn't know.
p2p_inv_type_name :: (type: u32) -> string {
  witness := (type & P2P_MSG_WITNESS_FLAG) != 0;
  names := [6] string.["", "tx", "block", "merkleblock", "cmpctblock", "wtx"];
  wnames := [6] string.["", "witness-tx", "witness-block", "witness-merkleblock", "witness-cmpctblock", "witness-wtx"];
  base := type & 0x3fffffff;
  if base == 0 || base > 5 { return ""; }
  return ifx witness then wnames[base] else names[base];
}
```

Core deserializes the whole list and only then checks that it has at most
50,000 entries. So a count of 50,001 whose entries are all present is
"too many", which Core punishes as misbehaviour, but a count longer than
the bytes is a failed read, which Core only logs. The reader keeps that
order.

## The block locator

`getheaders` and `getblocks` ask for the blocks after the last one both
nodes share. The asking node does not know where its chain and the
peer's part, so it sends a block locator: hashes of its own blocks, dense
near its tip and sparse further back. The peer finds the first hash it
knows and replies from there.

<!-- source: coax/lib/bitcoin/p2p.coax locator -->
```jai
// getblocks and getheaders: a version Core ignores, a block locator of at
// most 101 hashes, and the hash to stop at. first and stop are positions.
P2pLocator :: struct { code: int; version: int; count: int; first: int; stop: int; next: int; }
p2p_locator_read_vartime :: (b: [] u8) -> P2pLocator #export_c {
  bad := P2pLocator.{code=P2P_BAD_PAYLOAD};
  if b.count < 4 { return bad; }
  n := tx_compact_read_vartime(b, 4);
  if n.code != TX_OK { return bad; }
  if b.count - n.next < 32 * n.value + 32 {
    // Core reads the whole vector before checking its size, so a count
    // too long for the bytes fails as a read first.
    return bad;
  }
  if n.value > P2P_MAX_LOCATOR { return P2pLocator.{code=P2P_TOO_MANY}; }
  stop := n.next + 32 * n.value;
  return P2pLocator.{version=tx_le_vartime(b, 0, 4), count=n.value, first=n.next, stop=stop, next=stop+32};
}
p2p_locator_write_vartime :: (version: int, hashes: [] u8, stop: [] u8, out: [] u8) -> int #export_c {
  n := hashes.count / 32;
  if hashes.count != 32 * n || n > P2P_MAX_LOCATOR || stop.count != 32 { return -1; }
  if out.count < 4 + tx_compact_size_vartime(n) + hashes.count + 32 { return -1; }
  at := p2p_put_u32(out, 0, cast,trunc(u32) version);
  at = tx_compact_write_vartime(out, at, n);
  return p2p_put(out, p2p_put(out, at, hashes), stop);
}
// The heights of the blocks a locator names for a chain whose tip is at
// height tip, as Core's LocatorEntries (chain.cpp) walks back: the newest
// ten one by one, then steps that double, and always genesis. Returns how
// many, or -1 if out is too small or tip is negative.
p2p_locator_heights_vartime :: (tip: int, out: [] int) -> int #export_c {
  if tip < 0 { return -1; }
  height := tip; step := 1; n := 0;
  while true {
    if n == out.count { return -1; }
    out[n] = height; n += 1;
    if height == 0 { break; }
    height = ifx height - step > 0 then height - step else 0;
    if n > 10 { step *= 2; }
  }
  return n;
}
```

Core's `LocatorEntries` takes the last ten blocks one by one, then
doubles the step each time, and always ends at genesis. So the locator
grows with the logarithm of the height: from height 329,167 it names 30
blocks, and from 900,000 only 31. Core refuses a locator of more than 101
hashes, again after reading it.

### Worked example: the locator from height 329,167

The step is 1 until the locator holds eleven hashes, then it doubles
after each one:

```text
329167 329166 ... 329156   twelve heights, one apart
329154 329150 329142       steps 2, 4, 8
329126 329094 329030       steps 16, 32, 64
...
198086 67014               steps 65536, 131072
0                          a step of 262144 passes height 0, so genesis
```

## headers

`headers` answers `getheaders` with up to 2,000 block headers. Each
80-byte header (chapter 47) is followed by a transaction count, which is
always zero; Core reads it and ignores it. Here the count is checked
before the headers are read, as Core does:

<!-- source: coax/lib/bitcoin/p2p.coax headers -->
```jai
// headers: a count of at most 2,000, then each 80-byte header followed by
// a transaction count, which Core reads and ignores. out gets each
// header's position.
p2p_headers_read_vartime :: (b: [] u8, out: [] int) -> P2pList #export_c {
  n := tx_compact_read_vartime(b, 0);
  if n.code != TX_OK { return P2pList.{code=P2P_BAD_PAYLOAD}; }
  if n.value > P2P_MAX_HEADERS { return P2pList.{code=P2P_TOO_MANY}; }
  if n.value > out.count { return P2pList.{code=P2P_CAPACITY}; }
  at := n.next;
  for k: 0..<n.value {
    if b.count - at < POW_HEADER { return P2pList.{code=P2P_BAD_PAYLOAD}; }
    out[k] = at;
    c := tx_compact_read_vartime(b, at + POW_HEADER);
    if c.code != TX_OK { return P2pList.{code=P2P_BAD_PAYLOAD}; }
    at = c.next;
  }
  return P2pList.{count=n.value, next=at};
}
```

The developer reference's `headers` example holds a real mainnet header.
Its hash meets its target, so chapter 47's check accepts it.

## addr and addrv2

`addr` gossips addresses of other nodes: a count of at most 1,000, then
for each a 4-byte time, the services, the 16-byte address and the port.
It cannot carry a Tor v3 address, which is 32 bytes. BIP-155's `addrv2`
replaces it. Each entry has a network ID and an address of that network's
length:

| ID | Network | Address bytes |
| --- | --- | --- |
| 1 | IPv4 | 4 |
| 2 | IPv6 | 16 |
| 3 | Tor v2 (no longer used) | 10 |
| 4 | Tor v3 | 32 |
| 5 | I2P | 32 |
| 6 | CJDNS | 16 |

In `addrv2` the services are a CompactSize with no `MAX_SIZE` limit, but
still in its shortest form. Core refuses an address longer than 512 bytes
and a known network with the wrong length. It skips an unknown ID,
including Tor v2's, so that a future network doesn't break old nodes. The
reader marks such an entry unknown.

<!-- source: coax/lib/bitcoin/p2p.coax addr -->
```jai
// addr (v1) and addrv2 (BIP-155): a count of at most 1,000 addresses.
// A v1 entry is time, services, 16 address bytes and port. A v2 entry is
// time, services as a CompactSize, a network ID, the address with its
// length (at most 512), and port. A known network with the wrong length
// refuses; an unknown ID, Tor v2's 3 included, is read and marked unknown.
// Like inv, the count is checked after the entries are read.
P2P_V2_LENGTHS :: [7] int.[0, 4, 16, 0, 32, 32, 16];
p2p_addr_read_vartime :: (b: [] u8, v2: bool, out: [] P2pAddress) -> P2pList #export_c {
  bad := P2pList.{code=P2P_BAD_PAYLOAD};
  n := tx_compact_read_vartime(b, 0);
  if n.code != TX_OK { return bad; }
  at := n.next;
  for k: 0..<n.value {
    if b.count - at < 4 { return bad; }
    a := P2pAddress.{time=cast(u32) tx_le_vartime(b, at, 4)}; at += 4;
    if !v2 {
      if b.count - at < 26 { return bad; }
      a = p2p_addr_v1_read_vartime(b, at); a.time = cast(u32) tx_le_vartime(b, at - 4, 4); at += 26;
    } else {
      s := p2p_compact64_read_vartime(b, at);
      if s.code != P2P_OK || s.next == b.count { return bad; }
      net := cast(int) b[s.next];
      size := tx_compact_read_vartime(b, s.next + 1);
      if size.code != TX_OK || size.value > P2P_MAX_ADDRV2 { return bad; }
      a.known = net >= 1 && net <= 6 && net != P2P_NET_TORV2;
      if a.known && size.value != P2P_V2_LENGTHS[net] { return bad; }
      if b.count - size.next < size.value + 2 { return bad; }
      at = size.next + size.value;
      a.services = s.value; a.net = net; a.at = size.next; a.n = size.value;
      a.port = (cast(int) b[at] << 8) | cast(int) b[at+1];
      at += 2;
    }
    if k < out.count { out[k] = a; }
  }
  // Core reads the whole vector, then checks its size.
  if n.value > P2P_MAX_ADDR { return P2pList.{code=P2P_TOO_MANY}; }
  if n.value > out.count { return P2pList.{code=P2P_CAPACITY}; }
  return P2pList.{count=n.value, next=at};
}
// The message back from entries read out of src. Returns its size or -1.
p2p_addr_write_vartime :: (src: [] u8, entries: [] P2pAddress, v2: bool, out: [] u8) -> int #export_c {
  if entries.count > P2P_MAX_ADDR { return -1; }
  need := tx_compact_size_vartime(entries.count);
  for a: entries { need += ifx v2 then 4 + 9 + 1 + tx_compact_size_vartime(a.n) + a.n + 2 else 30; }
  if out.count < need { return -1; }
  at := tx_compact_write_vartime(out, 0, entries.count);
  for a: entries {
    at = p2p_put_u32(out, at, a.time);
    if !v2 { at = p2p_addr_v1_write_vartime(src, a, out, at); continue; }
    at = p2p_compact64_write_vartime(out, at, a.services);
    out[at] = cast(u8) a.net; at = tx_compact_write_vartime(out, at + 1, a.n);
    at = p2p_put(out, at, src[a.at..<a.at+a.n]);
    out[at] = cast,trunc(u8) (a.port >> 8); out[at+1] = cast,trunc(u8) a.port; at += 2;
  }
  return at;
}
```

## The example

The example is [bitcoin/examples/p2p-messages.coax](../examples/p2p-messages.coax).
It reads the developer reference's samples, frames the `version` message
for mainnet and breaks it in each way the header refuses, walks the
locator from height 329,167, checks the `headers` example's proof of work,
writes a `getdata` with the witness flag, reads each `addrv2` network from
entries Core's `messages.py` wrote, and frames `pong` and the empty
feature messages.

<!-- source: bitcoin/examples/p2p-messages.coax example -->
```jai
#load "../../coax/lib/bitcoin/p2p.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex, at most 256 of them.
show :: (b: [] u8) {
  text: [512] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
// A hash as explorers show it: the bytes reversed.
show_hash :: (b: [] u8) { for i: 0..<32 { show(b[31-i..<32-i]); } }
show_ip :: (b: [] u8, a: P2pAddress) {
  if a.net == P2P_NET_IPV4 { print("%.%.%.%", b[a.at], b[a.at+1], b[a.at+2], b[a.at+3]); }
  else { show(b[a.at..<a.at+a.n]); }
}
CODES :: [9] string.["ok", "truncated", "bad magic", "oversize", "bad checksum", "bad command", "bad payload", "too many", "capacity"];
NETS :: [7] string.["?", "IPv4", "IPv6", "TorV2", "TorV3", "I2P", "CJDNS"];
sample :: (name: string, out: [] u8) -> [] u8 { return p2p_sample(name, out); }

main :: () {
  // ---- the header
  print("networks: message start and default port\n");
  for n: P2P_NETWORKS { m := n.magic; print("  % ", n.name); show(m[..]); print(" %\n", n.port); }
  b: [512] u8; f: [640] u8; o: [640] u8;
  h := sample("verack header", b[..]);
  r := p2p_frame_read_vartime(h, P2P_NETWORKS[P2P_MAINNET].magic);
  print("the published verack header: % (%)\n", CODES[r.code], cast(string) h[4..<4+r.command_n]);
  print("  magic "); show(h[0..<4]); print(", length %, checksum ", r.length); show(h[20..<24]);
  print(" = SHA-256d of nothing, first 4 bytes\n");

  // ---- version
  p := sample("version", b[..]);
  v := p2p_version_read_vartime(p);
  print("version, % bytes: %\n", p.count, CODES[v.code]);
  print("  version %, services %, time %\n", v.version, v.services, v.time);
  print("  receiver "); show_ip(p, v.recv); print(" port %, sender ", v.recv.port); show_ip(p, v.from); print(" port %\n", v.from.port);
  print("  nonce %, agent %, height %, relay %\n", v.nonce, cast(string) p[v.agent..<v.agent+v.agent_n], v.height, v.relay);
  n := p2p_frame_write_vartime(P2P_NETWORKS[P2P_MAINNET].magic, "version", p, f[..]);
  print("  framed for mainnet: % bytes, header ", n); show(f[0..<24]); print("\n");
  r = p2p_frame_read_vartime(f[0..<n], P2P_NETWORKS[P2P_MAINNET].magic);
  w := p2p_version_write_vartime(f[24..<n], p2p_version_read_vartime(f[24..<n]), o[..]);
  print("  read back: %, \"%\", % bytes; written again, the same bytes: %\n",
    CODES[r.code], cast(string) f[4..<4+r.command_n], r.length, p2p_same(o[0..<w], p));
  for k: 0..<5 {
    ends := [5] int.[46, 80, 96, 100, 101];
    print("  first % bytes: %, % optional groups\n", ends[k], CODES[p2p_version_read_vartime(p[0..<ends[k]]).code], p2p_version_read_vartime(p[0..<ends[k]]).parts);
  }
  print("  first 47 bytes: %\n", CODES[p2p_version_read_vartime(p[0..<47]).code]);

  // ---- refusals
  print("refusals of the framed version message\n");
  print("  read as testnet4: %\n", CODES[p2p_frame_read_vartime(f[0..<n], P2P_NETWORKS[P2P_TESTNET4].magic).code]);
  print("  first 23 bytes: %\n", CODES[p2p_frame_read_vartime(f[0..<23], P2P_NETWORKS[P2P_MAINNET].magic).code]);
  print("  first 124 bytes: %\n", CODES[p2p_frame_read_vartime(f[0..<124], P2P_NETWORKS[P2P_MAINNET].magic).code]);
  f[100] ^= 1;
  print("  one payload bit changed: %\n", CODES[p2p_frame_read_vartime(f[0..<n], P2P_NETWORKS[P2P_MAINNET].magic).code]);
  f[100] ^= 1; f[16] = 0x01; f[17] = 0x09; f[18] = 0x3d;
  print("  length 4000001: %\n", CODES[p2p_frame_read_vartime(f[0..<n], P2P_NETWORKS[P2P_MAINNET].magic).code]);
  f[16] = 0x00;
  print("  length 4000000: % (the payload hasn't arrived)\n", CODES[p2p_frame_read_vartime(f[0..<n], P2P_NETWORKS[P2P_MAINNET].magic).code]);
  f[16] = 101; f[17] = 0; f[18] = 0; f[7] = 0;
  print("  command \"ver\" then \"sion\" after a zero: %\n", CODES[p2p_frame_read_vartime(f[0..<n], P2P_NETWORKS[P2P_MAINNET].magic).code]);
  f[7] = 0x07;
  print("  a 0x07 byte in the command: %\n", CODES[p2p_frame_read_vartime(f[0..<n], P2P_NETWORKS[P2P_MAINNET].magic).code]);
  f[7] = 0x73;
  print("  restored: %\n", CODES[p2p_frame_read_vartime(f[0..<n], P2P_NETWORKS[P2P_MAINNET].magic).code]);

  // ---- getblocks and the locator
  p = sample("getblocks", b[..]);
  l := p2p_locator_read_vartime(p);
  print("getblocks: %, version %, % hashes, stop ", CODES[l.code], l.version, l.count); show_hash(p[l.stop..<l.stop+32]); print("\n");
  for i: 0..<l.count { print("  "); show_hash(p[l.first+32*i..<l.first+32*i+32]); print("\n"); }
  heights: [64] int;
  c := p2p_locator_heights_vartime(329167, heights[..]);
  print("a locator from height 329167 names % blocks:", c);
  for i: 0..<c { print(" %", heights[i]); }
  print("\n");

  // ---- headers
  p = sample("headers", b[..]);
  at: [4] int;
  hl := p2p_headers_read_vartime(p, at[..]);
  hd := pow_header_read_vartime(p[at[0]..<at[0]+80]);
  hash: [32] u8; hashed := pow_block_hash_vartime(p[at[0]..<at[0]+80], hash[..]);
  print("headers: %, % header, version %, time %, bits %, nonce %\n", CODES[hl.code], hl.count, hd.version, hd.time, hd.bits, hd.nonce);
  print("  hash "); show_hash(hash[..]); print("\n  meets its target (chapter 47): %\n", pow_header_check_vartime(p[at[0]..<at[0]+80], POW_LIMIT_MAINNET) == POW_OK);

  // ---- inv and getdata
  p = sample("inv", b[..]);
  e: [4] P2pInv;
  il := p2p_inv_read_vartime(p, e[..]);
  print("inv: %, % entries\n", CODES[il.code], il.count);
  for i: 0..<il.count { print("  % ", p2p_inv_type_name(e[i].type)); show_hash(p[e[i].hash..<e[i].hash+32]); print("\n"); }
  for i: 0..<il.count { e[i].type |= P2P_MSG_WITNESS_FLAG; }
  w = p2p_inv_write_vartime(p, e[0..<il.count], o[..]);
  print("getdata for both with the witness flag: % bytes, types % and %, names % and %\n", w, e[0].type, e[1].type, p2p_inv_type_name(e[0].type), p2p_inv_type_name(e[1].type));
  print("  first entry: "); show(o[1..<5]); print(" then the hash\n");

  // ---- addr and addrv2
  a: [4] P2pAddress;
  p = sample("addr", b[..]);
  print("addr excerpt: count %, % bytes: %\n", tx_compact_read_vartime(p, 0).value, p.count, CODES[p2p_addr_read_vartime(p, false, a[..]).code]);
  q := p[2..<p.count]; q[0] = 1;
  al := p2p_addr_read_vartime(q, false, a[..]);
  print("  with the count set to 1: %, time %, services %, ", CODES[al.code], a[0].time, a[0].services); show_ip(q, a[0]); print(" port %\n", a[0].port);
  p = sample("addrv2", b[..]);
  q = p[2..<p.count]; q[0] = 1;
  al = p2p_addr_read_vartime(q, true, a[..]);
  print("addrv2 excerpt with the count set to 1: %, services % (hex fd4804), % ", CODES[al.code], a[0].services, NETS[a[0].net]); show_ip(q, a[0]); print(" port %\n", a[0].port);
  print("addrv2 entries Bitcoin Core's messages.py writes\n");
  for s: P2P_ADDRV2 {
    x := p2p_unhex(s.hex, b[1..<96]); b[0] = 1;
    y := b[0..<x.count+1];
    al = p2p_addr_read_vartime(y, true, a[..]);
    w = p2p_addr_write_vartime(y, a[0..<1], true, o[..]);
    print("  %: % % bytes ", CODES[al.code], NETS[a[0].net], a[0].n); show(y[a[0].at..<a[0].at+a[0].n]);
    print(", round trip %\n", w == y.count && p2p_same(o[0..<w], y));
  }
  x := p2p_unhex(P2P_ADDRV2[0].hex, b[1..<96]); b[0] = 1;
  y := b[0..<x.count+1];
  y[7] = 16;
  print("  IPv4 with length 16: %\n", CODES[p2p_addr_read_vartime(y, true, a[..]).code]);
  y[7] = 4; y[6] = 3;
  al = p2p_addr_read_vartime(y, true, a[..]);
  print("  network 3 (Tor v2): %, known %\n", CODES[al.code], a[0].known);

  // ---- the small ones
  p = sample("feefilter", b[..]);
  ff := p2p_feefilter_read_vartime(p);
  print("feefilter: %, % sat/kvB, in MoneyRange %\n", CODES[ff.code], ff.rate, ff.in_range);
  p = sample("ping", b[..]);
  nonce := p2p_u64_read_vartime(p);
  print("ping: %, nonce %\n", CODES[nonce.code], nonce.value);
  pong: [8] u8; k := p2p_u64_write_vartime(nonce.value, pong[..]);
  n = p2p_frame_write_vartime(P2P_NETWORKS[P2P_MAINNET].magic, "pong", pong[0..<k], f[..]);
  print("  the pong that answers it: "); show(f[0..<n]); print("\n");
  cmpct := [9] u8.[0, 2, 0, 0, 0, 0, 0, 0, 0];
  sc := p2p_sendcmpct_read_vartime(cmpct[..]);
  print("sendcmpct 000200000000000000: %, announce %, version %\n", CODES[sc.code], sc.announce, sc.version);
  for name: [3] string.["wtxidrelay", "sendaddrv2", "verack"] {
    empty: [1] u8;
    n = p2p_frame_write_vartime(P2P_NETWORKS[P2P_MAINNET].magic, name, empty[0..<0], f[..]);
    print("% for mainnet: ", name); show(f[0..<n]); print("\n");
  }
  print("limits: message %, inv %, locator %, headers %, addr %, user agent %, addrv2 address %\n",
    P2P_MAX_MESSAGE, P2P_MAX_INV, P2P_MAX_LOCATOR, P2P_MAX_HEADERS, P2P_MAX_ADDR, P2P_MAX_AGENT, P2P_MAX_ADDRV2);
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/p2p-messages.coax
```

The output:

```text
networks: message start and default port
  mainnet f9beb4d9 8333
  testnet3 0b110907 18333
  testnet4 1c163f28 48333
  signet 0a03cf40 38333
  regtest fabfb5da 18444
the published verack header: ok (verack)
  magic f9beb4d9, length 0, checksum 5df6e0e2 = SHA-256d of nothing, first 4 bytes
version, 101 bytes: ok
  version 70002, services 1, time 1415483324
  receiver 198.27.100.9 port 8333, sender 203.0.113.192 port 8333
  nonce 17893779652077781010, agent /Satoshi:0.9.3/, height 329167, relay true
  framed for mainnet: 125 bytes, header f9beb4d976657273696f6e0000000000650000005f1a69d2
  read back: ok, "version", 101 bytes; written again, the same bytes: true
  first 46 bytes: ok, 0 optional groups
  first 80 bytes: ok, 1 optional groups
  first 96 bytes: ok, 2 optional groups
  first 100 bytes: ok, 3 optional groups
  first 101 bytes: ok, 4 optional groups
  first 47 bytes: bad payload
refusals of the framed version message
  read as testnet4: bad magic
  first 23 bytes: truncated
  first 124 bytes: truncated
  one payload bit changed: bad checksum
  length 4000001: oversize
  length 4000000: truncated (the payload hasn't arrived)
  command "ver" then "sion" after a zero: bad command
  a 0x07 byte in the command: bad command
  restored: ok
getblocks: ok, version 70001, 2 hashes, stop 0000000000000000000000000000000000000000000000000000000000000000
  00000000000000001bd3146aa1555e10b23b63e6d484987237b575778a609fd3
  00000000000000000aea3be27cda4b71011c2b60fb8a2e0a113708d403643e5c
a locator from height 329167 names 30 blocks: 329167 329166 329165 329164 329163 329162 329161 329160 329159 329158 329157 329156 329154 329150 329142 329126 329094 329030 328902 328646 328134 327110 325062 320966 312774 296390 263622 198086 67014 0
headers: ok, 1 header, version 2, time 1415239972, bits 404472624, nonce 1678286846
  hash 000000000000000009a11b3972c8e532fe964de937c9e0096b43814e67af3728
  meets its target (chapter 47): true
inv: ok, 2 entries
  tx 5af521b6a11d0829e43247224f03ca42c41f0b5d92a009c55d1fac09d7ff55de
  tx c78dc6b217985ee84ae972d8f1aa55a0b8246f767829261880e03770996dd391
getdata for both with the witness flag: 73 bytes, types 1073741825 and 1073741825, names witness-tx and witness-tx
  first entry: 01000040 then the hash
addr excerpt: count 1000, 33 bytes: bad payload
  with the count set to 1: ok, time 1414012889, services 1, 192.0.2.51 port 8333
addrv2 excerpt with the count set to 1: ok, services 1096 (hex fd4804), IPv4 192.0.2.51 port 8333
addrv2 entries Bitcoin Core's messages.py writes
  ok: IPv4 4 bytes 0141c362, round trip true
  ok: IPv6 16 bytes 200141f00000000000626974636f696e, round trip true
  ok: TorV3 32 bytes d06063b563a7982a1b8f42b2f8d8f34a5b2c106488a75123ca58ad923da91f3c, round trip true
  ok: I2P 32 bytes d77a5389fe026a59b70ef86d9d81bb9f01c0c4ee7b36103307d229d8eccd8ea3, round trip true
  ok: CJDNS 16 bytes fc3217eae415c3bf9808149db5a2c9aa, round trip true
  IPv4 with length 16: bad payload
  network 3 (Tor v2): ok, known false
feefilter: ok, 48508 sat/kvB, in MoneyRange true
ping: ok, nonce 5597941425041871872
  the pong that answers it: f9beb4d9706f6e6700000000000000000800000088ea81760094102111e2af4d
sendcmpct 000200000000000000: ok, announce false, version 2
wtxidrelay for mainnet: f9beb4d9777478696472656c61790000000000005df6e0e2
sendaddrv2 for mainnet: f9beb4d973656e646164647276320000000000005df6e0e2
verack for mainnet: f9beb4d976657261636b000000000000000000005df6e0e2
limits: message 4000000, inv 50000, locator 101, headers 2000, addr 1000, user agent 256, addrv2 address 512
```

What the output shows:

- The `version` message reads with all four optional groups, frames into
  125 bytes, and writes back to the same bytes. Each prefix that ends
  between groups reads; 47 bytes, one byte into a group, refuses.
- Each header refusal has its own code. A length of 4,000,000 waits for
  the payload; 4,000,001 refuses at once.
- The locator from height 329,167 names 30 blocks.
- The `headers` example's header hashes to
  `000000000000000009a11b39…` and meets its target.
- The `addr` excerpt claims 1,000 addresses but holds one, so it refuses;
  with its count set to 1 it reads.
- Every `addrv2` network round-trips. An IPv4 entry of 16 bytes refuses,
  and network 3 reads as unknown.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/p2p-messages.out](../fixtures/p2p-messages.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/p2p-messages.coax](../examples/checks/p2p-messages.coax),
which loads the example:

- every developer-reference payload reads and writes back to the same
  bytes;
- every one of the 192 single-bit changes to the `verack` header refuses,
  or leaves a different command that is still printable;
- a message framed for each of the five networks reads on that network
  and refuses with `BAD_MAGIC` on the other four.

<!-- source: bitcoin/examples/checks/p2p-messages.coax check -->
```jai
// Each published payload reads, and written back it is the same bytes.
#test "the developer reference's payloads round-trip" {
  b: [256] u8; o: [256] u8; e: [4] P2pInv; a: [2] P2pAddress; at: [2] int;
  p := sample("version", b[..]);
  v := p2p_version_read_vartime(p);
  #assert v.code == P2P_OK && v.next == p.count && p2p_version_write_vartime(p, v, o[..]) == p.count && p2p_same(o[0..<p.count], p);
  p = sample("getblocks", b[..]);
  l := p2p_locator_read_vartime(p);
  #assert l.next == p.count && p2p_locator_write_vartime(l.version, p[l.first..<l.stop], p[l.stop..<l.next], o[..]) == p.count && p2p_same(o[0..<p.count], p);
  p = sample("inv", b[..]);
  i := p2p_inv_read_vartime(p, e[..]);
  #assert i.next == p.count && p2p_inv_write_vartime(p, e[0..<i.count], o[..]) == p.count && p2p_same(o[0..<p.count], p);
  p = sample("headers", b[..]);
  #assert p2p_headers_read_vartime(p, at[..]).next == p.count;
  for name: [2] string.["addr", "addrv2"] {
    q := sample(name, b[..]); r := q[2..<q.count]; r[0] = 1;
    v2 := name == "addrv2";
    c := p2p_addr_read_vartime(r, v2, a[..]);
    #assert c.code == P2P_OK && c.next == r.count && p2p_addr_write_vartime(r, a[0..<1], v2, o[..]) == r.count && p2p_same(o[0..<r.count], r);
  }
  #assert p2p_u64_write_vartime(p2p_u64_read_vartime(sample("ping", b[..])).value, o[..]) == 8 && p2p_same(o[0..<8], sample("ping", b[..]));
}
// Every one of the 192 single-bit changes to the verack header is refused,
// or is a different command that is still printable.
#forall "one changed bit of the verack header" bit: 0..<192 {
  b: [64] u8;
  h := sample("verack header", b[..]);
  h[bit / 8] ^= cast(u8) (1 << (bit % 8));
  f := p2p_frame_read_vartime(h, P2P_NETWORKS[P2P_MAINNET].magic);
  #assert f.code != P2P_OK || (bit >= 32 && bit < 128 && !p2p_command_is(h, f, "verack"));
}
// A message framed for one network reads on that network only.
#forall "each network reads only its own messages" i: 0..<5, j: 0..<5 {
  b: [256] u8; f: [300] u8;
  p := sample("version", b[..]);
  n := p2p_frame_write_vartime(P2P_NETWORKS[i].magic, "version", p, f[..]);
  r := p2p_frame_read_vartime(f[0..<n], P2P_NETWORKS[j].magic);
  #assert (r.code == P2P_OK) == (i == j) && (i == j || r.code == P2P_BAD_MAGIC);
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/p2p-messages.coax
```

`coax test` runs 105 checks, including the loaded files' own checks, in
about 5 seconds on 2026-11-03.

### The library's own checks

`p2p.coax` is an `#authority` file. Its checks cover each header refusal
and the order between them, the command's padding at its edges (0x1f,
0x20, 0x7e, 0x7f, a byte after the first zero, twelve letters, twelve
zeros), the frame writer's refusals, each `version` prefix and its
optional groups, a user agent of 256 and 257 bytes, the inventory names
and the 50,000 limit on both sides of the read, the locator's limit and
heights, the `headers` limit and its ignored transaction count, the
`addr` excerpt and the 1,000 limit, every `addrv2` network, the wrong
lengths and unknown IDs, and the 64-bit CompactSize.

```shell
coax/bin/coax test coax/lib/bitcoin/p2p.coax
```

`coax test` runs 99 checks, including those of the files it loads, in
about 5 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### Bitcoin Core's own serializer

[p2p.py](../../coax/tests/bitcoin/p2p.py) computes every line the example
prints, in Python that shares no code with Coax. It reads and writes the
payloads with Bitcoin Core's functional-test serializer,
`test_framework/messages.py`, pinned and loaded by
[core_messages.py](../../coax/tests/bitcoin/core_messages.py). It
transliterates the header checks from the pinned `net.cpp` and
`protocol.cpp` and the locator from `chain.cpp`, and reads the limits and
message starts from the pinned `net.h`, `net_processing.cpp`,
`netaddress.h` and `chainparams.cpp`. The transcript must equal the
fixture.

```shell
python3 -I coax/tests/bitcoin/p2p.py --coax coax/bin/coax
```

On 2026-11-03 all 61 lines agree with Python, and the example's output in
native debug and release builds equals them. Wasm was not run.

The samples come from the developer reference's annotated hexdumps.
[make_p2p_vectors.py](../../coax/tests/bitcoin/make_p2p_vectors.py) joins
each dump's hex column from the pinned page, and has `messages.py` write
the `addrv2` entries, into
[p2p_vectors.coax](../../coax/tests/bitcoin/p2p_vectors.coax).

## What this chapter leaves out

- The v2 transport: chapter 67.
- The handshake, feature negotiation and what a node does with each
  message: chapter 68.
- The address manager: chapter 69.
- Rate limits, misbehaviour scores and memory bounds per peer: chapter
  71.
- `block`, `tx`, `cmpctblock` and the filter messages. A `tx` payload is
  chapter 34's serialization, and a `block` is a header and its
  transactions; compact blocks are chapter 73 and filters chapter 74.
- Parsing addresses from text (`.onion`, `.b32.i2p`): chapter 69.

## Summary

- A v1 message is a 24-byte header (message start, command, length,
  checksum) and a payload.
- Core checks the message start and the length (at most 4,000,000) as
  soon as it has the header, and the checksum and the command once it has
  the payload. A reader that runs out of bytes waits.
- A command is printable ASCII, then zero bytes only.
- `version` has four required fields and four optional groups.
- `inv`, `addr` and the locator are read whole before their counts are
  checked; `headers` checks its count first.
- A block locator names the last ten blocks, then blocks at doubling
  distances, then genesis.
- `addrv2` adds network IDs, so Tor v3, I2P and CJDNS addresses fit.

## Dependencies

- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)
- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `net.cpp` (`V1Transport`),
  `net.h`, `protocol.h` and `protocol.cpp`, `net_processing.cpp`
  (`ProcessMessage`), `netaddress.h` and `netaddress.cpp`, `chain.cpp`
  (`LocatorEntries`), `kernel/chainparams.cpp`, and the functional-test
  serializer `test/functional/test_framework/messages.py`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/net.cpp).
- The Bitcoin developer reference's P2P network page,
  `reference/p2p_networking.rst` at commit `813ba3fb` of
  `bitcoin-dot-org/developer.bitcoin.org`, pinned in
  [coax/tests/vectors/upstream/developer-bitcoin-org/](../../coax/tests/vectors/upstream/developer-bitcoin-org/p2p_networking.rst).
- [BIP-155](https://github.com/bitcoin/bips/blob/master/bip-0155.mediawiki)
  (`addrv2`), pinned in
  [coax/tests/vectors/upstream/bip155/](../../coax/tests/vectors/upstream/bip155/bip-0155.mediawiki);
  BIP-133 (`feefilter`), BIP-152 (`sendcmpct`) and BIP-339 (`wtxidrelay`).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
