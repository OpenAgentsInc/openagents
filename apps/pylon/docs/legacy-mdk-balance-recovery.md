# Legacy MDK Balance Recovery

Date: 2026-06-26

Issue: [#6301](https://github.com/OpenAgentsInc/openagents/issues/6301)

`pylon wallet send --rail mdk` remains blocked for normal agent funds. MDK is
not the primary or default agent payment rail. The only supported self-serve MDK
movement path is a local, explicit recovery command for old residual MDK channel
balances.

## Command

Dry-run/preflight is the default:

```sh
pylon wallet recover-mdk
```

To prepare a recovery, provide the local destination payment request on the
operator's machine:

```sh
pylon wallet recover-mdk --destination <local-payment-request>
```

The command reports a public-safe `destinationRef` digest, detected residual
balance, blockers, and next actions. It does not print the raw destination,
payment hashes, preimages, wallet state, seed material, or local paths.

Funds move only after explicit consent and execution:

```sh
pylon wallet recover-mdk --destination <local-payment-request> --yes --execute
```

An optional amount can be supplied:

```sh
pylon wallet recover-mdk --destination <local-payment-request> --amount 1000 --yes --execute
```

Without `--amount`, the local MDK helper receives its normal send command
without an amount argument. Operators should verify the destination credit
locally after the command returns a
`receipt.pylon.legacy_mdk_recovery.<digest>` ref.

## Safety Boundary

- This command is for owner-local recovery of residual MDK balances only.
- It does not re-enable `wallet send --rail mdk`.
- It does not make MDK the agent primary rail.
- It emits only redacted refs and balances in the JSON projection.
- Raw payment material is accepted only as local CLI input and passed directly
  to the local MDK helper.

## Verification

Run from the repo root:

```sh
bun test apps/pylon/tests/wallet.test.ts
```
