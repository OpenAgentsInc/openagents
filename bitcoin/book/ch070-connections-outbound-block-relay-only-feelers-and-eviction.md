# 70. Connections: outbound, block-relay-only, feelers and eviction

A node's view of Bitcoin is only as good as its peers. If every peer is
an attacker's, the node sees the attacker's chain and nothing else. So
the choice of whom to connect to, and whom to drop when there's no room,
is a security decision, and Bitcoin Core makes it with care.

Core opens its own connections, called outbound, from addresses in its
address manager (chapter 69). It keeps 8 full-relay connections, which
carry blocks, transactions and addresses, and 2 block-relay-only
connections, which carry only blocks. Block-relay-only peers are harder
to find: they don't relay transactions or addresses, so an observer
can't map them from that traffic. Every couple of minutes it also opens a
short feeler connection to test an address it has never connected to. Other
nodes connect in, called inbound, up to a limit; when the inbound slots
are full, a new connection evicts one, chosen so that an attacker who
opens many connections can't push out the peers that are hardest to
fake.

This chapter builds that connection manager as Core's procedures, over
chapter 69's address manager: the slots, the choice of the next
connection and its address, the random timers, inbound eviction, and the
rules that drop an extra outbound peer.

## What this chapter teaches

- How `-maxconnections` divides into 8 full-relay, 2 block-relay-only
  and 1 feeler slot, with the rest inbound.
- The order in which Core fills its outbound slots: anchors from the
  last run first, then full-relay, then block-relay-only, and then, when
  a random timer fires, an extra block-relay-only peer or a feeler.
- Exponentially distributed timers, and why a feeler waits a random part
  of a second.
- The address loop: up to 100 draws, a different network group for each
  outbound peer, no recent tries, the right services, and no bad ports.
- `SelectNodeToEvict`: which inbound peers are protected, in which order,
  and which peer goes.
- How Core decides whether to accept an inbound connection at all.
- `EvictExtraOutboundPeers`: which extra outbound peer is dropped.

## Where Bitcoin uses connections

| Where | What it uses | Chapter |
| --- | --- | --- |
| The handshake | the connection type, which decides what each side sends | 68 |
| The address manager | `Select`, `Attempt`, `Good` and test-before-evict | 69 |
| Denial of service | discouraged and banned addresses, and limits per peer | 71 |
| Tor and I2P | other reachable networks, and peers that arrive through localhost | 75 |
| The node process | the sockets, and anchors saved to `anchors.dat` at shutdown | 89 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [connman.coax](../../coax/lib/bitcoin/connman.coax) | `cn_limits` | the slots for `-maxconnections` |
| | `cn_exp_delay`, `cn_ln` | `rand_exp_duration` |
| | `cn_bad_port` | `IsBadPort` |
| | `cn_start`, `cn_choose`, `cn_pick`, `cn_step`, `cn_result`, `cn_disconnect` | `ThreadOpenConnections` |
| | `cn_evict_extra_block`, `cn_evict_extra_full`, `cn_check_extra` | `EvictExtraOutboundPeers` |
| | `cn_keyed_group`, `cn_less`, `cn_sort`, `cn_erase_last` | eviction's orders |
| | `cn_protect_by_ratio`, `cn_select_to_evict` | `ProtectEvictionCandidatesByRatio` and `SelectNodeToEvict` |
| | `cn_accept` | `CreateNodeFromAcceptedSocket`'s decision |
| | `cn_random_candidates` | Core's test candidates |
| [addrman.coax](../../coax/lib/bitcoin/addrman.coax) | `am_select`, `am_attempt`, `am_good`, `am_group` | the address manager (chapter 69) |
| [handshake.coax](../../coax/lib/bitcoin/handshake.coax) | `hs_has_desirable` | the services a node wants (chapter 68) |

Everything here is pure and event-driven. The caller says what time it
is and whether a connection succeeded; the procedures say what Core would
do next. Nothing opens a socket. Peers here are simulated.

## Slots

`CConnman::Init` divides `-maxconnections` (default 200 at this
revision) into slots:

<!-- source: coax/lib/bitcoin/connman.coax limits -->
```jai
// The slots for -maxconnections=max (CConnman::Init): 8 full-relay and 2
// block-relay-only outbound connections as far as max allows, 1 feeler
// on top, and the rest inbound.
CnLimits :: struct { max: int; full: int; block: int; feelers: int; inbound: int; }
cn_limits :: (max: int) -> CnLimits {
  full := ifx max < CN_FULL_RELAY then max else CN_FULL_RELAY;
  block := ifx max - full < CN_BLOCK_RELAY then max - full else CN_BLOCK_RELAY;
  inbound := max - (full + block + CN_FEELERS);
  return CnLimits.{max=max, full=full, block=block, feelers=CN_FEELERS, inbound=ifx inbound < 0 then 0 else inbound};
}
```

```text
slots
  -maxconnections=200: 8 full-relay, 2 block-relay-only, 1 feeler, 189 inbound
  -maxconnections=125: 8 full-relay, 2 block-relay-only, 1 feeler, 114 inbound
  -maxconnections=10: 8 full-relay, 2 block-relay-only, 1 feeler, 0 inbound
  -maxconnections=4: 4 full-relay, 0 block-relay-only, 1 feeler, 0 inbound
```

The feeler slot is added on top, so a node with 10 or fewer connections
accepts no inbound peers at all.

## Random timers

Feelers and extra block-relay-only peers run on timers whose delays are
exponentially distributed: the chance that the timer fires in the next
second is the same however long it has waited, so an observer can't
predict it. `rand_exp_duration` takes the top 53 bits of a 64-bit draw as
`x` in [0, 1) and returns `-ln(1 - x)` times the mean. Coax has no
logarithm in its library, so this chapter writes one:

<!-- source: coax/lib/bitcoin/connman.coax timers -->
```jai
// The natural logarithm of y > 0: y = m * 2^e with m near 1, then
// ln m = 2 atanh((m - 1) / (m + 1)) by its series. Doubling and halving
// are exact, and the series' terms fall by 34 times or more each, so 24
// of them leave only rounding.
CN_LN2 :: 0.6931471805599453;
cn_ln :: (y_in: f64) -> f64 {
  y := y_in; e := 0;
  while y > 1.4142135623730951 { y *= 0.5; e += 1; }
  while y < 0.7071067811865476 { y *= 2.0; e -= 1; }
  s := (y - 1.0) / (y + 1.0);
  s2 := s * s;
  term := s; sum := 0.0;
  for k: 0..<24 { sum += term / cast(f64) (2 * k + 1); term *= s2; }
  return cast(f64) e * CN_LN2 + 2.0 * sum;
}
// rand_exp_duration: an exponentially distributed delay with this mean,
// in microseconds. MakeExponentiallyDistributed turns the top 53 bits of
// a 64-bit draw into x in [0, 1) and returns -ln(1 - x); the delay is
// that times the mean, plus half a microsecond, truncated.
cn_exp_delay :: (rng: [] AmRng, mean_us: int) -> int {
  u := am_rand64(rng);
  x := cast(f64) (u >> cast(u64) 11) * (1.0 / 9007199254740992.0);
  return cast(int) (-cn_ln(1.0 - x) * cast(f64) mean_us + 0.5);
}
```

```text
exponential timers
  feeler delays (us): 9911796 82385703 2125678 497529549 72239817
  mean of 2,000 more: 120675 ms (FEELER_INTERVAL is 120 s)
```

The five delays range from 2 seconds to over 8 minutes; their mean is
the 2 minutes of `FEELER_INTERVAL`.

## Bad ports

A peer could fill a node's address manager with the address of somebody
else's mail or SSH server, and turn the network into a port scanner.
Core avoids ports that browsers block and services that answer stray
connections:

<!-- source: coax/lib/bitcoin/connman.coax badport -->
```jai
// IsBadPort (netbase.cpp): ports browsers block and services that answer
// a stray connection, such as 22 (SSH), 6667 (IRC) and 10080 (Amanda).
// The node avoids them for its first 50 draws, so that a peer can't
// point it at somebody else's service.
CN_BAD_PORTS :: [85] int.[1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95,
  101, 102, 103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143, 161, 179, 389, 427, 465,
  512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563, 587, 601, 636, 989, 990, 993, 995, 1719,
  1720, 1723, 2049, 3306, 3389, 3659, 4045, 5060, 5061, 5432, 5900, 6000, 6566, 6665, 6666, 6667, 6668, 6669,
  6697, 10080, 27017];
cn_bad_port :: (port: int) -> bool {
  for p: CN_BAD_PORTS { if p == port { return true; } }
  return false;
}
```

```text
  bad ports among 8333 22 80 443 6667 10080 18333: 22 6667 10080 (85 in all)
```

## The connection manager

The state is the address manager, the thread's random context, the
outbound peers, the anchors and three timers. Anchors are the
block-relay-only peers Core saves to `anchors.dat` at shutdown and tries
first at the next start, so that a restart doesn't hand an attacker a
fresh chance to take every slot:

<!-- source: coax/lib/bitcoin/connman.coax node -->
```jai
// An outbound peer: its id (Core's NodeId, in connection order), its kind
// (HS_OUTBOUND, HS_BLOCK_RELAY), when it connected, when it last gave us
// a block and announced one, and whether it is protected from the
// stale-tip eviction (m_chain_sync.m_protect).
CnPeer :: struct { id: int; conn: int; addr: AmAddr; connected: int; last_block: int; last_announce: int; protect: bool; }
// The connection manager's state: the address manager, the thread's
// random context, the outbound peers (in the caller's memory), the
// anchors saved at the last shutdown, and the three timers in
// microseconds.
CnNode :: struct {
  m: [] AddrMan; rng: [] AmRng; lim: CnLimits;
  peers: [] CnPeer; n_peers: int; next_id: int;
  anchors: [2] AmRecord; n_anchors: int;
  next_feeler: int; next_extra_block: int; next_extra_network: int;
  extra_block_ok: bool;   // m_start_extra_block_relay_peers: set once the node has synced
  tip_depth: int;         // for the desirable services (chapter 68)
}
// ThreadOpenConnections' start: the three timers are drawn first, in this
// order, from the thread's own random context.
cn_start :: (node: [] CnNode, m: [] AddrMan, rng: [] AmRng, peers: [] CnPeer, lim: CnLimits, start_us: int) {
  node[0] = CnNode.{m=m, rng=rng, lim=lim, peers=peers};
  node[0].next_feeler = start_us + cn_exp_delay(rng, CN_FEELER_INTERVAL * 1000000);
  node[0].next_extra_block = start_us + cn_exp_delay(rng, CN_EXTRA_BLOCK_INTERVAL * 1000000);
  node[0].next_extra_network = start_us + cn_exp_delay(rng, CN_EXTRA_NETWORK_INTERVAL * 1000000);
}
cn_add_anchor :: (node: [] CnNode, r: AmRecord) -> bool {
  if node[0].n_anchors >= CN_ANCHORS { return false; }
  node[0].anchors[node[0].n_anchors] = r; node[0].n_anchors += 1;
  return true;
}
cn_count :: (node: [] CnNode, conn: int) -> int {
  n := 0;
  for i: 0..<node[0].n_peers { if node[0].peers[i].conn == conn { n += 1; } }
  return n;
}
// GetGroup equality (chapter 69's network groups).
cn_same_group :: (a: AmAddr, b: AmAddr) -> bool {
  ga: [40] u8; gb: [40] u8;
  x := am_group(a, ga[..]); y := am_group(b, gb[..]);
  if x.count != y.count { return false; }
  for i: 0..<x.count { if x[i] != y[i] { return false; } }
  return true;
}
// Is a's group among the outbound peers' (outbound_ipv46_peer_netgroups)?
cn_group_taken :: (node: [] CnNode, a: AmAddr) -> bool {
  for i: 0..<node[0].n_peers { if cn_same_group(node[0].peers[i].addr, a) { return true; } }
  return false;
}
cn_groups :: (node: [] CnNode) -> int {
  n := 0;
  for i: 0..<node[0].n_peers {
    first := true;
    for j: 0..<i { if cn_same_group(node[0].peers[j].addr, node[0].peers[i].addr) { first = false; } }
    if first { n += 1; }
  }
  return n;
}
// AlreadyConnectedToAddress: the same IP, whatever the port.
cn_connected_to :: (node: [] CnNode, a: AmAddr) -> bool {
  for i: 0..<node[0].n_peers { if am_same_ip(node[0].peers[i].addr, a) { return true; } }
  return false;
}
```

## Choosing the next connection

Every half second, `ThreadOpenConnections` decides what to open next:

<!-- source: coax/lib/bitcoin/connman.coax choose -->
```jai
// Which connection to open next, checked every half second. Anchors come
// first, then the 8 full-relay and 2 block-relay-only slots. With those
// full, a timer may fire: an extra block-relay-only peer about every 5
// minutes once the node has synced (it tests the tip; the stale peer is
// dropped below), else a feeler about every 2 minutes. A fired timer
// draws its next delay at once. -1: nothing to open.
CnChoice :: struct { conn: int; anchor: bool; }
cn_choose :: (node: [] CnNode, now_us: int) -> CnChoice {
  full := cn_count(node, HS_OUTBOUND); block := cn_count(node, HS_BLOCK_RELAY);
  lim := node[0].lim;
  if node[0].n_anchors > 0 && block < lim.block { return CnChoice.{conn=HS_BLOCK_RELAY, anchor=true}; }
  if full < lim.full { return CnChoice.{conn=HS_OUTBOUND}; }
  if block < lim.block { return CnChoice.{conn=HS_BLOCK_RELAY}; }
  if now_us > node[0].next_extra_block && node[0].extra_block_ok {
    node[0].next_extra_block = now_us + cn_exp_delay(node[0].rng, CN_EXTRA_BLOCK_INTERVAL * 1000000);
    return CnChoice.{conn=HS_BLOCK_RELAY};
  }
  if now_us > node[0].next_feeler {
    node[0].next_feeler = now_us + cn_exp_delay(node[0].rng, CN_FEELER_INTERVAL * 1000000);
    return CnChoice.{conn=HS_FEELER};
  }
  // The extra network peer (a full-relay peer on a network the node has
  // none on) needs a reachable network besides IPv4 and IPv6: none here.
  return CnChoice.{conn=-1};
}
```

The extra block-relay-only peer exists to check the tip: if the node's
peers all agree on a chain because an attacker controls them, a new peer
may show a better one. The stale peer is dropped later, by
`EvictExtraOutboundPeers`.

## Picking an address

Then the address loop:

<!-- source: coax/lib/bitcoin/connman.coax pick -->
```jai
// The address loop. Anchors are taken first while asked for. Then up to
// 100 draws from the address manager: a feeler tests a tried collision
// first (marking it good if we're already connected to it), else any new
// address; other connections draw from new and tried. A draw is skipped
// when its group already has an outbound peer (not for feelers), when its
// network isn't reachable, when it was tried in the last 10 minutes
// (only for the first 30 draws), when its services aren't what the
// connection needs, and on a bad port (first 50 draws). No address, or an
// invalid or local one, ends the loop.
CnPick :: struct { found: bool; addr: AmAddr; services: u64; tries: int; anchor: bool; }
cn_pick :: (node: [] CnNode, choice: CnChoice, now: int) -> CnPick {
  m := node[0].m; feeler := choice.conn == HS_FEELER;
  tries := 0;
  while true {
    if choice.anchor && node[0].n_anchors > 0 {
      node[0].n_anchors -= 1;
      r := node[0].anchors[node[0].n_anchors];
      if !am_valid(r.addr) || am_local(r.addr) || !am_in_nets(CN_IPV4_IPV6, am_network(r.addr)) ||
         !hs_has_desirable(r.services, node[0].tip_depth) || cn_group_taken(node, r.addr) { continue; }
      return CnPick.{found=true, addr=r.addr, services=r.services, tries=tries, anchor=true};
    }
    tries += 1;
    if tries > CN_MAX_TRIES { return CnPick.{tries=tries}; }
    e := -1;
    if feeler {
      e = am_select_tried_collision(m);
      if e < 0 { e = am_select(m, true, CN_IPV4_IPV6, now); }
      else if cn_connected_to(node, m[0].info[e].addr) {
        ok := am_good(m, m[0].info[e].addr, now);
        e = am_select(m, true, CN_IPV4_IPV6, now);
      }
    } else {
      e = am_select(m, false, CN_IPV4_IPV6, now);
    }
    if e < 0 { return CnPick.{tries=tries}; }
    x := m[0].info[e];
    if !feeler && cn_group_taken(node, x.addr) { continue; }
    if !am_valid(x.addr) || am_local(x.addr) { return CnPick.{tries=tries}; }
    if !am_in_nets(CN_IPV4_IPV6, am_network(x.addr)) { continue; }
    if now - x.last_try < CN_RECENT_TRY && tries < CN_RECENT_TRIES { continue; }
    if !feeler && !hs_has_desirable(x.services, node[0].tip_depth) { continue; }
    if feeler && (x.services & (HS_NODE_NETWORK | HS_NODE_NETWORK_LIMITED)) == 0 { continue; }
    if tries < CN_BAD_PORT_TRIES && cn_bad_port(x.addr.port) { continue; }
    return CnPick.{found=true, addr=x.addr, services=x.services, tries=tries};
  }
  return CnPick.{tries=tries};
}
```

The group rule is the main defense against an eclipse: an attacker
with many addresses in one /16 gets at most one outbound connection from
them. A feeler connects to a new address only to learn whether it works.
If it does, the address moves to the tried table and becomes eligible
for the full-relay slots.

## One step

<!-- source: coax/lib/bitcoin/connman.coax step -->
```jai
// One pass of ThreadOpenConnections: choose, resolve test-before-evict
// collisions, pick an address. A feeler first sleeps a random part of a
// second (drawn from the same random context, so it moves every later
// timer). The attempt is recorded; failures count only once the node has
// outbound peers in two groups, so that a node that is offline doesn't
// mark its whole table bad.
CnAttempt :: struct { conn: int; pick: CnPick; }
cn_step :: (node: [] CnNode, now_us: int) -> CnAttempt {
  choice := cn_choose(node, now_us);
  if choice.conn < 0 { return CnAttempt.{conn=-1}; }
  now := now_us / 1000000;
  am_resolve_collisions(node[0].m, now);
  p := cn_pick(node, choice, now);
  if !p.found { return CnAttempt.{conn=choice.conn, pick=p}; }
  if choice.conn == HS_FEELER { jitter := am_randrange(node[0].rng, CN_FEELER_WINDOW); }
  limit := ifx node[0].lim.max - 1 < 2 then node[0].lim.max - 1 else 2;
  am_attempt(node[0].m, p.addr, cn_groups(node) >= limit, now);
  return CnAttempt.{conn=choice.conn, pick=p};
}
// The attempt's outcome. A connection that finished its handshake is
// marked good in the address manager; a feeler then disconnects, any
// other becomes an outbound peer. Returns its id, or -1.
cn_result :: (node: [] CnNode, a: CnAttempt, ok: bool, now: int, last_block: int) -> int {
  if !a.pick.found || !ok { return -1; }
  good := am_good(node[0].m, a.pick.addr, now);
  id := node[0].next_id; node[0].next_id += 1;
  if a.conn != HS_FEELER && node[0].n_peers < node[0].peers.count {
    node[0].peers[node[0].n_peers] = CnPeer.{id=id, conn=a.conn, addr=a.pick.addr, connected=now, last_block=last_block};
    node[0].n_peers += 1;
  }
  return id;
}
cn_disconnect :: (node: [] CnNode, id: int) -> bool {
  for i: 0..<node[0].n_peers {
    if node[0].peers[i].id == id {
      for j: i..<node[0].n_peers - 1 { node[0].peers[j] = node[0].peers[j + 1]; }
      node[0].n_peers -= 1;
      return true;
    }
  }
  return false;
}
```

The example runs a node for 15 minutes against a simulated network of
48 /16 groups of 5 addresses. One address in every eleventh group
listens on port 22, a few serve only recent blocks, and an address fails
when its first and third bytes add up to a multiple of 4. The anchor
comes first and fails; the 8 full-relay and 2 block-relay-only slots
fill within 6 seconds, each from a different group:

```text
opening connections for 15 minutes
  first feeler after 14.857494 s, first extra block-relay-only after 311.886434 s
     0 s  block-relay 57.3.3.3:8333 (anchor) after 0 tries: failed
     0 s  full-relay  49.203.4.3:8333 after 1 tries: connected
     1 s  full-relay  47.189.1.1:8333 after 1 tries: failed
     1 s  full-relay  22.14.0.3:8333 after 4 tries: connected
```

The 4 tries at 1 second include draws that the rules above skipped:
an address in a group the node already had, or one it tried less than
10 minutes ago. Then the
timers take over:

```text
    15 s  feeler      65.59.3.1:8333 after 1 tries: failed
    23 s  feeler      60.24.4.2:8333 after 1 tries: failed
    76 s  feeler      38.126.0.1:8333 after 1 tries: connected
```

```text
   312 s  block-relay 47.189.3.1:8333 after 2 tries: connected
   315 s  3 block-relay-only peers: disconnect peer 9
```

After 15 minutes the node has its 10 outbound peers in 10 groups, and
the feelers have moved addresses into the tried table:

```text
  after 15 minutes: 8 full-relay, 2 block-relay-only, 10 groups; addrman 204 new, 19 tried, check 0
```

## Extra outbound peers

Every 45 seconds, `CheckForStaleTipAndEvictPeers` calls
`EvictExtraOutboundPeers`:

<!-- source: coax/lib/bitcoin/connman.coax extra -->
```jai
// EvictExtraOutboundPeers, every 45 seconds. With a block-relay-only peer
// too many, take the two youngest (highest ids, in connection order): the
// younger goes, unless it gave us a block more recently than the other,
// which then goes instead. A peer connected less than 30 s is kept. Core
// also keeps one with blocks in flight; these peers have none.
cn_evict_extra_block :: (peers: [] CnPeer, now: int) -> int {
  young := CnPeer.{id=-1}; next := CnPeer.{id=-1};
  for p: peers {
    if p.conn != HS_BLOCK_RELAY { continue; }
    if p.id > young.id { next = young; young = p; }
  }
  pick := ifx young.last_block > next.last_block then next else young;
  if pick.id >= 0 && now - pick.connected >= CN_MIN_CONNECT { return pick.id; }
  return -1;
}
// With a full-relay peer too many: the one whose last block announcement
// is oldest, the youngest on a tie, never a protected one or the only
// one on its network. Connected more than 30 s, it goes.
cn_evict_extra_full :: (peers: [] CnPeer, now: int) -> int {
  worst := -1;
  for p, i: peers {
    if p.conn != HS_OUTBOUND || p.protect { continue; }
    same := 0;
    for q: peers { if q.conn == HS_OUTBOUND && am_network(q.addr) == am_network(p.addr) { same += 1; } }
    if same < 2 { continue; }
    if worst < 0 || p.last_announce < peers[worst].last_announce ||
       (p.last_announce == peers[worst].last_announce && p.id > peers[worst].id) { worst = i; }
  }
  if worst >= 0 && now - peers[worst].connected > CN_MIN_CONNECT { return peers[worst].id; }
  return -1;
}
// The check on a node: the peer to disconnect, or -1.
cn_check_extra :: (node: [] CnNode, now: int) -> int {
  peers := node[0].peers[0..<node[0].n_peers];
  if cn_count(node, HS_BLOCK_RELAY) > node[0].lim.block {
    id := cn_evict_extra_block(peers, now);
    if id >= 0 { return id; }
  }
  if cn_count(node, HS_OUTBOUND) > node[0].lim.full { return cn_evict_extra_full(peers, now); }
  return -1;
}
```

```text
extra outbound peers
  block-relay-only 4, 7, 12; 12 sent the latest block: disconnect 7
  12 sent no block: disconnect 12
  12 connected 20 s ago: disconnect nobody
  9 full-relay: disconnect 7 (oldest block announcement, then the youngest)
  3 and 7 protected: disconnect 6; the only onion peer is never chosen
```

A new block-relay-only peer that gave us a newer block than the older
one survives; that is how the extra peer finds a better tip and keeps
it. The full-relay rule keeps a node's only peer on a network, so that
the node doesn't lose its one path through Tor.

## Inbound eviction

When all inbound slots are taken, Core evicts a peer to make room. The
rules protect peers by qualities that are expensive to fake in bulk: a
distinct network group, low latency, recent useful transactions and
blocks, and a long uptime. Each candidate carries what eviction knows:

<!-- source: coax/lib/bitcoin/connman.coax candidate -->
```jai
// NodeEvictionCandidate: what eviction knows of an inbound peer. Times
// are seconds (ping in microseconds); group is the keyed netgroup; net is
// its network; local means it came from localhost (a Tor or I2P proxy).
CnCand :: struct {
  id: int; connected: int; ping: int; block: int; tx: int;
  relevant: bool; relay: bool; bloom: bool; group: u64; prefer: bool; local: bool; net: int;
  noban: bool; inbound: bool = true;
}
// CalculateKeyedNetGroup: SipHash, under the node's secret seeds, of the
// randomizer and the address's group, so an attacker can't choose groups
// that sort first.
cn_keyed_group :: (seed0: u64, seed1: u64, a: AmAddr) -> u64 {
  buf: [48] u8;
  stored := cb_store_le64(buf[..], 0, CN_RANDOMIZER_NETGROUP);
  g := am_group(a, buf[8..<48]);
  return siphash24(seed0, seed1, buf[0..<8 + g.count]);
}
```

The group is keyed with the node's secret seeds, so an attacker can't
pick addresses whose groups sort last and get protected.

Each rule sorts the candidates and protects the last few of an order:

<!-- source: coax/lib/bitcoin/connman.coax order -->
```jai
// Eviction's orders, each "a sorts before b". The peers at the end of an
// order are the ones it protects.
CN_BY_GROUP :: 0;     // CompareNetGroupKeyed
CN_BY_PING :: 1;      // ReverseCompareNodeMinPingTime: the fastest last
CN_BY_TX :: 2;        // CompareNodeTXTime: the latest transaction last
CN_BY_BLOCK_ONLY :: 3; // CompareNodeBlockRelayOnlyTime: non-relaying, latest block last
CN_BY_BLOCK :: 4;     // CompareNodeBlockTime: the latest block last
CN_BY_TIME :: 5;      // ReverseCompareNodeTimeConnected: the longest connected last
CN_BY_NET :: 6;       // CompareNodeNetworkTime: the network's peers last, oldest last
// a and b are positions in c, so no peer is copied to compare.
cn_less :: (code: int, c: [] CnCand, a: int, b: int, local: bool, net: int) -> bool {
  if code == CN_BY_GROUP { return c[a].group < c[b].group; }
  if code == CN_BY_PING { return c[a].ping > c[b].ping; }
  if code == CN_BY_TIME { return c[a].connected > c[b].connected; }
  if code == CN_BY_TX {
    if c[a].tx != c[b].tx { return c[a].tx < c[b].tx; }
    if c[a].relay != c[b].relay { return c[b].relay; }
    if c[a].bloom != c[b].bloom { return c[a].bloom; }
    return c[a].connected > c[b].connected;
  }
  if code == CN_BY_BLOCK_ONLY && c[a].relay != c[b].relay { return c[a].relay; }
  if code == CN_BY_BLOCK || code == CN_BY_BLOCK_ONLY {
    if c[a].block != c[b].block { return c[a].block < c[b].block; }
    if c[a].relevant != c[b].relevant { return c[b].relevant; }
    return c[a].connected > c[b].connected;
  }
  if local && c[a].local != c[b].local { return c[b].local; }
  if (c[a].net == net) != (c[b].net == net) { return c[b].net == net; }
  return c[a].connected > c[b].connected;
}
// A stable insertion sort of positions, then one pass that moves each
// peer to its place. Core uses std::sort, which may order equal peers
// either way; ties here keep their order, as in the Python cross-check.
// At most CN_MAX_CANDIDATES peers.
CN_MAX_CANDIDATES :: 1024;
cn_sort :: (c: [] CnCand, code: int, local: bool, net: int) {
  order: [CN_MAX_CANDIDATES] int;
  n := c.count;
  for i: 0..<n {
    x := i; j := i;
    while j > 0 && cn_less(code, c, x, order[j - 1], local, net) { order[j] = order[j - 1]; j -= 1; }
    order[j] = x;
  }
  // order[k] is where the peer for place k is now. Follow each cycle,
  // marking places done with -1.
  for start: 0..<n {
    if order[start] < 0 { continue; }
    held := c[start]; k := start;
    while true {
      from := order[k]; order[k] = -1;
      if from == start { c[k] = held; break; }
      c[k] = c[from]; k = from;
    }
  }
}
```

<!-- source: coax/lib/bitcoin/connman.coax erase -->
```jai
// EraseLastKElements: sort, then protect (remove from the candidates) the
// last k that match: everyone (CN_ALL), non-relaying peers with the
// services we want (CN_NONRELAY), or the peers of one network (CN_ON_NET:
// localhost when local, else net). Returns the new count.
CN_ALL :: 0; CN_NONRELAY :: 1; CN_ON_NET :: 2;
cn_matches :: (pred: int, c: CnCand, local: bool, net: int) -> bool {
  if pred == CN_NONRELAY { return !c.relay && c.relevant; }
  if pred == CN_ON_NET { return ifx local then c.local else c.net == net; }
  return true;
}
cn_erase_last :: (c: [] CnCand, n: int, code: int, k_in: int, pred: int, local: bool, net: int) -> int {
  if n > CN_MAX_CANDIDATES { return n; }
  cn_sort(c[0..<n], code, local, net);
  k := ifx k_in < n then k_in else n;
  w := n - k;
  for i: n - k..<n { if !cn_matches(pred, c[i], local, net) { c[w] = c[i]; w += 1; } }
  return w;
}
```

After the fixed protections, half of the rest are protected: up to a
quarter for peers on networks that connect slowly, so they don't lose
every comparison on uptime, and the rest by uptime:

<!-- source: coax/lib/bitcoin/connman.coax ratio -->
```jai
// ProtectEvictionCandidatesByRatio: half of what is left. Up to a quarter
// goes to peers on networks that are slow to connect from, so they don't
// lose every race on uptime: CJDNS, I2P, localhost and Tor, the network
// with the fewest candidates first, a share each round until the quarter
// is used or nobody qualifies. The rest of the half goes by uptime.
CnNet :: struct { local: bool; net: int; count: int; }
cn_protect_by_ratio :: (c: [] CnCand, n_in: int) -> int {
  n := n_in;
  total := n / 2;
  nets := [4] CnNet.[CnNet.{net=AM_NET_CJDNS}, CnNet.{net=AM_NET_I2P}, CnNet.{local=true, net=CN_NET_LOCAL}, CnNet.{net=AM_NET_ONION}];
  for i: 0..<4 { for j: 0..<n { if cn_matches(CN_ON_NET, c[j], nets[i].local, nets[i].net) { nets[i].count += 1; } } }
  for i: 1..<4 {   // stable, by count
    x := nets[i]; j := i;
    while j > 0 && x.count < nets[j - 1].count { nets[j] = nets[j - 1]; j -= 1; }
    nets[j] = x;
  }
  max_by_net := total / 2;
  protected := 0;
  while protected < max_by_net {
    num := 0;
    for x: nets { if x.count > 0 { num += 1; } }
    if num == 0 { break; }
    per := (max_by_net - protected) / num;
    if per < 1 { per = 1; }
    any := false;
    for i: 0..<4 {
      if nets[i].count == 0 { continue; }
      before := n;
      n = cn_erase_last(c, n, CN_BY_NET, per, CN_ON_NET, nets[i].local, nets[i].net);
      delta := before - n;
      if delta > 0 {
        any = true; protected += delta;
        if protected >= max_by_net { break; }
        nets[i].count -= delta;
      }
    }
    if !any { break; }
  }
  return cn_erase_last(c, n, CN_BY_TIME, total - protected, CN_ALL, false, 0);
}
```

```text
Core's protection cases (12 peers)
  uptime: the 6 longest connected: protected 0 1 2 3 4 5
  3 onion peers: protected 1 3 6 8 9 10
  onion 3, 8-11 and uptime: protected 0 1 2 3 8 9
  localhost 1, 9, 11: protected 1 3 8 9 10 11
  I2P 2, 7, 10, CJDNS 5: protected 0 1 2 3 5 7
```

Half of 12 is 6, and a quarter of 12 is 3. With 3 onion peers, all 3
are protected by network, and uptime protects 3 more. With I2P and CJDNS
both present, the network with fewer peers (CJDNS, 1) goes first, and
each round gives each network an equal share of what is left.

Then the choice:

<!-- source: coax/lib/bitcoin/connman.coax select -->
```jai
// SelectNodeToEvict: the inbound peer to drop for a new one, or -1. The
// candidates are rearranged. Noban and outbound peers are never chosen;
// then each rule protects the peers an attacker finds hardest to fake:
// 4 by keyed netgroup, 8 by ping, 4 that sent transactions last, up to 8
// non-relaying peers that sent blocks last, 4 that sent blocks last, then
// half of the rest by network and uptime. Of who is left (only those
// marked prefer-evict, if any are), the youngest peer of the netgroup
// with the most connections goes. trace gets the count after each step.
cn_select_to_evict :: (c: [] CnCand, trace: [] int) -> int {
  if c.count > CN_MAX_CANDIDATES { return -1; }   // beyond this port's bound: evict nobody
  n := 0;
  for i: 0..<c.count { if !c[i].noban && c[i].inbound { c[n] = c[i]; n += 1; } }
  n = cn_erase_last(c, n, CN_BY_GROUP, 4, CN_ALL, false, 0); trace[0] = n;
  n = cn_erase_last(c, n, CN_BY_PING, 8, CN_ALL, false, 0); trace[1] = n;
  n = cn_erase_last(c, n, CN_BY_TX, 4, CN_ALL, false, 0); trace[2] = n;
  n = cn_erase_last(c, n, CN_BY_BLOCK_ONLY, 8, CN_NONRELAY, false, 0); trace[3] = n;
  n = cn_erase_last(c, n, CN_BY_BLOCK, 4, CN_ALL, false, 0); trace[4] = n;
  n = cn_protect_by_ratio(c, n); trace[5] = n;
  if n == 0 { return -1; }
  any_prefer := false;
  for i: 0..<n { if c[i].prefer { any_prefer = true; } }
  if any_prefer {
    w := 0;
    for i: 0..<n { if c[i].prefer { c[w] = c[i]; w += 1; } }
    n = w;
  }
  trace[6] = n;
  // mapNetGroupNodes, built in candidate order: a group wins with more
  // members, or as many and a first member connected later.
  most := 0; most_time := -1; most_group: u64 = 0;
  for i: 0..<n {
    size := 0; first := -1;
    for j: 0..<i + 1 { if c[j].group == c[i].group { size += 1; if first < 0 { first = j; } } }
    if size > most || (size == most && c[first].connected > most_time) {
      most = size; most_time = c[first].connected; most_group = c[i].group;
    }
  }
  trace[7] = most;
  for i: 0..<n { if c[i].group == most_group { return c[i].id; } }
  return -1;
}
```

```text
inbound eviction: 40 random candidates
  left after netgroup 36, ping 28, tx 24, block-relay-only 21, block 17, ratio 9, prefer-evict 5
  evict peer 10, youngest of the largest group (1 peers)
  0 to 20 random candidates: 21 of 21 evict no one; 29 to 59: 31 of 31 evict one
```

The fixed protections cover up to 28 peers (4, 8, 4, 8 and 4), and then
half of what remains. With 20 candidates or fewer, every one is
protected and nobody is evicted; with 29 or more, someone always is.

An attacker who opens connections from a few groups loses them to each
other, because the group with the most connections gives up its youngest
member:

```text
an attacker fills the inbound slots
  189 slots, 60 honest peers, 329 attacker connections from 3 groups: 0 honest peers evicted, 60 remain
```

## Accepting a connection

Before eviction, Core checks bans and discouragement (chapter 71):

<!-- source: coax/lib/bitcoin/connman.coax accept -->
```jai
// CreateNodeFromAcceptedSocket's decision on a new inbound connection.
// A banned address is dropped and a discouraged one too when at most one
// slot is free, unless it has the noban permission. With the slots full,
// a peer is evicted to make room; if every peer is protected, the new
// one is dropped. scratch holds a copy of the candidates.
CN_ACCEPT :: 0; CN_ACCEPT_EVICTING :: 1; CN_DROP_BANNED :: 2; CN_DROP_DISCOURAGED :: 3; CN_DROP_FULL :: 4;
CnAccept :: struct { code: int; victim: int; }
cn_accept :: (n_inbound: int, max_inbound: int, banned: bool, discouraged: bool, noban: bool, cands: [] CnCand, scratch: [] CnCand) -> CnAccept {
  if banned && !noban { return CnAccept.{code=CN_DROP_BANNED, victim=-1}; }
  if discouraged && !noban && n_inbound + 1 >= max_inbound { return CnAccept.{code=CN_DROP_DISCOURAGED, victim=-1}; }
  if n_inbound < max_inbound { return CnAccept.{code=CN_ACCEPT, victim=-1}; }
  for i: 0..<cands.count { scratch[i] = cands[i]; }
  trace: [8] int;
  victim := cn_select_to_evict(scratch[0..<cands.count], trace[..]);
  if victim < 0 { return CnAccept.{code=CN_DROP_FULL, victim=-1}; }
  return CnAccept.{code=CN_ACCEPT_EVICTING, victim=victim};
}
```

```text
accepting an inbound connection
  188 inbound: accepted
  189 inbound: accepted, evicting peer 361
  full, 20 candidates all protected: dropped (full)
  banned: dropped (banned); with noban: accepted
  discouraged, one slot left: dropped (discouraged); two left: accepted
```

Core's tests draw random candidates the same way:

<!-- source: coax/lib/bitcoin/connman.coax random -->
```jai
// GetRandomNodeEvictionCandidates (test/util/net.cpp): ids 0 to n - 1,
// every other field drawn from rng in Core's order.
cn_random_candidates :: (rng: [] AmRng, out: [] CnCand) {
  for i: 0..<out.count {
    c := CnCand.{id=i};
    c.connected = am_randrange(rng, 100); c.ping = am_randrange(rng, 100);
    c.block = am_randrange(rng, 100); c.tx = am_randrange(rng, 100);
    c.relevant = am_randbool(rng); c.relay = am_randbool(rng); c.bloom = am_randbool(rng);
    c.group = cast(u64) am_randrange(rng, 100); c.prefer = am_randbool(rng); c.local = am_randbool(rng);
    c.net = am_randrange(rng, 7);
    out[i] = c;
  }
}
```

## The example

[bitcoin/examples/connman.coax](../examples/connman.coax) prints every
transcript in this chapter:

<!-- source: bitcoin/examples/connman.coax example -->
```jai
#load "../../coax/lib/bitcoin/connman.coax";
#import "Basic";

NOW :: 1700000000;
GOOD_SERVICES :: HS_NODE_NETWORK | HS_NODE_WITNESS;

// The seed of a random context: 32 copies of one byte.
seeded :: (b: u8) -> AmRng { seed: [32] u8; for i: 0..<32 { seed[i] = b; } return am_rng(seed[..]); }
show :: (a: AmAddr) { print("%.%.%.%:%", a.ip[0], a.ip[1], a.ip[2], a.ip[3], a.port); }
// A number right-aligned in four columns.
pad4 :: (x: int) {
  if x < 1000 { print(" "); } if x < 100 { print(" "); } if x < 10 { print(" "); }
  print("%", x);
}
// Microseconds as seconds with six decimals.
print_secs :: (us: int) {
  print("%.", us / 1000000);
  frac := us % 1000000; div := 100000;
  while div > 0 { print("%", (frac / div) % 10); div /= 10; }
}
who :: (id: int) { if id < 0 { print("nobody"); } else { print("%", id); } }
kind :: (conn: int) -> string {
  if conn == HS_OUTBOUND { return "full-relay "; }
  if conn == HS_BLOCK_RELAY { return "block-relay"; }
  return "feeler     ";
}

slots :: () {
  print("slots\n");
  sizes := [4] int.[CN_MAX_CONNECTIONS, 125, 10, 4];
  for n: sizes {
    l := cn_limits(n);
    print("  -maxconnections=%: % full-relay, % block-relay-only, % feeler, % inbound\n", n, l.full, l.block, l.feelers, l.inbound);
  }
}

timers :: () {
  print("exponential timers\n");
  rng := [1] AmRng.[seeded(3)];
  print("  feeler delays (us):");
  for i: 0..<5 { print(" %", cn_exp_delay(rng[..], CN_FEELER_INTERVAL * 1000000)); }
  total := 0;
  for i: 0..<2000 { total += cn_exp_delay(rng[..], CN_FEELER_INTERVAL * 1000000); }
  print("\n  mean of 2,000 more: % ms (FEELER_INTERVAL is % s)\n", total / 2000 / 1000, CN_FEELER_INTERVAL);
  print("  bad ports among 8333 22 80 443 6667 10080 18333:");
  ports := [7] int.[8333, 22, 80, 443, 6667, 10080, 18333];
  for p: ports { if cn_bad_port(p) { print(" %", p); } }
  print(" (% in all)\n", CN_BAD_PORTS.count);
}

// The simulated network: 48 /16 groups of 5 addresses. One address in
// every eleventh group listens on port 22, one in some groups serves
// only recent blocks, and an address fails when its first and third
// bytes add up to a multiple of 4.
fails :: (a: AmAddr) -> bool { return (cast(int) a.ip[0] + cast(int) a.ip[2]) % 4 == 0; }
outbound :: () {
  print("opening connections for 15 minutes\n");
  new_t: [65536] int; tried_t: [16384] int; info: [4096] AmInfo; random: [4096] int; index: [8192] int;
  scratch: [4096] int; am_rng_: [1] AmRng; m: [1] AddrMan;
  zero: [32] u8; one: [32] u8; one[0] = 1;
  am_rng_[0] = am_rng(zero[..]);
  #assert am_init(m[..], one[..], am_rng_[..], new_t[..], tried_t[..], info[..], random[..], index[..]);
  for g: 0..<48 { for i: 0..<5 {
    services := GOOD_SERVICES;
    if g % 9 == 4 && i == 1 { services = HS_NODE_NETWORK_LIMITED; }
    a := am_v4(20 + g, (g * 7) % 256, i, 1 + g % 3, ifx g % 11 == 0 && i == 0 then 22 else 8333);
    r := [1] AmRecord.[AmRecord.{addr=a, services=services, time=NOW - 3600}];
    added := am_add(m[..], r[..], am_v4(99, 99, 1, 1, 0), 0, NOW);
  } }

  node: [1] CnNode; peers: [16] CnPeer; rng := [1] AmRng.[seeded(5)];
  start := NOW * 1000000;
  cn_start(node[..], m[..], rng[..], peers[..], cn_limits(CN_MAX_CONNECTIONS), start);
  stored := cn_add_anchor(node[..], AmRecord.{addr=am_v4(57, 3, 3, 3, 8333), services=GOOD_SERVICES});
  print("  first feeler after "); print_secs(node[0].next_feeler - start);
  print(" s, first extra block-relay-only after "); print_secs(node[0].next_extra_block - start); print(" s\n");

  next_check := NOW + CN_CHECK_INTERVAL;
  for step: 0..<15 * 60 * 2 {
    now_us := start + step * 500000; now := now_us / 1000000;
    if now >= next_check {
      next_check += CN_CHECK_INTERVAL;
      blocks := cn_count(node[..], HS_BLOCK_RELAY);
      id := cn_check_extra(node[..], now);
      if id >= 0 {
        gone := cn_disconnect(node[..], id);
        print("  "); pad4(now - NOW); print(" s  % block-relay-only peers: disconnect peer %\n", blocks, id);
      }
    }
    node[0].extra_block_ok = now >= NOW + 60;   // "synced" after a minute
    a := cn_step(node[..], now_us);
    if a.conn < 0 { continue; }
    print("  "); pad4(now - NOW);
    if !a.pick.found { print(" s  %: nothing after % tries\n", kind(a.conn), a.pick.tries); continue; }
    ok := !fails(a.pick.addr);
    print(" s  % ", kind(a.conn)); show(a.pick.addr);
    print("% after % tries: %\n", ifx a.pick.anchor then " (anchor)" else "", a.pick.tries, ifx ok then "connected" else "failed");
    n := node[0].next_id;
    id := cn_result(node[..], a, ok, now, ifx n % 3 == 0 then 0 else now + (n * 37) % 300);
  }
  print("  after 15 minutes: % full-relay, % block-relay-only, % groups; addrman % new, % tried, check %\n",
    cn_count(node[..], HS_OUTBOUND), cn_count(node[..], HS_BLOCK_RELAY), cn_groups(node[..]),
    am_size(m[..], -1, 1), am_size(m[..], -1, 0), am_check(m[..], scratch[..]));
}

// The protection cases, set up as in Core's tests.
setup :: (k: int, c: CnCand) -> CnCand {
  x := c; id := c.id;
  x.local = false; x.net = AM_NET_IPV4;
  if k == 0 { x.connected = id; }
  if k == 1 { if id == 3 || id == 8 || id == 9 { x.net = AM_NET_ONION; } }
  if k == 2 { x.connected = id; x.net = ifx id == 3 || id > 7 then AM_NET_ONION else AM_NET_IPV6; }
  if k == 3 { x.local = id == 1 || id == 9 || id == 11; }
  if k == 4 {
    x.connected = id;
    if id == 2 || id == 7 || id == 10 { x.net = AM_NET_I2P; }
    if id == 5 { x.net = AM_NET_CJDNS; }
  }
  return x;
}

inbound :: () {
  print("inbound eviction: 40 random candidates\n");
  rng := [1] AmRng.[seeded(4)]; r := rng[..];
  c: [200] CnCand; trace: [8] int;
  cn_random_candidates(r, c[0..<40]);
  victim := cn_select_to_evict(c[0..<40], trace[..]);
  print("  left after netgroup %, ping %, tx %, block-relay-only %, block %, ratio %, prefer-evict %\n",
    trace[0], trace[1], trace[2], trace[3], trace[4], trace[5], trace[6]);
  print("  evict peer %, youngest of the largest group (% peers)\n", victim, trace[7]);
  small := 0; large := 0;
  for n: 0..<21 { cn_random_candidates(r, c[0..<n]); if cn_select_to_evict(c[0..<n], trace[..]) < 0 { small += 1; } }
  for n: 29..<60 { cn_random_candidates(r, c[0..<n]); if cn_select_to_evict(c[0..<n], trace[..]) >= 0 { large += 1; } }
  print("  0 to 20 random candidates: % of 21 evict no one; 29 to 59: % of 31 evict one\n", small, large);

  print("Core's protection cases (12 peers)\n");
  labels := [5] string.["uptime: the 6 longest connected", "3 onion peers", "onion 3, 8-11 and uptime", "localhost 1, 9, 11", "I2P 2, 7, 10, CJDNS 5"];
  for k: 0..<5 {
    cn_random_candidates(r, c[0..<12]);
    for i: 0..<12 { c[i] = setup(k, c[i]); }
    left := cn_protect_by_ratio(c[0..<12], 12);
    print("  %: protected", labels[k]);
    for id: 0..<12 {
      kept := false;
      for i: 0..<left { if c[i].id == id { kept = true; } }
      if !kept { print(" %", id); }
    }
    print("\n");
  }

  print("an attacker fills the inbound slots\n");
  max_in := cn_limits(CN_MAX_CONNECTIONS).inbound;
  peers: [256] CnCand; scratch: [256] CnCand; n := 0;
  for i: 0..<60 {
    p := CnCand.{id=i, connected=NOW - 86400 + i * 600, ping=20000 + (i * 7919) % 180000, relevant=true, relay=i % 5 != 0, net=AM_NET_IPV4};
    if i % 4 == 0 { p.block = NOW - 600 * (i % 10); }
    if i % 3 == 0 { p.tx = NOW - 60 * (i % 13); }
    p.group = cn_keyed_group(11, 22, am_v4(30 + i, i * 3 % 256, 1, 1, 8333));
    peers[n] = p; n += 1;
  }
  attackers := max_in - 60 + 200; lost := 0;
  for k: 0..<attackers {
    if n >= max_in {
      for i: 0..<n { scratch[i] = peers[i]; }
      v := cn_select_to_evict(scratch[0..<n], trace[..]);
      if v < 60 { lost += 1; }
      w := 0;
      for i: 0..<n { if peers[i].id != v { peers[w] = peers[i]; w += 1; } }
      n = w;
    }
    peers[n] = CnCand.{id=60 + k, connected=NOW + k, ping=5000, relevant=true, relay=true, net=AM_NET_IPV4,
                       group=cn_keyed_group(11, 22, am_v4(66, k % 3, k / 250 % 256, 1 + k % 250, 8333))};
    n += 1;
  }
  honest := 0;
  for i: 0..<n { if peers[i].id < 60 { honest += 1; } }
  print("  % slots, 60 honest peers, % attacker connections from 3 groups: % honest peers evicted, % remain\n", max_in, attackers, lost, honest);

  print("accepting an inbound connection\n");
  few: [20] CnCand;
  cn_random_candidates(r, few[..]);
  all := peers[0..<n];
  print("  % inbound: ", max_in - 1); decision(cn_accept(max_in - 1, max_in, false, false, false, all, scratch[..]));
  print("\n  % inbound: ", max_in); decision(cn_accept(max_in, max_in, false, false, false, all, scratch[..]));
  print("\n  full, 20 candidates all protected: "); decision(cn_accept(20, 20, false, false, false, few[..], scratch[..]));
  print("\n  banned: "); decision(cn_accept(5, max_in, true, false, false, all, scratch[..]));
  print("; with noban: "); decision(cn_accept(5, max_in, true, false, true, all, scratch[..]));
  print("\n  discouraged, one slot left: "); decision(cn_accept(max_in - 1, max_in, false, true, false, all, scratch[..]));
  print("; two left: "); decision(cn_accept(max_in - 2, max_in, false, true, false, all, scratch[..]));
  print("\n");
}
decision :: (a: CnAccept) {
  if a.code == CN_ACCEPT { print("accepted"); }
  if a.code == CN_ACCEPT_EVICTING { print("accepted, evicting peer %", a.victim); }
  if a.code == CN_DROP_BANNED { print("dropped (banned)"); }
  if a.code == CN_DROP_DISCOURAGED { print("dropped (discouraged)"); }
  if a.code == CN_DROP_FULL { print("dropped (full)"); }
}

extra :: () {
  print("extra outbound peers\n");
  t := NOW + 3600;
  p := [3] CnPeer.[CnPeer.{id=4, conn=HS_BLOCK_RELAY, connected=t - 900, last_block=t - 100},
                   CnPeer.{id=7, conn=HS_BLOCK_RELAY, connected=t - 600, last_block=t - 50},
                   CnPeer.{id=12, conn=HS_BLOCK_RELAY, connected=t - 60, last_block=t - 10}];
  print("  block-relay-only 4, 7, 12; 12 sent the latest block: disconnect "); who(cn_evict_extra_block(p[..], t));
  p[2].last_block = 0;
  print("\n  12 sent no block: disconnect "); who(cn_evict_extra_block(p[..], t));
  p[2].connected = t - 20;
  print("\n  12 connected 20 s ago: disconnect "); who(cn_evict_extra_block(p[..], t));
  onion: [32] u8; f: [9] CnPeer;
  for i: 0..<8 { f[i] = CnPeer.{id=i, conn=HS_OUTBOUND, addr=am_v4(20 + i, 1, 1, 1, 8333), connected=t - 3000 + i, last_announce=t - 100 * (i % 4)}; }
  f[8] = CnPeer.{id=20, conn=HS_OUTBOUND, addr=am_bytes(AM_NET_ONION, onion[..], 8333), connected=t - 40, last_announce=t - 5};
  print("\n  9 full-relay: disconnect "); who(cn_evict_extra_full(f[..], t));
  print(" (oldest block announcement, then the youngest)\n");
  f[3].protect = true; f[7].protect = true;
  print("  3 and 7 protected: disconnect "); who(cn_evict_extra_full(f[..], t));
  print("; the only onion peer is never chosen\n");
}

main :: () {
  slots();
  timers();
  outbound();
  inbound();
  extra();
}
```

```shell
coax/bin/coax run bitcoin/examples/connman.coax
```

Its output, 67 lines, is
[bitcoin/fixtures/connman.out](../fixtures/connman.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/connman.coax](../examples/checks/connman.coax),
which loads the example. Eviction never picks a noban or outbound peer,
and nobody when 20 or fewer can be chosen; the ratio protection always
leaves the larger half; the logarithm turns products into sums; and a
banned address gets in only with noban:

<!-- source: bitcoin/examples/checks/connman.coax check -->
```jai
// Whatever the candidates, the peer chosen for eviction is an inbound
// peer without noban, and with 20 candidates or fewer nobody is chosen.
#forall "eviction never picks a noban or outbound peer" k: 0..<60 #steps 2000000000 {
  rng := [1] AmRng.[seeded(cast,trunc(u8) (k + 1))];
  n := 10 + k * 2;
  c: [200] CnCand; saved: [200] CnCand; trace: [8] int;
  cn_random_candidates(rng[..], c[0..<n]);
  for i: 0..<n { c[i].noban = am_randrange(rng[..], 5) == 0; c[i].inbound = am_randrange(rng[..], 4) != 0; saved[i] = c[i]; }
  eligible := 0;
  for i: 0..<n { if !c[i].noban && c[i].inbound { eligible += 1; } }
  v := cn_select_to_evict(c[0..<n], trace[..]);
  if eligible <= 20 { #assert v < 0; }
  if v >= 0 { #assert v < n && !saved[v].noban && saved[v].inbound; }
}
// ProtectEvictionCandidatesByRatio always leaves the larger half.
#forall "the ratio protection leaves n - n/2" n: 0..<120 #steps 400000000 {
  rng := [1] AmRng.[seeded(cast,trunc(u8) n)];
  c: [120] CnCand;
  cn_random_candidates(rng[..], c[0..<n]);
  #assert cn_protect_by_ratio(c[0..<n], n) == n - n / 2;
}
// The logarithm turns products into sums, and delays are never negative.
#forall "ln(xy) = ln x + ln y; delays are non-negative" k: 1..<200 #steps 40000000 {
  x := cast(f64) k / 37.0; y := 1.0 / cast(f64) (k * 3 + 1);
  d := cn_ln(x * y) - cn_ln(x) - cn_ln(y);
  #assert d < 0.000000000000005 && d > -0.000000000000005;
  rng := [1] AmRng.[seeded(cast,trunc(u8) k)];
  #assert cn_exp_delay(rng[..], CN_FEELER_INTERVAL * 1000000) >= 0;
}
// A banned address gets in only with noban, whatever the slots.
#forall "a banned address needs noban" k: 0..<40 {
  c: [1] CnCand; s: [1] CnCand;
  #assert cn_accept(k, 20, true, k % 2 == 0, false, c[0..<0], s[..]).code == CN_DROP_BANNED;
  #assert cn_accept(k, 20, true, false, true, c[0..<0], s[..]).code != CN_DROP_BANNED;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/connman.coax
```

`coax test` runs 300 checks, including the loaded files' own checks, in
about 2 minutes on 2026-11-03.

### The library's own checks

`connman.coax` is an `#authority` file. Its checks port Bitcoin Core's
`src/test/net_peer_eviction_tests.cpp`: 16 cases of
`peer_protection_test` (uptime, onion, localhost, I2P and CJDNS peers,
and the combined cases) and all of `peer_eviction_test` for 0 to 199
candidates (4 protected by netgroup, 8 by ping, 4 by transaction time,
up to 8 non-relaying peers by block time, 4 by block time, 12 and 20
combined, and someone evicted from 29 candidates, nobody from 20). Core
shuffles the candidates and runs every count from one generator; here
each count has its own seed and keeps the generated order, and every
outcome checked is one Core's test fixes for any order. The checks also
cover the slots, the logarithm, `IsBadPort`, noban and outbound peers,
prefer-evict, accepting inbound peers, the keyed netgroup,
`EvictExtraOutboundPeers`, and a node that fills its slots from an
anchor, takes a feeler and an extra block-relay-only peer, and drops it.

```shell
coax/bin/coax test coax/lib/bitcoin/connman.coax
```

`coax test` runs 296 checks, including those of the files it loads, in
about 3 minutes on 2026-11-03. Mutation testing: not run (owner speed
order).

### Bitcoin Core's code

[connman.py](../../coax/tests/bitcoin/connman.py) computes every line
the example prints. It transliterates `SelectNodeToEvict` and
`ProtectEvictionCandidatesByRatio` from the pinned `node/eviction.cpp`,
the connection choice and address loop of `ThreadOpenConnections` from
`net.cpp`, `IsBadPort` from `netbase.cpp`, `rand_exp_duration` from
`random.h` and `random.cpp`, `GetRandomNodeEvictionCandidates` from
`test/util/net.cpp`, and `EvictExtraOutboundPeers` from
`net_processing.cpp`, over chapter 69's transliterated address manager.
It reads the constants and the bad ports from the pinned sources, and
compares its transcript with the fixture:

```shell
python3 -I coax/tests/bitcoin/connman.py --coax coax/bin/coax
```

On 2026-11-03 all 67 lines agree, and the example's output in native
debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Sockets, the network thread and `anchors.dat`: chapter 89.
- Manual connections (`-addnode`, `-connect`), seed nodes and
  `addr-fetch` connections.
- The extra network peer, which needs a reachable network besides IPv4
  and IPv6 (chapter 75), and `-onlynet`.
- Blocks in flight, which keep an outbound peer from eviction; the
  simulated peers have none.
- The stale-tip check that sets `GetTryNewOutboundPeer` and protects
  peers with good chains (`m_chain_sync`).
- Ban scores and discouragement themselves: chapter 71.

## Summary

- 200 connections are 8 full-relay, 2 block-relay-only, 1 feeler and 189
  inbound.
- Outbound connections come from the address manager, one per network
  group, never to an address tried in the last 10 minutes or on a bad
  port while there are other choices.
- Feelers about every 2 minutes test new addresses; an extra
  block-relay-only peer about every 5 minutes tests the tip, and the
  younger of the two newest is dropped unless it brought a newer block.
- When the inbound slots are full, peers are protected by netgroup,
  ping, transactions, blocks, network and uptime, and the youngest peer
  of the largest group goes. 20 peers or fewer are never evicted.

## Dependencies

- [68. The handshake and feature negotiation](ch068-the-handshake-and-feature-negotiation.md)
- [69. Addresses and the address manager](ch069-addresses-and-the-address-manager.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `node/eviction.cpp`,
  `node/eviction.h`, `netbase.cpp`, `random.cpp`, `test/util/net.cpp`
  and `test/net_peer_eviction_tests.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/eviction.cpp)
  by this chapter; `net.cpp` (`ThreadOpenConnections`,
  `CreateNodeFromAcceptedSocket`), `net.h`, `net_processing.cpp`
  (`EvictExtraOutboundPeers`), `random.h`, `addrman.cpp` and
  `netgroup.cpp`, pinned there by earlier chapters.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
