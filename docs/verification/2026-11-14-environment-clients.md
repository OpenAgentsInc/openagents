# Computer-backed Claude clients, build1074

The implementation is the bounded bootstrap profile in
[environment-runtime-v1alpha1.md](../protocol/environment-runtime-v1alpha1.md),
not completion of the entire ENV/SESS/world-editing specification.

## Installed release

OpenAgents1.0.0(1074), `com.openagents.app`, uploaded2026-11-13 at23:57 CST;
App Store Connect reported VALID and IN_BETA_TESTING. Its released runtime is
`cab88dfff7be295a67bff159796c06d0d96273e2bb8e6d7411604dcacb5fef2c`.
The archive, IPA, runtime record and configuration are retained outside Git
under `~/.openagents/releases/ios-1074-environments/`.

The Mac and CoderOS resident services use the existing Coax Claude SDK and
NIP-HOST carriage. Chris's existing email account has two account-owned
connection rows. The delegated broker key is a Secret Manager file mount;
the account catalog exposes neither it nor grants. Provider credentials and
Claude processes remain on the chosen computer. The existing Linux host
identity is retained; the legacy Mac Keychain identity is not exported or
rotated. The new Mac enrollment is separate.

The native Mac and GTK desktop clients use the same Coax application, with a
protected local adapter for **This computer**. The installed desktop bundle
identity and icon are retained. Legacy executable/bundle backups live under
`~/.openagents/desktop/recovery/`; active legacy processes were not killed.
The new service is `com.openagents.coax.environment` on each computer.

## Connection incident and correction

The owner installed1074 and saw503 while loading CoderOS. The connection
catalog worked; both resident services and direct signed broker calls worked.
A Cloud Run job using the exact deployed image, service account and mounted
broker key also worked. This isolated the failure to the API subprocess
boundary. The execution-environment experiment did not resolve it.

Safe numeric diagnostics reported `saved=1 result=-1999`: a started broker
whose exit status was unavailable. `realms_refresh_pages()` sets
`SIGCHLD=SIG_IGN` before API workers fork. Production workers therefore
auto-reaped the broker. The process library correctly refused to invent an
exit status, and the API incorrectly reported the healthy computer as
unavailable. Earlier checks covered authentication and direct broker calls,
but missed a successful broker invocation inside a production-started worker.

The synchronous API adapter now scopes SIGCHLD to its default disposition
while it waits, then restores the worker's previous disposition. Request
admission, deadlines and command identities are unchanged. A native oracle
starts with SIGCHLD ignored, exercises the real request builder, private file,
process adapter and typed reply, and checks disposition restoration. The API
suite additionally reads through the real HTTP route after normal startup,
using an independent Coax broker fixture without credentials or model calls.

The compatible OTA corrects the accompanying UI error: transport failure no
longer claims that discovery ran or that the account has no computers. Reads
offer a straightforward reconnect; uncertain mutations retain their exact
command identity. A real indexing gap and ordinary pagination remain distinct.

## Automated evidence

The implementation before the incident passed:

- Environment authority:105 mutants,105 killed, zero survivors. No equivalent
  exceptions. Wire checks85/85; shared session checks6/6.
- Resident host checks829/829 and shared app checks250/250. The additional
  connection-error rendering regression passed after the owner's report.
- Runtime and HOST golden records, published-protocol compatibility, wire,
  layer and tools checks; no allowlist increase.
- Actual private durable file replacement and same-UID Unix socket checks.
- API history/email/account-isolation suite20/20, ASan/UBSan and the common
  API build/migration checks. This initial pass lacked a positive broker call
  inside the API worker, as described above.
- Generated Wasm/JavaScript parity through login, account chat, Markdown,
  computer selection, timed-out send, identical retry and polling.
- Headless iOS XCTest11/11, including native program, Markdown and signed
  update/activation tests. A checkbox fixture now compares its Unicode scalar
  because Swift treats the text-presentation variation selector as part of
  the grapheme; no product parser was duplicated or changed for that assertion.
- Actual hidden AppKit controls and field continuity, and real GTK controls
  under Xvfb through local discovery and an unsent multiline draft.
- Real Claude replies on both computers via signed HOST calls. Exact command
  replay produced one user message; conflicting reuse refused.

After the connection correction, the process oracle passed on macOS against
the real CoderOS broker and on Linux under ASan/UBSan against the independent
fixture, both starting with SIGCHLD ignored. Neither result substitutes for
the public deployed-path checks recorded below.

## Deployed-path results

Production revision `oa-api-00159-sc6` serves image
`sha256:6743ee55e356a8de4d535a906da50ba7940c6aff0f7bba237ef5fbedee637429`.
The deployment was built from pre-rebase commit `bc8e045411`; the corresponding
rebased signal fix is `ca1775219d`. Staging and production migrations, health,
build identity and release identity were verified by the deployment tool.

Using a temporary authenticated fixture account with explicitly installed
connection rows, the public `https://openagents.com/api/chat/environment/v1`
path returned200 for DescribeEnvironment and ListSessions on **both Mac and
CoderOS**, with16 conversations in each first page. OpenSession was accepted
on each. ReadSession then contained the exact requested assistant marker from
the actual Claude process on each computer. Identical OpenSession replay was
accepted without another user message; conflicting replay was refused with
`idempotency_conflict`. A subsequent SendMessage and StopTurn were accepted
on the Mac, and a later read confirmed it idle. A held permission was not
reached in the bounded live observation, so physical permission acceptance
remains an owner check rather than a claimed live pass.

The corrected API suite passed21 history checks, including the new positive
email-login-to-broker route after production-style startup. The native
process oracle passed under ASan/UBSan in the same run.

Signed OTA generation **1794638400** targets1074's exact released runtime.
The public feed matched the locally signed envelope, and all12 public objects
matched their manifest lengths and SHA-256 digests. The generated module
passed Wasm/JavaScript frame/action parity before packaging. This is a real
compatible application change, so an update offer is expected; the server
connection repair itself needs neither applying it nor reinstalling1074.

Both temporary fixture connection rows were removed and Chris's two rows
were checked again. The fixture account entered the normal deletion lifecycle,
its local signer was removed, and the diagnostic Cloud Run job and request
secret were deleted. The production broker secret and both resident services
remain installed. The temporary Cloud SQL proxy was stopped.

## Remaining scope

The implementation has four managed process slots, an eight-row saved
recovery catalog, bounded native-history discovery and explicit page cuts.
Service restart recovers old histories as observe-only/unknown; it does not
silently resume an unqualified native writer. Terminal histories are read
only. General GUI enrollment/workspace configuration, Windows, full durable
process reconciliation, realm editing/previews and complete ENV/SESS artifact
semantics remain work in the larger specification.

Physical iPhone acceptance, actual mail AutoFill behavior and personal
conversation selection remain owner checks in `NEEDS_OWNER.md`.
