# Sovereign lifecycle coordination

`sov-lifecycle` is the NIP-SOV lifecycle coordinator and shared fencing
authority for an agent's activations. It extends the explicitly trusted local
lifecycle in `openagents sov` with portable, fenced recovery and transfer
between hosts. Finding another host never grants transfer authority.

```sh
sov-lifecycle init --state DIR --key-file KEY --config CONFIG.json
sov-lifecycle submit --state DIR --key-file KEY --input BUNDLE
sov-lifecycle status --state DIR --key-file KEY --activation ID
sov-lifecycle expire --state DIR --key-file KEY
```

CONFIG names, for each agent, its governing authority and admitted profile,
plus the runtime hosts admitted as controllers. BUNDLE carries one signed
private `3188` request and the exact artifacts it references. Wiring this into
`openagents sov` waits for the next Coder release.

## Rules the coordinator enforces

- **One lifecycle per agent.** The authority admits an activation request.
  The coordinator binds `(agent, activation ID)` to its exact bytes forever:
  an exact retry is a duplicate, changed bytes refuse, and a stopped ID never
  reopens. An activation ID cannot replace any other agent's lifecycle in
  this coordinator. Exact retries still require the governing authority.
  One exclusive lifecycle claim covers every listed dispatcher, and a
  second lifecycle refuses until the claim is released or transferred.
- **Exact closure.** The profile must be the admitted one. The ENV lease and
  materialization must be exact retained records whose executor is the named
  controller. The reservation must parse, and the plan and claim bytes must be
  retained. The lease must be current and cover the activation's full
  lifetime; a future materialization observation refuses. The profile's
  state-disclosure policy must admit the controller host.
- **Checkpoints.** Only the current controller generation of an active
  lifecycle may checkpoint. Revisions chain exactly. The referenced RUN record
  must belong to this activation and generation and bind the exact state
  digest and obligation set. State must conform to the profile's state
  schema. Submissions from a fenced or stale controller are refused, and those
  from the old controller are retained as late results for evidence only.
- **Transfer.** The authority signs a transfer from the claim holder, from its
  latest complete checkpoint, to another admitted host. The target must be
  admitted by the disclosure policy, with a valid placement, and with the same
  reservation. The old lifecycle stops dispatch at once (`fencing`). The new
  activation is admitted at generation + 1, starting from that checkpoint and
  carrying any unknown effects, only after the target controller accepts and
  every old dispatcher acknowledges fencing. Final admission rechecks target
  expiry and placement; a refusal retains valid fencing and unknown effects.
  The target's original request remains the activation's retry identity.
  An unreachable dispatcher, or a target whose signer is down, leaves the transfer pending. The authority can
  then roll back to the same host under a fresh generation; the fenced
  generation stays fenced.
- **Recovery.** A new activation can start from the latest checkpoint of a
  stopped or expired predecessor of the same agent, at generation + 1, when the
  chain is complete and no unknown effects remain. A revoked lifecycle cannot
  be recovered.
- **Dispatch check.** `check_dispatch` admits only the claim holder's current
  controller and generation while that lifecycle is active and unexpired.

All fixtures use throwaway keys and local stores; nothing spends.
Cross-coordinator federation is not part of this crate, and neither is the
fencing of child COORD claims inside `coordination-host`.
