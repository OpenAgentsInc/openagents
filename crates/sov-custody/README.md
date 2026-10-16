# Sovereign custody and guardians

`sov-custody` adds two NIP-SOV roles beside the local no-spend lifecycle in
`openagents sov`:

- **Remote custody over NIP-46.** The custodian holds only its client
  transport key. The agent key stays in the remote signer and is never
  exported. Signing, NIP-44 encryption, and NIP-44 decryption are separately
  admitted purposes.
- **Guardian quorum admission.** Guardians approve exact POL actions for the
  operations their policy lists. They are approvers, not custodians.

```sh
sov-custody init --state DIR --key-file KEY --config CONFIG.json
sov-custody revoke --state DIR --key-file KEY --caller PUBKEY
sov-custody generation --state DIR --key-file KEY --agent PUBKEY
```

CONFIG maps each agent to its independently admitted policy authority and its
admitted profile ArtifactRef. The hosting process mounts each agent's NIP-46
connection (`Custodian::mount`) and submits signed private `3188` requests
(`Custodian::submit`). Wiring this into `openagents sov` waits for the next
Coder release.

## Rules the custodian enforces

- **Policies.** Custody and guardian policies must be signed by the agent's
  configured authority. Each new revision names its exact predecessor and
  advances by one. Only the `nip46` custody mechanism is supported; `local`,
  `hardware`, and `threshold` refuse with `cannot_enforce`. A custody policy
  must name the mounted signer and transport keys. A guardian policy change
  voids every pending quorum for the agent, and so does a controller
  generation change.
- **Custody requests.** A request binds the agent, exact profile, current
  custody and guardian policy revisions, requester, controller generation,
  purpose, operation, complete input, nonce, and deadline. The requester must
  be an admitted caller that has not been revoked. A sign request must carry
  the complete unsigned event for the agent's own key, and the returned event
  must match it exactly. Decryption returns plaintext only to the request's
  named recipient, in the local reply. Plaintext never goes into an artifact,
  and once the caller is revoked a retry returns only the status.
- **Idempotency.** `(agent, requester, nonce)` with the request digest. An
  exact retry returns the retained outcome without calling the signer again;
  reusing a nonce with different bytes refuses.
- **Outages.** Dispatch intent, and any quorum consumption, is persisted
  before the signer is called. An unreachable signer refuses as `unavailable`.
  A lost reply stays `unknown` and is never re-sent.
- **Guardians.** An action for a listed operation gets one approval request
  per member. Only an authenticated, unexpired decision from a current member,
  for that member's own request and the exact action, counts, and a member
  counts once. Any denial, including one guardian changing its decision,
  blocks the action. The quorum is consumed once, atomically, before dispatch.
  A later request cannot reuse it, and a decision for one action's request
  cannot count toward another action.

All fixtures use throwaway keys and an in-process NIP-46 signer with injected
outages; no money moves and no custody is added to a relay. Hardware and
threshold custody, guardian-approved spending, and portable recovery remain
separate work.
