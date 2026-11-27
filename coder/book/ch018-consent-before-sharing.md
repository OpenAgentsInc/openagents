# 18. Consent before sharing

> **Status: partly implemented.** OpenAgents enforces a hash-chained consent
> ledger (`coax/lib/agent/engine/consent-gate.coax`, `coder/tui/gate.coax`,
> Protocol C12 fixture `proto/coder/consent-memory-ledgers-v1.json`) gating
> outbound routing and session content sent to System One, paired with
> `#authority` bypass-immune spending and signing checks in
> `openagents/core/members.coax`. Exposing grant/revocation controls directly in
> OpenAgents Desktop UI and extending consent envelopes to every third-party
> cloud provider remain open (see [Chapter 42](ch042-beyond-the-desktop-app.md)).

Session transcripts contain private code, user preferences, tool outputs, and
identities. Nothing should leave the machine for optional routing, delegation,
or export unless a recorded, time-bounded, class-scoped grant covers every
sensitivity class in the payload—and the refusal or send is appended to a
hash-chained ledger before any bytes move.

OpenAgents implements consent enforcement across two production layers:

1. **Hash-chained consent gate (`coax/lib/agent/engine/consent-gate.coax`,
   `coder/tui/gate.coax`, backed by `psionic/lib/harness/consent.coax`):**
   - Stores `~/.openagents/coder/consent/events.jsonl` via one canonical JSONL
     codec (`gt_line` / `gt_unline`, checked by
     [`coax/lib/agent/engine/ledger-profile.coax`](../../coax/lib/agent/engine/ledger-profile.coax)
     against [`proto/coder/consent-memory-ledgers-v1.json`](../../proto/coder/consent-memory-ledgers-v1.json)).
   - Before `cg_route` sends a task to remote System One (`GT_SYSTEM_ONE`),
     `cg_session_to_so` replays the ledger (`cs_replay`), checks active grants
     covering the payload's sensitivity classes (`session`, `user`, `project`,
     `tool`, `identity`), appends either the covered send or the refusal event,
     and fails closed if the ledger cannot be verified or written.
   - Supports minimization (`cs_minimize`) and redaction (`cs_redact`) so
     ungranted classes can be stripped before resending, plus grantor-only
     revocation (`cs_revoke`) that records honest accounting (`sends`, `bytes`)
     of what already left the machine rather than pretending past sends can be
     recalled.
2. **Member-pack bypass-immune approval boundary (`openagents/core/members.coax`):**
   - `oa_member_is_bypass_immune` ensures irreversible outbound value/identity
     actions—`WalletPay` (satoshi spending), `OpenAgentsCli vault` secret
     reads, and `OpenAgentsCli nostr` signing/publishing—cannot be silently
     auto-approved even when the user enables `BYPASS` or `AUTO` mode.

## Profile

This chapter's profile is `cs-consent-v1 + consent-gate.coax + openagents/core/members.coax; coder consent gates System One routing and remembered facts`:

- Sends without a covering grant are refused (`none`) and never enter the sink.
- Grants are scoped by destination, purpose, class bitmask, and tick window
  (`start..<until`).
- Mixed-class messages fail with `classes`, minimize to covered parts, and
  succeed after `cs_redact`.
- Only the grantor may revoke a grant; revocation records prior `sends` and
  `bytes`; subsequent sends fail with `revoked`.
- `cs_verify` and `cs_replay` validate the entire SHA-256 chain and reject
  single-field tampering (`digest`).
- `WalletPay` and `nostr publish` remain bypass-immune in
  `openagents/core/members.coax`.

## Worked example

The runnable example [consent-gate.coax](../examples/consent-gate.coax) drives
the shared `CsLedger` consent engine backing `consent-gate.coax` and
`coder/tui/gate.coax` alongside `openagents/core/members.coax`.

<!-- source: coder/examples/consent-gate.coax example -->
```jai
main :: () {
  // 1. Initialize consent ledger & session content graph (System One destination = tool 1, call)
  st: CtStore;
  l := [1] CsLedger.[ct_open(st.events[..], st.grants[..])];
  g := [1] CsGraph.[ct_graph(st.texts[..], st.own[..], st.labels[..])];
  s := [1] CsSink.[ct_sink(st.bytes[..], st.ends[..], st.sent[..])];
  ct_session(g[..]);
  sys_one := CsDest.{kind = CsKind.tool, id = 1, purpose = CsPurpose.call};

  // 2. Outbound send refused before any grant is recorded
  p4 := [1] int.[4];
  r0 := cs_send(l[..], g[0], s[..], CsMessage.{dest = sys_one, parts = p4[..]}, 1);
  print("t1 send without grant: why=% sink_msgs=%\n", why_name(r0.why), s[0].count);

  // 3. Person 1 grants session+project to System One for ticks 2..<20 -> send succeeds
  g_ok := cs_grant(l[..], sys_one, CT_SESSION | CT_PROJECT, 2, 20, 1, 2);
  r1 := cs_send(l[..], g[0], s[..], CsMessage.{dest = sys_one, parts = p4[..]}, 3);
  print("t2-t3 grant=% send_covered=% sink_msgs=%\n", why_name(g_ok), why_name(r1.why), s[0].count);

  // 4. Message mixing ungranted user-class node 2 is refused; minimize + redact strips node 2
  both := [2] int.[4, 2];
  m := CsMessage.{dest = sys_one, parts = both[..]};
  r2 := cs_send(l[..], g[0], s[..], m, 4);
  keep: [4] int;
  k := cs_minimize(l[0], g[0], m, 4, keep[..]);
  bn := cs_redact(l[0], g[0], m, 4, st.redacted[..]);
  rn := cs_node(g[..], cast(string) st.redacted[0..<bn], 0, keep[0..<k]);
  pr := [1] int.[rn];
  r3 := cs_send(l[..], g[0], s[..], CsMessage.{dest = sys_one, parts = pr[..]}, 4);
  print("t4 mixed=% minimized_parts=% redacted_send=% sink_msgs=%\n",
    why_name(r2.why), k, why_name(r3.why), s[0].count);

  // 5. Only grantor (person 1) may revoke; revocation records already-sent accounting
  rv_bad := cs_revoke(l[..], 0, 2, 7);
  rv_ok := cs_revoke(l[..], 0, 1, 7);
  rev_ev := st.events[l[0].count - 1];
  r4 := cs_send(l[..], g[0], s[..], CsMessage.{dest = sys_one, parts = p4[..]}, 8);
  print("t7-t8 wrong_person=% revoke=% prior_sends=% prior_bytes=% after_revoke=%\n",
    why_name(rv_bad), why_name(rv_ok), rev_ev.sends, rev_ev.bytes, why_name(r4.why));

  // 6. Hash chain verifies, replays cleanly, and refuses a tampered event digest
  v := cs_verify(l[0]);
  st2: CtStore;
  u := [1] CsLedger.[ct_open(st2.events[..], st2.grants[..])];
  rp := cs_replay(l[0], u[..]);
  hb: [16] u8;
  print("ledger: verify=% events=% replay=% head=%\n",
    why_name(v.why), v.index, why_name(rp.why), hex8(st.events[l[0].count - 1].digest, hb[..]));
  st.events[2].bytes += 1;
  bad := cs_verify(l[0]);
  print("tamper verify: why=% at_event=%\n", why_name(bad.why), bad.index);

  // 7. Member-pack consent parity: Nostr signing & WalletPay stay bypass-immune
  print("member bypass-immune: WalletPay=% nostr_publish=% nostr_status=%\n",
    oa_member_is_bypass_immune("WalletPay", "lnbc100n"),
    oa_member_is_bypass_immune("OpenAgentsCli", "nostr publish hello"),
    oa_member_is_bypass_immune("OpenAgentsCli", "nostr status"));
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/consent-gate.coax
```

```text
t1 send without grant: why=none sink_msgs=0
t2-t3 grant=ok send_covered=ok sink_msgs=1
t4 mixed=classes minimized_parts=1 redacted_send=ok sink_msgs=2
t7-t8 wrong_person=person revoke=ok prior_sends=2 prior_bytes=86 after_revoke=revoked
ledger: verify=ok events=7 replay=ok head=b99897b05c56ca95
tamper verify: why=digest at_event=2
member bypass-immune: WalletPay=true nostr_publish=true nostr_status=false
```

The checked fixture is [consent-gate.out](../examples/consent-gate.out).

## Live CLI & protocol fixture

To verify the on-disk JSONL codec (`gt_line` / `gt_unline`) against real
recorded CLI ledgers and refused malformed lines:

```shell
coax/web/tool.sh coax/lib/agent/engine/ledger-profile.coax proto/coder/consent-memory-ledgers-v1.json
```

## What remains

- Desktop UI panel for viewing, granting, and revoking per-destination consent
  entries without dropping to the CLI.
- Extending consent class labels to every external cloud model and plugin
  transport.

## Done means

- [x] `consent-gate.coax`, `coder/tui/gate.coax`, and `ledger-profile.coax`
  enforce hash-chained grants, minimization, redaction, revocation accounting,
  and replay checks on disk.
- [x] `coder/examples/consent-gate.coax` matches `coder/examples/consent-gate.out`
  in debug and release.
- [ ] Desktop Settings/Privacy UI manages live consent grants and revocations.
