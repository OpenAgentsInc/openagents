# 75. Proxies, Tor and I2P

A node that dials its peers directly tells each of them, and everyone
watching its connection, its IP address. A transaction it relays first
can then be traced to that address. Bitcoin Core can instead reach its
peers through Tor or I2P, two networks built to hide who talks to whom,
and through any SOCKS5 proxy. It can also take connections on a Tor onion
service or an I2P destination, so that peers reach it without knowing its
address.

Those peers have addresses of their own kinds. A Tor v3 address is an
onion service's public key with a checksum, written in base32. An I2P
address is the SHA-256 of a destination, also in base32. Neither fits
the 16 bytes of the old `addr` message, which is why BIP-155's `addrv2`
exists (chapters 65 and 69).

This chapter builds what Core does with these networks, as pure
procedures: reading and writing the addresses, the SOCKS5 exchange Core
has with Tor, the SAM commands it sends to an I2P router, and the
options that decide which networks the node reaches and how. Nothing
here opens a socket or talks to a real Tor or I2P. The caller passes in
each reply, and the procedures say what Core would send next.

## What this chapter teaches

- The Tor v3 onion address: the 32-byte ed25519 key, a two-byte SHA3-256
  checksum and a version byte, in base32, and how BIP-155 carries the
  key alone.
- The I2P address: the SHA-256 of a destination, in unpadded base32 with
  `.b32.i2p`, and how a destination's length is read from its
  certificate.
- Core's base32 and its strict padding rules.
- SOCKS5 (RFC 1928) with username and password (RFC 1929), as Core uses
  it: always a host name, never an address, so that the proxy resolves
  it.
- Tor stream isolation: new credentials for each connection, which Tor
  puts on its own circuit.
- I2P's SAM v3.1 commands for a session, an outgoing stream and an
  incoming one, and what each reply means.
- How `-onlynet`, `-proxy`, `-onion`, `-listenonion` and `-i2psam` decide
  which networks are reachable, which ones go through a proxy, and which
  combinations Core refuses.

## Where Bitcoin uses this

| Where | What it uses | Chapter |
| --- | --- | --- |
| `addrv2` messages | BIP-155's network IDs 4 (Tor v3) and 5 (I2P), each with 32 bytes | 65 |
| The address manager | Tor and I2P addresses, kept and grouped by network | 69 |
| Outbound connections | the route to each address: direct, through a proxy, or through the SAM bridge | 70 |
| Addresses a node announces | its own onion or I2P address, when it takes connections there | 69, 89 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [proxy.coax](../../coax/lib/bitcoin/proxy.coax) | `px_b32_encode`, `px_b32_decode` | Core's `EncodeBase32` and `DecodeBase32` |
| | `px_onion_checksum`, `px_onion_text` | the Tor v3 checksum and `OnionToString` |
| | `px_set_special`, `px_set_tor`, `px_set_i2p`, `px_addr_text` | `CNetAddr::SetSpecial` and `ToStringAddr` |
| | `px_socks_begin`, `px_socks_read`, `px_socks_reply_text` | Core's SOCKS5 client, `Socks5` |
| | `px_isolation` | `TorStreamIsolationCredentialsGenerator` |
| | `px_proxy_read`, `px_proxy_answer`, `px_socks_pump` | a SOCKS5 proxy to test the client against |
| | `px_i2p_decode`, `px_i2p_encode`, `px_i2p_dest_len`, `px_i2p_addr` | I2P's base64, `MyDestination` and `DestBinToAddr` |
| | `px_sam_parse`, `px_sam_get`, `px_sam_begin`, `px_sam_read` | `i2p::sam::Session` |
| | `px_configure`, `px_configure_text`, `px_route` | `AppInitMain`'s network options and `ConnectNode`'s route |
| [sha3.coax](../../coax/lib/crypto/sha3.coax) | `sha3_256` | SHA3-256 (FIPS 202), for the onion checksum |
| [addrman.coax](../../coax/lib/bitcoin/addrman.coax) | `am_put_net`, `am_bip155` | BIP-155's network ID and bytes (chapter 69) |
| [base64.coax](../../coax/lib/nostr/base64.coax) | `base64_encode_checked`, `base64_decode_checked` | RFC 4648 base64 |

Addresses, proxy replies and SAM lines are public, so these procedures
run in variable time. The credentials for stream isolation are not
secret either: they only need to differ between connections.

The constants come from Core's sources and the specifications:

<!-- source: coax/lib/bitcoin/proxy.coax constants -->
```jai
PX_SOCKS5 :: 0x05;           // SOCKSVersion::SOCKS5
PX_NOAUTH :: 0x00;           // SOCKS5Method::NOAUTH
PX_USER_PASS :: 0x02;        // SOCKS5Method::USER_PASS (RFC 1929)
PX_NO_ACCEPTABLE :: 0xff;    // SOCKS5Method::NO_ACCEPTABLE
PX_CONNECT :: 0x01;          // SOCKS5Command::CONNECT
PX_ATYP_IPV4 :: 0x01; PX_ATYP_DOMAIN :: 0x03; PX_ATYP_IPV6 :: 0x04;
PX_MAX_NAME :: 255;          // a SOCKS5 host name, user name or password
PX_TOR_SOCKS_PORT :: 9050;   // DEFAULT_TOR_SOCKS_PORT (torcontrol.h)
PX_SAM_PORT :: 7656;         // -i2psam's default port (init.cpp)
PX_I2P_PORT :: 0;            // I2P_SAM31_PORT: SAM 3.1 streams have no ports
PX_PREFIX_BYTES :: 8;        // TorStreamIsolationCredentialsGenerator::PREFIX_BYTE_LENGTH
PX_ONION_CHECKSUM :: 2;      // torv3::CHECKSUM_LEN
PX_ONION_VERSION :: 3;       // torv3::VERSION
PX_ONION_TEXT :: 62;         // 56 base32 characters and ".onion"
PX_I2P_TEXT :: 60;           // 52 base32 characters and ".b32.i2p"
PX_DEST_BASE :: 387;         // MyDestination: DEST_LEN_BASE ...
PX_CERT_LEN_AT :: 385;       // ... and CERT_LEN_POS
PX_B64_MAX :: 2048;          // the longest I2P base64 these procedures take
PX_SAM_KEYS :: 16;           // the most key=value pairs read from one reply
```

## Base32

Both networks write their addresses in base32: RFC 4648's alphabet of 26
letters and the digits 2 to 7, five bits to a character. Core writes it
in lower case and reads either case. Its reader wants a multiple of 8
characters, of which up to 6 at the end may be padding. After the
padding, the bits left over must be fewer than 5 and all zero, so each
byte string has exactly one spelling in each case.

<!-- source: coax/lib/bitcoin/proxy.coax base32 -->
```jai
// Core's base32 (RFC 4648's alphabet in lower case). EncodeBase32 pads
// with '=' to a multiple of 8 characters unless told not to.
px_b32_encode :: (input: [] u8, pad: bool, out: [] u8) -> int {
  alphabet := "abcdefghijklmnopqrstuvwxyz234567";
  acc := 0; bits := 0; n := 0;
  for b: input {
    acc = ((acc << 8) | cast(int) b) & 0xfff; bits += 8;
    while bits >= 5 { bits -= 5; n = px_put_byte(out, n, cast(int) alphabet[(acc >> bits) & 31]); }
  }
  if bits > 0 { n = px_put_byte(out, n, cast(int) alphabet[(acc << (5 - bits)) & 31]); }
  while pad && n >= 0 && n % 8 != 0 { n = px_put_byte(out, n, '='); }
  return n;
}
// decode32_table: either case decodes.
px_b32_digit :: (c: u8) -> int {
  if c >= 'a' && c <= 'z' { return cast(int) (c - 'a'); }
  if c >= 'A' && c <= 'Z' { return cast(int) (c - 'A'); }
  if c >= '2' && c <= '7' { return cast(int) (c - '2') + 26; }
  return -1;
}
// DecodeBase32: a multiple of 8 characters, of which 1, 3, 4 or 6 at the
// end may be '='. ConvertBits<5, 8, false> then refuses any other
// character, 5 or more bits left over, or left-over bits that aren't 0.
// Returns the number of bytes, or -1.
px_b32_decode :: (text: string, out: [] u8) -> int {
  if text.count % 8 != 0 { return -1; }
  n := text.count;
  if n >= 1 && text[n - 1] == '=' { n -= 1; }
  if n >= 2 && text[n - 2] == '=' && text[n - 1] == '=' { n -= 2; }
  if n >= 1 && text[n - 1] == '=' { n -= 1; }
  if n >= 2 && text[n - 2] == '=' && text[n - 1] == '=' { n -= 2; }
  acc := 0; bits := 0; w := 0;
  for i: 0..<n {
    v := px_b32_digit(text[i]);
    if v < 0 { return -1; }
    acc = ((acc << 5) | v) & 0xfff; bits += 5;
    if bits >= 8 { bits -= 8; w = px_put_byte(out, w, (acc >> bits) & 255); if w < 0 { return -1; } }
  }
  if bits >= 5 || ((acc << (8 - bits)) & 255) != 0 { return -1; }
  return w;
}
```

## Tor v3 onion addresses

An onion service is named by its ed25519 public key. Tor's rend-spec
appends a two-byte checksum and a version byte, 3, and writes the 35
bytes in base32: 56 characters, then `.onion`. The checksum is the start
of a SHA3-256 hash, so a mistyped character is caught with probability
1 - 2^-16. SHA3-256 is new to this book; it is Keccak with a 136-byte
rate and the domain bits `01`:

<!-- source: coax/lib/crypto/sha3.coax keccak -->
```jai
SHA3_RC :: [24] u64.[
  0x0000000000000001, 0x0000000000008082, 0x800000000000808a, 0x8000000080008000,
  0x000000000000808b, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
  0x000000000000008a, 0x0000000000000088, 0x0000000080008009, 0x000000008000000a,
  0x000000008000808b, 0x800000000000008b, 0x8000000000008089, 0x8000000000008003,
  0x8000000000008002, 0x8000000000000080, 0x000000000000800a, 0x800000008000000a,
  0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008];
// The rotation of lane x + 5y in the rho step.
SHA3_RHO :: [25] int.[0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56, 14];
sha3_rotl :: (x: u64, n: int) -> u64 {
  if n == 0 { return x; }
  return (x << n) | (x >> (64 - n));
}
// Keccak-f[1600]: 24 rounds of theta, rho, pi, chi and iota.
sha3_permute :: (a: [] u64) {
  c: [5] u64; b: [25] u64;
  for round: 0..<24 {
    for x: 0..<5 { c[x] = a[x] ^ a[x + 5] ^ a[x + 10] ^ a[x + 15] ^ a[x + 20]; }
    for x: 0..<5 {
      d := c[(x + 4) % 5] ^ sha3_rotl(c[(x + 1) % 5], 1);
      for y: 0..<5 { a[x + 5 * y] ^= d; }
    }
    for x: 0..<5 { for y: 0..<5 { b[y + 5 * ((2 * x + 3 * y) % 5)] = sha3_rotl(a[x + 5 * y], SHA3_RHO[x + 5 * y]); } }
    for x: 0..<5 { for y: 0..<5 {
      a[x + 5 * y] = b[x + 5 * y] ^ ((b[(x + 1) % 5 + 5 * y] ^ 0xffffffffffffffff) & b[(x + 2) % 5 + 5 * y]);
    } }
    a[0] ^= SHA3_RC[round];
  }
}
// The padded message's byte at: SHA-3's domain bits 01, then pad10*1.
sha3_padded :: (input: string, at: int, total: int) -> u8 {
  if at < input.count { return input[at]; }
  v: u8 = 0;
  if at == input.count { v = 0x06; }
  if at == total - 1 { v |= 0x80; }
  return v;
}
// SHA3-256 of input into out[0..<32]; false (out untouched) when input is
// over SHA3_MAX_INPUT or out is shorter than 32.
sha3_256 :: (input: string, out: [] u8) -> bool #export_c {
  if input.count > SHA3_MAX_INPUT || out.count < 32 { return false; }
  total := (input.count / SHA3_RATE + 1) * SHA3_RATE;
  a: [25] u64;
  at := 0;
  while at < total {
    for i: 0..<SHA3_RATE / 8 {
      lane: u64 = 0;
      for j: 0..<8 { lane |= cast(u64) sha3_padded(input, at + 8 * i + j, total) << (8 * j); }
      a[i] ^= lane;
    }
    sha3_permute(a[..]);
    at += SHA3_RATE;
  }
  for i: 0..<32 { out[i] = cast,trunc(u8) (a[i / 8] >> (8 * (i % 8))); }
  return true;
}
```

<!-- source: coax/lib/bitcoin/proxy.coax onion -->
```jai
// torv3::Checksum: the first two bytes of
// SHA3-256(".onion checksum" | PUBKEY | VERSION).
px_onion_checksum :: (pubkey: [] u8, out: [] u8) {
  m: [48] u8;
  at := px_put(m[..], 0, ".onion checksum");
  for i: 0..<32 { m[at + i] = pubkey[i]; }
  m[47] = PX_ONION_VERSION;
  d: [32] u8;
  #assert sha3_256(cast(string) m[..], d[..]);
  out[0] = d[0]; out[1] = d[1];
}
// OnionToString: base32(PUBKEY | CHECKSUM | VERSION) + ".onion", 62
// characters.
px_onion_text :: (pubkey: [] u8, out: [] u8) -> int {
  b: [35] u8;
  for i: 0..<32 { b[i] = pubkey[i]; }
  px_onion_checksum(pubkey, b[32..<34]);
  b[34] = PX_ONION_VERSION;
  return px_put(out, px_b32_encode(b[..], true, out), ".onion");
}
```

BIP-155 sends only the 32-byte key, with network ID 4. The checksum and
version are written again from the key when the address is printed, so
two peers can't send the same key under different names.

## Reading a name

`SetSpecial` takes the name a user typed or a proxy returned. A name
with a NUL byte is refused first: C++ strings can hold one, and a name
that read as one thing in Bitcoin Core and another in Tor could be
used to confuse them. Core reads the base32 in either case but wants the
suffix `.onion` in lower case. It refuses Tor v2's 16-character names,
which Tor dropped in 2021. An I2P name must be 52 characters and
`.b32.i2p`, in any case; I2P's 56-character names for encrypted lease
sets are refused.

<!-- source: coax/lib/bitcoin/proxy.coax special -->
```jai
// What CNetAddr::SetSpecial made of a name: ok, and the address with port 0.
PxName :: struct { ok: bool; addr: AmAddr; }
px_ends :: (s: string, suffix: string, fold: bool) -> bool {
  if s.count < suffix.count { return false; }
  for i: 0..<suffix.count {
    c := s[s.count - suffix.count + i];
    if fold && c >= 'A' && c <= 'Z' { c += 32; }
    if c != suffix[i] { return false; }
  }
  return true;
}
// SetTor: 35 bytes of base32 before ".onion", version 3 and the right
// checksum. Core no longer reads Tor v2's 16-character names.
px_set_tor :: (s: string) -> PxName {
  if !px_ends(s, ".onion", false) { return PxName.{}; }
  b: [64] u8;
  n := px_b32_decode(s[0..<s.count - 6], b[..]);
  if n != 35 || b[34] != PX_ONION_VERSION { return PxName.{}; }
  c: [2] u8; px_onion_checksum(b[0..<32], c[..]);
  if c[0] != b[32] || c[1] != b[33] { return PxName.{}; }
  return PxName.{ok=true, addr=am_bytes(AM_NET_ONION, b[0..<32], 0)};
}
// SetI2P: exactly 52 base32 characters, then ".b32.i2p" in any case. The
// 56-character names of encrypted lease sets are refused.
px_set_i2p :: (s: string) -> PxName {
  if s.count != PX_I2P_TEXT || !px_ends(s, ".b32.i2p", true) { return PxName.{}; }
  t: [56] u8;
  for i: 0..<52 { t[i] = s[i]; }
  for i: 52..<56 { t[i] = '='; }
  b: [40] u8;
  if px_b32_decode(cast(string) t[..], b[..]) != 32 { return PxName.{}; }
  return PxName.{ok=true, addr=am_bytes(AM_NET_I2P, b[0..<32], 0)};
}
// SetSpecial: a name with a NUL in it is refused before either.
px_set_special :: (s: string) -> PxName {
  for c: s { if c == 0 { return PxName.{}; } }
  t := px_set_tor(s);
  if t.ok { return t; }
  return px_set_i2p(s);
}
// ToStringAddr for the networks a proxy carries: Tor, I2P and IPv4.
// Returns the length, or -1 for another network.
px_addr_text :: (a: AmAddr, out: [] u8) -> int {
  ip := a.ip;
  if a.net == AM_NET_ONION { return px_onion_text(ip[0..<32], out); }
  if a.net == AM_NET_I2P { return px_put(out, px_b32_encode(ip[0..<32], false, out), ".b32.i2p"); }
  if a.net != AM_NET_IPV4 { return -1; }
  at := 0;
  for i: 0..<4 { if i > 0 { at = px_put(out, at, "."); } at = px_put_int(out, at, cast(int) a.ip[i]); }
  return at;
}
```

## SOCKS5

Core reaches Tor, and any other proxy, with SOCKS5. The client offers the
authentication methods it supports; the proxy picks one. With
username and password, the client sends both after their lengths, and the
proxy answers with a status. Then the client asks the proxy to `CONNECT`
to a host and port, and the proxy answers with a reply code and the
address it bound.

Core always sends the destination as a host name (`ATYP` 3), even for an
IP address. The proxy then does any name lookup, so a DNS query never
leaves the node unproxied, and a `.onion` name, which only Tor can
resolve, works the same way. A reply other than 0 is the proxy's report
on the peer. Tor adds codes from `0xf0` for onion services: the
descriptor isn't found, introduction failed, and so on.

The client reads each reply in pieces of known size, as Core's
`InterruptibleRecv` does: two bytes for the method and the status, four
for the start of the reply, then the bound address, whose length depends
on its type.

<!-- source: coax/lib/bitcoin/proxy.coax socks -->
```jai
// The SOCKS5 client in Socks5 (netbase.cpp). Each reply is read in pieces
// of known size: need says how many bytes the next read takes.
PX_S_GREET :: 1; PX_S_AUTH :: 2; PX_S_REPLY :: 3; PX_S_ADDR_LEN :: 4; PX_S_ADDR :: 5; PX_S_DONE :: 6; PX_S_FAILED :: 7;
// How a SOCKS5 exchange failed, with Core's log message.
PX_E_NAME :: 1;          // "Hostname too long for SOCKS5 proxying"
PX_E_INIT :: 2;          // "SOCKS5 Proxy failed to initialize"
PX_E_CREDENTIALS :: 3;   // "SOCKS5 Proxy username or password too long"
PX_E_AUTH :: 4;          // "SOCKS5 Proxy authentication unsuccessful"
PX_E_METHOD :: 5;        // "SOCKS5 Proxy requested wrong authentication method"
PX_E_ACCEPT :: 6;        // "SOCKS5 Proxy failed to accept request"
PX_E_CONNECT :: 7;       // "connect to ... failed:" and the reply's meaning
PX_E_MALFORMED :: 8;     // "malformed SOCKS5 proxy response"
PxSocks :: struct {
  state: int; need: int; error: int; reply: int;
  auth: bool; user: [255] u8; user_n: int; pass: [255] u8; pass_n: int;
  dest: [255] u8; dest_n: int; port: int;
}
// What a step leaves the caller to do: send out[0..<send], then read
// s.need more bytes, until state is done or failed.
PxStep :: struct { state: int; send: int; }
px_socks_fail :: (s: [] PxSocks, e: int) -> PxStep {
  s[0].state = PX_S_FAILED; s[0].error = e; s[0].need = 0;
  return PxStep.{state=PX_S_FAILED};
}
// The method selection message: SOCKS5 with no authentication, and with
// username/password too when the caller has credentials.
px_socks_begin :: (s: [] PxSocks, dest: string, port: int, user: string, pass: string, auth: bool, out: [] u8) -> PxStep {
  s[0] = PxSocks.{auth=auth, port=port};
  if dest.count > PX_MAX_NAME { return px_socks_fail(s, PX_E_NAME); }
  for i: 0..<dest.count { s[0].dest[i] = dest[i]; } s[0].dest_n = dest.count;
  s[0].user_n = user.count; s[0].pass_n = pass.count;
  if user.count <= PX_MAX_NAME { for i: 0..<user.count { s[0].user[i] = user[i]; } }
  if pass.count <= PX_MAX_NAME { for i: 0..<pass.count { s[0].pass[i] = pass[i]; } }
  at := px_put_byte(out, 0, PX_SOCKS5);
  if auth { at = px_put_byte(out, at, 2); at = px_put_byte(out, at, PX_NOAUTH); at = px_put_byte(out, at, PX_USER_PASS); }
  else { at = px_put_byte(out, at, 1); at = px_put_byte(out, at, PX_NOAUTH); }
  s[0].state = PX_S_GREET; s[0].need = 2;
  return PxStep.{state=PX_S_GREET, send=at};
}
// CONNECT to a host name: Core always sends the name, never an address,
// so the proxy resolves it and a .onion name works.
px_socks_request :: (s: [] PxSocks, out: [] u8) -> PxStep {
  at := px_put_byte(out, 0, PX_SOCKS5); at = px_put_byte(out, at, PX_CONNECT); at = px_put_byte(out, at, 0);
  at = px_put_byte(out, at, PX_ATYP_DOMAIN); at = px_put_byte(out, at, s[0].dest_n);
  for i: 0..<s[0].dest_n { at = px_put_byte(out, at, cast(int) s[0].dest[i]); }
  at = px_put_byte(out, at, s[0].port >> 8); at = px_put_byte(out, at, s[0].port & 255);
  s[0].state = PX_S_REPLY; s[0].need = 4;
  return PxStep.{state=PX_S_REPLY, send=at};
}
// The next s.need bytes from the proxy.
px_socks_read :: (s: [] PxSocks, in: [] u8, out: [] u8) -> PxStep {
  st := s[0].state;
  if in.count != s[0].need || st == PX_S_DONE || st == PX_S_FAILED { return px_socks_fail(s, PX_E_MALFORMED); }
  if st == PX_S_GREET {
    if in[0] != PX_SOCKS5 { return px_socks_fail(s, PX_E_INIT); }
    if in[1] == PX_USER_PASS && s[0].auth {
      if s[0].user_n > PX_MAX_NAME || s[0].pass_n > PX_MAX_NAME { return px_socks_fail(s, PX_E_CREDENTIALS); }
      // RFC 1929: version 1, then each field after its length.
      at := px_put_byte(out, 0, 1); at = px_put_byte(out, at, s[0].user_n);
      for i: 0..<s[0].user_n { at = px_put_byte(out, at, cast(int) s[0].user[i]); }
      at = px_put_byte(out, at, s[0].pass_n);
      for i: 0..<s[0].pass_n { at = px_put_byte(out, at, cast(int) s[0].pass[i]); }
      s[0].state = PX_S_AUTH; s[0].need = 2;
      return PxStep.{state=PX_S_AUTH, send=at};
    }
    if in[1] == PX_NOAUTH { return px_socks_request(s, out); }
    return px_socks_fail(s, PX_E_METHOD);
  }
  if st == PX_S_AUTH {
    if in[0] != 0x01 || in[1] != 0x00 { return px_socks_fail(s, PX_E_AUTH); }
    return px_socks_request(s, out);
  }
  if st == PX_S_REPLY {
    if in[0] != PX_SOCKS5 { return px_socks_fail(s, PX_E_ACCEPT); }
    if in[1] != 0 { s[0].reply = cast(int) in[1]; return px_socks_fail(s, PX_E_CONNECT); }
    if in[2] != 0 { return px_socks_fail(s, PX_E_MALFORMED); }
    // The address the proxy bound, then its port; Core reads and drops them.
    if in[3] == PX_ATYP_IPV4 { s[0].state = PX_S_ADDR; s[0].need = 4 + 2; }
    else if in[3] == PX_ATYP_IPV6 { s[0].state = PX_S_ADDR; s[0].need = 16 + 2; }
    else if in[3] == PX_ATYP_DOMAIN { s[0].state = PX_S_ADDR_LEN; s[0].need = 1; }
    else { return px_socks_fail(s, PX_E_MALFORMED); }
    return PxStep.{state=s[0].state};
  }
  if st == PX_S_ADDR_LEN { s[0].state = PX_S_ADDR; s[0].need = cast(int) in[0] + 2; return PxStep.{state=PX_S_ADDR}; }
  s[0].state = PX_S_DONE; s[0].need = 0;
  return PxStep.{state=PX_S_DONE};
}
// Socks5ErrorString: RFC 1928's replies and Tor's onion service ones.
px_socks_reply_text :: (rep: int) -> string {
  if rep == 0x01 { return "general failure"; }
  if rep == 0x02 { return "connection not allowed"; }
  if rep == 0x03 { return "network unreachable"; }
  if rep == 0x04 { return "host unreachable"; }
  if rep == 0x05 { return "connection refused"; }
  if rep == 0x06 { return "TTL expired"; }
  if rep == 0x07 { return "protocol error"; }
  if rep == 0x08 { return "address type not supported"; }
  if rep == 0xf0 { return "onion service descriptor can not be found"; }
  if rep == 0xf1 { return "onion service descriptor is invalid"; }
  if rep == 0xf2 { return "onion service introduction failed"; }
  if rep == 0xf3 { return "onion service rendezvous failed"; }
  if rep == 0xf4 { return "onion service missing client authorization"; }
  if rep == 0xf5 { return "onion service wrong client authorization"; }
  if rep == 0xf6 { return "onion service invalid address"; }
  if rep == 0xf7 { return "onion service introduction timed out"; }
  return "unknown";
}
// TorStreamIsolationCredentialsGenerator: the user name and password of
// connection number counter, hex(8 random bytes) + "-" + counter. Tor
// puts streams with different credentials on different circuits.
px_isolation :: (prefix: [] u8, counter: int, out: [] u8) -> int {
  if prefix.count != PX_PREFIX_BYTES { return -1; }
  return px_put_int(out, px_put(out, px_put_hex(out, 0, prefix), "-"), counter);
}
```

With `-proxyrandomize`, on by default, each connection gets new
credentials: eight random bytes in hex, a dash and a counter. The
proxy doesn't check them. Tor's `IsolateSOCKSAuth` puts streams with
different credentials on different circuits, so that two of a node's
connections can't be linked by the exit or the rendezvous point they
share. The random prefix keeps two runs of the node, or two nodes, from
sharing circuits.

To test the client, the library has a proxy that behaves like Tor here:
it reads whole messages, picks username and password when offered, and
leaves its answer to the caller. `px_socks_pump` passes bytes between
the two until the client is connected or has failed.

<!-- source: coax/lib/bitcoin/proxy.coax proxy -->
```jai
// A SOCKS5 proxy, to test the client against: it reads each message
// whole, prefers username/password when offered, as Tor does, and leaves
// the reply to a CONNECT to its caller.
PX_P_GREET :: 1; PX_P_AUTH :: 2; PX_P_REQUEST :: 3; PX_P_CONNECTING :: 4; PX_P_CLOSED :: 5;
PxProxy :: struct {
  state: int = PX_P_GREET;
  user: [255] u8; user_n: int; pass: [255] u8; pass_n: int;
  dest: [255] u8; dest_n: int; port: int;
}
px_proxy_close :: (p: [] PxProxy) -> int { p[0].state = PX_P_CLOSED; return 0; }
// One message from the client; returns how many bytes of reply it wrote
// to out, 0 when the proxy waits for px_proxy_answer, or -1 when it closes.
px_proxy_read :: (p: [] PxProxy, in: [] u8, out: [] u8) -> int {
  st := p[0].state;
  if st == PX_P_GREET {
    if in.count < 2 || in[0] != PX_SOCKS5 || in.count != 2 + cast(int) in[1] { return px_proxy_close(p) - 1; }
    method := PX_NO_ACCEPTABLE;
    for i: 2..<in.count { if in[i] == PX_NOAUTH && method == PX_NO_ACCEPTABLE { method = PX_NOAUTH; } }
    for i: 2..<in.count { if in[i] == PX_USER_PASS { method = PX_USER_PASS; } }
    at := px_put_byte(out, px_put_byte(out, 0, PX_SOCKS5), method);
    p[0].state = ifx method == PX_USER_PASS then PX_P_AUTH else ifx method == PX_NOAUTH then PX_P_REQUEST else PX_P_CLOSED;
    return at;
  }
  if st == PX_P_AUTH {
    if in.count < 2 || in[0] != 1 { return px_proxy_close(p) - 1; }
    u := cast(int) in[1];
    if in.count < 3 + u || in.count != 3 + u + cast(int) in[2 + u] { return px_proxy_close(p) - 1; }
    for i: 0..<u { p[0].user[i] = in[2 + i]; } p[0].user_n = u;
    w := cast(int) in[2 + u];
    for i: 0..<w { p[0].pass[i] = in[3 + u + i]; } p[0].pass_n = w;
    p[0].state = PX_P_REQUEST;
    return px_put_byte(out, px_put_byte(out, 0, 1), 0);
  }
  if st == PX_P_REQUEST {
    if in.count < 5 || in[0] != PX_SOCKS5 || in[1] != PX_CONNECT || in[2] != 0 || in[3] != PX_ATYP_DOMAIN { return px_proxy_close(p) - 1; }
    n := cast(int) in[4];
    if in.count != 7 + n { return px_proxy_close(p) - 1; }
    for i: 0..<n { p[0].dest[i] = in[5 + i]; } p[0].dest_n = n;
    p[0].port = (cast(int) in[5 + n] << 8) | cast(int) in[6 + n];
    p[0].state = PX_P_CONNECTING;
    return 0;
  }
  return px_proxy_close(p) - 1;
}
// The reply to the CONNECT: rep, and Tor's bound address 0.0.0.0:0.
px_proxy_answer :: (p: [] PxProxy, rep: int, out: [] u8) -> int {
  at := px_put_byte(out, 0, PX_SOCKS5); at = px_put_byte(out, at, rep); at = px_put_byte(out, at, 0); at = px_put_byte(out, at, PX_ATYP_IPV4);
  for i: 0..<6 { at = px_put_byte(out, at, 0); }
  p[0].state = ifx rep == 0 then PX_P_CONNECTING else PX_P_CLOSED;
  return at;
}
// The client and the proxy talk until the client is done or either gives
// up. first is what px_socks_begin wrote; rep is the proxy's answer to
// the CONNECT. Returns the client's state.
px_socks_pump :: (s: [] PxSocks, p: [] PxProxy, first: [] u8, rep: int) -> int {
  a: [600] u8; b: [600] u8;
  for i: 0..<first.count { b[i] = first[i]; }
  pending := first.count;
  while s[0].state != PX_S_DONE && s[0].state != PX_S_FAILED {
    reply := px_proxy_read(p, b[0..<pending], a[..]);
    if reply < 0 { return px_socks_fail(s, PX_E_MALFORMED).state; }
    if reply == 0 && p[0].state == PX_P_CONNECTING { reply = px_proxy_answer(p, rep, a[..]); }
    // The client reads the reply in the pieces it asks for.
    at := 0; pending = 0;
    while at < reply && s[0].state != PX_S_DONE && s[0].state != PX_S_FAILED {
      need := s[0].need;
      if at + need > reply { return px_socks_fail(s, PX_E_MALFORMED).state; }
      step := px_socks_read(s, a[at..<at + need], b[..]);
      at += need;
      if step.send > 0 { pending = step.send; }
    }
    if pending == 0 && s[0].state != PX_S_DONE && s[0].state != PX_S_FAILED { return px_socks_fail(s, PX_E_MALFORMED).state; }
  }
  return s[0].state;
}
```

## I2P destinations

An I2P destination is a public encryption key, a public signing key and
a certificate. Core asks for signature type 7, Ed25519, whose certificate
says so in 4 bytes, so its destinations are 391 bytes. A private key
starts with the destination, so `MyDestination` finds where it ends from
the certificate's length at bytes 385 and 386. The address is the SHA-256
of the destination, and its name is that hash in base32 without padding,
52 characters, then `.b32.i2p`. BIP-155 sends the 32 bytes with network
ID 5.

I2P writes binary data in base64 with `-` and `~` in place of `+` and
`/`, so that it can appear in URLs and file names.

<!-- source: coax/lib/bitcoin/proxy.coax i2p -->
```jai
// SwapBase64: I2P's base64 uses '-' and '~' where the standard one uses
// '+' and '/'. The swap goes both ways.
px_i2p_swap :: (c: u8) -> u8 {
  if c == '-' { return '+'; }
  if c == '~' { return '/'; }
  if c == '+' { return '-'; }
  if c == '/' { return '~'; }
  return c;
}
// DecodeI2PBase64: the bytes, or -1.
px_i2p_decode :: (text: string, out: [] u8) -> int {
  if text.count > PX_B64_MAX { return -1; }
  t: [PX_B64_MAX] u8;
  for i: 0..<text.count { t[i] = px_i2p_swap(text[i]); }
  r := base64_decode_checked(cast(string) t[0..<text.count], out, false);
  return ifx r.error == NC_OK then r.written else -1;
}
// EncodeBase64, then SwapBase64.
px_i2p_encode :: (bin: [] u8, out: [] u8) -> int {
  r := base64_encode_checked(cast(string) bin, out, false);
  if r.error != NC_OK { return -1; }
  for i: 0..<r.written { out[i] = px_i2p_swap(out[i]); }
  return r.written;
}
// MyDestination: a private key starts with the destination, 387 bytes and
// the certificate whose big-endian length is at bytes 385 and 386.
px_i2p_dest_len :: (key: [] u8) -> int {
  if key.count < PX_CERT_LEN_AT + 2 { return -1; }
  n := PX_DEST_BASE + ((cast(int) key[PX_CERT_LEN_AT] << 8) | cast(int) key[PX_CERT_LEN_AT + 1]);
  return ifx n > key.count then -1 else n;
}
// DestBinToAddr: an I2P address is the SHA-256 of the destination, and its
// name is that hash in unpadded base32 + ".b32.i2p".
px_i2p_addr :: (dest: [] u8) -> AmAddr {
  h: [32] u8;
  #assert sha256_hash_checked(dest, h[..], false) == 0;
  return am_bytes(AM_NET_I2P, h[..], PX_I2P_PORT);
}
```

## The SAM bridge

An I2P router offers the SAM protocol on a local port, 7656 by default:
text commands, one per line, each answered by a line of words and
`key=value` pairs. Core speaks SAM 3.1, which has no ports, so every I2P
address in Core has port 0.

Each exchange starts on a new socket with `HELLO`. A session is the node's
I2P identity. A persistent one uses the private key in the data
directory's `i2p_private_key` file, made with `DEST GENERATE` the first
time; it is the node's address for incoming connections, which stays the
same across restarts. A transient one, which Core uses for outgoing
connections when it doesn't accept incoming ones, is made fresh by the
router. A stream to a peer looks its name up with `NAMING LOOKUP` and
then connects to the destination it gets back. A stream from a peer
starts with `STREAM ACCEPT`; when a peer connects, its destination
arrives as the first line.

Core decides from the reply whose fault a failure is.
`CANT_REACH_PEER` and `TIMEOUT` are the peer's: the address manager
counts a failed attempt. Anything else is the proxy's, and
`INVALID_ID` means the session is gone and must be made again. A
`RESULT=I2P_ERROR` where a peer's destination was expected also means
the session was closed.

<!-- source: coax/lib/bitcoin/proxy.coax sam -->
```jai
// A SAM reply line: Core splits it at each space and reads each word as
// key=value, or as a key alone. The first of two equal keys counts.
PxReply :: struct { n: int; key: [PX_SAM_KEYS] int; key_n: [PX_SAM_KEYS] int; val: [PX_SAM_KEYS] int; val_n: [PX_SAM_KEYS] int; }
px_sam_find :: (line: string, r: PxReply, key: string) -> int {
  for i: 0..<r.n {
    if r.key_n[i] == key.count && line[r.key[i]..<r.key[i] + r.key_n[i]] == key { return i; }
  }
  return -1;
}
px_sam_parse :: (line: string) -> PxReply {
  r: PxReply;
  start := 0;
  for i: 0..<line.count + 1 {
    if i < line.count && line[i] != ' ' { continue; }
    eq := start; while eq < i && line[eq] != '=' { eq += 1; }
    if r.n < PX_SAM_KEYS && px_sam_find(line, r, line[start..<eq]) < 0 {
      r.key[r.n] = start; r.key_n[r.n] = eq - start;
      r.val[r.n] = ifx eq < i then eq + 1 else -1; r.val_n[r.n] = ifx eq < i then i - eq - 1 else 0;
      r.n += 1;
    }
    start = i + 1;
  }
  return r;
}
// Reply::Get: the value of key, or "" with ok false when it is missing.
PxValue :: struct { ok: bool; text: string; }
px_sam_get :: (line: string, key: string) -> PxValue {
  r := px_sam_parse(line);
  i := px_sam_find(line, r, key);
  if i < 0 || r.val[i] < 0 { return PxValue.{}; }
  return PxValue.{ok=true, text=line[r.val[i]..<r.val[i] + r.val_n[i]]};
}
px_sam_ok :: (line: string) -> bool {
  v := px_sam_get(line, "RESULT");
  return v.ok && v.text == "OK";
}

// i2p::sam::Session as three exchanges with the SAM bridge. Each starts
// with HELLO on a new socket: creating the session (and the private key
// for a persistent one), connecting a stream to a peer, and accepting
// one from a peer.
PX_SAM_CREATE :: 1; PX_SAM_CONNECT :: 2; PX_SAM_ACCEPT :: 3;
PX_Q_HELLO :: 1; PX_Q_GENERATE :: 2; PX_Q_SESSION :: 3; PX_Q_LOOKUP :: 4; PX_Q_STREAM :: 5; PX_Q_WAIT :: 6; PX_Q_DONE :: 7; PX_Q_FAILED :: 8;
// Why an exchange failed.
PX_SAM_E_RESULT :: 1;     // a RESULT other than OK
PX_SAM_E_MISSING :: 2;    // "Missing ...= in the reply"
PX_SAM_E_KEY :: 3;        // a private key or destination that doesn't decode
PX_SAM_E_PORT :: 4;       // a peer with a port other than 0
PX_SAM_E_PEER :: 5;       // CANT_REACH_PEER or TIMEOUT: the peer, not the proxy
PX_SAM_E_ID :: 6;         // INVALID_ID: the session is gone
PX_SAM_E_CLOSED :: 7;     // RESULT=I2P_ERROR where a peer was expected
PxSam :: struct {
  kind: int; state: int; error: int;
  transient: bool; generated: bool;
  id: [10] u8;
  key: [1024] u8; key_n: int;  // the private key, destination first
  me: AmAddr; peer: AmAddr;
  proxy_error: bool;           // Connect's proxy_error: retry another way
  disconnect: bool;            // the session must be made again
}
px_sam_fail :: (q: [] PxSam, e: int) -> PxStep { q[0].state = PX_Q_FAILED; q[0].error = e; return PxStep.{state=PX_Q_FAILED}; }
px_sam_hello :: (out: [] u8) -> int { return px_put(out, 0, "HELLO VERSION MIN=3.1 MAX=3.1\n"); }
// Begin an exchange: HELLO. A connection to a peer needs its port to be 0.
px_sam_begin :: (q: [] PxSam, kind: int, out: [] u8) -> PxStep {
  q[0].kind = kind; q[0].error = 0; q[0].disconnect = false;
  if kind == PX_SAM_CONNECT {
    q[0].proxy_error = q[0].peer.port == PX_I2P_PORT;
    if q[0].peer.port != PX_I2P_PORT { return px_sam_fail(q, PX_SAM_E_PORT); }
  }
  q[0].state = PX_Q_HELLO;
  return PxStep.{state=PX_Q_HELLO, send=px_sam_hello(out)};
}
px_sam_create :: (q: [] PxSam, out: [] u8) -> PxStep {
  at := px_put(out, 0, "SESSION CREATE STYLE=STREAM ID=");
  at = px_put(out, at, cast(string) q[0].id[..]);
  if q[0].transient {
    at = px_put(out, at, " DESTINATION=TRANSIENT SIGNATURE_TYPE=7 i2cp.leaseSetEncType=6,4 inbound.quantity=1 outbound.quantity=1\n");
  } else {
    at = px_put(out, at, " DESTINATION=");
    if at >= 0 {
      n := px_i2p_encode(q[0].key[0..<q[0].key_n], out[at..<out.count]);
      at = ifx n < 0 then -1 else at + n;
    }
    at = px_put(out, at, " i2cp.leaseSetEncType=6,4 inbound.quantity=3 outbound.quantity=3\n");
  }
  q[0].state = PX_Q_SESSION;
  return PxStep.{state=PX_Q_SESSION, send=at};
}
// The session's own address, from the destination at the start of its key.
px_sam_me :: (q: [] PxSam) -> PxStep {
  n := px_i2p_dest_len(q[0].key[0..<q[0].key_n]);
  if n < 0 { return px_sam_fail(q, PX_SAM_E_KEY); }
  q[0].me = px_i2p_addr(q[0].key[0..<n]);
  q[0].state = PX_Q_DONE;
  return PxStep.{state=PX_Q_DONE};
}
// One reply line, without its '\n'.
px_sam_read :: (q: [] PxSam, line: string, out: [] u8) -> PxStep {
  st := q[0].state;
  if st == PX_Q_HELLO {
    if !px_sam_ok(line) { return px_sam_fail(q, PX_SAM_E_RESULT); }
    if q[0].kind == PX_SAM_CREATE {
      if q[0].transient || q[0].key_n > 0 { return px_sam_create(q, out); }
      q[0].state = PX_Q_GENERATE;
      return PxStep.{state=PX_Q_GENERATE, send=px_put(out, 0, "DEST GENERATE SIGNATURE_TYPE=7\n")};
    }
    if q[0].kind == PX_SAM_CONNECT {
      at := px_put(out, 0, "NAMING LOOKUP NAME=");
      at = px_put(out, at + px_addr_text(q[0].peer, out[at..<out.count]), "\n");
      q[0].state = PX_Q_LOOKUP;
      return PxStep.{state=PX_Q_LOOKUP, send=at};
    }
    at := px_put(out, 0, "STREAM ACCEPT ID=");
    at = px_put(out, at, cast(string) q[0].id[..]);
    at = px_put(out, at, " SILENT=false\n");
    q[0].state = PX_Q_STREAM;
    return PxStep.{state=PX_Q_STREAM, send=at};
  }
  if st == PX_Q_GENERATE {
    // DEST GENERATE's RESULT isn't checked; its PRIV is the new key, which
    // the caller saves before the session is created with it.
    v := px_sam_get(line, "PRIV");
    if !v.ok { return px_sam_fail(q, PX_SAM_E_MISSING); }
    n := px_i2p_decode(v.text, q[0].key[..]);
    if n < 0 { return px_sam_fail(q, PX_SAM_E_KEY); }
    q[0].key_n = n; q[0].generated = true;
    return px_sam_create(q, out);
  }
  if st == PX_Q_SESSION {
    if !px_sam_ok(line) { return px_sam_fail(q, PX_SAM_E_RESULT); }
    if q[0].transient {
      v := px_sam_get(line, "DESTINATION");
      if !v.ok { return px_sam_fail(q, PX_SAM_E_MISSING); }
      n := px_i2p_decode(v.text, q[0].key[..]);
      if n < 0 { return px_sam_fail(q, PX_SAM_E_KEY); }
      q[0].key_n = n;
    }
    return px_sam_me(q);
  }
  if st == PX_Q_LOOKUP {
    if !px_sam_ok(line) { return px_sam_fail(q, PX_SAM_E_RESULT); }
    v := px_sam_get(line, "VALUE");
    if !v.ok { return px_sam_fail(q, PX_SAM_E_MISSING); }
    at := px_put(out, 0, "STREAM CONNECT ID=");
    at = px_put(out, at, cast(string) q[0].id[..]);
    at = px_put(out, px_put(out, px_put(out, at, " DESTINATION="), v.text), " SILENT=false\n");
    q[0].state = PX_Q_STREAM;
    return PxStep.{state=PX_Q_STREAM, send=at};
  }
  if st == PX_Q_STREAM {
    v := px_sam_get(line, "RESULT");
    if !v.ok { return px_sam_fail(q, PX_SAM_E_MISSING); }
    if q[0].kind == PX_SAM_ACCEPT {
      if v.text != "OK" { return px_sam_fail(q, PX_SAM_E_RESULT); }
      q[0].state = PX_Q_WAIT;
      return PxStep.{state=PX_Q_WAIT};
    }
    if v.text == "OK" { q[0].state = PX_Q_DONE; return PxStep.{state=PX_Q_DONE}; }
    if v.text == "INVALID_ID" { q[0].disconnect = true; return px_sam_fail(q, PX_SAM_E_ID); }
    if v.text == "CANT_REACH_PEER" || v.text == "TIMEOUT" { q[0].proxy_error = false; return px_sam_fail(q, PX_SAM_E_PEER); }
    return px_sam_fail(q, PX_SAM_E_RESULT);
  }
  if st == PX_Q_WAIT {
    // Accept: the first line on the accepted socket is the peer's
    // destination. A RESULT=I2P_ERROR there means the session is gone.
    d: [1024] u8;
    n := px_i2p_decode(line, d[..]);
    if n < 0 {
      for i: 0..<line.count {
        if i + 16 <= line.count && line[i..<i + 16] == "RESULT=I2P_ERROR" { q[0].disconnect = true; return px_sam_fail(q, PX_SAM_E_CLOSED); }
      }
      return px_sam_fail(q, PX_SAM_E_KEY);
    }
    q[0].peer = px_i2p_addr(d[0..<n]);
    q[0].state = PX_Q_DONE;
    return PxStep.{state=PX_Q_DONE};
  }
  return px_sam_fail(q, PX_SAM_E_RESULT);
}
```

## Which networks are reachable

A network is reachable when the node makes automatic outbound
connections to it. By default that is IPv4, IPv6 and anything a proxy or
bridge makes possible: Tor with `-proxy` or `-onion`, I2P with
`-i2psam`, and CJDNS with `-cjdnsreachable`. `-onlynet` names the
networks to keep, and leaves out the rest. It affects automatic
connections only: an `-addnode` or `-connect` peer on another network is
still dialed.

Core checks the options in a fixed order and stops at the first
contradiction: `-onlynet=onion` with Tor forbidden or with no way to
reach it, `-onlynet=i2p` with no SAM bridge, or `-dnsseed=1` when
neither IPv4 nor IPv6 is reachable. An `-onlynet` without IPv4 or IPv6
turns DNS seeds off, since seeds are found over the clear net. Tor is
not reachable without a proxy, but with `-listenonion`, on by default,
Core may still learn Tor's SOCKS port from its control port and make
Tor reachable then.

A `-proxy` without a network applies to IPv4, IPv6, Tor and CJDNS, and to
name lookups; `-proxy=ADDR=NET` applies to one. I2P has no `-proxy`:
it goes through the SAM bridge or not at all.

<!-- source: coax/lib/bitcoin/proxy.coax reach -->
```jai
// Which networks a node reaches and how, after AppInitMain reads
// -onlynet, -cjdnsreachable, -dnsseed, -proxy, -onion, -listenonion and
// -i2psam, in Core's order. code names the first InitError, if any.
PX_C_OK :: 0;
PX_C_UNKNOWN_NET :: 1;       // "Unknown network specified in -onlynet"
PX_C_CJDNS :: 2;             // "-onlynet=cjdns but -cjdnsreachable is not provided"
PX_C_DNSSEED :: 3;           // "-dnsseed=1 ... but -onlynet forbids connections to IPv4/IPv6"
PX_C_PROXY_EQ :: 4;          // "Invalid -proxy address or hostname, ends with '='"
PX_C_PROXY_NET :: 5;         // "Unrecognized network in -proxy"
PX_C_ONION_FORBIDDEN :: 6;   // "-onlynet=onion but ... explicitly forbidden: -onion=0"
PX_C_ONION_MISSING :: 7;     // "-onlynet=onion but ... none of -proxy, -onion or -listenonion"
PX_C_I2P_MISSING :: 8;       // "-onlynet=i2p but -i2psam is not provided"
PxNet :: struct {
  code: int;
  reachable: [7] bool;       // by Core's Network: g_reachable_nets
  proxy: [7] bool;           // SetProxy(net): -proxy, -onion, or the SAM bridge for I2P
  name_proxy: bool;          // SetNameProxy: names are resolved by the proxy
  dnsseed: bool;
  onion_later: bool;         // -listenonion may still make Tor reachable through the control port
}
// ParseNetwork: a name in any case, or NET_UNROUTABLE.
px_parse_network :: (s: string) -> int {
  t: [8] u8;
  if s.count > 8 { return AM_NET_UNROUTABLE; }
  for i: 0..<s.count { c := s[i]; if c >= 'A' && c <= 'Z' { c += 32; } t[i] = c; }
  low := cast(string) t[0..<s.count];
  if low == "ipv4" { return AM_NET_IPV4; }
  if low == "ipv6" { return AM_NET_IPV6; }
  if low == "onion" { return AM_NET_ONION; }
  if low == "i2p" { return AM_NET_I2P; }
  if low == "cjdns" { return AM_NET_CJDNS; }
  return AM_NET_UNROUTABLE;
}
// Core's InitError for each code.
px_configure_text :: (code: int) -> string {
  if code == PX_C_UNKNOWN_NET { return "Unknown network specified in -onlynet"; }
  if code == PX_C_CJDNS { return "Outbound connections restricted to CJDNS (-onlynet=cjdns) but -cjdnsreachable is not provided"; }
  if code == PX_C_DNSSEED { return "Incompatible options: -dnsseed=1 was explicitly specified, but -onlynet forbids connections to IPv4/IPv6"; }
  if code == PX_C_PROXY_EQ { return "Invalid -proxy address or hostname, ends with '='"; }
  if code == PX_C_PROXY_NET { return "Unrecognized network in -proxy"; }
  if code == PX_C_ONION_FORBIDDEN { return "Outbound connections restricted to Tor (-onlynet=onion) but the proxy for reaching the Tor network is explicitly forbidden: -onion=0"; }
  if code == PX_C_ONION_MISSING { return "Outbound connections restricted to Tor (-onlynet=onion) but the proxy for reaching the Tor network is not provided: none of -proxy, -onion or -listenonion is given"; }
  if code == PX_C_I2P_MISSING { return "Outbound connections restricted to i2p (-onlynet=i2p) but -i2psam is not provided"; }
  return "";
}
// dnsseed is -1 when not given, else 0 or 1. onion is "" when not given.
px_configure :: (onlynet: [] string, proxies: [] string, onion: string, listenonion: bool, i2psam: bool, cjdns: bool, dnsseed: int) -> PxNet {
  n: PxNet;
  for i: 0..<7 { n.reachable[i] = true; }
  if onlynet.count > 0 {
    for i: 0..<7 { n.reachable[i] = false; }
    for s: onlynet {
      net := px_parse_network(s);
      if net == AM_NET_UNROUTABLE { n.code = PX_C_UNKNOWN_NET; return n; }
      n.reachable[net] = true;
    }
  }
  if !cjdns {
    if onlynet.count > 0 && n.reachable[AM_NET_CJDNS] { n.code = PX_C_CJDNS; return n; }
    n.reachable[AM_NET_CJDNS] = false;
  }
  // An -onlynet without IPv4 or IPv6 turns DNS seeds off, unless -dnsseed=1
  // was given, which is then an error.
  clearnet := onlynet.count == 0 || n.reachable[AM_NET_IPV4] || n.reachable[AM_NET_IPV6];
  n.dnsseed = ifx dnsseed < 0 then clearnet else dnsseed == 1;
  if dnsseed == 1 && !n.reachable[AM_NET_IPV4] && !n.reachable[AM_NET_IPV6] { n.code = PX_C_DNSSEED; return n; }
  // Each -proxy, for every network or, after '=', for one.
  v4 := false; v6 := false; tor := false; name := false; cj := false;
  for s: proxies {
    eq := s.count - 1; while eq >= 0 && s[eq] != '=' { eq -= 1; }
    if eq == s.count - 1 && eq >= 0 { n.code = PX_C_PROXY_EQ; return n; }
    value := ifx eq < 0 then s else s[0..<eq];
    set := value.count > 0 && value != "0";
    if eq < 0 { v4 = set; v6 = set; name = set; cj = set; tor = set; continue; }
    net := px_parse_network(s[eq + 1..<s.count]);
    if net == AM_NET_IPV4 { v4 = set; name = set; }
    else if net == AM_NET_IPV6 { v6 = set; name = set; }
    else if net == AM_NET_ONION { tor = set; }
    else if net == AM_NET_CJDNS { cj = set; }
    else { n.code = PX_C_PROXY_NET; return n; }
  }
  n.proxy[AM_NET_IPV4] = v4; n.proxy[AM_NET_IPV6] = v6; n.proxy[AM_NET_CJDNS] = cj; n.name_proxy = name;
  // -onion sets or overrides the proxy for .onion; -onion=0 forbids it.
  onion_only := onlynet.count > 0 && n.reachable[AM_NET_ONION];
  if onion == "0" {
    tor = false;
    if onion_only { n.code = PX_C_ONION_FORBIDDEN; return n; }
  } else if onion.count > 0 { tor = true; }
  if tor { n.proxy[AM_NET_ONION] = true; }
  else {
    if onion_only && !listenonion { n.code = PX_C_ONION_MISSING; return n; }
    n.reachable[AM_NET_ONION] = false;
    n.onion_later = listenonion && (onlynet.count == 0 || onion_only);
  }
  if i2psam { n.proxy[AM_NET_I2P] = true; }
  else {
    if onlynet.count > 0 && n.reachable[AM_NET_I2P] { n.code = PX_C_I2P_MISSING; return n; }
    n.reachable[AM_NET_I2P] = false;
  }
  return n;
}
// How CConnman::ConnectNode reaches an address: not at all if its network
// isn't reachable, through the SAM bridge for I2P, through the network's
// proxy if it has one, else directly.
PX_R_UNREACHABLE :: 0; PX_R_DIRECT :: 1; PX_R_PROXY :: 2; PX_R_SAM :: 3;
px_route :: (n: PxNet, a: AmAddr) -> int {
  net := am_network(a);
  if !n.reachable[net] { return PX_R_UNREACHABLE; }
  if a.net == AM_NET_I2P && n.proxy[AM_NET_I2P] { return PX_R_SAM; }
  if n.proxy[net] { return PX_R_PROXY; }
  return PX_R_DIRECT;
}
```

## The example

The example,
[bitcoin/examples/proxy.coax](../examples/proxy.coax), reads the three
onion addresses of Tor's rend-spec and Core's I2P test destinations,
connects to onion services through the test proxy with stream
isolation, runs a SAM session with replies from Core's tests and the SAM
specification, and sets up nine combinations of network options.

<!-- source: bitcoin/examples/proxy.coax example -->
```jai
#load "../../coax/lib/bitcoin/proxy.coax";
#import "Basic";

hex :: (b: [] u8) { for x: b { print("%", "0123456789abcdef"[cast(int) (x >> 4)..<cast(int) (x >> 4) + 1]); print("%", "0123456789abcdef"[cast(int) (x & 15)..<cast(int) (x & 15) + 1]); } }
name :: (a: AmAddr) { t: [80] u8; n := px_addr_text(a, t[..]); print("%", cast(string) t[0..<n]); }
// A long line, as its first 60 characters.
short :: (s: string) { if s.count <= 60 { print("%", s); } else { print("%...", s[0..<60]); } }
yes :: (b: bool) -> string { return ifx b then "yes" else "no"; }

onions :: () {
  print("Tor v3 onion addresses (rend-spec-v3)\n");
  names := [3] string.["pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscryd.onion",
                       "sp3k262uwy4r2k3ycr5awluarykdpag6a7y33jxop4cs2lu5uz5sseqd.onion",
                       "xa4r2iadxm55fbnqgwwi5mymqdcofiu3w6rpbtqn7b2dyn7mgwj64jyd.onion"];
  for s: names {
    x := px_set_special(s);
    ip := x.addr.ip; c: [2] u8; px_onion_checksum(ip[0..<32], c[..]);
    wire: [40] u8; n := am_put_net(wire[..], 0, x.addr);
    print("  %\n    key ", s); hex(ip[0..<32]); print("\n    checksum "); hex(c[..]); print(", version 3\n    addrv2 "); hex(wire[0..<n]); print("\n");
  }
  bad := [5] string.["pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscsad.onion",
                     "pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscrye.onion",
                     "6hzph5hv6337r6p2.onion", "PG6MMJIYJMCRSSLVYKFWNNTLARU7P5SVN6Y2YMMJU6NUBXNDF4PSCRYD.onion",
                     "pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscryd.ONION"];
  why := [5] string.["wrong checksum", "wrong version", "Tor v2", "upper case", "upper-case suffix"];
  for k: 0..<5 { print("  % (%): %\n", bad[k], why[k], ifx px_set_special(bad[k]).ok then "read" else "refused"); }
}

i2p :: () {
  print("I2P addresses\n");
  pub: [1024] u8; priv: [1024] u8;
  n := px_i2p_decode(I2P_TEST_PUB, pub[..]);
  m := px_i2p_decode(I2P_TEST_PRIV, priv[..]);
  d := px_i2p_dest_len(priv[0..<m]);
  a := px_i2p_addr(pub[0..<n]);
  print("  destination: % bytes, certificate type % of % bytes\n", n, pub[384], (cast(int) pub[385] << 8) | cast(int) pub[386]);
  print("  private key: % bytes, of which the first % are the destination\n", m, d);
  print("  address "); name(a); print("\n    SHA-256 of the destination "); hex(a.ip[0..<32]); print("\n");
  wire: [40] u8; w := am_put_net(wire[..], 0, a);
  print("    addrv2 "); hex(wire[0..<w]); print("\n");
  back := px_set_special("UDHDrtrcetjm5sxzskjyr5ztpeszydbh4dpl3pl4utgqqw2v4jna.b32.I2P");
  print("  UDHDrtrcetjm5sxzskjyr5ztpeszydbh4dpl3pl4utgqqw2v4jna.b32.I2P reads as "); name(back.addr); print("\n");
  print("  56-character name: %\n", ifx px_set_special("pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscsad.b32.i2p").ok then "read" else "refused");
}

// A proxy that gives each distinct set of credentials its own circuit, as
// Tor does with IsolateSOCKSAuth.
Circuits :: struct { user: [8] [40] u8; user_n: [8] int; n: int; }
circuit :: (c: [] Circuits, p: PxProxy) -> int {
  for i: 0..<c[0].n {
    if c[0].user_n[i] == p.user_n {
      same := true;
      for j: 0..<p.user_n { if c[0].user[i][j] != p.user[j] { same = false; } }
      if same { return i + 1; }
    }
  }
  for j: 0..<p.user_n { c[0].user[c[0].n][j] = p.user[j]; }
  c[0].user_n[c[0].n] = p.user_n; c[0].n += 1;
  return c[0].n;
}
socks :: () {
  print("SOCKS5 to Tor, with stream isolation\n");
  prefix := [8] u8.[0x9f, 0x31, 0x0c, 0x5e, 0xa2, 0x47, 0xd1, 0x08];
  dest := [4] string.["pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscryd.onion",
                      "sp3k262uwy4r2k3ycr5awluarykdpag6a7y33jxop4cs2lu5uz5sseqd.onion",
                      "xa4r2iadxm55fbnqgwwi5mymqdcofiu3w6rpbtqn7b2dyn7mgwj64jyd.onion",
                      "pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscryd.onion"];
  answer := [4] int.[0, 0, 0xf0, 0];
  c: [1] Circuits;
  for k: 0..<4 {
    cred: [40] u8; cn := px_isolation(prefix[..], k, cred[..]);
    s: [1] PxSocks; p: [1] PxProxy; out: [600] u8;
    st := px_socks_begin(s[..], dest[k], 8333, cast(string) cred[0..<cn], cast(string) cred[0..<cn], true, out[..]);
    if k == 0 { print("  greeting "); hex(out[0..<st.send]); print("\n"); }
    r := px_socks_pump(s[..], p[..], out[0..<st.send], answer[k]);
    print("  %...:8333 as %: ", dest[k][0..<12], cast(string) cred[0..<cn]);
    if r == PX_S_DONE { print("connected, circuit %\n", circuit(c[..], p[0])); }
    else { h: [1] u8; h[0] = cast,trunc(u8) s[0].reply; print("failed, % (0x", px_socks_reply_text(s[0].reply)); hex(h[..]); print(")\n"); }
  }
  // -proxyrandomize=0: no credentials, so one circuit for everything.
  s: [1] PxSocks; p: [1] PxProxy; out: [600] u8;
  st := px_socks_begin(s[..], "1.2.3.4", 8333, "", "", false, out[..]);
  print("  without isolation the greeting is "); hex(out[0..<st.send]); print("\n");
  r := px_socks_pump(s[..], p[..], out[0..<st.send], 0);
  print("  1.2.3.4:8333: %, credentials sent: %\n", ifx r == PX_S_DONE then "connected" else "failed", p[0].user_n);
}

sam :: () {
  print("I2P through the SAM bridge\n");
  q: [1] PxSam; out: [4096] u8; line: [2048] u8;
  q[0].transient = true; for i: 0..<10 { q[0].id[i] = "2b7c0e91a4"[i]; }
  st := px_sam_begin(q[..], PX_SAM_CREATE, out[..]);
  print("  > "); short(cast(string) out[0..<st.send - 1]); print("\n");
  reply := "HELLO REPLY RESULT=OK VERSION=3.1";
  print("  < %\n", reply);
  st = px_sam_read(q[..], reply, out[..]);
  print("  > "); short(cast(string) out[0..<st.send - 1]); print("\n");
  at := px_put(line[..], 0, "SESSION STATUS RESULT=OK DESTINATION="); at = px_put(line[..], at, I2P_TEST_PRIV);
  print("  < "); short(cast(string) line[0..<at]); print("\n");
  st = px_sam_read(q[..], cast(string) line[0..<at], out[..]);
  print("  session %, our address ", ifx st.state == PX_Q_DONE then "created" else "failed"); name(q[0].me); print(":0\n");
  // Connect to the peer of SAM's accept example by its name.
  peer: [1024] u8; pn := px_i2p_decode(I2P_TEST_PEER, peer[..]);
  q[0].peer = px_i2p_addr(peer[0..<pn]);
  results := [2] string.["STREAM STATUS RESULT=OK", "STREAM STATUS RESULT=CANT_REACH_PEER"];
  for k: 0..<2 {
    st = px_sam_begin(q[..], PX_SAM_CONNECT, out[..]);
    st = px_sam_read(q[..], reply, out[..]);
    print("  > "); short(cast(string) out[0..<st.send - 1]); print("\n");
    at = px_put(line[..], 0, "NAMING REPLY RESULT=OK NAME="); at += px_addr_text(q[0].peer, line[at..<2048]);
    at = px_put(line[..], at, " VALUE="); at = px_put(line[..], at, I2P_TEST_PEER);
    print("  < "); short(cast(string) line[0..<at]); print("\n");
    st = px_sam_read(q[..], cast(string) line[0..<at], out[..]);
    print("  > "); short(cast(string) out[0..<st.send - 1]); print("\n");
    print("  < %\n", results[k]);
    st = px_sam_read(q[..], results[k], out[..]);
    if st.state == PX_Q_DONE { print("  connected\n"); }
    else { print("  failed; the proxy is to blame: %\n", yes(q[0].proxy_error)); }
  }
  // Accept: the peer's destination arrives on the accepted socket.
  st = px_sam_begin(q[..], PX_SAM_ACCEPT, out[..]);
  st = px_sam_read(q[..], reply, out[..]);
  print("  > "); short(cast(string) out[0..<st.send - 1]); print("\n");
  st = px_sam_read(q[..], "STREAM STATUS RESULT=OK", out[..]);
  st = px_sam_read(q[..], I2P_TEST_PEER, out[..]);
  print("  accepted a peer at "); name(q[0].peer); print(":0\n");
  st = px_sam_begin(q[..], PX_SAM_ACCEPT, out[..]);
  st = px_sam_read(q[..], reply, out[..]);
  st = px_sam_read(q[..], "STREAM STATUS RESULT=OK", out[..]);
  st = px_sam_read(q[..], "STREAM STATUS RESULT=I2P_ERROR MESSAGE=\"Session was closed\"", out[..]);
  print("  a closed session: make it again: %\n", yes(q[0].disconnect));
}

NET_NAMES :: [7] string.["unroutable", "ipv4", "ipv6", "onion", "i2p", "cjdns", "internal"];
ROUTES :: [4] string.["unreachable", "direct", "proxy", "SAM"];
show :: (label: string, onlynet: [] string, proxies: [] string, onion: string, listenonion: bool, i2psam: bool, dnsseed: int) {
  print("  %\n", label);
  n := px_configure(onlynet, proxies, onion, listenonion, i2psam, false, dnsseed);
  if n.code != PX_C_OK { print("    error: %\n", px_configure_text(n.code)); return; }
  print("    reachable:");
  for i: 1..<6 { if n.reachable[i] { print(" %", NET_NAMES[i]); if n.proxy[i] { print("%", ifx i == AM_NET_I2P then "(SAM)" else "(proxy)"); } } }
  print("; DNS seeds %", ifx n.dnsseed then "on" else "off");
  if n.onion_later { print("; onion may come from -listenonion"); }
  print("\n    1.2.3.4: %, onion: %, i2p: %\n", ROUTES[px_route(n, am_v4(1, 2, 3, 4, 8333))],
        ROUTES[px_route(n, px_set_special("pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscryd.onion").addr)],
        ROUTES[px_route(n, px_set_special("ukeu3k5oycgaauneqgtnvselmt4yemvoilkln7jpvamvfx7dnkdq.b32.i2p").addr)]);
}
networks :: () {
  print("Reachable networks\n");
  empty: [1] string; none := empty[0..<0];
  tor := [1] string.["127.0.0.1:9050"]; only_onion := [1] string.["onion"];
  both := [2] string.["onion", "i2p"]; v4 := [1] string.["ipv4"];
  show("no options", none, none, "", true, false, -1);
  show("-proxy=127.0.0.1:9050", none, tor[..], "", true, false, -1);
  show("-onion=127.0.0.1:9050 -i2psam=127.0.0.1:7656", none, none, "127.0.0.1:9050", true, true, -1);
  show("-onlynet=onion -onlynet=i2p -proxy=127.0.0.1:9050 -i2psam=127.0.0.1:7656", both[..], tor[..], "", true, true, -1);
  show("-onlynet=onion -listenonion=0", only_onion[..], none, "", false, false, -1);
  show("-onlynet=onion -onion=0", only_onion[..], tor[..], "0", true, false, -1);
  show("-onlynet=onion -proxy=127.0.0.1:9050 -dnsseed=1", only_onion[..], tor[..], "", true, false, 1);
  show("-onlynet=i2p", both[1..<2], none, "", true, false, -1);
  show("-onlynet=ipv4 -onion=127.0.0.1:9050", v4[..], none, "127.0.0.1:9050", true, false, -1);
}

main :: () {
  onions();
  i2p();
  socks();
  sam();
  networks();
}
```

It prints:

```text
Tor v3 onion addresses (rend-spec-v3)
  pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscryd.onion
    key 79bcc625184b05194975c28b66b66b0469f7f6556fb1ac3189a79b40dda32f1f
    checksum 2147, version 3
    addrv2 042079bcc625184b05194975c28b66b66b0469f7f6556fb1ac3189a79b40dda32f1f
  sp3k262uwy4r2k3ycr5awluarykdpag6a7y33jxop4cs2lu5uz5sseqd.onion
    key 93f6ad7b54b6391d2b78147a0b2e808e143780de07f1bda6ee7f052d2e9da67b
    checksum 2912, version 3
    addrv2 042093f6ad7b54b6391d2b78147a0b2e808e143780de07f1bda6ee7f052d2e9da67b
  xa4r2iadxm55fbnqgwwi5mymqdcofiu3w6rpbtqn7b2dyn7mgwj64jyd.onion
    key b8391d2003bb3bd285b035ac8eb30c80c4e2a29bb7a2f0ce0df8743c37ec3593
    checksum ee27, version 3
    addrv2 0420b8391d2003bb3bd285b035ac8eb30c80c4e2a29bb7a2f0ce0df8743c37ec3593
  pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscsad.onion (wrong checksum): refused
  pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscrye.onion (wrong version): refused
  6hzph5hv6337r6p2.onion (Tor v2): refused
  PG6MMJIYJMCRSSLVYKFWNNTLARU7P5SVN6Y2YMMJU6NUBXNDF4PSCRYD.onion (upper case): read
  pg6mmjiyjmcrsslvykfwnntlaru7p5svn6y2ymmju6nubxndf4pscryd.ONION (upper-case suffix): refused
I2P addresses
  destination: 391 bytes, certificate type 5 of 4 bytes
  private key: 679 bytes, of which the first 391 are the destination
  address lwqtpjmazikszhrxpofaisknphjs4iusdwgekbk77jtvsth6jpya.b32.i2p
    SHA-256 of the destination 5da137a580ca152c9e377b8a04494d79d32e22921d8c45055ffa67594cfe4bf0
    addrv2 05205da137a580ca152c9e377b8a04494d79d32e22921d8c45055ffa67594cfe4bf0
  UDHDrtrcetjm5sxzskjyr5ztpeszydbh4dpl3pl4utgqqw2v4jna.b32.I2P reads as udhdrtrcetjm5sxzskjyr5ztpeszydbh4dpl3pl4utgqqw2v4jna.b32.i2p
  56-character name: refused
SOCKS5 to Tor, with stream isolation
  greeting 05020002
  pg6mmjiyjmcr...:8333 as 9f310c5ea247d108-0: connected, circuit 1
  sp3k262uwy4r...:8333 as 9f310c5ea247d108-1: connected, circuit 2
  xa4r2iadxm55...:8333 as 9f310c5ea247d108-2: failed, onion service descriptor can not be found (0xf0)
  pg6mmjiyjmcr...:8333 as 9f310c5ea247d108-3: connected, circuit 3
  without isolation the greeting is 050100
  1.2.3.4:8333: connected, credentials sent: 0
I2P through the SAM bridge
  > HELLO VERSION MIN=3.1 MAX=3.1
  < HELLO REPLY RESULT=OK VERSION=3.1
  > SESSION CREATE STYLE=STREAM ID=2b7c0e91a4 DESTINATION=TRANSI...
  < SESSION STATUS RESULT=OK DESTINATION=WnGOLXRBqHQhdVjFlWqRxJw...
  session created, our address lwqtpjmazikszhrxpofaisknphjs4iusdwgekbk77jtvsth6jpya.b32.i2p:0
  > NAMING LOOKUP NAME=vnygdvwd2l4tnpadlxytduukgilihhd6gg7habmxi...
  < NAMING REPLY RESULT=OK NAME=vnygdvwd2l4tnpadlxytduukgilihhd6...
  > STREAM CONNECT ID=2b7c0e91a4 DESTINATION=IchV608baDoXbqzQKSq...
  < STREAM STATUS RESULT=OK
  connected
  > NAMING LOOKUP NAME=vnygdvwd2l4tnpadlxytduukgilihhd6gg7habmxi...
  < NAMING REPLY RESULT=OK NAME=vnygdvwd2l4tnpadlxytduukgilihhd6...
  > STREAM CONNECT ID=2b7c0e91a4 DESTINATION=IchV608baDoXbqzQKSq...
  < STREAM STATUS RESULT=CANT_REACH_PEER
  failed; the proxy is to blame: no
  > STREAM ACCEPT ID=2b7c0e91a4 SILENT=false
  accepted a peer at vnygdvwd2l4tnpadlxytduukgilihhd6gg7habmxivedou7gb2qa.b32.i2p:0
  a closed session: make it again: yes
Reachable networks
  no options
    reachable: ipv4 ipv6; DNS seeds on; onion may come from -listenonion
    1.2.3.4: direct, onion: unreachable, i2p: unreachable
  -proxy=127.0.0.1:9050
    reachable: ipv4(proxy) ipv6(proxy) onion(proxy); DNS seeds on
    1.2.3.4: proxy, onion: proxy, i2p: unreachable
  -onion=127.0.0.1:9050 -i2psam=127.0.0.1:7656
    reachable: ipv4 ipv6 onion(proxy) i2p(SAM); DNS seeds on
    1.2.3.4: direct, onion: proxy, i2p: SAM
  -onlynet=onion -onlynet=i2p -proxy=127.0.0.1:9050 -i2psam=127.0.0.1:7656
    reachable: onion(proxy) i2p(SAM); DNS seeds off
    1.2.3.4: unreachable, onion: proxy, i2p: SAM
  -onlynet=onion -listenonion=0
    error: Outbound connections restricted to Tor (-onlynet=onion) but the proxy for reaching the Tor network is not provided: none of -proxy, -onion or -listenonion is given
  -onlynet=onion -onion=0
    error: Outbound connections restricted to Tor (-onlynet=onion) but the proxy for reaching the Tor network is explicitly forbidden: -onion=0
  -onlynet=onion -proxy=127.0.0.1:9050 -dnsseed=1
    error: Incompatible options: -dnsseed=1 was explicitly specified, but -onlynet forbids connections to IPv4/IPv6
  -onlynet=i2p
    error: Outbound connections restricted to i2p (-onlynet=i2p) but -i2psam is not provided
  -onlynet=ipv4 -onion=127.0.0.1:9050
    reachable: ipv4; DNS seeds on
    1.2.3.4: direct, onion: unreachable, i2p: unreachable
```

- Each onion name reads back to its key, and the checksum is the one
  SHA3-256 gives. The `addrv2` form is network 4, length 32 and the key,
  the same bytes as Core's `net_tests.cpp` for the first name.
- One changed character changes the checksum or the version, and the
  name is refused. An upper-case name is read, because Core's base32
  reads either case, but an upper-case `.ONION` is not.
- The I2P destination is 391 bytes: 387, then a certificate of type 5,
  the key certificate, with 4 bytes. The private key from `DEST
  GENERATE` starts with the same 391 bytes.
- Each connection to Tor sends new credentials, so Tor builds a new
  circuit for each one, even the fourth, which goes to the same onion
  service as the first. The third fails with Tor's code `0xf0`: no
  descriptor for that service.
- Without isolation, the client offers only "no authentication".
- The SAM session's address comes from the private key the router
  returned. A `CANT_REACH_PEER` is the peer's fault, not the proxy's,
  and a closed session must be made again.
- `-proxy` alone makes Tor reachable and sends IPv4 and IPv6 through
  the proxy too. `-onion` sends only Tor through it.
- `-onlynet=ipv4` leaves Tor unreachable even with `-onion` set: the
  proxy is configured, but automatic connections don't use it.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/proxy.out](../fixtures/proxy.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/proxy.coax](../examples/checks/proxy.coax),
which loads the example:

- an onion name made from any key reads back to the key, and changing
  any one of its 56 characters is refused
- an I2P name made from any hash reads back to it in either case
- for every host name length from 0 to 255, with and without
  credentials, the client and the proxy agree on the name, port and
  credentials, and every reply code other than 0 fails the client with
  that code
- for every `-onlynet` set of IPv4, IPv6, Tor and I2P, with and without
  a proxy and a SAM bridge, a network that isn't named is never
  reachable, Tor is reachable only with a proxy, I2P only with a bridge,
  and the only refusal is `-onlynet=i2p` without a bridge

<!-- source: bitcoin/examples/checks/proxy.coax check -->
```jai
// Any key's onion name reads back to the key, and changing any one of its
// 56 characters to another is refused: the checksum or the version
// catches it.
#forall "an onion name round-trips, and one changed character is refused" k: 0..<8 #steps 2000000000 {
  seed := [1] u8.[cast,trunc(u8) k]; key: [32] u8;
  #assert sha256_hash_checked(seed[..], key[..], false) == 0;
  t: [80] u8; n := px_onion_text(key[..], t[..]);
  x := px_set_special(cast(string) t[0..<n]);
  #assert n == PX_ONION_TEXT && x.ok && x.addr.net == AM_NET_ONION;
  for i: 0..<32 { #assert x.addr.ip[i] == key[i]; }
  for i: 0..<56 {
    keep := t[i];
    t[i] = ifx keep == 'a' then 'b' else 'a';
    #assert !px_set_special(cast(string) t[0..<n]).ok;
    t[i] = keep;
  }
}
// Any 32-byte hash is an I2P name of 52 characters that reads back to it,
// in either case.
#forall "an I2P name round-trips in either case" k: 0..<64 {
  seed := [1] u8.[cast,trunc(u8) k]; h: [32] u8;
  #assert sha256_hash_checked(seed[..], h[..], false) == 0;
  a := am_bytes(AM_NET_I2P, h[..], 0);
  t: [80] u8; n := px_addr_text(a, t[..]);
  #assert n == PX_I2P_TEXT && am_same(px_set_special(cast(string) t[0..<n]).addr, a);
  for i: 0..<n { if t[i] >= 'a' && t[i] <= 'z' { t[i] -= 32; } }
  #assert am_same(px_set_special(cast(string) t[0..<n]).addr, a);
}
// The client and the proxy agree for every name length SOCKS5 allows,
// with and without credentials, and the client stops at a refusal.
#forall "SOCKS5: every name length, with and without credentials" k: 0..<256 {
  name: [255] u8; for i: 0..<k { name[i] = cast,trunc(u8) (97 + i % 26); }
  for auth: 0..<2 {
    s: [1] PxSocks; p: [1] PxProxy; out: [600] u8;
    user := ifx auth == 1 then "0011223344556677-9" else "";
    st := px_socks_begin(s[..], cast(string) name[0..<k], 65535 - k, user, user, auth == 1, out[..]);
    rep := ifx k % 17 == 0 then 0 else ifx k % 2 == 0 then 0xf0 + k % 8 else k % 9;
    r := px_socks_pump(s[..], p[..], out[0..<st.send], rep);
    #assert p[0].dest_n == k && p[0].port == 65535 - k && p[0].user_n == user.count && p[0].pass_n == user.count;
    for i: 0..<k { #assert p[0].dest[i] == name[i]; }
    if rep == 0 { #assert r == PX_S_DONE; } else { #assert r == PX_S_FAILED && s[0].error == PX_E_CONNECT && s[0].reply == rep; }
  }
}
// For every -onlynet set, with or without a proxy and a SAM bridge, a
// network not named is never reached, and a named one is reached unless
// Core refused the options.
#forall "-onlynet: unnamed networks are never reached" k: 0..<64 {
  names := [4] string.["ipv4", "ipv6", "onion", "i2p"]; nets := [4] int.[AM_NET_IPV4, AM_NET_IPV6, AM_NET_ONION, AM_NET_I2P];
  only: [4] string; m := 0;
  for b: 0..<4 { if (k >> b) & 1 == 1 { only[m] = names[b]; m += 1; } }
  proxies := [1] string.["127.0.0.1:9050"]; empty: [1] string;
  n := px_configure(only[0..<m], ifx (k >> 4) & 1 == 1 then proxies[..] else empty[0..<0], "", true, (k >> 5) & 1 == 1, false, -1);
  for b: 0..<4 {
    named := m == 0 || (k >> b) & 1 == 1;
    if n.code == PX_C_OK && !named { #assert !n.reachable[nets[b]]; }
    if n.code == PX_C_OK && named && b < 2 { #assert n.reachable[nets[b]]; }
  }
  if n.code == PX_C_OK && ((k >> 2) & 1 == 1 || m == 0) { #assert n.reachable[AM_NET_ONION] == ((k >> 4) & 1 == 1); }
  if n.code == PX_C_OK { #assert n.reachable[AM_NET_I2P] == ((k >> 5) & 1 == 1 && (m == 0 || (k >> 3) & 1 == 1)); }
  if n.code != PX_C_OK { #assert m > 0 && ((k >> 3) & 1 == 1 && (k >> 5) & 1 == 0); }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/proxy.coax --run-steps 250000000
```

`coax test` runs 312 checks, including the loaded files' own checks, on
2026-11-03.

### The library's own checks

`proxy.coax` and `sha3.coax` are `#authority` files. Their checks cover
SHA3-256 against NIST's examples and Python's `hashlib` at the padding
boundaries; Core's base32 with RFC 4648's examples and each padding
refusal; the three rend-spec onion names and Core's `net_tests.cpp`
cases, valid and not; I2P names in either case and Core's refusals;
BIP-155's bytes for Tor v3 and I2P from `net_tests.cpp`; every step and
refusal of the SOCKS5 client, every reply text, the isolation
credentials and the test proxy's refusals; I2P's base64 and the
destination in Core's `i2p_tests.cpp` private key; SAM reply parsing; a
transient and a persistent session, a stream each way and each failure
Core distinguishes; and the network options, their errors and the route
to each network.

```shell
coax/bin/coax test coax/lib/bitcoin/proxy.coax --run-steps 250000000
coax/bin/coax test coax/lib/crypto/sha3.coax --run-steps 250000000
```

`coax test` runs 308 checks for `proxy.coax`, including those of the
files it loads, and 3 for `sha3.coax`, on 2026-11-03. Mutation testing:
not run (owner speed order).

### An independent model

[proxy.py](../../coax/tests/bitcoin/proxy.py) computes every line the
example prints with Python's own base32, base64, SHA3-256 and SHA-256.
It follows the pinned rend-spec text for onion names, `netbase.cpp`'s
`Socks5` against a proxy written from RFC 1928 and RFC 1929, `i2p.cpp`
for the SAM exchanges and `init.cpp` for the network options, and checks
that the pinned sources say what it assumes. It shares no code with
Coax.

```shell
python3 -I coax/tests/bitcoin/proxy.py --coax coax/bin/coax
```

On 2026-11-03 all 77 lines agree, and the example's output in native
debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Tor's control port: authenticating with a cookie, `ADD_ONION` to make
  the node's onion service, and learning Tor's SOCKS port. That is how
  `-listenonion` works.
- Private broadcast (`-privatebroadcast`), which sends the node's own
  transactions over short-lived Tor or I2P connections.
- CJDNS beyond being reachable or not.
- The sockets themselves: timeouts, interrupts, and a proxy on a Unix
  socket.
- Choosing which address to announce to a peer (`GetLocalAddress`),
  and the address manager's limits for each network.
- SOCKS4, other SOCKS5 commands, and replies with an IPv6 or name
  address that Core reads and drops.

## Summary

- A Tor v3 address is an ed25519 key with a SHA3-256 checksum and
  version 3, in base32; an I2P address is the SHA-256 of a destination.
  BIP-155 carries the 32 bytes of each.
- Core's SOCKS5 client sends host names, never addresses, and with
  stream isolation new credentials for each connection, so that Tor uses
  a new circuit.
- Core speaks SAM 3.1 to an I2P router, with port 0 everywhere, and tells
  a peer's failure from the router's by the reply.
- `-onlynet`, `-proxy`, `-onion` and `-i2psam` decide which networks are
  reachable and how, and Core refuses the combinations that leave a
  named network unreachable.

## Dependencies

- [68. The handshake and feature negotiation](ch068-the-handshake-and-feature-negotiation.md)
- [70. Connections: outbound, block-relay-only, feelers and eviction](ch070-connections-outbound-block-relay-only-feelers-and-eviction.md)

## Sources

- Tor's rend-spec,
  [Encoding onion addresses](https://spec.torproject.org/rend-spec/encoding-onion-addresses.html)
  (rend-spec-v3 section 6), at torspec revision `7391a5fb`, pinned in
  [coax/tests/vectors/upstream/tor/](../../coax/tests/vectors/upstream/tor/encoding-onion-addresses.md).
- I2P's [SAM v3 specification](https://geti2p.net/en/docs/api/samv3), at
  i2p.www revision `3afd7340`, pinned in
  [coax/tests/vectors/upstream/i2p/](../../coax/tests/vectors/upstream/i2p/samv3.html).
- [RFC 1928](https://www.rfc-editor.org/rfc/rfc1928) (SOCKS5) and
  [RFC 1929](https://www.rfc-editor.org/rfc/rfc1929) (username and
  password), pinned in
  [coax/tests/vectors/upstream/rfc/](../../coax/tests/vectors/upstream/rfc/rfc1928.txt).
- [BIP-155](https://github.com/bitcoin/bips/blob/master/bip-0155.mediawiki),
  pinned by chapter 65.
- Bitcoin Core at revision `ddea9196`: `i2p.cpp`, `i2p.h`, `netbase.h`,
  `init.cpp`, `torcontrol.h`, `util/strencodings.cpp`,
  `test/net_tests.cpp` and `test/i2p_tests.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/i2p.cpp)
  by this chapter; `netbase.cpp`, `netaddress.cpp`, `netaddress.h` and
  `net.cpp`, pinned there by earlier chapters.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
