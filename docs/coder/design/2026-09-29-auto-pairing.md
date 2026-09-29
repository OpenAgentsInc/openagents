# Automatic pairing without Tailscale

Status: proposal, 2026-09-29. Nothing in this document is implemented yet;
the plan at the end names the issues to open.

## Why

Pairing a phone with a computer today takes the steps in
[Link your devices](../guides/link-devices.md): install and sign in to
Tailscale on every device, turn on HTTPS certificates in the Tailscale admin
console, create an owner key, run `scripts/link-device.sh` on each computer,
mint an invitation with `coder link invite`, and scan or paste it on the phone
within five minutes. The 2026-09-29 Android validation
([PR #9964](https://github.com/OpenAgentsInc/openagents/pull/9964)) found
that even that last step breaks when the phone's clock is a few seconds
behind the computer's.

The model underneath is right and stays: the host is the only issuer of
access, a device holds a host-signed grant with typed rights
([NIP-HOST](../../../nips/openagents/NIP-HOST.md)), and a route or network
membership never grants anything
([NIP-REACH](../../../nips/openagents/NIP-REACH.md)). What has to go is the
ceremony around it. The goal:

1. On a computer: install one Rust program (`openagents`, or the desktop app
   that embeds it) and run one command. No Tailscale, no certificates, no
   owner-key step, no script.
2. On the phone: open the app, and the computer either shows up on its own
   (same network) or after one scan or one six-digit code (anywhere).
3. Everything after that — terminals, `computer exec`, `task.create` — uses
   the same grants and rights as today.

This document reviews what [iroh](https://docs.rs/iroh/latest/iroh/) and
[T3 Code](https://github.com/pingdotgg/t3code) do, and turns that into a
plan for this repository.

## What we have

| Piece | Today | Problem |
| --- | --- | --- |
| Transport | Tailscale WebSocket on port 47101 (`wss` with `tailscale cert`, or plain `ws`); `wss://relay.openagents.com/` as fallback for enrollment and control messages. | Every device needs Tailscale signed in to the same tailnet; certificates need an admin-console toggle; the relay fallback is a store-and-forward path, not a stream. |
| Identity | Owner key (one file), host key, device key. | Right split, but the owner key is a manual step the person has to understand before anything works. |
| Enrollment | `invite.create` → QR or `coder-host:` string → `enroll.redeem` over the relay within five minutes. | Manual on both ends; one invitation per computer; clock-sensitive. |
| Same-network shortcut | [Tailnet admission](../../../nips/openagents/NIP-HOST.md#tailnet-admission): the host hands an invitation to a caller whose Tailscale identity is the machine owner. | Depends on Tailscale identity. |
| Rights | `observe`, `operate`, `terminal`, `review`, `access_read`; grant epochs; per-message recheck; revocation. | Keep as is. |
| Terminals and exec | [NIP-TERM](../../../nips/openagents/NIP-TERM.md) over the direct channel. | Keep as is; only the channel underneath changes. |

## iroh

iroh (`iroh` 1.1 on crates.io) is a Rust library that gives a process an
`Endpoint` with a stable Ed25519 `SecretKey`; the public key is the
`EndpointId`. `Endpoint::connect(addr, alpn)` returns a QUIC connection
whose remote `EndpointId` is authenticated during the TLS handshake. The
application decides whether that peer is authorized. Connections start
through the peer's home relay and migrate to a direct path when hole
punching succeeds; when it does not, they stay on the relay. Relay traffic is
QUIC-encrypted end to end, so a relay cannot read payloads. ALPN strings
separate application protocols on one endpoint, and each connection carries
ordinary QUIC bidirectional and unidirectional streams.

`AddressLookup` resolves an `EndpointId` to addresses. The `iroh` crate
ships an in-memory lookup and DNS/PKARR publishing; mDNS (local network) and
Mainline DHT lookups are separate crates. With a lookup configured a caller
needs only the `EndpointId`.

What this buys us:

- **No Tailscale.** A phone and a computer behind two NATs connect by public
  key, with hole punching and encrypted relay fallback, and nothing to sign
  in to.
- **Authenticated streams.** The direct channel in NIP-REACH exists to prove
  both keys over a WebSocket and then carry frames. A QUIC connection whose
  remote is a known `EndpointId` already proves the peer; NIP-REACH frames
  become QUIC streams.
- **Same-network discovery.** mDNS lookup lets a phone see computers on the
  same Wi-Fi with no rendezvous at all.
- **Rust on every side.** `iroh` builds for Linux, macOS, Android, and iOS,
  so the same `openagents-connect` code runs in `openagents`, the desktop
  app, `crates/openagents-mobile`, and a host.

What it does not do, and where our contracts stay in charge:

- iroh authenticates a key; it does not say who the key belongs to. A
  connection from an unknown `EndpointId` gets exactly one thing from a
  host: the enrollment protocol. Every other ALPN is refused until that key
  holds a grant.
- iroh's default relays are run by n0. Payloads are encrypted, but the relay
  learns which endpoints talk. Run our own relay (`iroh-relay` is a binary in
  the same project) beside `relay.openagents.com`, and let the endpoint
  configuration name it. Direct addresses work with no relay at all.
- iroh's `EndpointId` is Ed25519; Nostr keys are secp256k1. The host and
  device keys of NIP-HOST stay Nostr keys, because grants, relay events, and
  every existing artifact are signed with them. Each endpoint binds its
  `EndpointId` to its Nostr key once, in a signed statement that the other
  side stores with the grant (see [Identity](#identity)).

## T3 Code

T3 Code (`pingdotgg/t3code`) is a TypeScript agent-control surface with a
local server and iOS, Android, web, and Electron clients; the shape of its
pairing is worth copying, the implementation is not (this repository is
Rust).

- **One command pairs.** `npx t3 pair` finds the running local server, mints
  a one-time pairing token, and prints a URL and a QR code. `npx t3 serve`
  on a headless box prints the same three things at start.
- **The server is the boundary.** Clients never run provider processes,
  terminals, or git; they hold a token and call a typed RPC over WebSocket.
  This matches NIP-HOST's host-as-only-issuer.
- **Scopes, not roles.** Ordinary pairing grants `orchestration:read`,
  `orchestration:operate`, `terminal:operate`, `review:write`; the
  administrative `access:*` and `relay:*` scopes need a separate step. This
  is our rights list under another name, and confirms that a default
  pairing should not hand out `access_read`.
- **Token in the URL fragment.** Their hosted pairing URL is
  `https://app.t3.codes/pair?host=HOST#token=CODE`; the fragment never
  reaches the hosted origin, the client exchanges it with the backend
  directly, then strips it from history.
- **Short-lived tickets for streams.** A bearer token authorizes an HTTP
  call that returns a short-lived WebSocket ticket; the socket never sees
  the long-lived token.
- **Tailscale is one endpoint provider among several.** LAN HTTP, custom
  HTTPS, SSH port forward, a future tunnel, and Tailscale Serve are all ways
  to reach the same server; none is required. Their SSH flow probes the
  host, starts or reuses a server, and forwards a port.

Where we can do better than T3: their pairing still needs the client to
reach the server's HTTP endpoint, so off-LAN pairing needs Tailscale, a
tunnel, or SSH. With iroh, the QR code carries an `EndpointId`, and
reachability is the library's problem.

## Design

### Components

- **`openagents connect`** (in `crates/openagents-cli`, library code in a
  new `crates/openagents-connect`): the iroh endpoint, the enrollment ALPN,
  the channel ALPN, key binding, and the pairing UI (QR, code, discovery).
  `coder host serve` and the desktop app embed it; `crates/openagents-mobile`
  uses the same crate through the existing Rust core, so the Kotlin and
  Swift hosts change only to render a QR scanner they already have.
- **Host side:** the existing NIP-HOST host gains an iroh listener beside
  the WebSocket listener. It keeps its host Nostr key and adds an iroh
  `SecretKey` in the same key store.
- **Device side:** the phone and `openagents` on another computer keep their
  device Nostr key and add an iroh `SecretKey`.

### Identity

Every endpoint holds two keys and a *binding*:

```text
openagents.key-binding.v1 = {
  v: 1, requires: [],
  nostr:  <64-hex x-only pubkey>,
  iroh:   <64-hex Ed25519 EndpointId>,
  issued_at: <unix seconds>,
  sig_nostr: <schnorr over the canonical body>,
  sig_iroh:  <ed25519 over the same body>
}
```

Both signatures are required, so neither key can claim the other. A host
stores the device's binding next to its grant; a device stores the host's
binding next to the host record. A channel is admitted when the QUIC
remote `EndpointId` equals the `iroh` field of a binding whose `nostr` key
holds a current grant. Rotating either key means enrolling again.

### Automatic pairing on the same network

1. The host publishes itself over mDNS (iroh's local lookup) under a service
   name that carries only its `EndpointId` and a display label. No rights,
   no owner data.
2. The phone lists nearby hosts in **Computers** as soon as the tab opens.
   Tapping one opens an iroh connection with the `openagents/enroll/1` ALPN
   and sends an enrollment request: the device binding, a label, and the
   rights it asks for (default `observe,operate,terminal`).
3. The host shows the request on its own screen — a notification from the
   desktop app, or a line from `openagents connect approve` in a terminal —
   with the device label, a six-character fingerprint of the device key, and
   the rights. The same fingerprint shows on the phone. The person compares
   and approves on the computer. A host with no interactive surface keeps
   the request for two minutes and lets `openagents connect approve CODE`
   admit it.
4. The host signs an ordinary NIP-HOST grant and returns it, plus its own
   binding, on the same connection. The phone stores both. From here the
   channel ALPN `openagents/reach/1` carries NIP-REACH frames, NIP-TERM, and
   everything else, exactly as the WebSocket channel does now.

The approval on the computer is the trust step; discovery only shortens
finding the peer. There is no automatic approval of an unknown key. The one
exception is the loopback case below.

### Pairing one computer with itself and with another computer

- `openagents connect` on the same machine as its host connects over the
  Unix socket the host already exposes, and the host admits it as the owner
  session without a grant — the person is already logged in to that
  machine. This replaces `coder link owner init` for the common case: the
  first host a person runs *is* the owner, and its owner key is created and
  kept by the host's key store. `openagents connect owner export` prints
  the public key for hosts on other machines.
- `openagents connect --ssh user@box` reuses T3's shape: probe, install or
  update the `openagents` binary, start the host, and run the enrollment
  over the SSH channel's standard streams as `coder link peer --ssh` does
  now. The remote host takes the local owner's public key in the same
  exchange, so the owner key still never leaves the first machine.

### Pairing from anywhere

When the phone is not on the computer's network, the computer shows a QR
code and a code word:

```sh
openagents connect invite --rights observe,operate,terminal
```

The QR carries `oa-pair:` followed by the host `EndpointId`, an optional
relay URL, and a 128-bit one-time secret. The phone connects by
`EndpointId` (relay or direct, iroh decides) and proves the secret in the
enrollment request; the host treats a valid secret as pre-approval, so no
second confirmation is needed on the computer. The secret is single-use and
lives ten minutes. The same string prints as a paste-able line, and the
desktop app shows it in a window. A person who has neither camera nor
clipboard types the host's short code (first eight characters of the
`EndpointId`) into the phone; the phone then finds the host through the
relay's lookup and the host asks for on-screen approval as in the
same-network flow.

### Rights and approval

Unchanged from NIP-HOST: `observe`, `operate`, `terminal`, `review`,
`access_read`; grants carry an epoch, the host rechecks on every message,
and revocation closes channels at once. Two rules from this design:

- Default pairing grants `observe,operate,terminal`. `review` and
  `access_read` need `--rights` on the inviting or approving side.
- A grant that admits `terminal` is displayed on the computer at approval
  time as "Open terminals on this computer" in the same words the phone
  uses, so the person approving sees what the phone will be able to do.

### Time

Invitations and requests keep `issued_at` and `expires_at`. Readers admit an
`issued_at` up to 60 s in the future (`coder_connect::protocol::CLOCK_SKEW`,
from PR #9964) and hold `expires_at` strictly. The host, not the device,
decides expiry, so a slow phone clock cannot extend a grant. The enrollment
reply includes the host's `now`, and the phone shows a warning when its own
clock is more than 60 s away from it, so the next failure of this kind names
itself instead of reading as "expired".

### Reachability and fallback

Order of preference for a channel, all decided by iroh from one
`EndpointId`:

1. Direct address on the same network (mDNS or a remembered address).
2. Hole-punched direct path across NATs.
3. Encrypted relay path through our `iroh-relay`, with n0's public relays
   off by default and available as an operator opt-in.
4. Tailscale, for people who already run it: a tailnet address is one more
   direct address a host publishes. Nothing else changes, and Tailnet
   admission stays as a documented option that hands out the same
   invitation.

`wss://relay.openagents.com/` remains the Nostr relay for everything that is
an event today (directory, presence, `enroll.redeem` for the legacy flow,
NIP-CJ). It stops being the fallback for the channel once the iroh listener
ships.

### Recovery and revocation

- `openagents connect devices` and `openagents connect revoke DEVICE` are
  the existing `device.list` and `device.revoke` over the new channel.
- Losing a phone: revoke it from any computer; the grant epoch moves and
  its channels close.
- Losing the owner computer: the owner key is the same file it is today
  (`~/.openagents/coder-owner/owner.key`), and the `wallet backup` shape
  from [PR #9826](https://github.com/OpenAgentsInc/openagents/pull/9826)
  extends to it. A host established with one owner still refuses another.
- Reinstalling a host: it keeps its iroh and Nostr keys in the same store,
  so paired phones reconnect without pairing again.

### Compatibility

The WebSocket channel and `coder-host:` invitations keep working until every
shipped client speaks the iroh channel. A host advertises both in its
NIP-REACH hints; a client prefers iroh when it has the host's binding. No
change to grants, rights, or `NIP-TERM` frames is needed, so a device
enrolled the old way gets the new channel the next time it connects and
receives the host's binding in the presence answer.

## Plan

Each step is one PR and one issue. Steps 1 through 3 give same-network
pairing with no Tailscale; 4 and 5 give pairing from anywhere; 6 and 7
remove the remaining ceremony.

1. **`crates/openagents-connect`: endpoint, binding, ALPNs.** Add `iroh`
   1.1 (pin a version at least seven days old), the key store, the
   `key-binding.v1` artifact with tests, the `openagents/enroll/1` and
   `openagents/reach/1` ALPNs, and NIP-REACH frames over QUIC streams with a
   loopback test. No relay configured; direct addresses only.
2. **Host listener and mDNS.** `coder host serve` binds the endpoint,
   publishes over local lookup, answers enrollment with on-terminal approval
   (`openagents connect approve`), and admits the channel by binding.
   `openagents connect` on the same machine goes over the Unix socket as the
   owner.
3. **Phone: nearby computers and approval fingerprint.** `openagents-mobile`
   lists mDNS hosts in **Computers**, sends the enrollment request, shows
   the fingerprint, stores binding and grant, and uses the iroh channel for
   NIP-TERM. Validate on the Android emulator against a host on the same
   box, with the fixed CLI-output card.
4. **Relay.** Deploy `iroh-relay` beside `relay.openagents.com`, configure
   it in the endpoint preset, and test a phone on mobile data pairing with a
   host behind NAT. Record the deploy in `docs/deployment/`.
5. **`openagents connect invite` and the `oa-pair:` QR.** One-time secret
   as pre-approval, short-code entry, ten-minute expiry, clock warning.
   Retire `coder link invite` to an alias.
6. **Owner without a step, and `--ssh`.** The first host creates and keeps
   the owner key; `openagents connect --ssh` installs and enrolls a second
   computer in one command. Retire `scripts/link-device.sh` to a wrapper.
7. **Desktop app surface.** The Rust desktop app shows approval requests,
   the invite QR, and the device list through the same `openagents-connect`
   API. Update [Link your devices](../guides/link-devices.md) to the new
   flow and move the Tailscale text to an "If you already use Tailscale"
   section.

Write the protocol parts (binding, ALPNs, enrollment over a stream, the
`oa-pair:` string) into NIP-HOST and NIP-REACH as a new section in step 1
and 5 respectively, so the NIPs stay the source of truth.

## Acceptance

- A fresh macOS or Linux computer with `openagents` installed and a phone on
  the same Wi-Fi pair in under a minute with one command on the computer and
  one tap plus one approval, with Tailscale not installed on either.
- The same phone on mobile data pairs with a NATed computer from one QR
  scan, with no Tailscale, and opens a terminal that runs `openagents --json
  verse who` on the computer.
- A phone whose clock is 30 s behind the computer pairs; a phone offered an
  invitation eleven minutes old is refused with `expired`.
- A device without `terminal` in its grant is refused a terminal by the
  host, not by the phone.
- Revoking a device on the computer closes its open channel within one
  round trip.
- A device enrolled with a `coder-host:` invitation before this work
  connects over the iroh channel after the host and app update, with no
  new pairing.

## Open questions

- Whether to also bind a Tailscale address as a direct address by default
  when `tailscale` is present, or only when asked.
- Whether the iroh relay should run in the same Cloud Run service as the
  Nostr relay or as its own service; iroh's relay is a long-lived QUIC/HTTPS
  server and Cloud Run's request model may not fit.
- iOS background behavior: iroh keeps a UDP socket, and iOS suspends it. The
  phone reconnects on foreground today with the WebSocket channel; confirm
  the same holds with QUIC before step 3 closes.
