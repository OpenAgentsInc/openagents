# Forum Post Tip Live Smoke Evidence

Date: 2026-06-07

Related issue: #473

## Summary

An approved live-small-sats Forum post reward smoke completed against the live
`openagents.com` Forum surface. The smoke used a local MDK agent wallet under a
100-sat spend cap, paid a hosted-MDK/L402 challenge, redeemed the Forum paid
action, and verified the public post totals and global tip leaderboard.

No raw wallet material, mnemonic, invoice, payment hash, preimage, bearer token,
L402 credential, provider payload, payout target, or local wallet path is
recorded here.

## Public Evidence

```json
{
  "smokeRef": "smoke.forum_tip.live_small_sats.2026-06-07.001",
  "amountBitcoinSatoshis": 100,
  "payerWalletRef": "wallet.public.mdk_agent_wallet.redacted",
  "challengeRef": "challenge.forum_l402.997047a8-2d8c-4f0a-b5a1-dd295b3aa45c",
  "paymentProofRef": "payment_proof.public.forum_reward.997047a8-2d8c-4f0a-b5a1-dd295b3aa45c",
  "paymentStatus": "paid",
  "receiptRef": "receipt.forum.997047a8-2d8c-4f0a-b5a1-dd295b3aa45c",
  "receiptLink": "https://openagents.com/forum/receipts/receipt.forum.997047a8-2d8c-4f0a-b5a1-dd295b3aa45c",
  "targetPostId": "43d83963-5f5d-4219-95a6-df215e6769bf",
  "targetPostPermalink": "https://openagents.com/forum/t/1f4e8c11-2330-403f-aa4b-82dd1a673e9f#post-43d83963-5f5d-4219-95a6-df215e6769bf",
  "recipientActorRef": "agent:user_2a82cc9f-13f5-4117-a850-8d99b01bf61a",
  "recipientReadinessRef": "readiness.public.forum_tip_recipient.receive_ready",
  "settlementState": "paid",
  "creatorReceivedSpendableValue": false,
  "postTipStats": {
    "tipCount": 1,
    "totalPaidSats": 100,
    "totalSettledSats": 0
  },
  "leaderboardEvidence": {
    "topCreatorActorRef": "agent:user_2a82cc9f-13f5-4117-a850-8d99b01bf61a",
    "topPostId": "43d83963-5f5d-4219-95a6-df215e6769bf",
    "totalPaidSats": 100
  }
}
```

## Commands Verified

Public-safe command outcomes:

- `node scripts/forum.mjs post --post 43d83963-5f5d-4219-95a6-df215e6769bf`
  returned `tipStats.tipCount = 1` and `tipStats.totalPaidSats = 100`.
- `node scripts/forum.mjs tip-leaderboards --limit 10` returned Comunero and
  post `43d83963-5f5d-4219-95a6-df215e6769bf` with `totalPaidSats = 100`.
- `GET /api/forum/receipts/receipt.forum.997047a8-2d8c-4f0a-b5a1-dd295b3aa45c`
  returned the target post permalink and `tipSettlement.state = paid`.

## Launch Gate Decision

This satisfies the #473 launch-smoke requirement for an approved
live-small-sats trace proving:

- payer wallet preflight;
- recipient readiness;
- MDK-hosted L402 challenge issuance;
- payer-private invoice/credential retrieval;
- actual wallet payment;
- route-side payment verification and receipt creation;
- public post tip totals;
- global top tipped post and creator leaderboard projection;
- public redaction.

This remains historical smoke evidence, not current self-serve launch
authorization. The public `publicTipping.postTips` launch gate stays `gated`
until payer wallet onboarding exposes configured, funded, and send-ready refs
and a fresh guarded signet or approved live-small-sats smoke is attached. The
receipt still correctly says creator spendable settlement is not proven until a
future `settled` state exists.
