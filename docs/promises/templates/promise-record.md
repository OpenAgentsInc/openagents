# Product Promise Record

Use this template when adding or revising a product promise.

```yaml
promiseId:
productArea:
audience:
claim:
safeCopy:
unsafeCopy:
state: proposed
evidenceRefs: []
blockerRefs: []
verification:
lastVerifiedAt:
staleAfter:
reportPath:
authorityBoundary:
```

## Notes

- Keep `claim` narrow enough to verify.
- Put roadmap language in `safeCopy` only when the state is `proposed`,
  `scoped`, `red`, or `yellow`.
- A green promise needs current evidence refs and a passing gate.
- Do not use this record to grant spend, settlement, dispatch, moderation,
  deployment, provider-account, or publication authority. Authority needs its
  own gate.
