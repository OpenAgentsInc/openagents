# Native email login and saved account history, 2026-11-13

Delivery target: existing TestFlight **1069**, runtime
`0495897de3936831430611873a4ded8d9b4bdec8a0ecb8c511b680fea3ba9450`,
signed OTA generation **4**. The user confirmed account-saved conversations,
not local Coder/Claude histories. No native rebuild is needed for this change.

The application is `apps/openagents-ios/home/`. Its request validation comes
from `apps/shared/app-login.coax`; fields and rendering use the shared kit
and DOM backend. The installed native `AppAuth` requests and verifies the
same email codes as `/login`, keeps device-bound tokens in the Keychain,
refreshes authenticated HTTP, and clears login on logout. Wasm receives no
tokens. A failed HTTP exchange probes native session expiry once, then
leaves a manual retry. A fresh challenge clears the input; login, logout
and changing email erase the pending challenge, code and inbox bytes.

`GET /api/chat/history/v1` is an account read, including email L1. It joins
conversation owners to the account's linked Nostr keys; the Coax authority
checks the person principal and each owning account before serializing.
The account id is not the Verse account returned by `auth.account_of`.
The codec bounds the list to 60 rows and shares a pinned protocol golden.
Existing admin, inference and conversation-write gates retain their rules.
The app currently lists titles; transcript and send behavior are later work.

## Checks

- Final home module: **191 checks passed**, including shared inputs, spaced
  codes, wrong-code retry, clearing pending state and bounded expiry probing.
- History codec and dependencies: **76 checks passed**. Exact profile golden
  reads and rewrites byte for byte.
- History authority: **6 mutants killed, 0 survivors**. Mutation exposed the
  missing one-character account case; its check was added, with no equivalent
  mutants declared.
- Tools, wire, component and layer gates passed. The new HTTP oracle is a
  reason-2 independent Python test under `apps/openagents-ios/Tests/`; it is
  excluded from app and test-bundle resources.

The first rendered OTA test found that the initial Coax frame's auth action
was not drained. `home/host.js` now draws that first frame and drains its
service request before announcing ready. The second test reached email
sending and wrong-code handling; its replacement typing inserted before
old text because of the simulator's cursor placement. The independent test
now requests a fresh code and enters it into the cleared field. The generation-3
retained-1069 test passed all steps in 76.1 seconds: signed activation,
three code requests, one wrong code, one device-bound login, two authorized
history reads across relaunch, and one logout followed by a logged-out relaunch.
The signature-verified module ran through the 30-second health window.
A later WebKit run stalled on animation-idle waits; the account surface now
uses the shared kit's still rendering mode. The generation-3 versioned package
passed all 190 checks and the full retained-binary UI test in 76.1 seconds.
Public generation 3 was downloaded after publication and matched the tested
signed envelope byte for byte.
The resigned fixture native executable SHA-256 is
`db13fde331d487cf2ebe9cdb673263662c93f42b196a080cf58c68e6b2de443d`;
the generation-3 Wasm SHA-256 is
`a4ad6f59edaa55a21c10ec5f4ec22bd1d3a94ebe1a0ca57993f7d41c746a6e20`.
Only the fixture feed endpoint in Native.plist and its ad-hoc signature were
changed in that retained simulator app; no native source was rebuilt for it.

The initial combined auth run passed its pre-existing auth cases, but
its appended history fixture did not establish a fresh login. The isolated
check exposed the cause: the new test address was missing from test.sh's
explicit fake-mail recipient allowlist. That test-only address is now listed;
production email policy is unchanged. History checks have their own
`history_checks.coax` process/mailbox. That combined run
also exposed two pre-existing `/api/auth/account` route-golden mismatches:
the response's `keys: []` is not a JSON name in the current AuthAccount
projection. No auth-account response/schema change is part of this feature;
the original `auth_checks.coax` is unchanged in the final diff.

The two existing BalanceUITests also passed against the retained native app
with generation 3 active: More opens the read-only sats view, refresh works,
and a missing wallet is distinct from zero. An additional exploratory
wallet-after-login probe stopped at an XCUITest keyboard-focus error before
reaching its wallet assertions; it is not included as a passing result or
as a committed test. No extras application or wallet code changed here.

## Final API deployment and verification

The versioned route is additive: production already uses `/api/chat/history`
for room chat (`last`, `messages`). It remains HTTP 200 with that format.
The new account contract is `/api/chat/history/v1`; an unauthenticated call
is HTTP 401. Staging and production serve the same verified image digest:
`sha256:b828a158ed38857441aca70ccfa04f1698a2968c68f2ff26d46d586546528fef`.
Revisions: `oa-api-staging-00217-92k`, `oa-api-00154-clc`.

The final focused API run passed in 255 seconds on the temporary dedicated
build host. All seven history checks pass: no anonymous access; fake mail
accepted; the app's `client: app`/pending/code login yields an access token;
its L1 bearer sees only linked account owners; no caching; the stronger
admin gate remains; and history cannot be written. All five captured route
goldens passed, as did the harness's deployment/policy, migration, signer,
CLI identity, realm and append-only-log checks. No production account was
used to obtain these results. The temporary host was removed after verification.
Its label excluded it from the shared pool.

The component gate initially mistook the recently landed legacy chat codec's
protocol counts/indices for pixel sizes. Eighteen nonvisual lines now carry
the checker's existing `not a size` annotation; no codec bytes or UI sizes
changed. The component and layer gates then passed.

## Phone acceptance

An actual TestFlight phone must receive its real email, accept the code and
show that account's saved titles. Development tests use fake mail/accounts
and do not read the owner's mailbox, tokens, wallet or conversations. This
remaining personal-data acceptance is recorded in `NEEDS_OWNER.md`.

## Generation 4 cache boundary

Generation 3 was published and its main source landed. Final review found
that HTTP 401 returned to the login screen without clearing cached titles;
a subsequent successful login could briefly draw those titles while loading.
Generation 4 erases the complete history record on expiry, logout, a failed
native auth-state probe, and before loading a newly signed-in account. It
also clears the email buffer with the code/challenge, so a blank login field
cannot retain a previous address in its model. The new regression check
asserts both zero count and zeroed title/email bytes. All 191 home checks
pass. No API or native SDK change was needed for this correction.

The final generation-4 test passed in **67.376 seconds**, upgrading the retained
1069 executable from generation 3. It exercised resend, wrong-code retry,
device-bound login, history across relaunch, the update health window, logout
and a logged-out relaunch. The oracle recorded three code requests, one wrong
code, one successful verification, two authorized history reads and one logout.
The tested Wasm SHA-256 is
`dd3130d0577df5dacae8bd13c8b053012285dc4cfda47c26c085c1862b93f400`.
After publication, both `updates.openagents.com` and the fallback Cloud Run
endpoint served generation 4 with all 11 artifact hashes verified and the
signed envelope identical to the tested package.
Evidence is retained outside the repository under
`~/.openagents/releases/openagents-ios-1069-ota4/`.

## Generation 5 fixed viewport

The owner reported page zoom from pinches. The shared starter now loads
`viewport.js`: a fixed window with an internal vertical scroller, constrained
viewport metadata, touch/gesture zoom cancellation and visible-height updates
when the keyboard opens. Both starter HTML entry points use the same host glue;
the package preparation and publication inventory include it. The native SDK
and both Wasm applications are unchanged from generation 4.

Signed generation 5 passed the retained-1069 OTA/email/history/relaunch/logout
flow in 66.301 seconds. A separate real WebKit test checks geometry after each
pinch-in, pinch-out and double-tap: all passed in 7.662 seconds. Keyboard code
entry and login buttons remained reachable. Resource preparation and JS syntax
checks passed. No native app source was rebuilt for these observations.
