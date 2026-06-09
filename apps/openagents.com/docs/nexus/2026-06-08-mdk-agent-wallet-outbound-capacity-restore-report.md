# MDK Agent-Wallet Outbound Capacity Restore Report

Date: 2026-06-08

Status: live incident report for the Artanis/Pylon paid GEPA smoke.

## Summary

The failed send was executed through Money Dev Kit:

```bash
npx -y @moneydevkit/agent-wallet@latest send <redacted_bolt11_invoice>
```

The installed package resolved to `@moneydevkit/agent-wallet@0.20.0`.

The command failed with MDK's own bounded error:

```json
{"error":"insufficient outbound capacity: required 1000msat, available 0msat"}
```

The payer wallet had a positive sats balance, but the failed run had restored
the payer by using mnemonic material in a fresh agent-wallet home. That fresh
home generated a new `walletId`, so it did not reuse the original payer wallet
home and active channel/LSP state that had successfully sent the #503 and #504
payments.

When the smoke was rerun with the original payer wallet home, MDK reported
usable outbound capacity in its daemon log and the same package completed two
1-sat Lightning payments for accepted Pylon work.

## What Was Tried

The first paid GEPA smoke created:

- `pylon.artanis.gepa_paid.20260608214500.1`
- `assignment.public.probe_gepa.paid_multi_pylon.20260608214500.1`

That assignment reached accepted work before payment dispatch. The payer send
then failed before any public settlement bridge receipt was created.

The failed run used a fresh wallet home with the old payer mnemonic. The
resulting payer config had a new wallet identifier, while the previously
successful payer wallet home retained the original wallet identifier and daemon
state.

## MDK Documentation Expectation

The current MDK agent-wallet docs describe the wallet as a self-custodial
Lightning wallet for agents, say the daemon connects to MDK Lightning
infrastructure, and state that no node management is required. The package
README also says receive uses LSPS4 JIT channels and that the daemon handles
payment polling locally.

The docs do not describe this restore boundary:

```text
Mnemonic-only restore can show a positive sats balance while lacking the
outbound channel state needed to send even a 1-sat BOLT11 payment.
```

If MDK intends mnemonic restore to be enough for agent send flows, this is an
MDK behavior bug. If MDK requires the original wallet home, wallet id, VSS
state, or channel state to preserve outbound capacity, the agent-wallet docs
need to state that directly and expose a preflight field or command that agents
can check before attempting a paid action.

## Source Evidence

The TypeScript CLI layer in `@moneydevkit/agent-wallet@0.20.0` sends through:

```text
WalletServer.handleSend -> node.payWhileRunning(destination, amountMsat, 0)
```

It does not preflight or repair outbound capacity before calling the underlying
Lightning runtime. On synchronous failure it returns `SEND_FAILED` with the
runtime message.

The local MDK source tree also contains an integration-test comment for a
different daemon path saying that with no LSP and no channels there is zero
outbound capacity and dispatch can fail synchronously. That matches the shape
of the observed error.

## Successful Workaround

The paid smoke was recovered by using the original payer wallet home and its
running daemon, not mnemonic-only restore.

The same MDK package then settled two accepted GEPA assignments:

| Pylon | Assignment | Receipt | Amount |
| --- | --- | --- | --- |
| `pylon.artanis.gepa_paid.20260608214500.1` | `assignment.public.probe_gepa.paid_multi_pylon.20260608214500.1` | `receipt.nexus_pylon.settlement.assignment_public_probe_gepa_paid_multi_pylon_20260608214500_1` | 1 sat |
| `pylon.artanis.gepa_paid.20260608214500.2` | `assignment.public.probe_gepa.paid_multi_pylon.20260608214500.2` | `receipt.nexus_pylon.settlement.assignment_public_probe_gepa_paid_multi_pylon_20260608214500_2` | 1 sat |

Both public receipt APIs returned:

- `receiptKind: settlement_recorded`
- `realBitcoinMoved: true`
- `movementMode: real_bitcoin`
- `state: settled`

The public Pylon stats endpoint also updated after the smoke:

- `pylonsOnlineNow: 2`
- `pylonsWalletReadyNow: 2`
- `pylonsAssignmentReadyNow: 2`
- `pylonsSeen24h: 3`
- `pylonsRegisteredTotal: 3`

## Ask For MDK Author

Please clarify and/or fix the agent-wallet restore and liquidity contract:

1. If the wallet is advertised as requiring no Lightning infrastructure
   management, `send` should either repair/acquire usable outbound capacity or
   return a documented, actionable recovery path.
2. If active outbound capacity depends on wallet home, wallet id, VSS state, or
   channel monitor state beyond the mnemonic, the docs should say mnemonic-only
   restore is not enough for live send readiness.
3. `balance` should not be the only readiness signal for send flows. Expose a
   public JSON field such as `max_sendable_sats`, `outbound_capacity_sats`, or
   `send_ready` from the agent-wallet CLI.
4. `send` errors should classify this case with a stable code, for example
   `INSUFFICIENT_OUTBOUND_CAPACITY`, instead of only a message string.
5. The docs should include the safe operational rule for agents: preserve the
   wallet home or explicitly run a documented restore/sync procedure before
   relying on an existing balance for outbound payments.

## OpenAgents Operational Rule

Until MDK clarifies the contract, OpenAgents paid Pylon smokes must use the
original funded agent-wallet home for payer sends. Mnemonic-only restore is not
accepted as send-ready evidence, even if `balance` reports enough sats.

For public Pylon readiness, keep these claims separate:

- wallet configured;
- receive-ready;
- positive balance;
- outbound send-ready;
- accepted work;
- payment sent;
- recipient settlement observed; and
- Nexus/Pylon public settlement receipt recorded.

Raw invoices, payment hashes, preimages, mnemonics, wallet homes, and daemon
logs remain under ignored local secret storage only.
