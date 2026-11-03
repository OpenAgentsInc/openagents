# 68. The handshake and feature negotiation

Before two nodes exchange blocks or transactions, they introduce
themselves. Each sends a `version` message that says which protocol
version it speaks, which services it offers and how high its chain is.
Each answers the other's `version` with `verack`. Between the two, each
announces the features it wants for the rest of the connection:
`wtxidrelay` (BIP-339), `sendaddrv2` (BIP-155) and, from protocol version
70017, `feature` (BIP-434). After `verack` come `sendcmpct` (BIP-152),
`sendheaders` (BIP-130) and `feefilter` (BIP-133), which may change at any
time.

Underneath the handshake is a transport. Chapter 65's v1 frames travel in
the clear; chapter 67's v2 packets are encrypted, and start with a key
exchange of their own. A node that wants v2 must still talk to peers that
only know v1, so BIP-324 tells a v1 `version` from a v2 key by its first 16
bytes, and a node whose v2 attempt meets silence tries again in v1.

This chapter builds both as pure state machines. Nothing here opens a
socket: the tests and the example feed bytes from one side to the other in
memory.

## What this chapter teaches

- The order of a handshake: who sends `version` first, what each side
  sends when it reads the other's, and when the connection counts as up.
- The checks Bitcoin Core makes on a `version`, in Core's order: services,
  then the minimum version, then the payload's tail, then self-connection.
- Which messages must come before `verack`, which may come at any time,
  and what Core does with each one out of place: ignore it or disconnect.
- Service bits and how Core decides that a peer offers what it needs.
- How a v2 responder detects a v1 peer, and when a v2 initiator
  reconnects in v1.
- The inactivity timeouts that end a stalled handshake.

## Where Bitcoin uses the handshake

| Where | What it uses | Chapter |
| --- | --- | --- |
| The address manager | the services a peer reports in `version`, and `sendaddrv2` | 69 |
| Connections | the connection types, feelers, v2 first and the v1 retry | 70 |
| Denial-of-service limits | the disconnects here, and the timeouts | 71 |
| Headers sync | `sendheaders`, and the peer's start height | 72 |
| Compact blocks | `sendcmpct` and its high-bandwidth flag | 73 |
| Transaction relay | `wtxidrelay`, `feefilter` and the relay flag | 83 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [handshake.coax](../../coax/lib/bitcoin/handshake.coax) | `hs_open(p, cfg, now)` | a connection opens; an outbound one sends `version` |
| | `hs_receive_vartime(p, command, payload, now, pending)` | one message from the peer |
| | `hs_tick(p, now, detecting)` | the inactivity timeouts |
| | `hs_payload_write_vartime(p, kind, out)` | the payload of a message the handshake sends |
| | `hs_desirable`, `hs_has_desirable` | the services a peer must offer |
| | `hs_wire_init`, `hs_wire_read_vartime` | the transport: v1 frames, or the v2 handshake and packets |
| | `hs_wire_key_write_vartime`, `hs_wire_ready_write_vartime`, `hs_wire_write_vartime` | what the transport sends |
| | `hs_wire_reconnect_v1(w, received)` | whether to retry a failed v2 connection in v1 |
| [p2p.coax](../../coax/lib/bitcoin/p2p.coax) | `p2p_frame_read_vartime`, `p2p_version_read_vartime` | v1 frames and the `version` payload (chapter 65) |
| [v2transport.coax](../../coax/lib/bitcoin/v2transport.coax) | `v2_cipher_init`, `v2_packet_seal`, `v2_garbage_vartime` | the v2 ciphers, packets and garbage (chapter 67) |
| [ellswift.coax](../../coax/lib/bitcoin/ellswift.coax) | `ellswift_create`, `ellswift_xdh` | the v2 keys (chapter 66) |

Messages, and the bytes that carry them, are public, so the readers run in
variable time and end in `_vartime` (chapter 3). The wire holds one secret,
its private key, and wipes it as soon as the shared secret is derived.

The numbers come from Bitcoin Core's `node/protocol_version.h`,
`protocol.h` and `net.h`:

<!-- source: coax/lib/bitcoin/handshake.coax constants -->
```jai
HS_PROTOCOL_VERSION :: 70017;   // node/protocol_version.h PROTOCOL_VERSION
HS_MIN_PEER_VERSION :: 31800;   // MIN_PEER_PROTO_VERSION: older peers are disconnected
HS_BIP31_VERSION :: 60000;      // BIP0031_VERSION: pong is sent to peers above this
HS_ALERT_VERSION :: 70012;      // peers at or below get the final alert
HS_SHORT_IDS_VERSION :: 70014;  // SHORT_IDS_BLOCKS_VERSION: sendcmpct after verack
HS_WTXID_VERSION :: 70016;      // WTXID_RELAY_VERSION: wtxidrelay before verack
HS_ADDRV2_VERSION :: 70016;     // sendaddrv2 is sent from this version, as a courtesy
HS_FEATURE_VERSION :: 70017;    // FEATURE_VERSION: the feature message (BIP-434)
HS_CMPCT_VERSION :: 2;          // CMPCTBLOCKS_VERSION: compact blocks with witnesses

// Service bits (protocol.h ServiceFlags).
HS_NODE_NETWORK :: cast(u64) 1;            // serves the whole chain
HS_NODE_BLOOM :: cast(u64) 4;              // BIP-111 bloom filters
HS_NODE_WITNESS :: cast(u64) 8;            // BIP-144 witnesses
HS_NODE_COMPACT_FILTERS :: cast(u64) 64;   // BIP-157 filters
HS_NODE_NETWORK_LIMITED :: cast(u64) 1024; // BIP-159: the last 288 blocks
HS_NODE_P2P_V2 :: cast(u64) 2048;          // BIP-324 transport
HS_LIMITED_DEPTH :: 144;        // NODE_NETWORK_LIMITED_ALLOW_CONN_BLOCKS

// Timeouts in seconds (net.h).
HS_CONNECT_TIMEOUT :: 60;       // DEFAULT_PEER_CONNECT_TIMEOUT
HS_TIMEOUT_INTERVAL :: 1200;    // TIMEOUT_INTERVAL: 20 minutes
HS_MAX_FEATURE_ID :: 80;        // protocol.h MAX_FEATUREID_LENGTH
HS_MAX_FEATURE_DATA :: 512;     // protocol.h MAX_FEATUREDATA_LENGTH
```

Each event answers with a verdict and a reason. The reasons are Core's
log messages:

<!-- source: coax/lib/bitcoin/handshake.coax codes -->
```jai
// Connection types (net.h ConnectionType).
HS_INBOUND :: 0; HS_OUTBOUND :: 1; HS_BLOCK_RELAY :: 2; HS_MANUAL :: 3; HS_FEELER :: 4; HS_ADDR_FETCH :: 5;

// What became of an event.
HS_DONE :: 0;        // handled
HS_IGNORED :: 1;     // dropped, the connection stays
HS_DISCONNECT :: 2;  // Core sets fDisconnect
HS_DELIVER :: 3;     // a message for the rest of the node (later chapters)

// Why, as Core's log says it.
HS_R_NONE :: 0;
HS_R_SERVICES :: 1;           // "peer does not offer the expected services"
HS_R_OBSOLETE :: 2;           // "peer using obsolete version"
HS_R_SELF :: 3;               // "connected to self"
HS_R_REDUNDANT_VERSION :: 4;  // "redundant version message"
HS_R_NO_VERSION :: 5;         // "non-version message before version handshake"
HS_R_REDUNDANT_VERACK :: 6;   // "ignoring redundant verack message"
HS_R_BAD_PAYLOAD :: 7;        // the payload doesn't read: "Exception ... caught"
HS_R_AFTER_VERACK :: 8;       // "wtxidrelay received after verack" and the like
HS_R_ANNOUNCE :: 9;           // "invalid sendcmpct announce field" (Misbehaving)
HS_R_CMPCT_VERSION :: 10;     // sendcmpct of a version other than 2 is ignored
HS_R_OLD_COMMON :: 11;        // "ignoring wtxidrelay due to old common version"
HS_R_DUPLICATE :: 12;         // "ignoring duplicate wtxidrelay"
HS_R_BEFORE_VERACK :: 13;     // "Unsupported message prior to verack"
HS_R_FEELER :: 14;            // "feeler connection completed"
HS_R_NO_MESSAGE :: 15;        // "socket no message in first 60 seconds"
HS_R_SEND_TIMEOUT :: 16;      // "socket sending timeout"
HS_R_RECV_TIMEOUT :: 17;      // "socket receive timeout"
HS_R_HANDSHAKE_TIMEOUT :: 18; // "version handshake timeout"
HS_R_V2_TIMEOUT :: 19;        // "V2 handshake timeout"
HS_R_FEATURE_VERSION :: 20;   // "feature received with incompatible version"
HS_R_FEATURE_PAYLOAD :: 21;   // "invalid feature payload"
HS_R_UNKNOWN_FEATURE :: 22;   // "unknown feature advertised"
HS_R_RECONCILIATION :: 23;    // "sendtxrcncl ... ignored": reconciliation is off by default
HS_R_FEE_RANGE :: 24;         // a feefilter outside MoneyRange is ignored

// Messages the handshake sends.
HS_SEND_VERSION :: 1; HS_SEND_VERACK :: 2; HS_SEND_WTXIDRELAY :: 3; HS_SEND_SENDADDRV2 :: 4;
HS_SEND_SENDCMPCT :: 5; HS_SEND_GETADDR :: 6; HS_SEND_ALERT :: 7; HS_SEND_PONG :: 8;
HS_COMMANDS :: [9] string.["", "version", "verack", "wtxidrelay", "sendaddrv2", "sendcmpct", "getaddr", "alert", "pong"];
```

## The order of a handshake

A connection has two ends. The node that dialed is *outbound*; the node
that accepted is *inbound*. The outbound side speaks first:

1. Outbound sends `version`.
2. Inbound reads it and answers with its own `version`, then `wtxidrelay`,
   `sendaddrv2` and `verack`.
3. Outbound reads that `version` and sends `wtxidrelay`, `sendaddrv2`,
   `verack` and `getaddr`. It reads `wtxidrelay` and `sendaddrv2`, then
   `verack`: its connection is up, and it sends `sendcmpct`.
4. Inbound reads `wtxidrelay`, `sendaddrv2` and `verack`: its connection
   is up, and it sends `sendcmpct`.

Each side's `verack` comes after its feature messages. That is the point
of the order: BIP-339 and BIP-155 say a feature is set between `version`
and `verack`, so by the time a side reads `verack` it knows every feature
the peer will use. A message that would change one after `verack` is a
protocol violation, and Core disconnects.

The peer state follows Core's `CNode` and `Peer`. `their_version` stays 0
until a `version` is accepted, as `pfrom.nVersion` does; `connected` is
Core's `fSuccessfullyConnected`, set by `verack`:

<!-- source: coax/lib/bitcoin/handshake.coax peer -->
```jai
// What the node is and what it wants from this connection.
HsConfig :: struct {
  conn: int;                // HS_INBOUND, HS_OUTBOUND, ...
  version: int = HS_PROTOCOL_VERSION;
  services: u64;            // ours
  peer_services: u64;       // what we believe the peer offers (its address's services)
  nonce: u64;               // this connection's random nonce
  agent: string;            // our user agent
  height: int;              // our best height
  relay: bool = true;       // whether we want transactions
  tip_depth: int;           // how many blocks our tip is behind the present
}
// One peer, as net_processing.cpp's CNode and Peer keep it.
HsPeer :: struct {
  cfg: HsConfig;
  their_version: int;       // 0 until a version is accepted (pfrom.nVersion)
  common: int;              // min(theirs, ours)
  their_services: u64; their_nonce: u64; their_height: int = -1; their_relay: bool;
  time: int; time_offset: int;
  connected: bool;          // a verack came: fSuccessfullyConnected
  wtxid_relay: bool; wants_addrv2: bool; prefers_headers: bool;
  provides_cmpct: bool; cmpct_high: bool; fee_filter: int;
  ping: u64;                // the last ping's nonce, for the pong
  opened: int; last_send: int = -1; last_recv: int = -1;
}
// The answer to one event: a verdict, its reason, and messages to send in
// order (their payloads come from hs_payload_write_vartime).
HsStep :: struct { verdict: int; reason: int; n: int; sends: [8] int; }
hs_step :: (verdict: int, reason: int) -> HsStep { return HsStep.{verdict = verdict, reason = reason}; }
hs_send :: (s: HsStep, kind: int) -> HsStep { t := s; t.sends[t.n] = kind; t.n += 1; return t; }
// The connection opens. An outbound connection speaks first, with its version.
hs_open :: (p: [] HsPeer, cfg: HsConfig, now: int) -> HsStep {
  p[0] = HsPeer.{cfg = cfg, opened = now, time = now};
  if cfg.conn == HS_INBOUND { return hs_step(HS_DONE, HS_R_NONE); }
  p[0].last_send = now;
  return hs_send(hs_step(HS_DONE, HS_R_NONE), HS_SEND_VERSION);
}
// The caller sent bytes at now, outside an event's answer.
hs_sent :: (p: [] HsPeer, now: int) { p[0].last_send = now; }
```

## Services

The `services` field of `version` is a bitmap. Core knows six bits:

| Bit | Value | Name | Meaning |
| --- | --- | --- | --- |
| 0 | 1 | `NODE_NETWORK` | serves every block |
| 2 | 4 | `NODE_BLOOM` | serves BIP-37 bloom filters (BIP-111) |
| 3 | 8 | `NODE_WITNESS` | serves witnesses (BIP-144) |
| 6 | 64 | `NODE_COMPACT_FILTERS` | serves BIP-157 filters |
| 10 | 1,024 | `NODE_NETWORK_LIMITED` | serves the last 288 blocks (BIP-159) |
| 11 | 2,048 | `NODE_P2P_V2` | speaks BIP-324 |

An outbound connection exists to get something from the peer, so Core
checks that the peer offers it. For a full-relay, block-relay-only or
address-fetch connection, the peer must offer `NODE_NETWORK` and
`NODE_WITNESS`. A pruned peer that offers `NODE_NETWORK_LIMITED` instead
is enough while our tip is within 144 blocks of the present, because the
blocks we still need are recent. Inbound, manual and feeler connections
don't check.

<!-- source: coax/lib/bitcoin/handshake.coax services -->
```jai
// GetDesirableServiceFlags: a full node with witnesses, or near the tip a
// limited one. HasAllDesirableServiceFlags: every desirable bit is offered.
hs_desirable :: (services: u64, tip_depth: int) -> u64 {
  if (services & HS_NODE_NETWORK_LIMITED) != 0 && tip_depth < HS_LIMITED_DEPTH { return HS_NODE_NETWORK_LIMITED | HS_NODE_WITNESS; }
  return HS_NODE_NETWORK | HS_NODE_WITNESS;
}
hs_has_desirable :: (services: u64, tip_depth: int) -> bool {
  wanted := hs_desirable(services, tip_depth);
  return (wanted & services) == wanted;
}
// ExpectServicesFromConn: outbound connections we chose for what they serve.
hs_expects_services :: (conn: int) -> bool { return conn == HS_OUTBOUND || conn == HS_BLOCK_RELAY || conn == HS_ADDR_FETCH; }
```

### Worked example: a pruned peer

A peer offers `NODE_NETWORK_LIMITED | NODE_WITNESS` = 1,024 + 8 = 1,032.

- Our tip is 10 blocks behind. The limited bit is set and 10 < 144, so the
  desirable flags are 1,032 itself. 1,032 AND 1,032 = 1,032: the peer has
  them all.
- Our tip is 500 blocks behind. The desirable flags are `NODE_NETWORK |
  NODE_WITNESS` = 9. 9 AND 1,032 = 8, not 9: bit 0 is missing, and Core
  disconnects with "peer does not offer the expected services".

## The `version` checks

Core reads a `version` in pieces and checks between them. The order
matters, because a message can fail more than one check, and the log and
the outcome follow the first:

1. A second `version` is ignored.
2. The required fields are read: version, services, time and the
   receiver's address. Too few bytes throw, and Core drops the message.
3. Services, for the connection types that check them: disconnect.
4. A version below 31,800: disconnect.
5. The optional tail is read: the sender's address and nonce, the user
   agent (at most 256 bytes), the start height and the relay flag. A
   group cut short throws, and the message is dropped. A missing nonce
   is 1.
6. On an inbound connection, a nonce that matches one of our own pending
   outbound connections means we dialed ourselves: disconnect.

An old peer without witnesses fails both step 3 and step 4; Core reports
the services. An obsolete version with a truncated tail fails step 4
before step 5 reads the tail, so Core disconnects instead of dropping the
message.

Self-connection works because every outbound connection draws a random
64-bit nonce and sends it in its `version`. If that `version` arrives on
one of our own inbound sockets, the address we dialed is us. Core compares
only against outbound connections whose handshake hasn't finished, so the
caller passes their nonces as `pending`.

Once the checks pass, the common version is the lower of the two. It
decides what follows: `wtxidrelay` from 70016, `sendaddrv2` from 70016 (BIP-155
allows it earlier, but Core doesn't send it to older peers as a courtesy),
`sendcmpct` after `verack` from 70014, and the final alert to peers at or
below 70012, which still run the alert system Bitcoin removed in 2016. An
outbound full-relay, manual or address-fetch connection also asks for
addresses with `getaddr`; a block-relay-only connection and a feeler
don't, so they leak nothing about our address book. A feeler exists only
to test that an address is up, and disconnects as soon as the `version`
is in.

<!-- source: coax/lib/bitcoin/handshake.coax version -->
```jai
// ProcessMessage for "version", check by check in Core's order.
hs_version_vartime :: (p: [] HsPeer, payload: [] u8, now: int, pending: [] u64) -> HsStep {
  if p[0].their_version != 0 { return hs_step(HS_IGNORED, HS_R_REDUNDANT_VERSION); }
  // The required fields: version, services, time and the receiver's address.
  if payload.count < 46 { return hs_step(HS_IGNORED, HS_R_BAD_PAYLOAD); }
  head := p2p_version_read_vartime(payload[0..<46]);
  if hs_expects_services(p[0].cfg.conn) && !hs_has_desirable(head.services, p[0].cfg.tip_depth) {
    return hs_step(HS_DISCONNECT, HS_R_SERVICES);
  }
  if head.version < HS_MIN_PEER_VERSION { return hs_step(HS_DISCONNECT, HS_R_OBSOLETE); }
  // Then the optional groups; one cut short throws, and Core drops the message.
  v := p2p_version_read_vartime(payload);
  if v.code != P2P_OK { return hs_step(HS_IGNORED, HS_R_BAD_PAYLOAD); }
  nonce := ifx v.parts >= 1 then v.nonce else 1;
  inbound := p[0].cfg.conn == HS_INBOUND;
  if inbound { for ours: pending { if ours == nonce { return hs_step(HS_DISCONNECT, HS_R_SELF); } } }
  s := hs_step(HS_DONE, HS_R_NONE);
  if inbound { s = hs_send(s, HS_SEND_VERSION); p[0].last_send = now; }
  p[0].common = ifx v.version < p[0].cfg.version then v.version else p[0].cfg.version;
  p[0].their_version = v.version; p[0].their_services = v.services; p[0].their_nonce = nonce;
  p[0].their_height = v.height; p[0].their_relay = v.relay;
  p[0].time_offset = (ifx v.time < 0 then 0 else v.time) - now;
  if p[0].common >= HS_WTXID_VERSION { s = hs_send(s, HS_SEND_WTXIDRELAY); }
  if p[0].common >= HS_ADDRV2_VERSION { s = hs_send(s, HS_SEND_SENDADDRV2); }
  s = hs_send(s, HS_SEND_VERACK);
  // SetupAddressRelay: outbound, but not block-relay-only or a feeler.
  conn := p[0].cfg.conn;
  if conn == HS_OUTBOUND || conn == HS_MANUAL || conn == HS_ADDR_FETCH { s = hs_send(s, HS_SEND_GETADDR); }
  if p[0].common <= HS_ALERT_VERSION { s = hs_send(s, HS_SEND_ALERT); }
  p[0].last_send = now;
  if conn == HS_FEELER { s.verdict = HS_DISCONNECT; s.reason = HS_R_FEELER; }
  return s;
}
```

## Messages before and after `verack`

Every other message goes through `hs_receive_vartime`, in the order of
Core's `ProcessMessage`. Before a `version`, everything is ignored. Then:

| Message | Before `verack` | After `verack` |
| --- | --- | --- |
| `verack` | the connection is up; send `sendcmpct` | ignored |
| `sendheaders` | announce blocks with headers | the same |
| `sendcmpct` | an announce byte above 1 disconnects; a version other than 2 is ignored | the same |
| `wtxidrelay` | relay by wtxid, from common version 70016 | disconnect |
| `sendaddrv2` | send addresses as `addrv2` | disconnect |
| `feature` | read and checked; no feature is defined yet | disconnect |
| `sendtxrcncl` | ignored: reconciliation (BIP-330) is off by default | ignored |
| anything else | ignored | handled by the node |

`sendheaders` and `sendcmpct` are not negotiation: a node may change its
mind about how it wants blocks announced, so they are allowed at any time.
The handshake keeps `ping` and `feefilter` too, because they shape the
connection itself. A `ping` gets a `pong` with the same nonce, and a
`feefilter` sets the lowest fee rate the peer wants to hear about, if the
rate is inside Bitcoin's money range. Everything else after `verack` is
`HS_DELIVER`: later chapters handle it.

<!-- source: coax/lib/bitcoin/handshake.coax receive -->
```jai
// One message from the peer. pending holds the nonces of our outbound
// connections whose handshake hasn't finished, for self-connection.
hs_receive_vartime :: (p: [] HsPeer, command: string, payload: [] u8, now: int, pending: [] u64) -> HsStep {
  p[0].last_recv = now;
  if command == "version" { return hs_version_vartime(p, payload, now, pending); }
  if p[0].their_version == 0 { return hs_step(HS_IGNORED, HS_R_NO_VERSION); }
  if command == "verack" {
    if p[0].connected { return hs_step(HS_IGNORED, HS_R_REDUNDANT_VERACK); }
    s := hs_step(HS_DONE, HS_R_NONE);
    if p[0].common >= HS_SHORT_IDS_VERSION { s = hs_send(s, HS_SEND_SENDCMPCT); p[0].last_send = now; }
    p[0].connected = true;
    return s;
  }
  if command == "sendheaders" { p[0].prefers_headers = true; return hs_step(HS_DONE, HS_R_NONE); }
  if command == "sendcmpct" {
    if payload.count < 9 { return hs_step(HS_IGNORED, HS_R_BAD_PAYLOAD); }
    if payload[0] > 1 { return hs_step(HS_DISCONNECT, HS_R_ANNOUNCE); }
    if cb_load_le64(payload, 1).value != HS_CMPCT_VERSION { return hs_step(HS_IGNORED, HS_R_CMPCT_VERSION); }
    p[0].provides_cmpct = true; p[0].cmpct_high = payload[0] == 1;
    return hs_step(HS_DONE, HS_R_NONE);
  }
  // BIP-339, BIP-155 and BIP-434: only between version and verack.
  if command == "wtxidrelay" {
    if p[0].connected { return hs_step(HS_DISCONNECT, HS_R_AFTER_VERACK); }
    if p[0].common < HS_WTXID_VERSION { return hs_step(HS_IGNORED, HS_R_OLD_COMMON); }
    if p[0].wtxid_relay { return hs_step(HS_IGNORED, HS_R_DUPLICATE); }
    p[0].wtxid_relay = true;
    return hs_step(HS_DONE, HS_R_NONE);
  }
  if command == "sendaddrv2" {
    if p[0].connected { return hs_step(HS_DISCONNECT, HS_R_AFTER_VERACK); }
    p[0].wants_addrv2 = true;
    return hs_step(HS_DONE, HS_R_NONE);
  }
  if command == "feature" {
    if p[0].connected { return hs_step(HS_DISCONNECT, HS_R_AFTER_VERACK); }
    if p[0].common < HS_FEATURE_VERSION { return hs_step(HS_DISCONNECT, HS_R_FEATURE_VERSION); }
    return hs_feature_vartime(payload);
  }
  if command == "sendtxrcncl" { return hs_step(HS_IGNORED, HS_R_RECONCILIATION); }
  if !p[0].connected { return hs_step(HS_IGNORED, HS_R_BEFORE_VERACK); }
  if command == "ping" {
    n := p2p_u64_read_vartime(payload);
    if n.code != P2P_OK { return hs_step(HS_IGNORED, HS_R_BAD_PAYLOAD); }
    p[0].ping = n.value;
    if p[0].common <= HS_BIP31_VERSION { return hs_step(HS_DONE, HS_R_NONE); }
    p[0].last_send = now;
    return hs_send(hs_step(HS_DONE, HS_R_NONE), HS_SEND_PONG);
  }
  if command == "feefilter" {
    f := p2p_feefilter_read_vartime(payload);
    if f.code != P2P_OK { return hs_step(HS_IGNORED, HS_R_BAD_PAYLOAD); }
    if !f.in_range { return hs_step(HS_IGNORED, HS_R_FEE_RANGE); }
    p[0].fee_filter = f.rate;
    return hs_step(HS_DONE, HS_R_NONE);
  }
  return hs_step(HS_DELIVER, HS_R_NONE);
}
// The feature message: an ID of 4 to 80 bytes, data of at most 512, and
// nothing after. No feature is defined yet, so a good one is ignored.
hs_feature_vartime :: (payload: [] u8) -> HsStep {
  bad := hs_step(HS_DISCONNECT, HS_R_FEATURE_PAYLOAD);
  id := tx_span_vartime(payload, 0);
  if id.code != TX_OK || id.n > HS_MAX_FEATURE_ID || id.n < 4 { return bad; }
  data := tx_span_vartime(payload, id.next);
  if data.code != TX_OK || data.n > HS_MAX_FEATURE_DATA || data.next != payload.count { return bad; }
  return hs_step(HS_IGNORED, HS_R_UNKNOWN_FEATURE);
}
```

## Timeouts

Core runs `InactivityCheck` on every connection once 60 seconds have
passed since it opened. A connection then ends if one side never sent or
never received a byte, if 20 minutes passed since the last send or the
last receive, or if the handshake still hasn't finished. The last case has
two names in Core's log: "V2 handshake timeout" while a v2 responder still
can't tell v1 from v2, and "version handshake timeout" otherwise.

<!-- source: coax/lib/bitcoin/handshake.coax tick -->
```jai
// InactivityCheck, run as the clock moves. Nothing is checked in the
// first 60 seconds. After them: a side that never spoke, 20 minutes of
// silence either way, then a handshake still unfinished. detecting is true
// while a v2 responder still can't tell v1 from v2.
hs_tick :: (p: [] HsPeer, now: int, detecting: bool) -> HsStep {
  if now <= p[0].opened + HS_CONNECT_TIMEOUT { return hs_step(HS_DONE, HS_R_NONE); }
  if p[0].last_recv < 0 || p[0].last_send < 0 { return hs_step(HS_DISCONNECT, HS_R_NO_MESSAGE); }
  if now > p[0].last_send + HS_TIMEOUT_INTERVAL { return hs_step(HS_DISCONNECT, HS_R_SEND_TIMEOUT); }
  if now > p[0].last_recv + HS_TIMEOUT_INTERVAL { return hs_step(HS_DISCONNECT, HS_R_RECV_TIMEOUT); }
  if !p[0].connected { return hs_step(HS_DISCONNECT, ifx detecting then HS_R_V2_TIMEOUT else HS_R_HANDSHAKE_TIMEOUT); }
  return hs_step(HS_DONE, HS_R_NONE);
}
```

## The messages the handshake sends

`version` follows Core's `PushNodeVersion`. The receiver's address field
carries the services we believe the peer has, and an address of 16 zero
bytes and port 0: Core sends the peer's real address only when it is
routable and not behind a proxy, and leaves it out otherwise. The sender's
address is always 16 zero bytes. The rest are our nonce, user agent,
height and relay flag. `sendcmpct` is 0 (low bandwidth) and version 2.
`verack`, `wtxidrelay`, `sendaddrv2` and `getaddr` are empty.

<!-- source: coax/lib/bitcoin/handshake.coax payloads -->
```jai
// The payload of a message the handshake sends, into out: its size, or -1
// if out is too small. version is PushNodeVersion's: the peer's services
// and an unroutable address, ours the same, our nonce, agent, height and
// relay flag.
hs_payload_write_vartime :: (p: [] HsPeer, kind: int, out: [] u8) -> int {
  c := p[0].cfg;
  if kind == HS_SEND_VERSION {
    need := 4 + 8 + 8 + 26 + 26 + 8 + tx_compact_size_vartime(c.agent.count) + c.agent.count + 4 + 1;
    if out.count < need { return -1; }
    at := p2p_put_u32(out, 0, cast,trunc(u32) c.version);
    at = p2p_put_u64(out, at, c.services);
    at = p2p_put_u64(out, at, cast,trunc(u64) p[0].time);
    at = p2p_put_u64(out, at, c.peer_services);
    for i: 0..<18 { out[at + i] = 0; }
    at = p2p_put_u64(out, at + 18, c.services);
    for i: 0..<18 { out[at + i] = 0; }
    at = p2p_put_u64(out, at + 18, c.nonce);
    at = tx_compact_write_vartime(out, at, c.agent.count);
    for i: 0..<c.agent.count { out[at + i] = c.agent[i]; }
    at = p2p_put_u32(out, at + c.agent.count, cast,trunc(u32) c.height);
    out[at] = ifx c.relay then 1 else 0;
    return at + 1;
  }
  if kind == HS_SEND_SENDCMPCT {
    if out.count < 9 { return -1; }
    out[0] = 0;                                   // low bandwidth: we ask for no announcements
    return p2p_put_u64(out, 1, HS_CMPCT_VERSION);
  }
  if kind == HS_SEND_PONG { return p2p_u64_write_vartime(p[0].ping, out); }
  if kind == HS_SEND_ALERT {
    if out.count < HS_FINAL_ALERT.count / 2 { return -1; }
    return hs_unhex(HS_FINAL_ALERT, out).count;
  }
  return 0;                                       // verack, wtxidrelay, sendaddrv2 and getaddr are empty
}
// The final alert, sent to peers from before the alert system was removed
// (net_processing.cpp).
HS_FINAL_ALERT :: "60010000000000000000000000ffffff7f00000000ffffff7ffeffff7f01ffffff7f00000000ffffff7f00ffffff7f002f555247454e543a20416c657274206b657920636f6d70726f6d697365642c2075706772616465207265717569726564004630440220653febd6410f470f6bae11cad19c48413becb1ac2c17f908fd0fd53bdc3abd5202206d0e9c96fe88d4a0f01ed9dedae2b6f9e00da94cad0fecaae66ecf689bf71b50";
```

## The wire

The transport has the states of Core's `V2Transport`. A v1 wire reads
chapter 65's frames. A v2 initiator sends its 64-byte key and some garbage
at once and waits for the peer's key. A v2 responder starts in
`HS_W_DETECT`, Core's `KEY_MAYBE_V1`:

- If the first bytes differ from the start of a v1 `version` (the network's
  message start, then `version` and five zeros), the peer is v2. The
  responder sends its key and garbage and moves to `HS_W_KEY`. One byte can
  decide this.
- If all 16 match, the peer is v1, and the same bytes are read as a v1
  frame.

Once a side has the peer's key, it derives the ciphers and sends its
16-byte garbage terminator, then its version packet: an empty packet whose
AAD is the garbage it sent. Application messages can follow at once. The
receiver looks for the peer's terminator, then opens the first packet with
the garbage as AAD. Decoys may come before the version packet.

<!-- source: coax/lib/bitcoin/handshake.coax wire -->
```jai
// The transport under the handshake, as V2Transport's receive states.
HS_W_V1 :: 0;       // v1 frames (chapter 65)
HS_W_DETECT :: 1;   // a v2 responder: are the first 16 bytes a v1 version? (KEY_MAYBE_V1)
HS_W_KEY :: 2;      // waiting for the peer's 64-byte key
HS_W_GARBAGE :: 3;  // looking for the peer's terminator
HS_W_VERSION :: 4;  // decoys, then the peer's version packet
HS_W_APP :: 5;      // messages

// What the bytes held.
HS_EV_WAIT :: 0;     // nothing yet: read more bytes
HS_EV_V1 :: 1;       // a v1 peer: carry on in v1, with the same bytes
HS_EV_SEND_KEY :: 2; // a v2 peer: send our key and garbage (hs_wire_key_write_vartime)
HS_EV_KEYS :: 3;     // their key is in: send our terminator and version packet
HS_EV_DECOY :: 4;    // a packet with the ignore bit, dropped
HS_EV_READY :: 5;    // their version packet: messages may flow
HS_EV_MESSAGE :: 6;  // a message: command and payload
HS_EV_REJECT :: 7;   // a message Core drops (bad checksum or type); the connection stays
HS_EV_REFUSE :: 8;   // Core disconnects; code says why

HS_MAX_GARBAGE :: 64;   // the most garbage this model sends (Core: up to 4,095 random bytes)
HsWire :: struct {
  state: int; initiating: bool; magic: [4] u8;
  key: [32] u8; ours: [64] u8;                  // our secret key until the ECDH, and its encoding
  garbage: [64] u8; garbage_n: int;             // what we send after our key
  cipher: [1] V2Cipher;
  their_garbage: int = -1;                      // its length, once the terminator is found
  pending: int = -1;                            // a decrypted length waiting for its packet
  sent: int;                                    // bytes sent, for ShouldReconnectV1
}
HsRead :: struct { event: int; code: int; used: int; command: string; payload: int; n: int; }
// A wire for one side. v2 needs a key, 32 random bytes for its encoding and
// up to 64 bytes of garbage. Returns false if the key or garbage refuses.
hs_wire_init :: (w: [] HsWire, initiating: bool, v2: bool, magic: [4] u8, key: [] u8, rnd: [] u8, garbage: [] u8) -> bool {
  w[0] = HsWire.{initiating = initiating, magic = magic};
  if !v2 { w[0].state = HS_W_V1; return true; }
  if garbage.count > HS_MAX_GARBAGE || key.count != 32 { return false; }
  work: [1] EcdhScratch;
  code := ellswift_create(key, rnd, w[0].ours[..], work[..]);
  work[0] = EcdhScratch.{};
  if code != EC_OK { return false; }
  for i: 0..<32 { w[0].key[i] = key[i]; }
  for i: 0..<garbage.count { w[0].garbage[i] = garbage[i]; }
  w[0].garbage_n = garbage.count;
  w[0].state = ifx initiating then HS_W_KEY else HS_W_DETECT;
  return true;
}
// Our key and garbage: an initiator sends them at once, a responder once
// it knows the peer is v2. Returns the size, or -1.
hs_wire_key_write_vartime :: (w: [] HsWire, out: [] u8) -> int {
  n := 64 + w[0].garbage_n;
  if out.count < n { return -1; }
  for i: 0..<64 { out[i] = w[0].ours[i]; }
  for i: 0..<w[0].garbage_n { out[64 + i] = w[0].garbage[i]; }
  w[0].sent += n;
  return n;
}
// Once their key is in: our terminator, then our version packet, whose
// AAD is our garbage and whose contents are empty.
hs_wire_ready_write_vartime :: (w: [] HsWire, out: [] u8) -> int {
  n := V2_TERMINATOR + V2_EXPANSION;
  if out.count < n { return -1; }
  for i: 0..<16 { out[i] = w[0].cipher[0].send_terminator[i]; }
  m := v2_packet_seal(w[0].cipher[..], 0, w[0].garbage[0..<w[0].garbage_n], false, out[16..<n]);
  w[0].sent += n;
  return n;
}
// A message in the wire's format: a v1 frame or a v2 packet.
hs_wire_write_vartime :: (w: [] HsWire, command: string, payload: [] u8, out: [] u8) -> int {
  n := -1;
  if w[0].state == HS_W_V1 { n = p2p_frame_write_vartime(w[0].magic, command, payload, out); }
  else if w[0].state >= HS_W_GARBAGE && out.count >= 4 {
    c := v2_contents_write_vartime(command, payload, out[4..<out.count]);
    empty: [1] u8;
    if c >= 0 && out.count >= c + V2_EXPANSION { n = v2_packet_seal(w[0].cipher[..], c, empty[0..<0], false, out); }
  }
  if n > 0 { w[0].sent += n; }
  return n;
}
// ShouldReconnectV1: we started v2, sent at least a v1 header's worth, and
// the connection closed before a byte came back. Try again in v1.
hs_wire_reconnect_v1 :: (w: [] HsWire, received: int) -> bool {
  return w[0].initiating && w[0].state == HS_W_KEY && received == 0 && w[0].sent >= P2P_HEADER;
}
```

A v2 node dialing a peer that only knows v1 sends 64 random-looking bytes.
The v1 peer reads the first 24 as a header, finds the wrong message start
and hangs up. Core's `ShouldReconnectV1` then retries in v1 when four
things hold: we initiated, we are still waiting for the peer's key, nothing
came back, and we sent at least 24 bytes, a v1 header's worth, so the
peer had enough to reject.

The reader takes all the bytes received so far and says what they hold.
`used` is how many the caller may drop. The garbage stays in the caller's
buffer until the first packet opens, because it is that packet's AAD. A
packet whose length has been decrypted but whose body hasn't arrived keeps
its length in `pending`: the length cipher must not run twice over the same
three bytes.

<!-- source: coax/lib/bitcoin/handshake.coax read -->
```jai
// What the bytes received so far, b, hold. used is how many the caller
// drops before the next call. v2 packets are decrypted in place; garbage
// stays in b until the first packet, whose AAD it is.
hs_wire_read_vartime :: (w: [] HsWire, b: [] u8) -> HsRead {
  wait := HsRead.{event = HS_EV_WAIT};
  if w[0].state == HS_W_DETECT {
    prefix: [16] u8; v := [12] u8.['v', 'e', 'r', 's', 'i', 'o', 'n', 0, 0, 0, 0, 0];
    for i: 0..<4 { prefix[i] = w[0].magic[i]; } for i: 0..<12 { prefix[4 + i] = v[i]; }
    n := ifx b.count < 16 then b.count else 16;
    for i: 0..<n { if b[i] != prefix[i] { w[0].state = HS_W_KEY; return HsRead.{event = HS_EV_SEND_KEY}; } }
    if n < 16 { return wait; }
    w[0].state = HS_W_V1;
    return HsRead.{event = HS_EV_V1};
  }
  if w[0].state == HS_W_V1 { return hs_wire_v1_vartime(w, b); }
  if w[0].state == HS_W_KEY {
    if b.count < 64 { return wait; }
    secret: [32] u8; work: [1] EcdhScratch;
    code := ellswift_xdh(w[0].ours[..], b[0..<64], w[0].key[..], w[0].initiating, secret[..], work[..]);
    work[0] = EcdhScratch.{}; for i: 0..<32 { w[0].key[i] = 0; }
    if code != EC_OK { return HsRead.{event = HS_EV_REFUSE, code = V2_KEYS}; }
    init := v2_cipher_init(w[0].cipher[..], secret[..], w[0].magic, w[0].initiating);
    for i: 0..<32 { secret[i] = 0; }
    if init != V2_OK { return HsRead.{event = HS_EV_REFUSE, code = init}; }
    w[0].state = HS_W_GARBAGE;
    return HsRead.{event = HS_EV_KEYS, used = 64};
  }
  if w[0].state == HS_W_GARBAGE {
    g := v2_garbage_vartime(b, w[0].cipher[0].recv_terminator);
    if g.code == V2_TRUNCATED { return wait; }
    if g.code != V2_OK { return HsRead.{event = HS_EV_REFUSE, code = g.code}; }
    w[0].their_garbage = g.n; w[0].state = HS_W_VERSION;
  }
  return hs_wire_packet_vartime(w, b);
}
// v1: a frame as chapter 65 reads it. A bad magic or an oversized length
// disconnects; a bad checksum or command drops just that message.
hs_wire_v1_vartime :: (w: [] HsWire, b: [] u8) -> HsRead {
  f := p2p_frame_read_vartime(b, w[0].magic);
  if f.code == P2P_TRUNCATED { return HsRead.{event = HS_EV_WAIT}; }
  if f.code == P2P_BAD_MAGIC || f.code == P2P_OVERSIZE { return HsRead.{event = HS_EV_REFUSE, code = f.code}; }
  if f.code != P2P_OK { return HsRead.{event = HS_EV_REJECT, code = f.code, used = P2P_HEADER + tx_le_vartime(b, 16, 4)}; }
  return HsRead.{event = HS_EV_MESSAGE, used = f.next, command = cast(string) b[4..<4 + f.command_n], payload = P2P_HEADER, n = f.length};
}
// v2: one packet. The first one's AAD is their garbage, which comes first
// in b with the terminator after it.
hs_wire_packet_vartime :: (w: [] HsWire, b: [] u8) -> HsRead {
  wait := HsRead.{event = HS_EV_WAIT};
  first := w[0].their_garbage >= 0;
  skip := ifx first then w[0].their_garbage + V2_TERMINATOR else 0;
  if b.count < skip { return wait; }
  aad := b[0..<ifx first then w[0].their_garbage else 0];
  p := b[skip..<b.count];
  if w[0].pending < 0 {
    if p.count < V2_LENGTH { return wait; }
    l := v2_packet_length_vartime(w[0].cipher[..], p[0..<V2_LENGTH]);
    if l.code != V2_OK { return HsRead.{event = HS_EV_REFUSE, code = l.code}; }
    w[0].pending = l.n;
  }
  n := w[0].pending;
  if p.count < n + V2_EXPANSION { return wait; }
  o := v2_packet_open(w[0].cipher[..], aad, p[V2_LENGTH..<n + V2_EXPANSION]);
  if o.code != V2_OK { return HsRead.{event = HS_EV_REFUSE, code = o.code}; }
  w[0].pending = -1; w[0].their_garbage = -1;
  used := skip + n + V2_EXPANSION;
  if o.ignore { return HsRead.{event = HS_EV_DECOY, used = used, n = n}; }
  if w[0].state == HS_W_VERSION { w[0].state = HS_W_APP; return HsRead.{event = HS_EV_READY, used = used, n = n}; }
  t := v2_contents_read_vartime(p[4..<4 + n]);
  if t.code != V2_OK { return HsRead.{event = HS_EV_REJECT, code = t.code, used = used}; }
  command := ifx t.id > 0 then V2_MESSAGE_IDS[t.id] else cast(string) p[5..<5 + t.command_n];
  return HsRead.{event = HS_EV_MESSAGE, used = used, command = command, payload = skip + 4 + t.payload, n = n - t.payload};
}
```

What each side does with a bad message differs by transport, as in Core:

| Problem | v1 | v2 |
| --- | --- | --- |
| wrong message start | disconnect | (no message start) |
| length above the limit | disconnect | disconnect |
| bad checksum or tag | drop the message | disconnect |
| bad command or message type | drop the message | drop the message |
| no garbage terminator in 4,095 bytes | — | disconnect |

### Worked example: the size of `version` on each wire

The example's `version` payload is 97 bytes: 4 + 8 + 8 for version,
services and time, 26 for each address, 8 for the nonce, 1 + 11 for the
user agent `/Coax:0.68/`, 4 for the height and 1 for the relay flag.

- v1: a 24-byte header and the payload, 121 bytes.
- v2: `version` has no short ID in BIP-324's table, so the contents are a
  zero byte, the 12-byte command and the payload, 110 bytes. The packet
  adds a 3-byte length, a header byte and a 16-byte tag: 130 bytes.

`verack`, `wtxidrelay` and `sendaddrv2` have no short IDs either: each is
24 bytes in v1 and 33 in v2. `sendcmpct` has short ID 20, so it is 33 bytes
in v1 and 1 + 9 + 20 = 30 in v2.

## The example

The example is [bitcoin/examples/handshake.coax](../examples/handshake.coax).
Two nodes, A (outbound) and B (inbound), handshake over four pairings of
transports, with every byte passed through memory. Then single messages
test the version checks, the messages around `verack` and the timeouts.

<!-- source: bitcoin/examples/handshake.coax example -->
```jai
#load "../../coax/lib/bitcoin/handshake.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

show :: (b: [] u8) {
  text: [512] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
VERDICTS :: [4] string.["done", "ignored", "disconnect", "deliver"];
REASONS :: [25] string.["", "no expected services", "obsolete version", "connected to self",
  "redundant version", "no version yet", "redundant verack", "payload doesn't read", "after verack",
  "bad announce field", "not version 2", "old common version", "duplicate", "before verack",
  "feeler done", "no message in 60 s", "send timeout", "receive timeout", "version handshake timeout",
  "V2 handshake timeout", "feature before version 70017", "bad feature payload", "unknown feature",
  "reconciliation off", "fee out of range"];
P2P_CODES :: [9] string.["ok", "truncated", "bad magic", "oversize", "bad checksum", "bad command", "bad payload", "too many", "capacity"];
V2_CODES :: [8] string.["ok", "truncated", "no terminator", "too large", "bad tag", "bad type", "capacity", "keys"];
MAINNET :: [4] u8.[0xf9, 0xbe, 0xb4, 0xd9];

// One node's end of an in-memory connection: its handshake, its wire,
// the bytes it has received and not yet used, and messages waiting for
// the v2 keys.
Side :: struct {
  name: string; peer: [1] HsPeer; wire: [1] HsWire;
  inbox: [2048] u8; have: int; got: int;
  queued: HsStep; closed: bool;
}
config :: (conn: int, nonce: u64, services: u64) -> HsConfig {
  return HsConfig.{conn = conn, services = services, peer_services = ifx conn == HS_INBOUND then cast(u64) 0 else HS_NODE_NETWORK | HS_NODE_WITNESS,
    nonce = nonce, agent = "/Coax:0.68/", height = 900000};
}
// A opens to B. v2 sides use keys 1 and 2 and a little garbage.
open :: (a: [] Side, b: [] Side, a_v2: bool, b_v2: bool) {
  a[0] = Side.{name = "A"}; b[0] = Side.{name = "B"};
  ka: [32] u8; ka[31] = 1; kb: [32] u8; kb[31] = 2; ra: [32] u8; rb: [32] u8;
  for i: 0..<32 { ra[i] = cast(u8) i; rb[i] = cast(u8) (32 + i); }
  ga := [5] u8.['h', 'e', 'l', 'l', 'o']; gb := [3] u8.[1, 2, 3];
  ok := hs_wire_init(a[0].wire[..], true, a_v2, MAINNET, ka[..], ra[..], ga[..]) && hs_wire_init(b[0].wire[..], false, b_v2, MAINNET, kb[..], rb[..], gb[..]);
  sb := hs_open(b[0].peer[..], config(HS_INBOUND, 0x1112131415161718, HS_NODE_NETWORK | HS_NODE_WITNESS | HS_NODE_P2P_V2), 1000);
  if a_v2 {
    n := hs_wire_key_write_vartime(a[0].wire[..], b[0].inbox[..]); b[0].have = n; b[0].got = n;
    print("  A sends its key and % bytes of garbage: % bytes\n", ga.count, n);
  }
  send(a, b, hs_open(a[0].peer[..], config(HS_OUTBOUND, 0x0102030405060708, HS_NODE_NETWORK | HS_NODE_WITNESS | HS_NODE_P2P_V2), 1000));
}
// Messages from one side to the other, or queued until the v2 keys.
send :: (from: [] Side, to: [] Side, s: HsStep) {
  w := from[0].wire[0].state;
  if w != HS_W_V1 && w < HS_W_GARBAGE {
    for i: 0..<s.n { from[0].queued.sends[from[0].queued.n] = s.sends[i]; from[0].queued.n += 1; }
    return;
  }
  for i: 0..<s.n {
    payload: [256] u8;
    n := hs_payload_write_vartime(from[0].peer[..], s.sends[i], payload[..]);
    m := hs_wire_write_vartime(from[0].wire[..], HS_COMMANDS[s.sends[i]], payload[0..<n], to[0].inbox[to[0].have..<2048]);
    to[0].have += m; to[0].got += m;
  }
}
names :: (s: HsStep) {
  if s.n == 0 { return; }
  print("; sends");
  for i: 0..<s.n { print(" %", HS_COMMANDS[s.sends[i]]); }
}
// Everything me can make of its received bytes. Returns whether it did anything.
pump :: (me: [] Side, them: [] Side, now: int) -> bool {
  progress := false; none: [1] u64;
  while !me[0].closed {
    r := hs_wire_read_vartime(me[0].wire[..], me[0].inbox[0..<me[0].have]);
    if r.event == HS_EV_WAIT { break; }
    progress = true;
    if r.event == HS_EV_V1 { print("  %: the first 16 bytes are a v1 version, so v1 it is\n", me[0].name); }
    else if r.event == HS_EV_SEND_KEY {
      n := hs_wire_key_write_vartime(me[0].wire[..], them[0].inbox[them[0].have..<2048]);
      them[0].have += n; them[0].got += n;
      print("  %: not a v1 version; sends its key and garbage: % bytes\n", me[0].name, n);
    } else if r.event == HS_EV_KEYS {
      n := hs_wire_ready_write_vartime(me[0].wire[..], them[0].inbox[them[0].have..<2048]);
      them[0].have += n; them[0].got += n;
      print("  %: has the peer's key; sends its terminator and version packet: % bytes\n", me[0].name, n);
      q := me[0].queued; me[0].queued = HsStep.{};
      send(me, them, q);
    } else if r.event == HS_EV_DECOY { print("  %: a decoy\n", me[0].name); }
    else if r.event == HS_EV_READY { print("  %: the peer's version packet, after its garbage: v2 is up\n", me[0].name); }
    else if r.event == HS_EV_MESSAGE {
      s := hs_receive_vartime(me[0].peer[..], r.command, me[0].inbox[r.payload..<r.payload + r.n], now, none[0..<0]);
      print("  % <- %, % bytes: %", me[0].name, r.command, r.used, VERDICTS[s.verdict]);
      names(s); print("\n");
      send(me, them, s);
      if s.verdict == HS_DISCONNECT { me[0].closed = true; }
    } else {
      why := ifx me[0].wire[0].state == HS_W_V1 then P2P_CODES[r.code] else V2_CODES[r.code];
      print("  %: % (%)\n", me[0].name, ifx r.event == HS_EV_REFUSE then "disconnects" else "drops a message", why);
      if r.event == HS_EV_REFUSE { me[0].closed = true; }
    }
    for i: r.used..<me[0].have { me[0].inbox[i - r.used] = me[0].inbox[i]; }
    me[0].have -= r.used;
  }
  return progress;
}
run :: (a: [] Side, b: [] Side) {
  for round: 0..<20 {
    x := pump(b, a, 1001); y := pump(a, b, 1001);
    if !x && !y { break; }
  }
}
summary :: (s: [] Side) {
  p := s[0].peer[0];
  print("  %: connected %, common version %, wtxidrelay %, addrv2 %, compact blocks % (high bandwidth %), % bytes received\n",
    s[0].name, p.connected, p.common, p.wtxid_relay, p.wants_addrv2, p.provides_cmpct, p.cmpct_high, s[0].got);
}
// One version message from a peer of version and services, to a new
// connection of type conn.
check :: (label: string, conn: int, version: int, services: u64) {
  p: [1] HsPeer; b: [256] u8; none: [1] u64;
  cfg := config(conn, 1, HS_NODE_NETWORK | HS_NODE_WITNESS);
  o := hs_open(p[..], cfg, 1000);
  s := hs_receive_vartime(p[..], "version", hs_their_version(version, services, b[..]), 1001, none[0..<0]);
  print("  %: %", label, VERDICTS[s.verdict]);
  if s.reason != HS_R_NONE { print(" (%)", REASONS[s.reason]); }
  names(s); print("\n");
}
// A message to a peer that has the version but not yet a verack (or, with
// after, has both).
after :: (label: string, verack: bool, command: string, payload: [] u8) {
  p: [1] HsPeer; b: [256] u8; e: [1] u8; none: [1] u64;
  o := hs_open(p[..], config(HS_OUTBOUND, 1, HS_NODE_NETWORK | HS_NODE_WITNESS), 1000);
  s := hs_receive_vartime(p[..], "version", hs_their_version(70017, HS_NODE_NETWORK | HS_NODE_WITNESS, b[..]), 1001, none[0..<0]);
  if verack { s = hs_receive_vartime(p[..], "verack", e[0..<0], 1001, none[0..<0]); }
  s = hs_receive_vartime(p[..], command, payload, 1002, none[0..<0]);
  print("  %: %", label, VERDICTS[s.verdict]);
  if s.reason != HS_R_NONE { print(" (%)", REASONS[s.reason]); }
  names(s); print("\n");
}

main :: () {
  a: [1] Side; b: [1] Side; e: [1] u8;
  print("v1: outbound A, inbound B\n");
  open(a[..], b[..], false, false); run(a[..], b[..]); summary(a[..]); summary(b[..]);

  print("v2: both sides speak BIP-324\n");
  open(a[..], b[..], true, true); run(a[..], b[..]); summary(a[..]); summary(b[..]);
  print("  session id "); show(a[0].wire[0].cipher[0].session_id[..]);
  print(", the same on both sides: %\n", p2p_same(a[0].wire[0].cipher[0].session_id[..], b[0].wire[0].cipher[0].session_id[..]));

  print("v1 A, v2 B: B detects v1\n");
  open(a[..], b[..], false, true); run(a[..], b[..]); summary(a[..]); summary(b[..]);

  print("v2 A, v1-only B: A reconnects in v1\n");
  open(a[..], b[..], true, false); run(a[..], b[..]);
  print("  A sent %, received %: reconnect in v1 %\n", a[0].wire[0].sent, a[0].got, hs_wire_reconnect_v1(a[0].wire[..], a[0].got));
  open(a[..], b[..], false, false); run(a[..], b[..]); summary(a[..]); summary(b[..]);

  print("the version checks, in Core's order\n");
  check("outbound, 70017, NETWORK|WITNESS", HS_OUTBOUND, 70017, HS_NODE_NETWORK | HS_NODE_WITNESS);
  check("outbound, 70017, NETWORK only", HS_OUTBOUND, 70017, HS_NODE_NETWORK);
  check("outbound, 31799, NETWORK|WITNESS", HS_OUTBOUND, 31799, HS_NODE_NETWORK | HS_NODE_WITNESS);
  check("outbound, 31799, NETWORK only", HS_OUTBOUND, 31799, HS_NODE_NETWORK);
  check("outbound, 70017, NETWORK_LIMITED|WITNESS", HS_OUTBOUND, 70017, HS_NODE_NETWORK_LIMITED | HS_NODE_WITNESS);
  check("inbound, 70017, no services", HS_INBOUND, 70017, 0);
  check("block-relay-only, 70017, NETWORK|WITNESS", HS_BLOCK_RELAY, 70017, HS_NODE_NETWORK | HS_NODE_WITNESS);
  check("feeler, 70017, NETWORK|WITNESS", HS_FEELER, 70017, HS_NODE_NETWORK | HS_NODE_WITNESS);
  check("manual, 70015", HS_MANUAL, 70015, 0);
  check("manual, 70012", HS_MANUAL, 70012, 0);
  // Self-connection: A's version arrives at A's own listening socket.
  p: [1] HsPeer; v: [256] u8; none: [1] u64;
  pending := [1] u64.[0x0102030405060708];
  o := hs_open(a[0].peer[..], config(HS_OUTBOUND, pending[0], HS_NODE_NETWORK | HS_NODE_WITNESS), 1000);
  n := hs_payload_write_vartime(a[0].peer[..], HS_SEND_VERSION, v[..]);
  o = hs_open(p[..], config(HS_INBOUND, 99, HS_NODE_NETWORK | HS_NODE_WITNESS), 1000);
  s := hs_receive_vartime(p[..], "version", v[0..<n], 1001, pending[..]);
  print("  inbound, carrying our pending nonce: % (%)\n", VERDICTS[s.verdict], REASONS[s.reason]);

  print("messages around verack\n");
  q: [1] HsPeer; o = hs_open(q[..], config(HS_OUTBOUND, 1, 0), 1000);
  s = hs_receive_vartime(q[..], "ping", v[0..<8], 1001, none[0..<0]);
  print("  ping with no version yet: % (%)\n", VERDICTS[s.verdict], REASONS[s.reason]);
  after("ping before verack", false, "ping", v[0..<8]);
  after("ping after verack", true, "ping", v[0..<8]);
  after("wtxidrelay before verack", false, "wtxidrelay", e[0..<0]);
  after("wtxidrelay after verack", true, "wtxidrelay", e[0..<0]);
  after("sendaddrv2 after verack", true, "sendaddrv2", e[0..<0]);
  after("sendheaders before verack", false, "sendheaders", e[0..<0]);
  cmpct := [9] u8.[1, 2, 0, 0, 0, 0, 0, 0, 0];
  after("sendcmpct 1, version 2, after verack", true, "sendcmpct", cmpct[..]);
  cmpct[1] = 1; after("sendcmpct 1, version 1", true, "sendcmpct", cmpct[..]);
  cmpct[0] = 2; cmpct[1] = 2; after("sendcmpct 2, version 2", true, "sendcmpct", cmpct[..]);
  feature := [7] u8.[4, 't', 'e', 's', 't', 1, 0];
  after("feature \"test\" before verack", false, "feature", feature[..]);
  after("feature \"test\" after verack", true, "feature", feature[..]);
  after("sendtxrcncl before verack", false, "sendtxrcncl", v[0..<12]);
  after("headers after verack", true, "headers", e[0..<0]);
  after("version again", true, "version", hs_their_version(70017, HS_NODE_NETWORK | HS_NODE_WITNESS, v[..]));

  print("timeouts\n");
  // The connection opens at 1000 s; the peer's messages come at 1001 s.
  times := [6] int.[1060, 1061, 1061, 1061, 2201, 2202];
  stages := [6] int.[0, 0, 1, 2, 2, 2];
  STAGES := [3] string.["nothing received", "version only", "connected"];
  for k: 0..<6 {
    w: [1] HsPeer; x: [256] u8;
    o = hs_open(w[..], config(HS_OUTBOUND, 1, 0), 1000);
    if stages[k] >= 1 { s = hs_receive_vartime(w[..], "version", hs_their_version(70017, HS_NODE_NETWORK | HS_NODE_WITNESS, x[..]), 1001, none[0..<0]); }
    if stages[k] >= 2 { s = hs_receive_vartime(w[..], "verack", e[0..<0], 1001, none[0..<0]); }
    t := hs_tick(w[..], times[k], false);
    print("  %, at % s: %", STAGES[stages[k]], times[k] - 1000, VERDICTS[t.verdict]);
    if t.reason != HS_R_NONE { print(" (%)", REASONS[t.reason]); }
    print("\n");
  }
}
```

It prints:

```text
v1: outbound A, inbound B
  B <- version, 121 bytes: done; sends version wtxidrelay sendaddrv2 verack
  A <- version, 121 bytes: done; sends wtxidrelay sendaddrv2 verack getaddr
  A <- wtxidrelay, 24 bytes: done
  A <- sendaddrv2, 24 bytes: done
  A <- verack, 24 bytes: done; sends sendcmpct
  B <- wtxidrelay, 24 bytes: done
  B <- sendaddrv2, 24 bytes: done
  B <- verack, 24 bytes: done; sends sendcmpct
  B <- getaddr, 24 bytes: deliver
  B <- sendcmpct, 33 bytes: done
  A <- sendcmpct, 33 bytes: done
  A: connected true, common version 70017, wtxidrelay true, addrv2 true, compact blocks true (high bandwidth false), 226 bytes received
  B: connected true, common version 70017, wtxidrelay true, addrv2 true, compact blocks true (high bandwidth false), 250 bytes received
v2: both sides speak BIP-324
  A sends its key and 5 bytes of garbage: 69 bytes
  B: not a v1 version; sends its key and garbage: 67 bytes
  B: has the peer's key; sends its terminator and version packet: 36 bytes
  A: has the peer's key; sends its terminator and version packet: 36 bytes
  A: the peer's version packet, after its garbage: v2 is up
  B: the peer's version packet, after its garbage: v2 is up
  B <- version, 130 bytes: done; sends version wtxidrelay sendaddrv2 verack
  ...
```

The full output is in
[bitcoin/fixtures/handshake.out](../fixtures/handshake.out). What it shows:

- In v1, B answers A's `version` with four messages, and A answers B's
  with four of its own. Each side's connection is up when it reads the
  other's `verack`. `getaddr` is the first message delivered to the rest
  of the node.
- In v2, both sides exchange keys first. A's `version` waits in a queue
  until A has B's key, then goes out right after A's version packet. The
  same 11 messages follow as packets, and both sides derive the same
  session ID, the one chapter 67's handshake between keys 1 and 2 found.
- A v1 A meets a v2 B: B sees the 16 bytes of a v1 `version` and carries
  on in v1. The transcript is the v1 one.
- A v2 A meets a v1-only B: B reads a bad message start and disconnects.
  A sent 69 bytes and got none back, so it reconnects in v1, and that
  handshake succeeds.
- The version checks show Core's order: `NETWORK` without `WITNESS` fails
  on services, version 31,799 with both fails on version, and with neither
  the services come first. A limited peer passes because the example's tip
  is current. Common version 70015 gets no `wtxidrelay` or `sendaddrv2`,
  and 70012 also gets the final alert.
- A `version` carrying the nonce of one of our own pending connections is
  a self-connection.
- The three negotiation messages disconnect after `verack`; `sendheaders`
  and `sendcmpct` don't. `sendcmpct` with announce byte 2 disconnects,
  and version 1 is ignored.
- Nothing is checked until 60 seconds have passed. At 61 seconds a peer
  that never spoke, or one that sent `version` but no `verack`, is
  disconnected. A connected peer lasts until 20 minutes of silence.

The book's checks run this program in debug and release builds and
compare the output with the fixture.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/handshake.coax](../examples/checks/handshake.coax),
which loads the example:

- the v1 handshake and the v1 fallback at a v2 responder finish with both
  sides connected, every feature negotiated and no bytes left over
- a `version` frame read from every prefix of itself waits until its last
  byte
- every one of the 16 bytes of the v1 prefix decides: a shorter prefix
  waits, and one changed byte means v2
- `wtxidrelay`, `sendaddrv2` and `feature` are accepted before `verack`
  and disconnect after it

<!-- source: bitcoin/examples/checks/handshake.coax check -->
```jai
// The example's pump without its printing: every event of me's bytes.
quiet :: (me: [] Side, them: [] Side) -> bool {
  progress := false; none: [1] u64;
  while !me[0].closed {
    r := hs_wire_read_vartime(me[0].wire[..], me[0].inbox[0..<me[0].have]);
    if r.event == HS_EV_WAIT { break; }
    progress = true;
    if r.event == HS_EV_MESSAGE {
      s := hs_receive_vartime(me[0].peer[..], r.command, me[0].inbox[r.payload..<r.payload + r.n], 1001, none[0..<0]);
      send(me, them, s);
      if s.verdict == HS_DISCONNECT { me[0].closed = true; }
    } else if r.event != HS_EV_V1 { me[0].closed = true; }
    for i: r.used..<me[0].have { me[0].inbox[i - r.used] = me[0].inbox[i]; }
    me[0].have -= r.used;
  }
  return progress;
}
// v1, and a v1 initiator at a v2 responder, finish with both sides
// connected and every feature negotiated. (The example opens quietly
// when A speaks v1.)
#forall "v1 and the fallback finish the handshake" k: 0..<2 #steps 400000000 {
  a: [1] Side; b: [1] Side;
  open(a[..], b[..], false, false);
  if k == 1 { b[0].wire[0].state = HS_W_DETECT; }
  for round: 0..<20 { x := quiet(b[..], a[..]); y := quiet(a[..], b[..]); if !x && !y { break; } }
  for s: [2] HsPeer.[a[0].peer[0], b[0].peer[0]] {
    #assert s.connected && s.common == HS_PROTOCOL_VERSION && s.wtxid_relay && s.wants_addrv2 && s.provides_cmpct && !s.cmpct_high;
  }
  #assert !a[0].closed && !b[0].closed && a[0].have == 0 && b[0].have == 0;
}
// A version frame read from every prefix of itself: nothing until the
// last byte, then the whole message.
#forall "a frame waits for its last byte" cut: 0..<121 #steps 400000000 {
  w: [1] HsWire; p: [1] HsPeer; e: [1] u8; k: [32] u8; m: [256] u8; v: [256] u8;
  #assert hs_wire_init(w[..], false, false, MAINNET, k[..], k[..], e[0..<0]);
  o := hs_open(p[..], config(HS_OUTBOUND, 1, HS_NODE_NETWORK), 1000);
  n := hs_payload_write_vartime(p[..], HS_SEND_VERSION, v[..]);
  f := hs_wire_write_vartime(w[..], "version", v[0..<n], m[..]);
  #assert f == 121 && hs_wire_read_vartime(w[..], m[0..<cut]).event == HS_EV_WAIT;
  r := hs_wire_read_vartime(w[..], m[0..<f]);
  #assert r.event == HS_EV_MESSAGE && r.command == "version" && r.n == n && r.used == f;
}
// A v1 version is told from v2 by its first 16 bytes: any shorter prefix
// waits, and any one changed byte among them means v2.
#forall "the v1 prefix decides at 16 bytes" at: 0..<16 {
  w: [1] HsWire; e: [1] u8; k: [32] u8; k[31] = 2; m: [64] u8;
  head := [16] u8.[0xf9, 0xbe, 0xb4, 0xd9, 'v', 'e', 'r', 's', 'i', 'o', 'n', 0, 0, 0, 0, 0];
  for i: 0..<16 { m[i] = head[i]; }
  #assert hs_wire_init(w[..], false, false, MAINNET, k[..], k[..], e[0..<0]);
  w[0].state = HS_W_DETECT;                       // a v2 responder, without its key
  #assert hs_wire_read_vartime(w[..], m[0..<at]).event == HS_EV_WAIT;
  m[at] ^= 0x01;
  #assert hs_wire_read_vartime(w[..], m[0..<16]).event == HS_EV_SEND_KEY && w[0].state == HS_W_KEY;
}
// After verack, each of the three negotiation messages disconnects; before
// it, none does.
#forall "negotiation only before verack" k: 0..<3 {
  names := [3] string.["wtxidrelay", "sendaddrv2", "feature"];
  p: [1] HsPeer; b: [256] u8; e: [1] u8; none: [1] u64; f := [6] u8.[4, 't', 'e', 's', 't', 0];
  o := hs_open(p[..], config(HS_MANUAL, 1, 0), 1000);
  s := hs_receive_vartime(p[..], "version", hs_their_version(70017, 0, b[..]), 1001, none[0..<0]);
  payload := ifx k == 2 then f[..] else e[0..<0];
  #assert hs_receive_vartime(p[..], names[k], payload, 1001, none[0..<0]).verdict != HS_DISCONNECT;
  s = hs_receive_vartime(p[..], "verack", e[0..<0], 1001, none[0..<0]);
  s = hs_receive_vartime(p[..], names[k], payload, 1001, none[0..<0]);
  #assert s.verdict == HS_DISCONNECT && s.reason == HS_R_AFTER_VERACK;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/handshake.coax
```

`coax test` runs 270 checks, including the loaded files' own checks, in
about 50 seconds on 2026-11-03.

### The library's own checks

`handshake.coax` is an `#authority` file. Its checks cover the outbound
`version` byte for byte against Core's, the outbound handshake in Core's
order with the redundant and late messages, the version checks in order
(services before version, inbound and manual connections exempt, a limited
peer near and far from the tip), payloads that don't read, the minimum
version before the tail, self-connection, the messages sent by common
version (70012, 70014, 70016 and 70017), the `feature`, `sendcmpct`,
`sendtxrcncl`, `ping` and `feefilter` payloads, every timeout, feeler and
block-relay-only connections, every payload against Core's bytes, a v2
handshake byte by byte with a message split across reads, the v1 fallback
and the v1 retry, v1 frames with a bad checksum and a bad message start,
and v2 garbage with no terminator and a changed packet.

```shell
coax/bin/coax test coax/lib/bitcoin/handshake.coax
```

`coax test` runs 263 checks, including those of the files it loads, in
about 50 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### Bitcoin Core's sources and BIP-324's reference

[handshake.py](../../coax/tests/bitcoin/handshake.py) computes every line
the example prints in Python that shares no code with Coax. It
transliterates `ProcessMessage` from `version` to `sendtxrcncl`,
`CheckIncomingNonce` and `InactivityCheck` from the pinned
`net_processing.cpp` and `net.cpp`, and reads every constant from the
pinned `protocol_version.h`, `protocol.h`, `net.h` and
`net_processing.cpp`. The v2 side runs BIP-324's `reference.py` for the
keys and packets, with `V2Transport`'s receive states transliterated: the
v1 prefix, the key, the garbage search and the first packet's AAD.

[make_handshake_vectors.py](../../coax/tests/bitcoin/make_handshake_vectors.py)
writes Core's bytes for the handshake's payloads into
[handshake_vectors.coax](../../coax/tests/bitcoin/handshake_vectors.coax):
a peer's `version` from Core's `messages.py`, our `version` from
`PushNodeVersion` read back by `messages.py`, the empty messages and
`sendcmpct` from `messages.py`, and the final alert from
`net_processing.cpp`.

```shell
python3 -I coax/tests/bitcoin/handshake.py --coax coax/bin/coax
```

On 2026-11-03 all 102 lines agree, and the example's output in native
debug and release builds equals them. Wasm was not run.

No test here talks to a running Bitcoin Core: the pinned sources and their
Python stand in for it. Chapter 118 runs Core's own functional tests, which drive this handshake, against the node.

## What this chapter leaves out

- Sockets, and choosing whom to connect to: chapter 70.
- Which connections get v2 first: chapter 70 tries v2 when the peer
  advertises `NODE_P2P_V2`.
- When the node sends `sendheaders` and `feefilter`: Core sends them from
  `SendMessages` once its headers are synced, and they belong to chapters
  72 and 83.
- High-bandwidth compact block peers, chosen after the handshake: chapter
  73.
- Transaction reconciliation (BIP-330), which Core ships switched off.
- Private broadcast connections, a newer Core connection type for sending
  one transaction.
- Evicting an inbound peer to make room for a transaction-relaying one:
  chapter 70.

## Summary

- The outbound side sends `version` first; each side answers the other's
  `version` with its feature messages and `verack`, and a side is
  connected when it reads `verack`.
- Core checks a `version` in a fixed order: services, minimum version,
  the optional tail, then self-connection by nonce.
- `wtxidrelay`, `sendaddrv2` and `feature` must come before `verack`; after
  it, they disconnect. `sendheaders` and `sendcmpct` may come at any time.
- A v2 responder tells v1 from v2 by the first 16 bytes, and a v2
  initiator that hears nothing back after 24 bytes retries in v1.
- After 60 seconds, a connection that never spoke or never finished its
  handshake ends; after that, 20 minutes of silence ends it.

## Dependencies

- [65. P2P messages and checksums](ch065-p2p-messages-and-checksums.md)
- [67. The v2 encrypted transport](ch067-the-v2-encrypted-transport.md)

## Sources

- [BIP-130](https://github.com/bitcoin/bips/blob/master/bip-0130.mediawiki)
  (`sendheaders`), [BIP-133](https://github.com/bitcoin/bips/blob/master/bip-0133.mediawiki)
  (`feefilter`), [BIP-152](https://github.com/bitcoin/bips/blob/master/bip-0152.mediawiki)
  (`sendcmpct`), [BIP-155](https://github.com/bitcoin/bips/blob/master/bip-0155.mediawiki)
  (`sendaddrv2`), [BIP-324](https://github.com/bitcoin/bips/blob/master/bip-0324.mediawiki)
  (v1 detection and the v2 handshake) and
  [BIP-339](https://github.com/bitcoin/bips/blob/master/bip-0339.mediawiki)
  (`wtxidrelay`).
- Bitcoin Core at revision `ddea9196`: `node/protocol_version.h`, pinned
  in [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/protocol_version.h),
  and `net_processing.cpp` (`ProcessMessage`, `PushNodeVersion`,
  `GetDesirableServiceFlags`, `SetupAddressRelay`), `net.cpp`
  (`CheckIncomingNonce`, `InactivityCheck`, `V2Transport`), `net.h`,
  `protocol.h` and `test_framework/messages.py`, pinned there by chapter
  65.
- BIP-324's `reference.py`, pinned in
  [coax/tests/vectors/upstream/bip324/](../../coax/tests/vectors/upstream/bip324/reference.py)
  by chapter 66.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
