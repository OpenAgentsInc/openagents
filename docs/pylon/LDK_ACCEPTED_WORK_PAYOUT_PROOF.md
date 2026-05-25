# Pylon LDK Accepted-Work Payout Proof

Issue 14's local proof is layered on the regtest wallet harness:

```bash
scripts/pylon/ldk-accepted-work-payout-harness.sh
```

On Apple Silicon, set `ELECTRS_EXE` exactly as described in
`docs/pylon/LDK_WALLET_REGTEST_HARNESS.md`.

The wrapper writes:

```text
target/pylon-ldk-accepted-work-payout/latest/harness-summary.json
```

and then asserts the accepted-work section of that artifact.

## Local Proof Semantics

The local proof uses two real `ldk-node` wallets on regtest:

- receiver = the Pylon wallet-owned target;
- payer = the local Treasury LDK authority fixture.

The artifact records a Nexus-style wallet registration receipt, a Treasury
accepted-work dispatch receipt, the Pylon wallet-observed incoming payment, a
withdrawal receipt, and a reconciliation record tying the Nexus operation ID,
Treasury operation ID, and Pylon payment receipt together.

The local proof intentionally does not require a manual external payout target.
The target is wallet-generated and marked as:

```text
wallet_generated_bolt11_fallback
```

when the registration target is represented as the paid invoice used by the
fixture. The live Pylon registration path prefers BOLT12 where available and
falls back to BOLT11 as implemented in the wallet registration code.

## Staging Nexus Proof

For a deployed Nexus/Treasury environment, use the existing smoke:

```bash
scripts/deploy/nexus/31-smoke-ldk-accepted-work-proof.sh
```

That script checks the live Treasury LDK rail, launches or verifies a bounded
accepted-work run, waits for a confirmed and settled payout, and writes a
receipt under `docs/reports/nexus/`.
