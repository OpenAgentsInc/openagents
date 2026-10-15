# Native work history

`work-host` implements NIP-WORK's native authority, command client, history
reader, scoped WS views, attention, and bounded pure execution. An issue and a
work item share one history. Assignment never dispatches a job.

Use a private state directory and a key file with mode `0600`:

```sh
openagents work init --state DIR --key-file KEY
openagents work scope --state DIR --key-file KEY --verifiers KEYS.json
openagents work apply --state DIR --key-file KEY --input BUNDLE
```

BUNDLE contains a signed private `event` and `attachments`, a digest-to-byte-array
map. The API checks the current grant and exact expected revision, validates the
full replacement, records RUN admission, and atomically retains the revision and
result. Exact retries return the retained result. Changed bytes under the same
issuer/command ID conflict. An interrupted admission remains unknown and is not
replayed. Item IDs remain used even when an admission's result is unknown.

Scope grants separately control read, create, revise, assign, requirements,
evidence, dispose, and integrate. Artifact closure admission checks current read
rights; knowing a cached digest supplies no permission. Scope ownership admits
the owner's local retained bytes. History, command identities, original signed
declarations, and source mappings remain private and bounded. Two signed
successors freeze an item's reader and native mutations; timestamps choose
neither branch. Readers receive original author envelopes when their keys can
open them; another recipient gets an explicitly labeled authority observation
when the original author supplied no envelope for that recipient.

`declare`, `control`, `history`, `grant-execution`, `prepare-job`, `view`, `page`,
and `attention` accept signed request files through the same API. Administration
is local; do not expose the key or initialization as an unauthenticated service.
The `issue` alias under `work` uses this same store, not GitHub's writable state.
`plan` takes a full proposed `snapshot`, `scope`, `work`, and nullable `expected`,
`source`, and `reason`; it returns an inert signed bundle for `apply`. Use
`run-job` with a separate worker key/state and `verify-job` with a separate
verifier key only after the corresponding grants and delegation are admitted.

Pure execution uses a separately admitted, attenuated no-spend grant and the
existing encrypted CJ service. Its registered Rust operation echoes a frozen
objective as data. The worker retains the exact request and RUN root before its
operation. Restart returns a known result or leaves uncertainty recorded; it
never repeats an uncertain attempt. A separate admitted key runs the actual
`exact-output` Rust check. Explicit WORK disposition is still required after
checks pass. Terminal work must be reopened before requirements change.

Commercial linking mounts the actual `coder-labor` store, holding its lock for
the WORK admission. LAB owns order negotiation, dispatch, verification,
commercial acceptance, and payment. WORK records their exact identities and
never synthesizes those outcomes. Without a mounted book, commercial requests
refuse. General effectful execution, nonempty POL instruction resolution,
external provider writes, SESS control, and graphical boards remain with their
own adapters; this crate does not claim those roles.

The GitHub bridge uses the existing tracker/project snapshot adapter. It imports
an observed node identity and bounded excerpt as a one-time copy. GitHub status
never becomes WORK verification or acceptance. Repeated observations retain the
same native identity and require an explicit conflict resolution before replace.

Views use the shared WS pager, cut validation, deltas, and portable client.
Current grants gate every page and artifact read. Attention is deduplicated
principal-owned view state; dismissing it changes no work or execution state.
All fixtures use throwaway keys, local state, and in-process transports.

The native store retains at most 16 scopes, 256 items per scope, 128 revisions
per item, and 4,096 command identities per scope. Planning validation bounds
nodes, depth, and time. Shared artifact storage bounds state to 16 MiB and
artifact bytes to 128 MiB across 8,192 objects. Limits stop new admissions;
they never silently discard idempotency or outcome history. Authority policy
and grant bytes remain outside the item read closure. Commercial source bytes
also require the mounted LAB book's recipient and retention rights.

The LAB process fixture now joins the actual bounded source edit, separate
buyer comparison, signed LAB acceptance, native WORK acceptance/reopen, two
readers, and restart. The paid variant preserves authenticated fake funding
without changing it when work reopens. Those fixtures make no real payment.
